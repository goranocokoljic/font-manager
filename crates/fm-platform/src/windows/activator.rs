//! The Windows [`FontActivator`]: one per-user registry value per file (ADR 0008).
//!
//! Activation writes a `REG_SZ` value under
//! `HKCU\Software\Microsoft\Windows NT\CurrentVersion\Fonts` whose data is the
//! file's full path in place, then broadcasts `WM_FONTCHANGE`. GDI and
//! DirectWrite apps, new and running, see it; no admin rights are needed.
//! Session and permanent mode write the same value; session values are removed
//! by [`FontActivator::clear_session`] on quit and again at the next start
//! after a crash. `AddFontResourceEx` is not used: DirectWrite never sees it.
//!
//! Rule 3: the activator keeps a ledger of the values it wrote under
//! `HKCU\Software\FontManager\Activation\{Session,Permanent}` (value name →
//! path). A Fonts value is ours only when the ledger lists it with the same
//! path; anything else is never changed or deleted. A value name taken by a
//! value the app did not write gets a numeric suffix.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::{Mutex, PoisonError};

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{HWND_BROADCAST, SendNotifyMessageW, WM_FONTCHANGE};

use super::dwrite;
use super::registry::Key;
use crate::activator::{
    ActivationError, ActivationErrorKind, Conflict, ConflictKind, FaceFile, FontActivator, Mode,
    Outcome, OutcomeStatus, Registration,
};

/// The per-user font list Windows reads (Windows 10 1809 and later).
pub const FONTS_KEY: &str = r"Software\Microsoft\Windows NT\CurrentVersion\Fonts";
/// Where the activator records the values it wrote.
pub const LEDGER_KEY: &str = r"Software\FontManager\Activation";

/// Value names are `<PostScript name> (Font Manager)`, then
/// `<PostScript name> (Font Manager 2)` and up when the name is taken.
const NAME_TAG: &str = "Font Manager";
/// More same-named values than this for one PostScript name is a bug, not a
/// font library.
const MAX_SUFFIX: u32 = 1000;

/// Activates fonts for the current user through the registry. See the module docs.
#[derive(Debug)]
pub struct WindowsActivator {
    fonts_key: String,
    ledger_key: String,
    broadcast: bool,
    /// Serialises read-check-write sequences, so two calls never pick the same name.
    lock: Mutex<()>,
}

impl Default for WindowsActivator {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsActivator {
    /// The activator the app uses: the real per-user Fonts key, the app's
    /// ledger, and a `WM_FONTCHANGE` broadcast after every change.
    pub fn new() -> Self {
        Self::with_keys(FONTS_KEY, LEDGER_KEY, true)
    }

    /// An activator over other `HKCU` subkeys. Tests point the ledger (and in
    /// unit tests the Fonts key too) at scratch keys so they never see or
    /// change the app's own records. `broadcast` sends `WM_FONTCHANGE`.
    pub fn with_keys(fonts_key: &str, ledger_key: &str, broadcast: bool) -> Self {
        Self {
            fonts_key: fonts_key.to_owned(),
            ledger_key: ledger_key.to_owned(),
            broadcast,
            lock: Mutex::new(()),
        }
    }

    fn open(&self) -> Result<Keys, ActivationError> {
        let open = |subkey: &str| {
            Key::create_current_user(subkey).map_err(|e| os_error(&e, &format!("HKCU\\{subkey}")))
        };
        Ok(Keys {
            fonts: open(&self.fonts_key)?,
            session: open(&format!("{}\\Session", self.ledger_key))?,
            permanent: open(&format!("{}\\Permanent", self.ledger_key))?,
        })
    }

    fn broadcast_font_change(&self) {
        if !self.broadcast {
            return;
        }
        // SAFETY: a broadcast with no pointers in it. `SendNotifyMessageW`
        // returns at once for windows of other threads instead of waiting for
        // each to answer, which took most of a second with a desktop full of
        // windows and broke the 100-face activation budget. A failed
        // broadcast only delays when running apps notice.
        unsafe {
            let _ = SendNotifyMessageW(HWND_BROADCAST, WM_FONTCHANGE, WPARAM(0), LPARAM(0));
        }
    }
}

/// The Fonts key and the two ledger keys, open for one call.
struct Keys {
    fonts: Key,
    session: Key,
    permanent: Key,
}

impl Keys {
    fn ledger(&self, mode: Mode) -> &Key {
        match mode {
            Mode::Session => &self.session,
            Mode::Permanent => &self.permanent,
        }
    }

