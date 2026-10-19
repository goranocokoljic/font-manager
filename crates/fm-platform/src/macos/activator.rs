//! The macOS [`FontActivator`]: CoreText registration of the file in place (ADR 0008).
//!
//! Activation calls `CTFontManagerRegisterFontsForURL` with the session scope
//! for [`Mode::Session`] and the user scope for [`Mode::Permanent`]; new and
//! running apps see the font within about a second, and no admin rights are
//! needed. Deactivation calls `CTFontManagerUnregisterFontsForURL` with the
//! same scope. A session registration outlives the app until logout, so
//! [`FontActivator::clear_session`] runs on quit and again at the next start.
//!
//! Rule 3: CoreText keeps no record of which app registered a file, so the
//! activator keeps a [ledger](super::ledger) in the app's data folder. A file
//! is ours only when the ledger lists it in the registration's mode; anything
//! else is never unregistered. [`FontActivator::list_active`] is the ledger:
//! CoreText cannot list another process's session registrations reliably, and
//! one dropped by a logout comes back [`OutcomeStatus::NotActive`] from
//! `deactivate` (ADR 0008).
//!
//! The logic here runs on every OS against a [`Registrar`]; only the CoreText
//! registrar is macOS code.

use std::collections::HashSet;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use super::ledger::Ledger;
use crate::activator::{
    ActivationError, ActivationErrorKind, Conflict, ConflictKind, FaceFile, FontActivator, Mode,
    Outcome, OutcomeStatus, Registration,
};
#[cfg(target_os = "macos")]
use crate::paths::AppPaths;

/// `CTFontManagerError` codes (CoreText `CTFontManagerErrors.h`).
pub(crate) mod code {
    pub(crate) const FILE_NOT_FOUND: isize = 101;
    pub(crate) const INSUFFICIENT_PERMISSIONS: isize = 102;
    pub(crate) const UNRECOGNIZED_FORMAT: isize = 103;
    pub(crate) const INVALID_FONT_DATA: isize = 104;
    pub(crate) const ALREADY_REGISTERED: isize = 105;
    pub(crate) const EXCEEDED_RESOURCE_LIMIT: isize = 106;
    pub(crate) const NOT_REGISTERED: isize = 201;
    pub(crate) const IN_USE: isize = 202;
    pub(crate) const SYSTEM_REQUIRED: isize = 203;
    pub(crate) const REGISTRATION_FAILED: isize = 301;
    pub(crate) const MISSING_ENTITLEMENT: isize = 302;
    pub(crate) const CANCELLED_BY_USER: isize = 304;
    pub(crate) const INVALID_FILE_PATH: isize = 306;
    pub(crate) const UNSUPPORTED_SCOPE: isize = 307;
}

/// A failed CoreText call: the `CFError` code and its description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegistrarError {
    pub(crate) code: isize,
    pub(crate) detail: String,
}

/// The OS font registration the activator drives: CoreText on macOS, a fake in tests.
pub(crate) trait Registrar: Send + Sync + fmt::Debug {
    /// Registers the file at `path` in the scope for `mode`.
    fn register(&self, path: &Path, mode: Mode) -> Result<(), RegistrarError>;
    /// Unregisters the file at `path` from the scope for `mode`.
    fn unregister(&self, path: &Path, mode: Mode) -> Result<(), RegistrarError>;
    /// Registers every file of `paths`; one result per path, in order. One
    /// CoreText call per file took most of the 100-face activation budget,
    /// so CoreText overrides this with a single call.
    fn register_all(&self, paths: &[&Path], mode: Mode) -> Vec<Result<(), RegistrarError>> {
        paths.iter().map(|p| self.register(p, mode)).collect()
    }
    /// Unregisters every file of `paths`; one result per path, in order.
    fn unregister_all(&self, paths: &[&Path], mode: Mode) -> Vec<Result<(), RegistrarError>> {
        paths.iter().map(|p| self.unregister(p, mode)).collect()
    }
    /// Every font the OS offers whose PostScript name is in `names`, with its file.
    fn fonts_named(&self, names: &HashSet<&str>) -> Result<Vec<(String, PathBuf)>, String>;
}

