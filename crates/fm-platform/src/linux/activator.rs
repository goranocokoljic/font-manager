//! The Linux [`FontActivator`]: a symbolic link to the file in place, in a
//! folder fontconfig scans (ADR 0008).
//!
//! Permanent links live in `$XDG_DATA_HOME/fonts/font-manager/`, session
//! links in `…/font-manager/session/` (default `~/.local/share/fonts/…`).
//! fontconfig scans `$XDG_DATA_HOME/fonts` and its subfolders, so a new
//! process sees a link at once; after each change the activator runs
//! `fc-cache` on the folder, without waiting, so other apps start fast. A
//! running app sees the change when it next re-checks fontconfig (about 30 s).
//! No admin rights, no write to a system folder (rule 4).
//!
//! Rule 3: the links are the record, so there is no ledger. A link is ours
//! only when it is a symbolic link in the mode's folder whose name is the one
//! [`link_name`] gives its absolute target; any other file or link there is
//! never removed or claimed. Session links stay after a crash, so
//! [`FontActivator::clear_session`] runs on quit and again at the next start.
//!
//! The logic builds on every Unix target, so its unit tests run on the Linux
//! and macOS runners; only Linux uses it.

use std::collections::HashSet;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use super::fontconfig::Fontconfig;
use crate::activator::{
    ActivationError, ActivationErrorKind, Conflict, ConflictKind, FaceFile, FontActivator, Mode,
    Outcome, OutcomeStatus, Registration,
};
use crate::fs::{path_to_bytes, symlink_file};
#[cfg(target_os = "linux")]
use crate::paths::AppPaths;
use crate::paths::XDG_DIR;

/// The session folder's name inside the permanent folder.
pub(crate) const SESSION_DIR: &str = "session";

/// The longest file name Linux file systems take, in bytes.
const NAME_MAX: usize = 255;

/// Activates fonts for the current user through symbolic links. See the module docs.
#[derive(Debug)]
pub struct LinuxActivator {
    /// The permanent folder; the session folder is inside it.
    root: PathBuf,
    fontconfig: Box<dyn Fontconfig>,
    /// Serialises link changes within the process.
    lock: Mutex<()>,
}

/// The app's permanent link folder for the app data folder `data`
/// (`$XDG_DATA_HOME/font-manager`): `$XDG_DATA_HOME/fonts/font-manager`.
pub(crate) fn fonts_dir(data: &Path) -> PathBuf {
    data.parent().unwrap_or(data).join("fonts").join(XDG_DIR)
}

impl LinuxActivator {
    /// The activator the app uses: links under `$XDG_DATA_HOME/fonts/font-manager/`.
    /// Creates both link folders now, before any activation, because a
    /// running fontconfig app only notices changes in folders that existed
    /// when it loaded (ADR 0008).
    #[cfg(target_os = "linux")]
    pub fn new(paths: &AppPaths) -> Self {
        Self::with_root(fonts_dir(&paths.data))
    }

    /// The fontconfig tools, with the permanent folder at `root` and the
    /// session folder inside it. Tests use a scratch folder so they never
    /// see or change the app's own links.
    #[cfg(target_os = "linux")]
    pub fn with_root(root: PathBuf) -> Self {
        Self::with_fontconfig(root, Box::new(super::fontconfig::FcTools))
    }

    pub(crate) fn with_fontconfig(root: PathBuf, fontconfig: Box<dyn Fontconfig>) -> Self {
        // Best effort: when this fails, each activation fails per face.
        let _ = std::fs::create_dir_all(root.join(SESSION_DIR));
        Self {
            root,
            fontconfig,
            lock: Mutex::new(()),
        }
    }

    /// The folder that holds the links of `mode`.
    pub fn dir(&self, mode: Mode) -> PathBuf {
        match mode {
            Mode::Permanent => self.root.clone(),
            Mode::Session => self.root.join(SESSION_DIR),
        }
    }

