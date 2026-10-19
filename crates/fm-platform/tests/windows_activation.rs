//! The Windows activator against the real per-user Fonts key: a second process
//! finds the family through DirectWrite enumeration while it is active, and
//! not after deactivation or session clean-up (ADR 0008).
//!
//! The second process is this test binary again, running only `probe_child`
//! with `FM_PROBE_FAMILY` set. The ledger is a scratch key, so the app's own
//! activation records are never read or changed.
//!
//! Fonts: Diplomata (session) and Emblema One (permanent) from
//! `fixtures/fonts/`; no Windows image ships them, so "before" is a real negative.
#![cfg(windows)]
// The DirectWrite probe and the scratch-key clean-up are FFI. The whole file is
// test code: its helpers fail the test by panicking, which clippy's in-test
// allowance only sees inside `#[test]` functions.
#![allow(unsafe_code, clippy::expect_used, clippy::panic)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use fm_platform::{FaceFile, FontActivator, Mode, OutcomeStatus, Registration, WindowsActivator};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_ISOLATED, DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection,
};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RegDeleteTreeW};
use windows::core::{BOOL, PCWSTR};

const PROBE_ENV: &str = "FM_PROBE_FAMILY";
const PROBE_TAG: &str = "FM_PROBE_FOUND=";
/// A new process sees a change at once (ADR 0008); the margin is for slow runners.
const WAIT: Duration = Duration::from_secs(10);

fn font_file(rel: &str) -> PathBuf {
    // `absolute`, not `canonicalize`: no verbatim `\\?\` prefix in the registry value.
    let path = std::path::absolute(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts")
            .join(rel),
    )
    .expect("fixture path");
    assert!(path.is_file(), "missing fixture {}", path.display());
    path
}

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

/// DirectWrite's answer, in this process, to "is `family` installed?".
fn directwrite_has(family: &str) -> bool {
    let name = wide(family);
    // SAFETY: COM calls on interfaces DirectWrite returned; out pointers are
    // locals; `name` is NUL-terminated and outlives the call.
    unsafe {
        let factory: IDWriteFactory =
            DWriteCreateFactory(DWRITE_FACTORY_TYPE_ISOLATED).expect("DirectWrite factory");
        let mut collection: Option<IDWriteFontCollection> = None;
        factory
            .GetSystemFontCollection(&mut collection, true)
            .expect("system font collection");
        let collection = collection.expect("system font collection");
        let mut index = 0u32;
        let mut exists = BOOL::default();
        collection
            .FindFamilyName(PCWSTR(name.as_ptr()), &mut index, &mut exists)
            .expect("FindFamilyName");
        exists.as_bool()
    }
}

/// Not a check of its own: the child half of [`seen_by_second_process`].
#[test]
fn probe_child() {
    if let Some(family) = std::env::var_os(PROBE_ENV) {
        let family = family.to_string_lossy();
        println!("{PROBE_TAG}{}", directwrite_has(&family));
    }
}

/// Starts a new process that enumerates fonts through DirectWrite and says
/// whether it finds `family`.
fn seen_by_second_process(family: &str) -> bool {
    let out = Command::new(std::env::current_exe().expect("test binary"))
        .args(["probe_child", "--exact", "--nocapture", "--test-threads=1"])
        .env(PROBE_ENV, family)
        .output()
        .expect("second process runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let answer = stdout
        .lines()
        .find_map(|l| l.split_once(PROBE_TAG).map(|(_, answer)| answer.trim()))
        .unwrap_or_else(|| panic!("probe gave no answer: {stdout}"));
    answer == "true"
}

/// Polls new processes until they agree with `expected`, up to [`WAIT`].
fn second_process_agrees(family: &str, expected: bool) -> bool {
    let start = Instant::now();
    loop {
        if seen_by_second_process(family) == expected {
            return true;
        }
        if start.elapsed() > WAIT {
            return false;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// The scratch ledger, and on drop: deactivate whatever is still listed, then
/// delete the scratch key, so a failed test leaves no font behind.
struct Fixture {
    ledger: String,
}

impl Fixture {
    fn new(name: &str) -> Self {
        Self {
            ledger: format!(
                r"Software\FontManager-test\it-{}-{name}",
                std::process::id()
            ),
        }
    }

    fn activator(&self) -> WindowsActivator {
        WindowsActivator::with_keys(fm_platform::FONTS_KEY, &self.ledger, true)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let activator = self.activator();
        if let Ok(left) = activator.list_active() {
            let _ = activator.deactivate(&left);
        }
        let key = wide(&self.ledger);
        // SAFETY: `key` is NUL-terminated and outlives the call.
        let _ = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(key.as_ptr())) };
    }
}

fn activate(activator: &WindowsActivator, path: PathBuf, ps: &str, mode: Mode) -> Registration {
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

#[test]
fn permanent_activation_is_seen_by_a_second_process_until_deactivated() {
    let family = "Emblema One";
    assert!(
        !seen_by_second_process(family),
        "{family} is installed on this machine already; the test cannot prove anything"
    );
    let fixture = Fixture::new("permanent");
    let activator = fixture.activator();

    let reg = activate(
        &activator,
        font_file("google-repo/emblemaone/EmblemaOne-Regular.ttf"),
        "EmblemaOne-Regular",
        Mode::Permanent,
    );
    assert!(
        second_process_agrees(family, true),
        "a second process does not find {family} through DirectWrite"
    );

    let outcomes = activator.deactivate(&[reg]).expect("activator works");
    assert_eq!(outcomes[0].status, OutcomeStatus::Deactivated);
    assert!(
        second_process_agrees(family, false),
        "{family} is still listed after deactivation"
    );
}

#[test]
fn session_activation_is_seen_by_a_second_process_and_gone_after_clear_session() {
    let family = "Diplomata";
    assert!(
        !seen_by_second_process(family),
        "{family} is installed on this machine already; the test cannot prove anything"
    );
    let fixture = Fixture::new("session");

    let reg = activate(
        &fixture.activator(),
        font_file("google-repo/diplomata/Diplomata-Regular.ttf"),
        "Diplomata-Regular",
        Mode::Session,
    );
    assert!(
        second_process_agrees(family, true),
        "a second process does not find {family} through DirectWrite"
    );

    // A fresh activator stands for quit, or for the next start after a crash.
    let cleared = fixture
        .activator()
        .clear_session()
        .expect("activator works");
    assert_eq!(cleared.len(), 1);
    assert_eq!(cleared[0].path, reg.path);
    assert_eq!(cleared[0].status, OutcomeStatus::Deactivated);
    assert!(
        second_process_agrees(family, false),
        "{family} is still listed after the session was cleared"
    );
}