    /// The ledger entry for value `name`: its mode and recorded path.
    fn ledger_entry(&self, name: &OsStr) -> io::Result<Option<(Mode, PathBuf)>> {
        for mode in [Mode::Session, Mode::Permanent] {
            if let Some(path) = self.ledger(mode).get_string(name)? {
                return Ok(Some((mode, PathBuf::from(path))));
            }
        }
        Ok(None)
    }

    /// Ledger entries whose Fonts value still points at the recorded path.
    fn live_registrations(&self) -> io::Result<Vec<Registration>> {
        let mut out = Vec::new();
        for mode in [Mode::Session, Mode::Permanent] {
            for (name, path) in self.ledger(mode).string_values()? {
                let path = PathBuf::from(path);
                if self.fonts.get_string(&name)? == Some(registry_data(&path)) {
                    out.push(Registration {
                        path,
                        mode,
                        handle: name,
                    });
                }
            }
        }
        Ok(out)
    }

    /// The first value name for `postscript_name` that neither the Fonts key
    /// nor the ledger uses.
    fn free_name(&self, postscript_name: &str) -> io::Result<Option<OsString>> {
        for n in 1..=MAX_SUFFIX {
            let name = OsString::from(if n == 1 {
                format!("{postscript_name} ({NAME_TAG})")
            } else {
                format!("{postscript_name} ({NAME_TAG} {n})")
            });
            if self.fonts.get_string(&name)?.is_none() && self.ledger_entry(&name)?.is_none() {
                return Ok(Some(name));
            }
        }
        Ok(None)
    }

    fn activate_one(
        &self,
        face: &FaceFile,
        mode: Mode,
        live: &mut Vec<Registration>,
    ) -> OutcomeStatus {
        if let Some(existing) = live.iter().find(|r| r.path == face.path) {
            return OutcomeStatus::AlreadyActive(existing.clone());
        }
        if !face.path.is_absolute() {
            return failed(
                ActivationErrorKind::Unsupported,
                format!("{}: activation needs a full path", face.path.display()),
            );
        }
        match face.path.metadata() {
            Ok(meta) if meta.is_file() => {}
            Ok(_) => {
                return failed(
                    ActivationErrorKind::FileNotFound,
                    format!("{}: not a file", face.path.display()),
                );
            }
            Err(e) => return OutcomeStatus::Failed(io_error(&e, &face.path.display().to_string())),
        }
        let base = value_name_base(face);
        let base = base.as_str();
        let name = match self.free_name(base) {
            Ok(Some(name)) => name,
            Ok(None) => {
                return failed(
                    ActivationErrorKind::Os,
                    format!("no free registry value name for {base}"),
                );
            }
            Err(e) => return OutcomeStatus::Failed(os_error(&e, "HKCU fonts key")),
        };
        // Ledger first: a crash between the two writes leaves a ledger entry
        // with no Fonts value, which `list_active` ignores; never a value the
        // app wrote without a record of it.
        if let Err(e) = self.ledger(mode).set_string(&name, face.path.as_os_str()) {
            return OutcomeStatus::Failed(os_error(&e, "HKCU activation ledger"));
        }
        if let Err(e) = self.fonts.set_string(&name, &registry_data(&face.path)) {
            let _ = self.ledger(mode).delete_value(&name);
            return OutcomeStatus::Failed(os_error(&e, "HKCU fonts key"));
        }
        let registration = Registration {
            path: face.path.clone(),
            mode,
            handle: name,
        };
        live.push(registration.clone());
        OutcomeStatus::Activated(registration)
    }

