//! The Linux activator against real fontconfig: a second process (`fc-list`)
//! finds the family while it is active, and not after deactivation or session
//! clean-up (ADR 0008).
//!
//! The activator is built for a scratch `XDG_DATA_HOME`, the way the app builds
//! it from its paths, so its links land in `<scratch>/fonts/font-manager/`.
//! `fc-list` runs with the same `XDG_DATA_HOME` (and a scratch cache), so it
//! scans that folder through fontconfig's own `<dir prefix="xdg">fonts</dir>`
//! rule; the user's own `~/.local/share/fonts` is never read or changed.
//!
//! Fonts: Diplomata (session) and Emblema One (permanent) from
//! `fixtures/fonts/`; no Linux image ships them, so "before" is a real negative.
#![cfg(target_os = "linux")]
// The whole file is test code: its helpers fail the test by panicking, which
// clippy's in-test allowance only sees inside `#[test]` functions.
#![allow(clippy::expect_used, clippy::panic)]

use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use fm_platform::{
    AppPaths, FaceFile, FontActivator, LinuxActivator, Mode, Os, OutcomeStatus, Registration,
};

/// A new process sees a link at once (ADR 0008); the margin is for slow runners.
const WAIT: Duration = Duration::from_secs(10);

fn font_file(rel: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts")
        .join(rel)
        .canonicalize()
        .expect("fixture path");
    assert!(path.is_file(), "missing fixture {}", path.display());
    path
}

/// A scratch home and `XDG_DATA_HOME`. On drop: deactivate whatever the
/// activator still lists, then delete the scratch folder.
struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        Self {
            dir: std::env::temp_dir().join(format!("fm-it-linux-{}-{name}", std::process::id())),
        }
    }

    fn data_home(&self) -> PathBuf {
        self.dir.join("data")
    }

    /// The activator the app would build with this `XDG_DATA_HOME`. A fresh
    /// one stands for the next start of the app.
    fn activator(&self) -> LinuxActivator {
        let home = self.dir.as_os_str().to_owned();
        let data = self.data_home().into_os_string();
        let paths = AppPaths::resolve(Os::Linux, move |var: &str| -> Option<OsString> {
            match var {
                "HOME" => Some(home.clone()),
                "XDG_DATA_HOME" => Some(data.clone()),
                _ => None,
            }
        })
        .expect("app paths");
        LinuxActivator::new(&paths)
    }

    /// The files `fc-list`, in a new process, lists for `family`.
    fn fc_list(&self, family: &str) -> Vec<PathBuf> {
        let out = Command::new("fc-list")
            .args(["--format", "%{file}\n", &format!(":family={family}")])
            .env("XDG_DATA_HOME", self.data_home())
            .env("XDG_CACHE_HOME", self.dir.join("cache"))
            .output()
            .expect("fc-list runs (install the fontconfig package)");
        assert!(out.status.success(), "fc-list failed: {out:?}");
        out.stdout
            .split(|b| *b == b'\n')
            .filter(|l| !l.is_empty())
            .map(|l| PathBuf::from(std::ffi::OsStr::from_bytes(l)))
            .collect()
    }

    /// Polls `fc-list` until `check` holds for its answer, up to [`WAIT`].
    fn fc_list_until(&self, family: &str, check: impl Fn(&[PathBuf]) -> bool) -> Vec<PathBuf> {
        let start = Instant::now();
        loop {
            let files = self.fc_list(family);
            if check(&files) || start.elapsed() > WAIT {
                return files;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let activator = self.activator();
        if let Ok(left) = activator.list_active() {
            let _ = activator.deactivate(&left);
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn activate(activator: &LinuxActivator, path: PathBuf, ps: &str, mode: Mode) -> Registration {
    let outcomes = activator
        .activate(
            &[FaceFile {
                path,
                postscript_name: ps.to_owned(),
            }],
            mode,
        )
        .expect("activator works");
    match &outcomes[0].status {
        OutcomeStatus::Activated(r) => r.clone(),
        other => panic!("activation failed: {other:?}"),
    }
}

/// `files` is exactly the one link `reg` made, in `folder`.
fn listed_through(files: &[PathBuf], reg: &Registration, folder: &Path) -> bool {
    files.len() == 1 && files[0] == Path::new(&reg.handle) && files[0].parent() == Some(folder)
}

#[test]
fn permanent_activation_is_seen_by_fc_list_until_deactivated() {
    let family = "Emblema One";
    let fixture = Fixture::new("permanent");
    assert!(
        fixture.fc_list(family).is_empty(),
        "{family} is installed on this machine already; the test cannot prove anything"
    );
    let activator = fixture.activator();
    let folder = fixture.data_home().join("fonts").join("font-manager");
    assert_eq!(activator.dir(Mode::Permanent), folder);

    let reg = activate(
        &activator,
        font_file("google-repo/emblemaone/EmblemaOne-Regular.ttf"),
        "EmblemaOne-Regular",
        Mode::Permanent,
    );
    let files = fixture.fc_list_until(family, |f| !f.is_empty());
    assert!(
        listed_through(&files, &reg, &folder),
        "fc-list does not find {family} through the permanent link: {files:?}"
    );

    let outcomes = activator.deactivate(&[reg]).expect("activator works");
    assert_eq!(outcomes[0].status, OutcomeStatus::Deactivated);
    let files = fixture.fc_list_until(family, <[PathBuf]>::is_empty);
    assert!(files.is_empty(), "{family} is still listed: {files:?}");
}

#[test]
fn session_activation_is_seen_by_fc_list_and_gone_after_clear_session() {
    let family = "Diplomata";
    let fixture = Fixture::new("session");
    assert!(
        fixture.fc_list(family).is_empty(),
        "{family} is installed on this machine already; the test cannot prove anything"
    );
    let folder = fixture
        .data_home()
        .join("fonts")
        .join("font-manager")
        .join("session");

    let reg = activate(
        &fixture.activator(),
        font_file("google-repo/diplomata/Diplomata-Regular.ttf"),
        "Diplomata-Regular",
        Mode::Session,
    );
    let files = fixture.fc_list_until(family, |f| !f.is_empty());
    assert!(
        listed_through(&files, &reg, &folder),
        "fc-list does not find {family} through the session link: {files:?}"
    );

    // A fresh activator stands for quit, or for the next start after a crash.
    let cleared = fixture
        .activator()
        .clear_session()
        .expect("activator works");
    assert_eq!(cleared.len(), 1);
    assert_eq!(cleared[0].path, reg.path);
    assert_eq!(cleared[0].status, OutcomeStatus::Deactivated);
    let files = fixture.fc_list_until(family, <[PathBuf]>::is_empty);
    assert!(
        files.is_empty(),
        "{family} is still listed after the session was cleared: {files:?}"
    );
    assert!(!Path::new(&reg.handle).exists());
}