    fn activate_one(&self, face: &FaceFile, mode: Mode) -> OutcomeStatus {
        if !face.path.is_absolute() {
            return failed(
                ActivationErrorKind::Unsupported,
                format!("{}: activation needs a full path", face.path.display()),
            );
        }
        let name = link_name(&face.path);
        // Active already, in either mode: one file, one link.
        for m in [mode, other(mode)] {
            let link = self.dir(m).join(&name);
            if std::fs::read_link(&link).is_ok_and(|t| t == face.path) {
                return OutcomeStatus::AlreadyActive(registration(&face.path, m, link));
            }
        }
        match face.path.metadata() {
            Ok(meta) if meta.is_file() => {}
            Ok(_) => {
                return failed(
                    ActivationErrorKind::FileNotFound,
                    format!("{}: not a file", face.path.display()),
                );
            }
            Err(e) => return OutcomeStatus::Failed(io_error(&e, &face.path)),
        }
        let dir = self.dir(mode);
        let link = dir.join(&name);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return OutcomeStatus::Failed(io_error(&e, &dir));
        }
        if let Err(e) = symlink_file(&face.path, &link) {
            if e.kind() == io::ErrorKind::AlreadyExists {
                // Something the app did not make, or a link to another file:
                // never replaced (rule 3).
                return failed(
                    ActivationErrorKind::Os,
                    format!(
                        "{}: {} is taken by a file the app did not create",
                        face.path.display(),
                        link.display()
                    ),
                );
            }
            return OutcomeStatus::Failed(io_error(&e, &link));
        }
        OutcomeStatus::Activated(registration(&face.path, mode, link))
    }

    /// Removes the link of `reg`. `Ok(true)` when its target was gone, so
    /// fontconfig needs a forced rescan to forget it (ADR 0008).
    fn deactivate_one(&self, reg: &Registration) -> Result<bool, OutcomeStatus> {
        let link = self.dir(reg.mode).join(link_name(&reg.path));
        // `activate` sets the handle to this link; anything else it never issued.
        if !reg.path.is_absolute() || reg.handle != link.as_os_str() {
            return Err(OutcomeStatus::NotOurs);
        }
        match std::fs::symlink_metadata(&link) {
            Ok(meta) if meta.file_type().is_symlink() => {}
            Ok(_) => return Err(OutcomeStatus::NotOurs),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(OutcomeStatus::NotActive);
            }
            Err(e) => return Err(OutcomeStatus::Failed(io_error(&e, &link))),
        }
        match std::fs::read_link(&link) {
            Ok(target) if target == reg.path => {}
            Ok(_) => return Err(OutcomeStatus::NotOurs),
            Err(e) => return Err(OutcomeStatus::Failed(io_error(&e, &link))),
        }
        let dangling = std::fs::metadata(&reg.path).is_err();
        // The link only; the font file it points at is never touched (rule 2).
        std::fs::remove_file(&link).map_err(|e| OutcomeStatus::Failed(io_error(&e, &link)))?;
        Ok(dangling)
    }

    /// Runs `fc-cache` once per folder that changed.
    fn refresh(&self, changed: &[(Mode, bool)]) {
        for mode in [Mode::Permanent, Mode::Session] {
            let forced: Vec<bool> = changed
                .iter()
                .filter(|(m, _)| *m == mode)
                .map(|(_, force)| *force)
                .collect();
            if !forced.is_empty() {
                self.fontconfig
                    .refresh(&self.dir(mode), forced.contains(&true));
            }
        }
    }

    fn links_in(&self, mode: Mode) -> Result<Vec<Registration>, ActivationError> {
        let dir = self.dir(mode);
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            // A root under a file holds no links either: Linux says "not a
            // directory" there, and activation fails per face.
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) =>
            {
                return Ok(Vec::new());
            }
            Err(e) => return Err(io_error(&e, &dir)),
        };
        let mut found = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| io_error(&e, &dir))?;
            let link = entry.path();
            let Ok(target) = std::fs::read_link(&link) else {
                continue; // Not a link: the session folder, or not ours.
            };
            if target.is_absolute() && link_name(&target) == entry.file_name() {
                found.push(registration(&target, mode, link));
            }
        }
        found.sort_by(|a, b| a.handle.cmp(&b.handle));
        Ok(found)
    }
}