    fn deactivate_one(&self, registration: &Registration) -> OutcomeStatus {
        let name = registration.handle.as_os_str();
        if name.to_string_lossy().contains('\0') {
            // `activate` never issues such a name; Win32 would read a shorter one.
            return OutcomeStatus::NotOurs;
        }
        let (mode, recorded) = match self.ledger_entry(name) {
            Ok(Some(entry)) => entry,
            Ok(None) => return OutcomeStatus::NotOurs,
            Err(e) => return OutcomeStatus::Failed(os_error(&e, "HKCU activation ledger")),
        };
        if recorded != registration.path {
            return OutcomeStatus::NotOurs;
        }
        let current = match self.fonts.get_string(name) {
            Ok(current) => current,
            Err(e) => return OutcomeStatus::Failed(os_error(&e, "HKCU fonts key")),
        };
        let ours = current == Some(registry_data(&recorded));
        if ours && let Err(e) = self.fonts.delete_value(name) {
            return OutcomeStatus::Failed(os_error(&e, "HKCU fonts key"));
        }
        // The value is gone, or someone else replaced it: either way it is no
        // longer ours to track, and a replaced value stays as it is (rule 3).
        if let Err(e) = self.ledger(mode).delete_value(name) {
            return OutcomeStatus::Failed(os_error(&e, "HKCU activation ledger"));
        }
        if ours {
            OutcomeStatus::Deactivated
        } else {
            OutcomeStatus::NotActive
        }
    }
}

impl FontActivator for WindowsActivator {
    fn activate(&self, faces: &[FaceFile], mode: Mode) -> Result<Vec<Outcome>, ActivationError> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let keys = self.open()?;
        let mut live = keys
            .live_registrations()
            .map_err(|e| os_error(&e, "HKCU activation ledger"))?;
        let outcomes: Vec<Outcome> = faces
            .iter()
            .map(|face| Outcome {
                path: face.path.clone(),
                status: keys.activate_one(face, mode, &mut live),
            })
            .collect();
        if outcomes
            .iter()
            .any(|o| matches!(o.status, OutcomeStatus::Activated(_)))
        {
            self.broadcast_font_change();
        }
        Ok(outcomes)
    }

    fn deactivate(&self, registrations: &[Registration]) -> Result<Vec<Outcome>, ActivationError> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let keys = self.open()?;
        let outcomes: Vec<Outcome> = registrations
            .iter()
            .map(|registration| Outcome {
                path: registration.path.clone(),
                status: keys.deactivate_one(registration),
            })
            .collect();
        if outcomes
            .iter()
            .any(|o| o.status == OutcomeStatus::Deactivated)
        {
            self.broadcast_font_change();
        }
        Ok(outcomes)
    }

    fn list_active(&self) -> Result<Vec<Registration>, ActivationError> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        self.open()?
            .live_registrations()
            .map_err(|e| os_error(&e, "HKCU activation ledger"))
    }

    fn conflicts(&self, faces: &[FaceFile]) -> Result<Vec<Conflict>, ActivationError> {
        let names: HashSet<&str> = faces.iter().map(|f| f.postscript_name.as_str()).collect();
        let found = dwrite::fonts_named(&names).map_err(|e| {
            ActivationError::new(
                ActivationErrorKind::Unsupported,
                format!("DirectWrite: {e}"),
            )
        })?;
        let system_fonts = system_fonts_dir();
        let mut conflicts = Vec::new();
        for face in faces {
            for (name, with) in &found {
                if *name != face.postscript_name || same_path(with, &face.path) {
                    continue;
                }
                let kind = match &system_fonts {
                    Some(dir) if is_under(with, dir) => ConflictKind::System,
                    _ => ConflictKind::User,
                };
                conflicts.push(Conflict {
                    path: face.path.clone(),
                    postscript_name: name.clone(),
                    with: with.clone(),
                    kind,
                });
            }
        }
        Ok(conflicts)
    }
}

/// The start of the registry value name for `face`: its PostScript name
/// without control characters (a malformed name table can hold a NUL, which
/// Win32 would read as the end of the name), else the file name, else "Font".
fn value_name_base(face: &FaceFile) -> String {
    let clean = |s: &str| -> String {
        s.chars()
            .filter(|c| !c.is_control())
            .collect::<String>()
            .trim()
            .to_owned()
    };
    let from_name = clean(&face.postscript_name);
    if !from_name.is_empty() {
        return from_name;
    }
    let from_file = face
        .path
        .file_stem()
        .map(|s| clean(&s.to_string_lossy()))
        .unwrap_or_default();
    if from_file.is_empty() {
        "Font".to_owned()
    } else {
        from_file
    }
}

