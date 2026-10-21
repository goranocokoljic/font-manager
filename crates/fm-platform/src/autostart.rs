//! Start at login (E17.2, ADR 0011): a per-user entry that launches the app
//! with [`AUTOSTART_ARG`] when the user logs in, so it starts in the tray.
//!
//! | OS | The entry | Admin rights |
//! |----|-----------|--------------|
//! | Windows | value `org.fontmanager.app` under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` | none |
//! | macOS | `~/Library/LaunchAgents/org.fontmanager.app.autostart.plist` (a login item) | none |
//! | Linux | `$XDG_CONFIG_HOME/autostart/org.fontmanager.app.desktop` (default `~/.config/autostart`) | none |
//!
//! Inside a Flatpak sandbox ([`sandboxed`]) there is no entry: [`system`]
//! answers `Unsupported` and the option is hidden.
//!
//! The entry itself is the state: [`Autostart::is_enabled`] reads it from the
//! OS every time, nothing is cached. [`Autostart::disable`] removes only an
//! entry this app wrote.

use std::fmt;
use std::io;
use std::sync::{Arc, Mutex, PoisonError};

/// The launch argument of a start at login: the app starts in the tray
/// without opening its window.
pub const AUTOSTART_ARG: &str = "--autostart";

/// The login entry of this OS.
pub trait Autostart: Send + Sync + fmt::Debug {
    /// Registers the entry (or writes it again, pointing at this program).
    fn enable(&self) -> io::Result<()>;
    /// Removes the entry when this app wrote it; `false` when there was none
    /// of ours. An entry of the same name the app did not write stays.
    fn disable(&self) -> io::Result<bool>;
    /// Whether the entry is there and launches this program at login.
    fn is_enabled(&self) -> io::Result<bool>;
}

/// The login entry of the OS the app runs on, launching the running program
/// (on Linux the AppImage file when the app runs from one). A Flatpak sandbox
/// and any other OS get an entry whose every call answers
/// [`io::ErrorKind::Unsupported`].
pub fn system() -> Arc<dyn Autostart> {
    let program = match std::env::current_exe() {
        Ok(program) => program,
        Err(e) => return Arc::new(Unsupported(format!("the program path is unknown: {e}"))),
    };
    #[cfg(windows)]
    {
        Arc::new(crate::windows::WindowsAutostart::new(
            crate::windows::RUN_KEY,
            crate::paths::BUNDLE_ID,
            &program,
        ))
    }
    #[cfg(target_os = "macos")]
    {
        match crate::macos::MacosAutostart::for_user(&program) {
            Ok(entry) => Arc::new(entry),
            Err(e) => Arc::new(Unsupported(e.to_string())),
        }
    }
    #[cfg(target_os = "linux")]
    {
        if sandboxed() {
            return Arc::new(Unsupported(
                "start at login is not available inside the Flatpak sandbox".into(),
            ));
        }
        let get = |name: &str| std::env::var_os(name);
        let env = crate::paths::Env(&get);
        let program = crate::linux::autostart_program(&env, program);
        match crate::linux::LinuxAutostart::for_user(&env, crate::paths::BUNDLE_ID, &program) {
            Ok(entry) => Arc::new(entry),
            Err(e) => Arc::new(Unsupported(e.to_string())),
        }
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        let _ = program;
        Arc::new(Unsupported(
            "start at login is not available on this system".into(),
        ))
    }
}

/// Whether the app runs in a sandbox where it offers no login entry: a
/// Flatpak on Linux (ADR 0011). Always `false` on Windows and macOS.
pub fn sandboxed() -> bool {
    #[cfg(target_os = "linux")]
    {
        crate::linux::in_flatpak(
            &|name| std::env::var_os(name),
            std::path::Path::new("/.flatpak-info").exists(),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// An OS without a login entry: every call fails with `Unsupported`.
#[derive(Debug)]
struct Unsupported(String);

impl Unsupported {
    fn error(&self) -> io::Error {
        io::Error::new(io::ErrorKind::Unsupported, self.0.clone())
    }
}

impl Autostart for Unsupported {
    fn enable(&self) -> io::Result<()> {
        Err(self.error())
    }
    fn disable(&self) -> io::Result<bool> {
        Err(self.error())
    }
    fn is_enabled(&self) -> io::Result<bool> {
        Err(self.error())
    }
}

/// A login entry in memory, for tests: it can be told to refuse changes as
/// an OS policy would.
#[derive(Debug, Default)]
pub struct MockAutostart {
    state: Mutex<MockState>,
}

#[derive(Debug, Default)]
struct MockState {
    enabled: bool,
    refuse: Option<String>,
}

impl MockAutostart {
    /// An entry that is off and accepts changes.
    pub fn new() -> Self {
        Self::default()
    }

    /// From now on `enable` and `disable` fail with `PermissionDenied` and
    /// `reason`, as when a policy forbids login entries.
    pub fn refuse(&self, reason: impl Into<String>) {
        self.lock().refuse = Some(reason.into());
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, MockState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn change(&self, on: bool) -> io::Result<bool> {
        let mut state = self.lock();
        if let Some(reason) = &state.refuse {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                reason.clone(),
            ));
        }
        let was = state.enabled;
        state.enabled = on;
        Ok(was)
    }
}

impl Autostart for MockAutostart {
    fn enable(&self) -> io::Result<()> {
        self.change(true).map(|_| ())
    }
    fn disable(&self) -> io::Result<bool> {
        self.change(false)
    }
    fn is_enabled(&self) -> io::Result<bool> {
        Ok(self.lock().enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mock_entry_turns_on_and_off_and_can_refuse() {
        let entry = MockAutostart::new();
        assert!(!entry.is_enabled().expect("read"));
        entry.enable().expect("on");
        assert!(entry.is_enabled().expect("read"));
        assert!(entry.disable().expect("off"));
        assert!(!entry.disable().expect("off again"));
        entry.refuse("policy");
        let e = entry.enable().expect_err("refused");
        assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
        assert!(!entry.is_enabled().expect("read"));
    }

    #[test]
    fn the_system_entry_matches_the_os() {
        let entry = system();
        let read = entry.is_enabled();
        if cfg!(any(windows, target_os = "macos", target_os = "linux")) && !sandboxed() {
            // Reading never changes anything; a CI runner has no entry.
            assert!(read.is_ok(), "{read:?}");
        } else {
            assert_eq!(
                read.expect_err("unsupported").kind(),
                io::ErrorKind::Unsupported
            );
        }
    }
}