impl FontActivator for LinuxActivator {
    fn activate(&self, faces: &[FaceFile], mode: Mode) -> Result<Vec<Outcome>, ActivationError> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let outcomes: Vec<Outcome> = faces
            .iter()
            .map(|face| Outcome {
                path: face.path.clone(),
                status: self.activate_one(face, mode),
            })
            .collect();
        if outcomes
            .iter()
            .any(|o| matches!(o.status, OutcomeStatus::Activated(_)))
        {
            self.refresh(&[(mode, false)]);
        }
        Ok(outcomes)
    }

    fn deactivate(&self, registrations: &[Registration]) -> Result<Vec<Outcome>, ActivationError> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let mut changed = Vec::new();
        let outcomes = registrations
            .iter()
            .map(|reg| Outcome {
                path: reg.path.clone(),
                status: match self.deactivate_one(reg) {
                    Ok(dangling) => {
                        changed.push((reg.mode, dangling));
                        OutcomeStatus::Deactivated
                    }
                    Err(status) => status,
                },
            })
            .collect();
        self.refresh(&changed);
        Ok(outcomes)
    }

    fn list_active(&self) -> Result<Vec<Registration>, ActivationError> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let mut active = self.links_in(Mode::Permanent)?;
        active.extend(self.links_in(Mode::Session)?);
        Ok(active)
    }

    fn conflicts(&self, faces: &[FaceFile]) -> Result<Vec<Conflict>, ActivationError> {
        let names: HashSet<&str> = faces.iter().map(|f| f.postscript_name.as_str()).collect();
        if names.is_empty() {
            return Ok(Vec::new());
        }
        let found = self
            .fontconfig
            .fonts_named(&names)
            .map_err(|e| ActivationError::new(ActivationErrorKind::Unsupported, e))?;
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

/// The link's file name for `target`: a hash of the full path, so two files
/// with one name in different folders never share a link, then the file's
/// own name for people who look in the folder. When that is too long for
/// the file system, the hash and the extension.
pub(crate) fn link_name(target: &Path) -> OsString {
    let mut name = OsString::from(format!("{:016x}", fnv1a(&path_to_bytes(target))));
    match target.file_name() {
        Some(file) if name.len() + 1 + file.len() <= NAME_MAX => {
            name.push("-");
            name.push(file);
        }
        _ => {
            if let Some(ext) = target.extension().filter(|e| e.len() <= 16) {
                name.push(".");
                name.push(ext);
            }
        }
    }
    name
}

/// 64-bit FNV-1a: stable across builds and OS versions, unlike `DefaultHasher`.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, b| {
        (hash ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn other(mode: Mode) -> Mode {
    match mode {
        Mode::Session => Mode::Permanent,
        Mode::Permanent => Mode::Session,
    }
}

fn registration(path: &Path, mode: Mode, link: PathBuf) -> Registration {
    Registration {
        path: path.to_owned(),
        mode,
        handle: link.into_os_string(),
    }
}

/// Folders only an administrator or the OS writes to: a font there is a system font.
const SYSTEM_FONT_DIRS: [&str; 4] = ["/usr/", "/opt/", "/etc/", "/var/"];

fn conflict_kind(with: &Path) -> ConflictKind {
    let bytes = path_to_bytes(with);
    if SYSTEM_FONT_DIRS
        .iter()
        .any(|dir| bytes.starts_with(dir.as_bytes()))
    {
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

fn failed(kind: ActivationErrorKind, detail: String) -> OutcomeStatus {
    OutcomeStatus::Failed(ActivationError::new(kind, detail))
}

/// A file or folder error at `path`.
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
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::fs::path_from_bytes;
    use crate::paths::{AppPaths, Os, test_env};

    /// A fontconfig stand-in: the refreshes asked for, and the fonts it lists.
    #[derive(Debug, Default)]
    struct Fake {
        refreshed: Mutex<Vec<(PathBuf, bool)>>,
        fonts: Vec<(String, PathBuf)>,
    }

    impl Fontconfig for Arc<Fake> {
        fn refresh(&self, dir: &Path, force: bool) {
            self.refreshed
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((dir.to_owned(), force));
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
                "fm-platform-linux-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::create_dir_all(dir.join("library"));
            Self {
                dir,
                fake: Arc::default(),
            }
        }

        fn root(&self) -> PathBuf {
            self.dir.join("fonts").join(XDG_DIR)
        }

        /// A fresh activator over the same folders: what the app is after a restart.
        fn activator(&self) -> LinuxActivator {
            LinuxActivator::with_fontconfig(self.root(), Box::new(Arc::clone(&self.fake)))
        }

        fn font(&self, file: &str) -> PathBuf {
            let path = self.dir.join("library").join(file);
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&path, b"not parsed by the activator");
            path
        }

        fn refreshed(&self) -> Vec<(PathBuf, bool)> {
            std::mem::take(&mut *self.fake.refreshed.lock().expect("fake lock"))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let session = self.root().join(SESSION_DIR);
            let _ = fs::set_permissions(session, fs::Permissions::from_mode(0o755));
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

    fn links(dir: &Path) -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = fs::read_dir(dir)
            .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
            .unwrap_or_default();
        found.retain(|p| p.is_symlink());
        found.sort();
        found
    }

    #[test]
    fn the_app_links_under_xdg_data_home_fonts() {
        let paths =
            AppPaths::resolve(Os::Linux, test_env(&[("HOME", "/home/ada")])).expect("resolves");
        assert_eq!(
            fonts_dir(&paths.data),
            PathBuf::from("/home/ada/.local/share/fonts/font-manager")
        );
        let paths = AppPaths::resolve(
            Os::Linux,
            test_env(&[("HOME", "/home/ada"), ("XDG_DATA_HOME", "/data")]),
        )
        .expect("resolves");
        assert_eq!(
            fonts_dir(&paths.data),
            PathBuf::from("/data/fonts/font-manager")
        );
    }

    #[test]
    fn both_folders_exist_before_any_activation() {
        let s = Scratch::new();
        let a = s.activator();
        assert!(a.dir(Mode::Permanent).is_dir());
        assert!(a.dir(Mode::Session).is_dir());
        assert_eq!(a.dir(Mode::Session), s.root().join(SESSION_DIR));
    }

    #[test]
    fn each_mode_links_in_its_own_folder_and_refreshes_it() {
        let s = Scratch::new();
        let a = s.activator();
        let session = s.font("Session.ttf");
        let permanent = s.font("Permanent.otf");

        let session_reg = activated(
            &a.activate(&[face(&session, "S")], Mode::Session)
                .expect("works")[0],
        );
        assert_eq!(s.refreshed(), vec![(a.dir(Mode::Session), false)]);
        let permanent_reg = activated(
            &a.activate(&[face(&permanent, "P")], Mode::Permanent)
                .expect("works")[0],
        );
        assert_eq!(s.refreshed(), vec![(a.dir(Mode::Permanent), false)]);

        let session_link = PathBuf::from(&session_reg.handle);
        let permanent_link = PathBuf::from(&permanent_reg.handle);
        assert_eq!(session_link.parent(), Some(a.dir(Mode::Session).as_path()));
        assert_eq!(
            permanent_link.parent(),
            Some(a.dir(Mode::Permanent).as_path())
        );
        assert_eq!(fs::read_link(&session_link).expect("a link"), session);
        assert_eq!(fs::read_link(&permanent_link).expect("a link"), permanent);
        assert_eq!(links(&a.dir(Mode::Session)), vec![session_link.clone()]);
        assert_eq!(links(&a.dir(Mode::Permanent)), vec![permanent_link.clone()]);

        let out = a.deactivate(&[session_reg, permanent_reg]).expect("works");
        assert_eq!(out[0].status, OutcomeStatus::Deactivated);
        assert_eq!(out[1].status, OutcomeStatus::Deactivated);
        assert!(!session_link.is_symlink() && !permanent_link.is_symlink());
        // The font files stay where they are (rule 2).
        assert!(session.is_file() && permanent.is_file());
        assert_eq!(
            s.refreshed(),
            vec![
                (a.dir(Mode::Permanent), false),
                (a.dir(Mode::Session), false)
            ]
        );
        assert!(a.list_active().expect("works").is_empty());
    }

    #[test]
    fn a_second_face_of_one_file_is_already_active_in_either_mode() {
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
        assert_eq!(out[1].status, OutcomeStatus::AlreadyActive(reg.clone()));
        let out = a
            .activate(&[face(&ttc, "Family-Regular")], Mode::Session)
            .expect("works");
        assert_eq!(out[0].status, OutcomeStatus::AlreadyActive(reg));
        assert!(links(&a.dir(Mode::Session)).is_empty());
    }

    #[test]
    fn files_with_one_name_in_two_folders_get_two_links() {
        let s = Scratch::new();
        let a = s.activator();
        let one = s.font("a/Brand.otf");
        let two = s.font("b/Brand.otf");
        let out = a
            .activate(&[face(&one, "A"), face(&two, "B")], Mode::Permanent)
            .expect("works");
        let (r1, r2) = (activated(&out[0]), activated(&out[1]));
        assert_ne!(r1.handle, r2.handle);
        assert_eq!(a.list_active().expect("works").len(), 2);
    }

    #[test]
    fn list_active_survives_a_restart_and_clear_session_keeps_permanent_links() {
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
        s.refreshed();
        // The next start after a crash, or quit.
        let a = s.activator();
        assert_eq!(a.list_active().expect("works").len(), 2);
        let cleared = a.clear_session().expect("works");
        assert_eq!(cleared.len(), 1);
        assert_eq!(cleared[0].path, session);
        assert_eq!(cleared[0].status, OutcomeStatus::Deactivated);
        assert!(links(&a.dir(Mode::Session)).is_empty());
        assert_eq!(s.refreshed(), vec![(a.dir(Mode::Session), false)]);
        let left = a.list_active().expect("works");
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].path, permanent);
        assert_eq!(left[0].mode, Mode::Permanent);
    }

    #[test]
    fn a_link_whose_font_was_deleted_is_removed_with_a_forced_refresh() {
        let s = Scratch::new();
        let a = s.activator();
        let font = s.font("Gone.ttf");
        activated(
            &a.activate(&[face(&font, "G")], Mode::Session)
                .expect("works")[0],
        );
        s.refreshed();
        fs::remove_file(&font).expect("delete the font");

        let cleared = s.activator().clear_session().expect("works");
        assert_eq!(cleared[0].status, OutcomeStatus::Deactivated);
        assert!(links(&a.dir(Mode::Session)).is_empty());
        assert_eq!(s.refreshed(), vec![(a.dir(Mode::Session), true)]);
    }

    #[test]
    fn foreign_files_and_links_in_the_folders_are_never_claimed_or_removed() {
        let s = Scratch::new();
        let a = s.activator();
        let theirs = s.font("Theirs.ttf");
        let session = a.dir(Mode::Session);
        // A link someone else put there, a file, and a link with our name
        // that points at another file.
        let foreign_link = session.join("user-link.ttf");
        symlink_file(&theirs, &foreign_link).expect("link");
        let foreign_file = session.join("plain.ttf");
        fs::write(&foreign_file, b"a user file").expect("file");
        let mine = s.font("Mine.ttf");
        let squatter = session.join(link_name(&mine));
        symlink_file(&theirs, &squatter).expect("link");

        assert!(a.list_active().expect("works").is_empty());
        assert!(a.clear_session().expect("works").is_empty());

        // Our name is taken: activation fails and replaces nothing.
        let out = a
            .activate(&[face(&mine, "M")], Mode::Session)
            .expect("works");
        assert_eq!(failed_kind(&out[0]), ActivationErrorKind::Os);
        assert_eq!(fs::read_link(&squatter).expect("a link"), theirs);

        // Registrations the activator never issued stay untouched.
        let forged = [
            Registration {
                path: theirs.clone(),
                mode: Mode::Session,
                handle: foreign_link.clone().into_os_string(),
            },
            Registration {
                path: mine.clone(),
                mode: Mode::Session,
                handle: squatter.clone().into_os_string(),
            },
        ];
        let out = a.deactivate(&forged).expect("works");
        assert_eq!(out[0].status, OutcomeStatus::NotOurs);
        assert_eq!(out[1].status, OutcomeStatus::NotOurs);
        assert!(foreign_link.is_symlink() && squatter.is_symlink());
        assert!(foreign_file.is_file() && theirs.is_file());
    }

    #[test]
    fn a_regular_file_with_our_name_is_not_ours() {
        let s = Scratch::new();
        let a = s.activator();
        let font = s.font("Font.ttf");
        let reg = activated(
            &a.activate(&[face(&font, "F")], Mode::Permanent)
                .expect("works")[0],
        );
        let link = PathBuf::from(&reg.handle);
        fs::remove_file(&link).expect("unlink");
        fs::write(&link, b"the user put a real file here").expect("file");

        assert_eq!(
            a.deactivate(&[reg]).expect("works")[0].status,
            OutcomeStatus::NotOurs
        );
        assert!(link.is_file());
    }

    #[test]
    fn deactivate_refuses_a_wrong_mode_or_handle_and_a_removed_link_is_not_active() {
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
        assert!(PathBuf::from(&reg.handle).is_symlink());

        // The user removed the link by hand.
        fs::remove_file(PathBuf::from(&reg.handle)).expect("unlink");
        assert_eq!(
            a.deactivate(&[reg]).expect("works")[0].status,
            OutcomeStatus::NotActive
        );
    }

    #[test]
    fn missing_folder_and_relative_files_fail_per_face() {
        let s = Scratch::new();
        let a = s.activator();
        let good = s.font("Good.ttf");
        let out = a
            .activate(
                &[
                    face(&s.dir.join("gone.ttf"), "G"),
                    face(&s.dir, "D"),
                    face(Path::new("relative.ttf"), "R"),
                    face(&good, "OK"),
                ],
                Mode::Session,
            )
            .expect("works");
        assert_eq!(failed_kind(&out[0]), ActivationErrorKind::FileNotFound);
        assert_eq!(failed_kind(&out[1]), ActivationErrorKind::FileNotFound);
        assert_eq!(failed_kind(&out[2]), ActivationErrorKind::Unsupported);
        activated(&out[3]);
        assert_eq!(links(&a.dir(Mode::Session)).len(), 1);
    }

    #[test]
    fn a_folder_that_cannot_be_written_fails_per_face() {
        let s = Scratch::new();
        let font = s.font("Font.ttf");
        // The link root is a file, so no link can be made under it.
        let blocker = s.dir.join("blocker");
        fs::write(&blocker, b"").expect("file");
        let a =
            LinuxActivator::with_fontconfig(blocker.join(XDG_DIR), Box::new(Arc::clone(&s.fake)));
        let out = a
            .activate(&[face(&font, "F")], Mode::Permanent)
            .expect("works");
        assert!(matches!(out[0].status, OutcomeStatus::Failed(_)));
        assert!(s.refreshed().is_empty());
        assert!(a.list_active().expect("works").is_empty());
    }

    #[test]
    fn a_read_only_link_folder_is_permission_denied_per_face() {
        let s = Scratch::new();
        let a = s.activator();
        let font = s.font("Font.ttf");
        let session = a.dir(Mode::Session);
        fs::set_permissions(&session, fs::Permissions::from_mode(0o555)).expect("chmod");
        if fs::write(session.join("probe"), b"").is_ok() {
            return; // Running as root: permissions are not enforced, nothing to prove.
        }
        let out = a
            .activate(&[face(&font, "F")], Mode::Session)
            .expect("works");
        assert_eq!(failed_kind(&out[0]), ActivationErrorKind::PermissionDenied);
        assert!(s.refreshed().is_empty());
    }

    #[test]
    fn a_non_utf8_path_activates_and_lists_exactly() {
        let s = Scratch::new();
        let a = s.activator();
        let mut bytes = path_to_bytes(&s.dir.join("library"));
        bytes.extend_from_slice(b"/Schrift-\xff.ttf");
        let font = path_from_bytes(&bytes).expect("a path");
        if fs::write(&font, b"font").is_err() {
            return; // A file system that refuses non-UTF-8 names (APFS): nothing to prove.
        }
        let reg = activated(
            &a.activate(&[face(&font, "S")], Mode::Session)
                .expect("works")[0],
        );
        assert_eq!(a.list_active().expect("works"), vec![reg.clone()]);
        assert_eq!(
            a.deactivate(&[reg]).expect("works")[0].status,
            OutcomeStatus::Deactivated
        );
    }

    #[test]
    fn link_names_are_stable_distinct_and_fit_the_file_system() {
        let a = link_name(Path::new("/fonts/a/Brand.otf"));
        let b = link_name(Path::new("/fonts/b/Brand.otf"));
        assert_ne!(a, b);
        assert_eq!(a, link_name(Path::new("/fonts/a/Brand.otf")));
        assert!(a.to_string_lossy().ends_with("-Brand.otf"), "{a:?}");
        // FNV-1a of "/fonts/a/Brand.otf": the name must not change between builds.
        assert_eq!(
            &a.to_string_lossy()[..16],
            &format!("{:016x}", fnv1a(b"/fonts/a/Brand.otf"))
        );
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);

        let long = format!("/fonts/{}.ttf", "x".repeat(250));
        let name = link_name(Path::new(&long));
        assert!(name.len() <= NAME_MAX, "{}", name.len());
        assert!(name.to_string_lossy().ends_with(".ttf"));
    }

    #[test]
    fn conflicts_skip_the_file_itself_and_tell_system_from_user() {
        let s = Scratch::new();
        let mine = s.font("Brand-Bold.otf");
        let fake = Fake {
            fonts: vec![
                (
                    "DejaVuSans".to_owned(),
                    PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
                ),
                (
                    "Brand-Bold".to_owned(),
                    PathBuf::from("/home/ada/.local/share/fonts/Brand-Bold.otf"),
                ),
                ("Brand-Bold".to_owned(), mine.clone()),
            ],
            ..Fake::default()
        };
        let a = LinuxActivator::with_fontconfig(s.root(), Box::new(Arc::new(fake)));
        // Our own link to the file is the file itself, not a conflict.
        let reg = activated(
            &a.activate(&[face(&mine, "Brand-Bold")], Mode::Permanent)
                .expect("works")[0],
        );
        assert!(same_file(Path::new(&reg.handle), &mine));

        let found = a
            .conflicts(&[face(&mine, "Brand-Bold"), face(&mine, "DejaVuSans")])
            .expect("works");
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(
            found[0].with,
            PathBuf::from("/home/ada/.local/share/fonts/Brand-Bold.otf")
        );
        assert_eq!(found[0].kind, ConflictKind::User);
        assert_eq!(found[1].postscript_name, "DejaVuSans");
        assert_eq!(found[1].kind, ConflictKind::System);
        assert!(a.conflicts(&[]).expect("works").is_empty());
    }
}