/// Activates fonts for the current user through CoreText. See the module docs.
#[derive(Debug)]
pub struct MacosActivator {
    ledger_file: PathBuf,
    registrar: Box<dyn Registrar>,
    /// Serialises ledger read-modify-write sequences within the process.
    lock: Mutex<()>,
}

impl MacosActivator {
    /// The activator the app uses: CoreText, with the ledger in the app's data folder.
    #[cfg(target_os = "macos")]
    pub fn new(paths: &AppPaths) -> Self {
        Self::with_ledger(paths.data.join(super::ledger::LEDGER_FILE))
    }

    /// CoreText, with the ledger at `ledger_file`. Tests use a scratch file so
    /// they never see or change the app's own records.
    #[cfg(target_os = "macos")]
    pub fn with_ledger(ledger_file: PathBuf) -> Self {
        Self::with_registrar(ledger_file, Box::new(super::coretext::CoreText))
    }

    pub(crate) fn with_registrar(ledger_file: PathBuf, registrar: Box<dyn Registrar>) -> Self {
        Self {
            ledger_file,
            registrar,
            lock: Mutex::new(()),
        }
    }

    fn load(&self) -> Result<Ledger, ActivationError> {
        Ledger::load(&self.ledger_file).map_err(|e| self.ledger_error(&e))
    }

    fn save(&self, ledger: &Ledger) -> Result<(), ActivationError> {
        ledger
            .save(&self.ledger_file)
            .map_err(|e| self.ledger_error(&e))
    }

    fn ledger_error(&self, e: &io::Error) -> ActivationError {
        let kind = match e.kind() {
            io::ErrorKind::PermissionDenied => ActivationErrorKind::PermissionDenied,
            _ => ActivationErrorKind::Os,
        };
        ActivationError::new(
            kind,
            format!("activation ledger {}: {e}", self.ledger_file.display()),
        )
    }

    /// Why `face` cannot be registered, before anything is written.
    fn check(face: &FaceFile) -> Option<OutcomeStatus> {
        if !face.path.is_absolute() {
            return Some(failed(
                ActivationErrorKind::Unsupported,
                format!("{}: activation needs a full path", face.path.display()),
            ));
        }
        match face.path.metadata() {
            Ok(meta) if meta.is_file() => None,
            Ok(_) => Some(failed(
                ActivationErrorKind::FileNotFound,
                format!("{}: not a file", face.path.display()),
            )),
            Err(e) => Some(OutcomeStatus::Failed(io_error(&e, &face.path))),
        }
    }
}

