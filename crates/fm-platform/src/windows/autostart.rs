//! Start at login on Windows (ADR 0011): a `REG_SZ` value under the per-user
//! `Run` key, `"<program>" --autostart`. Writing `HKEY_CURRENT_USER` needs no
//! admin rights. Only the value this app names is ever touched, and
//! `disable` deletes it only when its data is a command with `--autostart`,
//! so a value of the same name someone else wrote stays.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::Path;

use super::registry::Key;
use crate::autostart::{AUTOSTART_ARG, Autostart};

/// The per-user key whose values Windows runs at login.
pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// A login entry: value `name` under `key` (of `HKEY_CURRENT_USER`).
#[derive(Debug, Clone)]
pub struct WindowsAutostart {
    key: String,
    name: OsString,
    command: OsString,
}

impl WindowsAutostart {
    /// The entry `name` under `key` that launches `program` with `--autostart`.
    pub fn new(key: &str, name: &str, program: &Path) -> Self {
        Self {
            key: key.to_owned(),
            name: name.into(),
            command: command(program),
        }
    }

    /// The command line the entry holds.
    pub fn command(&self) -> &OsStr {
        &self.command
    }
}

/// `"<program>" --autostart`: the path quoted, as it may hold spaces.
fn command(program: &Path) -> OsString {
    let mut line = OsString::from("\"");
    line.push(program.as_os_str());
    line.push("\" ");
    line.push(AUTOSTART_ARG);
    line
}

/// Whether `data` is a command line this app wrote: it ends with the
/// autostart argument after a quoted program.
fn is_ours(data: &OsStr) -> bool {
    data.to_string_lossy()
        .trim_end()
        .ends_with(&format!("\" {AUTOSTART_ARG}"))
}

impl Autostart for WindowsAutostart {
    fn enable(&self) -> io::Result<()> {
        Key::create_current_user(&self.key)?.set_string(&self.name, &self.command)
    }

    fn disable(&self) -> io::Result<bool> {
        let key = Key::create_current_user(&self.key)?;
        match key.get_string(&self.name)? {
            Some(data) if is_ours(&data) => key.delete_value(&self.name),
            _ => Ok(false),
        }
    }

    fn is_enabled(&self) -> io::Result<bool> {
        let data = Key::create_current_user(&self.key)?.get_string(&self.name)?;
        Ok(data.is_some_and(|d| d == self.command))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch key, deleted before and after the test.
    struct Scratch(String);

    impl Scratch {
        fn new(label: &str) -> Self {
            let key = format!(
                r"Software\FontManagerTests\autostart-{label}-{}",
                std::process::id()
            );
            Key::delete_current_user_tree(&key).expect("clean");
            Self(key)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = Key::delete_current_user_tree(&self.0);
        }
    }

    #[test]
    fn the_command_quotes_the_program_and_carries_the_argument() {
        let entry = WindowsAutostart::new(
            RUN_KEY,
            "x",
            Path::new(r"C:\Program Files\Font Manager\fm-app.exe"),
        );
        assert_eq!(
            entry.command(),
            r#""C:\Program Files\Font Manager\fm-app.exe" --autostart"#
        );
        assert!(is_ours(entry.command()));
        assert!(!is_ours(OsStr::new(r"C:\Other\app.exe")));
    }

    #[test]
    fn enable_and_disable_write_and_remove_the_value() {
        let scratch = Scratch::new("cycle");
        let entry = WindowsAutostart::new(
            &scratch.0,
            "org.fontmanager.test",
            Path::new(r"C:\Fm\fm-app.exe"),
        );
        assert!(!entry.is_enabled().expect("read"));
        entry.enable().expect("on");
        assert!(entry.is_enabled().expect("read"));
        let key = Key::create_current_user(&scratch.0).expect("key");
        assert_eq!(
            key.get_string(OsStr::new("org.fontmanager.test"))
                .expect("value"),
            Some(entry.command().to_owned())
        );
        assert!(entry.disable().expect("off"));
        assert!(!entry.is_enabled().expect("read"));
        assert!(!entry.disable().expect("nothing left"));
    }

    #[test]
    fn disable_leaves_values_the_app_did_not_write() {
        let scratch = Scratch::new("foreign");
        let key = Key::create_current_user(&scratch.0).expect("key");
        key.set_string(
            OsStr::new("Other App"),
            OsStr::new(r#""C:\Other\other.exe" /tray"#),
        )
        .expect("other");
        // A value under our name that this app did not write.
        key.set_string(
            OsStr::new("org.fontmanager.test"),
            OsStr::new(r"C:\Someone\else.exe"),
        )
        .expect("same name");
        let entry = WindowsAutostart::new(
            &scratch.0,
            "org.fontmanager.test",
            Path::new(r"C:\Fm\fm-app.exe"),
        );
        assert!(!entry.is_enabled().expect("read"));
        assert!(!entry.disable().expect("not ours"));
        assert!(
            key.get_string(OsStr::new("org.fontmanager.test"))
                .expect("kept")
                .is_some()
        );

        entry.enable().expect("on");
        assert!(entry.disable().expect("off"));
        assert_eq!(
            key.get_string(OsStr::new("Other App")).expect("kept"),
            Some(OsString::from(r#""C:\Other\other.exe" /tray"#))
        );
    }

    #[test]
    fn an_entry_for_another_copy_of_the_app_is_not_this_one() {
        let scratch = Scratch::new("moved");
        let old = WindowsAutostart::new(
            &scratch.0,
            "org.fontmanager.test",
            Path::new(r"C:\Old\fm-app.exe"),
        );
        old.enable().expect("old on");
        let new = WindowsAutostart::new(
            &scratch.0,
            "org.fontmanager.test",
            Path::new(r"C:\New\fm-app.exe"),
        );
        assert!(!new.is_enabled().expect("read"));
        // Turning it off removes the old copy's entry: same name, our command.
        assert!(new.disable().expect("off"));
        assert!(!old.is_enabled().expect("read"));
    }
}