/// The Fonts value data for `path`: the path itself, without a verbatim
/// `\\?\` prefix on a drive path, which `canonicalize` adds and font loaders
/// may not read.
fn registry_data(path: &Path) -> OsString {
    let mut components = path.components();
    if let Some(Component::Prefix(prefix)) = components.next()
        && let Prefix::VerbatimDisk(drive) = prefix.kind()
    {
        let mut data = OsString::from(format!("{}:", char::from(drive)));
        data.push(components.as_path().as_os_str());
        return data;
    }
    path.as_os_str().to_owned()
}

/// `%WINDIR%\Fonts`, where system-wide fonts live.
fn system_fonts_dir() -> Option<PathBuf> {
    std::env::var_os("WINDIR")
        .or_else(|| std::env::var_os("SystemRoot"))
        .map(|w| PathBuf::from(w).join("Fonts"))
}

/// Windows paths compare without case.
fn same_path(a: &Path, b: &Path) -> bool {
    a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
}

fn is_under(path: &Path, dir: &Path) -> bool {
    path.parent().is_some_and(|parent| same_path(parent, dir))
}

fn failed(kind: ActivationErrorKind, detail: String) -> OutcomeStatus {
    OutcomeStatus::Failed(ActivationError::new(kind, detail))
}

/// A registry error: access denied is the user's settings, the rest is the OS.
fn os_error(e: &io::Error, what: &str) -> ActivationError {
    let kind = match e.kind() {
        io::ErrorKind::PermissionDenied => ActivationErrorKind::PermissionDenied,
        _ => ActivationErrorKind::Os,
    };
    ActivationError::new(kind, format!("{what}: {e}"))
}