impl FontActivator for MacosActivator {
    fn activate(&self, faces: &[FaceFile], mode: Mode) -> Result<Vec<Outcome>, ActivationError> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let mut ledger = self.load()?;
        let mut statuses: Vec<Option<OutcomeStatus>> = vec![None; faces.len()];
        // The faces to register (one per file) and, for a later face of the
        // same file, the face whose outcome it shares.
        let mut first: Vec<usize> = Vec::new();
        let mut same_file: Vec<(usize, usize)> = Vec::new();
        for (i, face) in faces.iter().enumerate() {
            if let Some(existing) = ledger.mode_of(&face.path) {
                match first.iter().find(|&&j| faces[j].path == face.path) {
                    Some(&j) => same_file.push((i, j)),
                    None => {
                        statuses[i] = Some(OutcomeStatus::AlreadyActive(registration(
                            &face.path, existing,
                        )));
                    }
                }
                continue;
            }
            if let Some(status) = Self::check(face) {
                statuses[i] = Some(status);
                continue;
            }
            ledger.insert(&face.path, mode);
            first.push(i);
        }
        // Ledger first, once for the whole request (each save is a full
        // flush, slow on APFS): a crash before a registration leaves an entry
        // with none, which `deactivate` answers `NotActive`; never a
        // registration the app made without a record of it.
        if !first.is_empty()
            && let Err(e) = self.save(&ledger)
        {
            for &i in &first {
                statuses[i] = Some(OutcomeStatus::Failed(e.clone()));
            }
            first.clear();
        }
        let mut dropped = false;
        let paths: Vec<&Path> = first.iter().map(|&i| faces[i].path.as_path()).collect();
        let results = self.registrar.register_all(&paths, mode);
        for (&i, result) in first.iter().zip(results) {
            let path = &faces[i].path;
            statuses[i] = Some(match result {
                Ok(()) => OutcomeStatus::Activated(registration(path, mode)),
                Err(e) => {
                    ledger.remove(path);
                    dropped = true;
                    OutcomeStatus::Failed(error_for(&e, path))
                }
            });
        }
        if dropped {
            // The entries stay in the file only if this save fails too; then
            // `deactivate` finds no registration and drops them.
            let _ = self.save(&ledger);
        }
        for (i, j) in same_file {
            statuses[i] = statuses[j].clone().map(|status| match status {
                OutcomeStatus::Activated(reg) => OutcomeStatus::AlreadyActive(reg),
                other => other,
            });
        }
        Ok(faces
            .iter()
            .zip(statuses)
            .map(|(face, status)| Outcome {
                path: face.path.clone(),
                status: status
                    .unwrap_or_else(|| failed(ActivationErrorKind::Os, "no outcome".to_owned())),
            })
            .collect())
    }

    fn deactivate(&self, registrations: &[Registration]) -> Result<Vec<Outcome>, ActivationError> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let mut ledger = self.load()?;
        let mut statuses: Vec<Option<OutcomeStatus>> = vec![None; registrations.len()];
        // Ours, grouped by mode: CoreText unregisters per scope.
        let mut ours: Vec<usize> = Vec::new();
        for (i, reg) in registrations.iter().enumerate() {
            // `activate` sets the handle to the path; anything else it never issued.
            if reg.handle != reg.path.as_os_str() || ledger.mode_of(&reg.path) != Some(reg.mode) {
                statuses[i] = Some(OutcomeStatus::NotOurs);
            } else {
                ours.push(i);
            }
        }
        let mut removed: Vec<usize> = Vec::new();
        for mode in [Mode::Session, Mode::Permanent] {
            let batch: Vec<usize> = ours
                .iter()
                .copied()
                .filter(|&i| registrations[i].mode == mode)
                .collect();
            if batch.is_empty() {
                continue;
            }
            let paths: Vec<&Path> = batch
                .iter()
                .map(|&i| registrations[i].path.as_path())
                .collect();
            for (&i, result) in batch
                .iter()
                .zip(self.registrar.unregister_all(&paths, mode))
            {
                let reg = &registrations[i];
                let status = match result {
                    Ok(()) => OutcomeStatus::Deactivated,
                    // Gone already: the user logged out (session scope) or removed it.
                    Err(e) if e.code == code::NOT_REGISTERED => OutcomeStatus::NotActive,
                    Err(e) => OutcomeStatus::Failed(error_for(&e, &reg.path)),
                };
                if !matches!(status, OutcomeStatus::Failed(_)) {
                    ledger.remove(&reg.path);
                    removed.push(i);
                }
                statuses[i] = Some(status);
            }
        }
        // One save for the whole request; the OS side is done either way, and
        // a stale entry answers `NotActive` next time.
        if !removed.is_empty()
            && let Err(e) = self.save(&ledger)
        {
            for i in removed {
                statuses[i] = Some(OutcomeStatus::Failed(e.clone()));
            }
        }
        Ok(registrations
            .iter()
            .zip(statuses)
            .map(|(reg, status)| Outcome {
                path: reg.path.clone(),
                status: status
                    .unwrap_or_else(|| failed(ActivationErrorKind::Os, "no outcome".to_owned())),
            })
            .collect())
    }

    fn list_active(&self) -> Result<Vec<Registration>, ActivationError> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        Ok(self
            .load()?
            .entries()
            .map(|(path, mode)| registration(path, mode))
            .collect())
    }

    fn conflicts(&self, faces: &[FaceFile]) -> Result<Vec<Conflict>, ActivationError> {
        let names: HashSet<&str> = faces.iter().map(|f| f.postscript_name.as_str()).collect();
        if names.is_empty() {
            return Ok(Vec::new());
        }
        let found = self.registrar.fonts_named(&names).map_err(|e| {
            ActivationError::new(ActivationErrorKind::Unsupported, format!("CoreText: {e}"))
        })?;
        let mut conflicts = Vec::new();
        for face in faces {
            for (name, with) in &found {
                if *name != face.postscript_name || same_file(with, &face.path) {
                    continue;
                }
                conflicts.push(Conflict {
                    path: face.path.clone(),
                    postscript_name: name.clone(),
                    with: with.clone(),
                    kind: conflict_kind(with),
                });
            }
        }
        Ok(conflicts)
    }
}