/// A font-file error.
fn io_error(e: &io::Error, what: &str) -> ActivationError {
    let kind = match e.kind() {
        io::ErrorKind::NotFound => ActivationErrorKind::FileNotFound,
        io::ErrorKind::PermissionDenied => ActivationErrorKind::PermissionDenied,
        _ => ActivationErrorKind::Os,
    };
    ActivationError::new(kind, format!("{what}: {e}"))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::windows::ffi::OsStringExt;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    /// Scratch `HKCU` keys for one test, deleted on drop. The real Fonts key
    /// and the app's ledger are never touched.
    struct Scratch {
        root: String,
        activator: WindowsActivator,
        dir: PathBuf,
    }

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let id = format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let root = format!(r"Software\FontManager-test\{id}");
            let activator = WindowsActivator::with_keys(
                &format!(r"{root}\Fonts"),
                &format!(r"{root}\Ledger"),
                false,
            );
            let dir = std::env::temp_dir().join(format!("fm-platform-win-{id}"));
            let _ = fs::create_dir_all(&dir);
            Self {
                root,
                activator,
                dir,
            }
        }

        fn font(&self, file: &str) -> PathBuf {
            let path = self.dir.join(file);
            let _ = fs::write(&path, b"not parsed by the activator");
            path
        }

        fn fonts(&self) -> Key {
            Key::create_current_user(&format!(r"{}\Fonts", self.root)).expect("scratch key")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = Key::delete_current_user_tree(&self.root);
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn face(path: &Path, postscript_name: &str) -> FaceFile {
        FaceFile {
            path: path.to_path_buf(),
            postscript_name: postscript_name.to_owned(),
        }
    }

    fn activated(outcomes: Result<Vec<Outcome>, ActivationError>) -> Vec<Registration> {
        outcomes
            .unwrap_or_default()
            .into_iter()
            .filter_map(|o| match o.status {
                OutcomeStatus::Activated(r) => Some(r),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn activation_writes_a_value_with_the_path_in_place() {
        let s = Scratch::new();
        let path = s.font("A.ttf");
        let regs = activated(
            s.activator
                .activate(&[face(&path, "A-Regular")], Mode::Permanent),
        );

        assert_eq!(regs.len(), 1);
        assert_eq!(regs[0].handle, OsString::from("A-Regular (Font Manager)"));
        assert_eq!(regs[0].mode, Mode::Permanent);
        assert_eq!(
            s.fonts().get_string(&regs[0].handle).ok().flatten(),
            Some(path.clone().into_os_string())
        );
        assert_eq!(s.activator.list_active(), Ok(regs));
        assert!(path.exists(), "the file stays where it is");
    }

    #[test]
    fn a_second_activation_of_the_same_file_is_already_active() {
        let s = Scratch::new();
        let path = s.font("A.ttc");
        let outcomes = s
            .activator
            .activate(
                &[face(&path, "A-Regular"), face(&path, "A-Bold")],
                Mode::Session,
            )
            .unwrap_or_default();

        let OutcomeStatus::Activated(first) = &outcomes[0].status else {
            panic!("first face activates: {outcomes:?}");
        };
        assert_eq!(
            outcomes[1].status,
            OutcomeStatus::AlreadyActive(first.clone())
        );
        assert_eq!(s.activator.list_active().map(|l| l.len()), Ok(1));
    }

    #[test]
    fn a_name_taken_by_a_foreign_value_gets_a_suffix_and_the_foreign_value_stays() {
        let s = Scratch::new();
        let foreign = OsString::from("A-Regular (Font Manager)");
        let _ = s
            .fonts()
            .set_string(&foreign, OsStr::new(r"C:\Other\A.ttf"));
        let path = s.font("A.ttf");

        let regs = activated(
            s.activator
                .activate(&[face(&path, "A-Regular")], Mode::Permanent),
        );
        assert_eq!(regs[0].handle, OsString::from("A-Regular (Font Manager 2)"));

        // The foreign value is not ours: deactivating it by name does nothing.
        let foreign_reg = Registration {
            path: PathBuf::from(r"C:\Other\A.ttf"),
            mode: Mode::Permanent,
            handle: foreign.clone(),
        };
        let outcomes = s.activator.deactivate(&[foreign_reg]).unwrap_or_default();
        assert_eq!(outcomes[0].status, OutcomeStatus::NotOurs);
        assert_eq!(
            s.fonts().get_string(&foreign).ok().flatten(),
            Some(OsString::from(r"C:\Other\A.ttf"))
        );
        // Nor does the full clean-up.
        let _ = s.activator.deactivate(&regs);
        assert!(s.fonts().get_string(&foreign).ok().flatten().is_some());
    }

    #[test]
    fn two_files_with_one_postscript_name_get_two_values() {
        let s = Scratch::new();
        let a = s.font("A1.otf");
        let b = s.font("A2.otf");
        let regs = activated(s.activator.activate(
            &[face(&a, "A-Regular"), face(&b, "A-Regular")],
            Mode::Permanent,
        ));
        assert_eq!(regs.len(), 2);
        assert_ne!(regs[0].handle, regs[1].handle);
    }

    #[test]
    fn deactivate_removes_exactly_the_value_it_wrote() {
        let s = Scratch::new();
        let _ = s
            .fonts()
            .set_string(OsStr::new("Arial (TrueType)"), OsStr::new("arial.ttf"));
        let path = s.font("A.ttf");
        let regs = activated(s.activator.activate(&[face(&path, "A")], Mode::Permanent));

        let outcomes = s.activator.deactivate(&regs).unwrap_or_default();
        assert_eq!(outcomes[0].status, OutcomeStatus::Deactivated);
        assert_eq!(s.fonts().get_string(&regs[0].handle).ok().flatten(), None);
        assert!(
            s.fonts()
                .get_string(OsStr::new("Arial (TrueType)"))
                .ok()
                .flatten()
                .is_some()
        );
        assert_eq!(s.activator.list_active(), Ok(vec![]));
        assert!(path.exists(), "deactivation never touches the file");
    }

    #[test]
    fn a_value_removed_or_replaced_behind_the_apps_back_is_not_active_and_not_deleted() {
        let s = Scratch::new();
        let a = s.font("A.ttf");
        let b = s.font("B.ttf");
        let regs = activated(
            s.activator
                .activate(&[face(&a, "A"), face(&b, "B")], Mode::Permanent),
        );
        let _ = s.fonts().delete_value(&regs[0].handle);
        let _ = s
            .fonts()
            .set_string(&regs[1].handle, OsStr::new(r"C:\Else\B.ttf"));

        assert_eq!(s.activator.list_active(), Ok(vec![]));
        let outcomes = s.activator.deactivate(&regs).unwrap_or_default();
        assert_eq!(outcomes[0].status, OutcomeStatus::NotActive);
        assert_eq!(outcomes[1].status, OutcomeStatus::NotActive);
        assert_eq!(
            s.fonts().get_string(&regs[1].handle).ok().flatten(),
            Some(OsString::from(r"C:\Else\B.ttf"))
        );
    }

    #[test]
    fn a_registration_with_another_path_is_not_ours() {
        let s = Scratch::new();
        let path = s.font("A.ttf");
        let mut regs = activated(s.activator.activate(&[face(&path, "A")], Mode::Permanent));
        regs[0].path = s.dir.join("Other.ttf");
        let outcomes = s.activator.deactivate(&regs).unwrap_or_default();
        assert_eq!(outcomes[0].status, OutcomeStatus::NotOurs);
        assert_eq!(s.activator.list_active().map(|l| l.len()), Ok(1));
    }

    #[test]
    fn session_values_are_listed_and_cleared_permanent_ones_stay() {
        let s = Scratch::new();
        let session = s.font("S.ttf");
        let permanent = s.font("P.ttf");
        let _ = s.activator.activate(&[face(&session, "S")], Mode::Session);
        let _ = s
            .activator
            .activate(&[face(&permanent, "P")], Mode::Permanent);

        // A new activator stands for the next start after a crash.
        let next = WindowsActivator::with_keys(
            &format!(r"{}\Fonts", s.root),
            &format!(r"{}\Ledger", s.root),
            false,
        );
        let cleared = next.clear_session().unwrap_or_default();
        assert_eq!(cleared.len(), 1);
        assert_eq!(cleared[0].path, session);
        assert_eq!(cleared[0].status, OutcomeStatus::Deactivated);

        let left = next.list_active().unwrap_or_default();
        assert_eq!(left.len(), 1);
        assert_eq!(
            (left[0].path.clone(), left[0].mode),
            (permanent, Mode::Permanent)
        );
    }

    #[test]
    fn a_missing_file_or_a_relative_path_fails_that_face_only() {
        let s = Scratch::new();
        let good = s.font("A.ttf");
        let outcomes = s
            .activator
            .activate(
                &[
                    face(&s.dir.join("gone.ttf"), "Gone"),
                    face(Path::new(r"fonts\rel.ttf"), "Rel"),
                    face(&s.dir, "Folder"),
                    face(&good, "A"),
                ],
                Mode::Session,
            )
            .unwrap_or_default();
        let kinds: Vec<_> = outcomes
            .iter()
            .map(|o| match &o.status {
                OutcomeStatus::Failed(e) => Some(e.kind),
                _ => None,
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                Some(ActivationErrorKind::FileNotFound),
                Some(ActivationErrorKind::Unsupported),
                Some(ActivationErrorKind::FileNotFound),
                None
            ]
        );
        assert_eq!(s.activator.list_active().map(|l| l.len()), Ok(1));
    }

    #[test]
    fn a_non_utf8_path_round_trips_through_the_registry() {
        let s = Scratch::new();
        // "X" + an unpaired surrogate + ".ttf": a legal NTFS name, not UTF-8.
        let name = OsString::from_wide(&[0x58, 0xD800, 0x2E, 0x74, 0x74, 0x66]);
        let path = s.dir.join(&name);
        if fs::write(&path, b"x").is_err() {
            return; // a file system without such names; nothing to prove here
        }
        let regs = activated(s.activator.activate(&[face(&path, "X")], Mode::Permanent));
        assert_eq!(regs.len(), 1);
        assert_eq!(regs[0].path, path);
        assert_eq!(s.activator.list_active(), Ok(regs.clone()));
        let outcomes = s.activator.deactivate(&regs).unwrap_or_default();
        assert_eq!(outcomes[0].status, OutcomeStatus::Deactivated);
    }

    #[test]
    fn an_empty_postscript_name_falls_back_to_the_file_name() {
        let s = Scratch::new();
        let path = s.font("NoName.ttf");
        let regs = activated(s.activator.activate(&[face(&path, " ")], Mode::Session));
        assert_eq!(regs[0].handle, OsString::from("NoName (Font Manager)"));
    }

    #[test]
    fn system_fonts_conflict_as_system_and_a_file_never_with_itself() {
        let Some(dir) = system_fonts_dir() else {
            return;
        };
        let arial = dir.join("arial.ttf");
        if !arial.exists() {
            return; // a Windows image without Arial
        }
        let s = Scratch::new();
        let mine = s.font("MyArial.ttf");
        let conflicts = s
            .activator
            .conflicts(&[face(&mine, "ArialMT"), face(&arial, "ArialMT")])
            .unwrap_or_default();
        assert!(
            conflicts.iter().any(|c| c.path == mine
                && same_path(&c.with, &arial)
                && c.kind == ConflictKind::System),
            "{conflicts:?}"
        );
        assert!(!conflicts.iter().any(|c| same_path(&c.path, &c.with)));
    }

    #[test]
    fn no_faces_no_conflicts() {
        let s = Scratch::new();
        assert_eq!(s.activator.conflicts(&[]), Ok(vec![]));
    }

    #[test]
    fn registry_data_drops_only_a_verbatim_drive_prefix() {
        assert_eq!(
            registry_data(Path::new(r"\\?\C:\Fonts\A.ttf")),
            OsString::from(r"C:\Fonts\A.ttf")
        );
        for kept in [
            r"C:\Fonts\A.ttf",
            r"\\server\share\A.ttf",
            r"\\?\UNC\server\share\A.ttf",
        ] {
            assert_eq!(registry_data(Path::new(kept)), OsString::from(kept));
        }
    }

    #[test]
    fn a_verbatim_path_is_written_plain_and_still_ours() {
        let s = Scratch::new();
        let path = s.font("V.ttf");
        let verbatim = PathBuf::from(format!(r"\\?\{}", path.display()));
        let regs = activated(
            s.activator
                .activate(&[face(&verbatim, "V")], Mode::Permanent),
        );
        assert_eq!(regs[0].path, verbatim);
        assert_eq!(
            s.fonts().get_string(&regs[0].handle).ok().flatten(),
            Some(path.into_os_string())
        );
        assert_eq!(s.activator.list_active(), Ok(regs.clone()));
        let outcomes = s.activator.deactivate(&regs).unwrap_or_default();
        assert_eq!(outcomes[0].status, OutcomeStatus::Deactivated);
    }

    /// A malformed name table can hold a NUL in the PostScript name. Win32
    /// stops reading a name there, so the handle must never contain one.
    #[test]
    fn a_postscript_name_with_nul_or_control_characters_gets_a_clean_value_name() {
        let s = Scratch::new();
        let a = s.font("A.ttf");
        let b = s.font("B.ttf");
        let c = s.font("C.ttf");
        let regs = activated(s.activator.activate(
            &[face(&a, "Foo\0"), face(&b, "Foo\0Bold"), face(&c, "\0\t")],
            Mode::Session,
        ));

        let handles: Vec<_> = regs.iter().map(|r| r.handle.clone()).collect();
        assert_eq!(
            handles,
            vec![
                OsString::from("Foo (Font Manager)"),
                OsString::from("FooBold (Font Manager)"),
                OsString::from("C (Font Manager)"),
            ]
        );
        assert_eq!(s.activator.list_active(), Ok(regs.clone()));
        // The unnamed (Default) value of the Fonts key is never written.
        assert_eq!(s.fonts().get_string(OsStr::new("")).ok().flatten(), None);
        let outcomes = s.activator.deactivate(&regs).unwrap_or_default();
        assert!(
            outcomes
                .iter()
                .all(|o| o.status == OutcomeStatus::Deactivated)
        );
    }

    #[test]
    fn a_handle_with_a_nul_is_not_ours_and_deletes_nothing() {
        let s = Scratch::new();
        let path = s.font("Foo.ttf");
        let regs = activated(s.activator.activate(&[face(&path, "Foo")], Mode::Permanent));
        let forged = Registration {
            path: path.clone(),
            mode: Mode::Permanent,
            handle: OsString::from("Foo (Font Manager)\0tail"),
        };
        let outcomes = s.activator.deactivate(&[forged]).unwrap_or_default();
        assert_eq!(outcomes[0].status, OutcomeStatus::NotOurs);
        assert_eq!(s.activator.list_active(), Ok(regs));
    }

    #[test]
    fn the_registry_wrapper_refuses_a_nul_inside_a_name_or_data() {
        let s = Scratch::new();
        let fonts = s.fonts();
        let err = fonts.set_string(OsStr::new("A\0B"), OsStr::new("x"));
        assert_eq!(err.map_err(|e| e.kind()), Err(io::ErrorKind::InvalidInput));
        let err = fonts.set_string(OsStr::new("A"), OsStr::new("C:\\a\0b"));
        assert_eq!(err.map_err(|e| e.kind()), Err(io::ErrorKind::InvalidInput));
        assert_eq!(fonts.get_string(OsStr::new("A")).ok().flatten(), None);
    }
}