fn registration(path: &Path, mode: Mode) -> Registration {
    Registration {
        path: path.to_owned(),
        mode,
        handle: path.as_os_str().to_owned(),
    }
}

/// Folders only an administrator or the OS writes to: a font there is a system font.
const SYSTEM_FONT_DIRS: [&str; 4] = [
    "/System/Library/",
    "/Library/Fonts/",
    "/Network/Library/Fonts/",
    "/System/Library/AssetsV2/",
];

fn conflict_kind(with: &Path) -> ConflictKind {
    let with = with.to_string_lossy();
    if SYSTEM_FONT_DIRS.iter().any(|dir| with.starts_with(dir)) {
        ConflictKind::System
    } else {
        ConflictKind::User
    }
}

/// The same file, also through a symbolic link or a different spelling.
fn same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// The [`ActivationError`] for a failed CoreText call on `path`.
pub(crate) fn error_for(e: &RegistrarError, path: &Path) -> ActivationError {
    let kind = match e.code {
        code::FILE_NOT_FOUND | code::INVALID_FILE_PATH => ActivationErrorKind::FileNotFound,
        code::INSUFFICIENT_PERMISSIONS | code::CANCELLED_BY_USER => {
            ActivationErrorKind::PermissionDenied
        }
        code::UNRECOGNIZED_FORMAT
        | code::INVALID_FONT_DATA
        | code::MISSING_ENTITLEMENT
        | code::UNSUPPORTED_SCOPE => ActivationErrorKind::Unsupported,
        _ => ActivationErrorKind::Os,
    };
    let note = match e.code {
        // Not in the ledger, so another app (or Font Book) registered it: not ours (rule 3).
        code::ALREADY_REGISTERED => " (registered by another app)",
        code::IN_USE | code::SYSTEM_REQUIRED => " (in use by the system)",
        code::EXCEEDED_RESOURCE_LIMIT => " (too many fonts registered)",
        code::REGISTRATION_FAILED => " (registration failed)",
        _ => "",
    };
    ActivationError::new(
        kind,
        format!(
            "{}: CoreText error {}{note}: {}",
            path.display(),
            e.code,
            e.detail
        ),
    )
}

fn failed(kind: ActivationErrorKind, detail: String) -> OutcomeStatus {
    OutcomeStatus::Failed(ActivationError::new(kind, detail))
}

/// A font-file error.
fn io_error(e: &io::Error, path: &Path) -> ActivationError {
    let kind = match e.kind() {
        io::ErrorKind::NotFound => ActivationErrorKind::FileNotFound,
        io::ErrorKind::PermissionDenied => ActivationErrorKind::PermissionDenied,
        _ => ActivationErrorKind::Os,
    };
    ActivationError::new(kind, format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::macos::ledger::LEDGER_FILE;

    /// A CoreText stand-in: what is registered in which mode, and the error
    /// the next register or unregister of a file returns.
    #[derive(Debug, Default)]
    struct Fake {
        registered: Mutex<HashMap<PathBuf, Mode>>,
        fail: Mutex<HashMap<PathBuf, isize>>,
        fonts: Vec<(String, PathBuf)>,
    }

    impl Fake {
        fn take_failure(&self, path: &Path) -> Option<RegistrarError> {
            let code = self.fail.lock().ok()?.remove(path)?;
            Some(RegistrarError {
                code,
                detail: "fake".to_owned(),
            })
        }

        fn mode_of(&self, path: &Path) -> Option<Mode> {
            self.registered.lock().ok()?.get(path).copied()
        }
    }

    impl Registrar for Arc<Fake> {
        fn register(&self, path: &Path, mode: Mode) -> Result<(), RegistrarError> {
            if let Some(e) = self.take_failure(path) {
                return Err(e);
            }
            let mut registered = self
                .registered
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if registered.contains_key(path) {
                return Err(RegistrarError {
                    code: code::ALREADY_REGISTERED,
                    detail: "fake".to_owned(),
                });
            }
            registered.insert(path.to_owned(), mode);
            Ok(())
        }

        fn unregister(&self, path: &Path, mode: Mode) -> Result<(), RegistrarError> {
            if let Some(e) = self.take_failure(path) {
                return Err(e);
            }
            let mut registered = self
                .registered
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if registered.get(path) != Some(&mode) {
                return Err(RegistrarError {
                    code: code::NOT_REGISTERED,
                    detail: "fake".to_owned(),
                });
            }
            registered.remove(path);
            Ok(())
        }

        fn fonts_named(&self, names: &HashSet<&str>) -> Result<Vec<(String, PathBuf)>, String> {
            Ok(self
                .fonts
                .iter()
                .filter(|(n, _)| names.contains(n.as_str()))
                .cloned()
                .collect())
        }
    }

    struct Scratch {
        dir: PathBuf,
        fake: Arc<Fake>,
    }

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let dir = std::env::temp_dir().join(format!(
                "fm-platform-mac-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::create_dir_all(&dir);
            Self {
                dir,
                fake: Arc::default(),
            }
        }

        /// A fresh activator over the same ledger and fake CoreText: what the
        /// app is after a restart.
        fn activator(&self) -> MacosActivator {
            MacosActivator::with_registrar(
                self.dir.join("data").join(LEDGER_FILE),
                Box::new(Arc::clone(&self.fake)),
            )
        }

        fn font(&self, file: &str) -> PathBuf {
            let path = self.dir.join(file);
            let _ = fs::write(&path, b"not parsed by the activator");
            path
        }

        fn fail_next(&self, path: &Path, code: isize) {
            self.fake
                .fail
                .lock()
                .expect("fake lock")
                .insert(path.to_owned(), code);
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn face(path: &Path, ps: &str) -> FaceFile {
        FaceFile {
            path: path.to_owned(),
            postscript_name: ps.to_owned(),
        }
    }

    fn activated(outcome: &Outcome) -> Registration {
        match &outcome.status {
            OutcomeStatus::Activated(r) => r.clone(),
            other => panic!("not activated: {other:?}"),
        }
    }

    fn failed_kind(outcome: &Outcome) -> ActivationErrorKind {
        match &outcome.status {
            OutcomeStatus::Failed(e) => e.kind,
            other => panic!("not failed: {other:?}"),
        }
    }

    #[test]
    fn modes_register_in_their_own_scope_and_unregister_from_it() {
        let s = Scratch::new();
        let a = s.activator();
        let session = s.font("Session.ttf");
        let permanent = s.font("Permanent.otf");

        let out = a
            .activate(&[face(&session, "S")], Mode::Session)
            .expect("works");
        let session_reg = activated(&out[0]);
        let out = a
            .activate(&[face(&permanent, "P")], Mode::Permanent)
            .expect("works");
        let permanent_reg = activated(&out[0]);
        assert_eq!(s.fake.mode_of(&session), Some(Mode::Session));
        assert_eq!(s.fake.mode_of(&permanent), Some(Mode::Permanent));
        assert_eq!(session_reg.handle, session.as_os_str());

        let out = a.deactivate(&[session_reg, permanent_reg]).expect("works");
        assert_eq!(out[0].status, OutcomeStatus::Deactivated);
        assert_eq!(out[1].status, OutcomeStatus::Deactivated);
        assert_eq!(s.fake.mode_of(&session), None);
        assert_eq!(s.fake.mode_of(&permanent), None);
        assert!(a.list_active().expect("works").is_empty());
    }

    #[test]
    fn a_second_face_of_one_file_is_already_active() {
        let s = Scratch::new();
        let a = s.activator();
        let ttc = s.font("Family.ttc");
        let out = a
            .activate(
                &[face(&ttc, "Family-Regular"), face(&ttc, "Family-Bold")],
                Mode::Permanent,
            )
            .expect("works");
        let reg = activated(&out[0]);
        assert_eq!(out[1].status, OutcomeStatus::AlreadyActive(reg));
    }

    #[test]
    fn list_active_survives_a_restart_and_clear_session_keeps_permanent_fonts() {
        let s = Scratch::new();
        let session = s.font("Session.ttf");
        let permanent = s.font("Permanent.ttf");
        {
            let a = s.activator();
            activated(
                &a.activate(&[face(&session, "S")], Mode::Session)
                    .expect("works")[0],
            );
            activated(
                &a.activate(&[face(&permanent, "P")], Mode::Permanent)
                    .expect("works")[0],
            );
        }
        // The next start after a crash.
        let a = s.activator();
        assert_eq!(a.list_active().expect("works").len(), 2);
        let cleared = a.clear_session().expect("works");
        assert_eq!(cleared.len(), 1);
        assert_eq!(cleared[0].path, session);
        assert_eq!(cleared[0].status, OutcomeStatus::Deactivated);
        assert_eq!(s.fake.mode_of(&permanent), Some(Mode::Permanent));
        let left = a.list_active().expect("works");
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].path, permanent);
    }

    #[test]
    fn a_session_font_dropped_by_logout_is_not_active_and_leaves_the_ledger() {
        let s = Scratch::new();
        let a = s.activator();
        let font = s.font("Session.ttf");
        activated(
            &a.activate(&[face(&font, "S")], Mode::Session)
                .expect("works")[0],
        );
        // Logout: CoreText forgets the session registration.
        s.fake.registered.lock().expect("fake lock").clear();

        let cleared = s.activator().clear_session().expect("works");
        assert_eq!(cleared[0].status, OutcomeStatus::NotActive);
        assert!(a.list_active().expect("works").is_empty());
    }

    #[test]
    fn a_file_registered_by_another_app_is_not_claimed() {
        let s = Scratch::new();
        let font = s.font("Theirs.ttf");
        s.fake
            .registered
            .lock()
            .expect("fake lock")
            .insert(font.clone(), Mode::Permanent);
        let a = s.activator();

        let out = a
            .activate(&[face(&font, "T")], Mode::Permanent)
            .expect("works");
        assert_eq!(failed_kind(&out[0]), ActivationErrorKind::Os);
        assert!(a.list_active().expect("works").is_empty());

        // A registration the core never got from this activator stays untouched.
        let foreign = registration(&font, Mode::Permanent);
        let out = a.deactivate(&[foreign]).expect("works");
        assert_eq!(out[0].status, OutcomeStatus::NotOurs);
        assert_eq!(s.fake.mode_of(&font), Some(Mode::Permanent));
    }

    #[test]
    fn deactivate_refuses_a_wrong_mode_or_handle() {
        let s = Scratch::new();
        let a = s.activator();
        let font = s.font("Mine.ttf");
        let reg = activated(
            &a.activate(&[face(&font, "M")], Mode::Session)
                .expect("works")[0],
        );

        let wrong_mode = Registration {
            mode: Mode::Permanent,
            ..reg.clone()
        };
        let wrong_handle = Registration {
            handle: "/elsewhere.ttf".into(),
            ..reg.clone()
        };
        let out = a.deactivate(&[wrong_mode, wrong_handle]).expect("works");
        assert_eq!(out[0].status, OutcomeStatus::NotOurs);
        assert_eq!(out[1].status, OutcomeStatus::NotOurs);
        assert_eq!(s.fake.mode_of(&font), Some(Mode::Session));
    }

    #[test]
    fn a_later_face_of_a_file_that_failed_fails_too_and_a_restart_sees_the_rest() {
        let s = Scratch::new();
        let a = s.activator();
        let bad = s.font("Broken.ttc");
        let good = s.font("Good.ttf");
        s.fail_next(&bad, code::INVALID_FONT_DATA);
        let out = a
            .activate(
                &[face(&bad, "B-1"), face(&good, "G"), face(&bad, "B-2")],
                Mode::Session,
            )
            .expect("works");
        assert_eq!(failed_kind(&out[0]), ActivationErrorKind::Unsupported);
        let reg = activated(&out[1]);
        assert_eq!(failed_kind(&out[2]), ActivationErrorKind::Unsupported);
        // One ledger write for the request: a new activator reads it back.
        assert_eq!(s.activator().list_active(), Ok(vec![reg.clone()]));
        let off = s.activator().deactivate(&[reg]).expect("works");
        assert_eq!(off[0].status, OutcomeStatus::Deactivated);
        assert_eq!(s.activator().list_active(), Ok(vec![]));
    }

    #[test]
    fn failed_registration_is_a_failed_outcome_and_leaves_no_ledger_entry() {
        let s = Scratch::new();
        let a = s.activator();
        let bad = s.font("Broken.ttf");
        let good = s.font("Good.ttf");
        s.fail_next(&bad, code::INVALID_FONT_DATA);

        let out = a
            .activate(&[face(&bad, "B"), face(&good, "G")], Mode::Session)
            .expect("works");
        assert_eq!(failed_kind(&out[0]), ActivationErrorKind::Unsupported);
        activated(&out[1]);
        let active = a.list_active().expect("works");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].path, good);
    }

    #[test]
    fn a_failed_unregister_keeps_the_ledger_entry() {
        let s = Scratch::new();
        let a = s.activator();
        let font = s.font("Busy.ttf");
        let reg = activated(
            &a.activate(&[face(&font, "B")], Mode::Permanent)
                .expect("works")[0],
        );
        s.fail_next(&font, code::IN_USE);

        let out = a.deactivate(std::slice::from_ref(&reg)).expect("works");
        assert_eq!(failed_kind(&out[0]), ActivationErrorKind::Os);
        assert_eq!(a.list_active().expect("works"), vec![reg.clone()]);
        // It can be retried.
        assert_eq!(
            a.deactivate(&[reg]).expect("works")[0].status,
            OutcomeStatus::Deactivated
        );
    }

    #[test]
    fn missing_folder_and_relative_files_fail_per_face() {
        let s = Scratch::new();
        let a = s.activator();
        let out = a
            .activate(
                &[
                    face(&s.dir.join("gone.ttf"), "G"),
                    face(&s.dir, "D"),
                    face(Path::new("relative.ttf"), "R"),
                ],
                Mode::Session,
            )
            .expect("works");
        assert_eq!(failed_kind(&out[0]), ActivationErrorKind::FileNotFound);
        assert_eq!(failed_kind(&out[1]), ActivationErrorKind::FileNotFound);
        assert_eq!(failed_kind(&out[2]), ActivationErrorKind::Unsupported);
        assert!(s.fake.registered.lock().expect("fake lock").is_empty());
    }

    #[test]
    fn an_unwritable_ledger_fails_before_registering() {
        let s = Scratch::new();
        let font = s.font("Font.ttf");
        // The ledger's folder is a file, so the ledger cannot be written.
        let blocker = s.dir.join("blocker");
        let _ = fs::write(&blocker, b"");
        let a = MacosActivator::with_registrar(
            blocker.join(LEDGER_FILE),
            Box::new(Arc::clone(&s.fake)),
        );
        let out = a
            .activate(&[face(&font, "F")], Mode::Session)
            .expect("works");
        assert!(matches!(out[0].status, OutcomeStatus::Failed(_)));
        assert_eq!(s.fake.mode_of(&font), None);
    }

    #[test]
    fn error_codes_map_to_activation_error_kinds() {
        let cases = [
            (code::FILE_NOT_FOUND, ActivationErrorKind::FileNotFound),
            (code::INVALID_FILE_PATH, ActivationErrorKind::FileNotFound),
            (
                code::INSUFFICIENT_PERMISSIONS,
                ActivationErrorKind::PermissionDenied,
            ),
            (
                code::CANCELLED_BY_USER,
                ActivationErrorKind::PermissionDenied,
            ),
            (code::UNRECOGNIZED_FORMAT, ActivationErrorKind::Unsupported),
            (code::INVALID_FONT_DATA, ActivationErrorKind::Unsupported),
            (code::MISSING_ENTITLEMENT, ActivationErrorKind::Unsupported),
            (code::UNSUPPORTED_SCOPE, ActivationErrorKind::Unsupported),
            (code::ALREADY_REGISTERED, ActivationErrorKind::Os),
            (code::EXCEEDED_RESOURCE_LIMIT, ActivationErrorKind::Os),
            (code::IN_USE, ActivationErrorKind::Os),
            (code::SYSTEM_REQUIRED, ActivationErrorKind::Os),
            (code::REGISTRATION_FAILED, ActivationErrorKind::Os),
            (-1, ActivationErrorKind::Os),
        ];
        for (c, kind) in cases {
            let e = error_for(
                &RegistrarError {
                    code: c,
                    detail: "why".to_owned(),
                },
                Path::new("/f.ttf"),
            );
            assert_eq!(e.kind, kind, "code {c}");
            assert!(e.detail.contains(&c.to_string()), "{}", e.detail);
            assert!(e.detail.contains("/f.ttf"), "{}", e.detail);
        }
    }

    #[test]
    fn conflicts_skip_the_file_itself_and_tell_system_from_user() {
        let s = Scratch::new();
        let mine = s.font("Brand-Bold.otf");
        let fake = Fake {
            fonts: vec![
                (
                    "Helvetica".to_owned(),
                    PathBuf::from("/System/Library/Fonts/Helvetica.ttc"),
                ),
                (
                    "Brand-Bold".to_owned(),
                    PathBuf::from("/Users/ada/Library/Fonts/Brand-Bold.otf"),
                ),
                ("Brand-Bold".to_owned(), mine.clone()),
                (
                    "Other".to_owned(),
                    PathBuf::from("/Library/Fonts/Other.ttf"),
                ),
            ],
            ..Fake::default()
        };
        let a = MacosActivator::with_registrar(s.dir.join(LEDGER_FILE), Box::new(Arc::new(fake)));

        let found = a
            .conflicts(&[face(&mine, "Brand-Bold"), face(&mine, "Helvetica")])
            .expect("works");
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(
            found[0].with,
            PathBuf::from("/Users/ada/Library/Fonts/Brand-Bold.otf")
        );
        assert_eq!(found[0].kind, ConflictKind::User);
        assert_eq!(found[1].postscript_name, "Helvetica");
        assert_eq!(found[1].kind, ConflictKind::System);
        assert_eq!(
            conflict_kind(Path::new("/Library/Fonts/Other.ttf")),
            ConflictKind::System
        );
        assert!(a.conflicts(&[]).expect("works").is_empty());
    }
}
