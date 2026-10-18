//! The macOS activator against real CoreText: a second process finds the
//! family through CoreText enumeration while it is active, and not after
//! deactivation or session clean-up (ADR 0008).
//!
//! The second process is this test binary again, running only `probe_child`
//! with `FM_PROBE_FAMILY` set; it asks `CTFontManagerCopyAvailableFontFamilyNames`.
//! The ledger is a scratch file, so the app's own activation records are never
//! read or changed.
//!
//! Fonts: Diplomata (session) and Emblema One (permanent) from
//! `fixtures/fonts/`; no macOS image ships them, so "before" is a real negative.
#![cfg(target_os = "macos")]
// The CoreText probe is FFI. The whole file is test code: its helpers fail the
// test by panicking, which clippy's in-test allowance only sees inside
// `#[test]` functions.
#![allow(unsafe_code, clippy::expect_used, clippy::panic)]

use std::ffi::{c_char, c_void};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use fm_platform::{FaceFile, FontActivator, MacosActivator, Mode, OutcomeStatus, Registration};

const PROBE_ENV: &str = "FM_PROBE_FAMILY";
const PROBE_TAG: &str = "FM_PROBE_FOUND=";
/// A new process sees a change at once (ADR 0008); the margin is for slow runners.
const WAIT: Duration = Duration::from_secs(10);

type CFTypeRef = *const c_void;
const UTF8: u32 = 0x0800_0100; // kCFStringEncodingUTF8

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: CFTypeRef);
    fn CFArrayGetCount(array: CFTypeRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CFTypeRef, index: isize) -> CFTypeRef;
    fn CFStringGetLength(s: CFTypeRef) -> isize;
    fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: u32) -> isize;
    fn CFStringGetCString(s: CFTypeRef, buffer: *mut c_char, size: isize, encoding: u32) -> u8;
    fn CFURLCreateFromFileSystemRepresentation(
        allocator: CFTypeRef,
        buffer: *const u8,
        len: isize,
        is_directory: u8,
    ) -> CFTypeRef;
    fn CFErrorGetCode(err: CFTypeRef) -> isize;
}

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    fn CTFontManagerCopyAvailableFontFamilyNames() -> CFTypeRef;
    fn CTFontManagerUnregisterFontsForURL(
        url: CFTypeRef,
        scope: u32,
        error: *mut CFTypeRef,
    ) -> bool;
}

/// `kCTFontManagerScopeUser` and `kCTFontManagerScopeSession`.
const SCOPE_USER: u32 = 2;
const SCOPE_SESSION: u32 = 3;
/// `kCTFontManagerErrorNotRegistered`.
const NOT_REGISTERED: isize = 201;

/// Asks CoreText directly to unregister `path` from `scope` and returns the
/// error code, or 0 on success. Used on the scope the activator did NOT use:
/// CoreText answers "not registered" there only if the activator registered
/// the file in the other scope.
fn unregister_code(path: &Path, scope: u32) -> isize {
    let bytes = path.as_os_str().as_bytes();
    // SAFETY: `bytes` lives through the call; the URL and the error are
    // Create results released below; `err` is a local out pointer.
    unsafe {
        let url = CFURLCreateFromFileSystemRepresentation(
            std::ptr::null(),
            bytes.as_ptr(),
            isize::try_from(bytes.len()).expect("path length"),
            0,
        );
        assert!(!url.is_null(), "file URL for {}", path.display());
        let mut err: CFTypeRef = std::ptr::null();
        let ok = CTFontManagerUnregisterFontsForURL(url, scope, &mut err);
        CFRelease(url);
        let code = if err.is_null() {
            0
        } else {
            CFErrorGetCode(err)
        };
        if !err.is_null() {
            CFRelease(err);
        }
        if ok { 0 } else { code }
    }
}

/// CoreText's answer, in this process, to "is `family` available?".
fn coretext_has(family: &str) -> bool {
    // SAFETY: the array is a Copy result released below; its items are
    // CFStrings borrowed while the array lives; `buffer` is writable for `size`.
    unsafe {
        let names = CTFontManagerCopyAvailableFontFamilyNames();
        assert!(!names.is_null(), "CoreText lists no families");
        let mut found = false;
        for i in 0..CFArrayGetCount(names) {
            let name = CFArrayGetValueAtIndex(names, i);
            let size = CFStringGetMaximumSizeForEncoding(CFStringGetLength(name), UTF8) + 1;
            let mut buffer = vec![0u8; usize::try_from(size).expect("size")];
            if CFStringGetCString(name, buffer.as_mut_ptr().cast(), size, UTF8) == 0 {
                continue;
            }
            let end = buffer.iter().position(|b| *b == 0).unwrap_or(buffer.len());
            if &buffer[..end] == family.as_bytes() {
                found = true;
                break;
            }
        }
        CFRelease(names);
        found
    }
}

/// Not a check of its own: the child half of [`seen_by_second_process`].
#[test]
fn probe_child() {
    if let Some(family) = std::env::var_os(PROBE_ENV) {
        let family = family.to_string_lossy();
        println!("{PROBE_TAG}{}", coretext_has(&family));
    }
}

/// Starts a new process that enumerates fonts through CoreText and says
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

fn font_file(rel: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts")
        .join(rel)
        .canonicalize()
        .expect("fixture path");
    assert!(path.is_file(), "missing fixture {}", path.display());
    path
}

/// The scratch ledger, and on drop: deactivate whatever is still listed, then
/// delete the scratch folder, so a failed test leaves no font behind.
struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        Self {
            dir: std::env::temp_dir().join(format!("fm-it-{}-{name}", std::process::id())),
        }
    }

    fn activator(&self) -> MacosActivator {
        MacosActivator::with_ledger(self.dir.join(fm_platform::LEDGER_FILE))
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

fn activate(activator: &MacosActivator, path: PathBuf, ps: &str, mode: Mode) -> Registration {
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
        "a second process does not find {family} through CoreText"
    );
    // Criterion 1: the registration is in the user scope, not the session one.
    assert_eq!(
        unregister_code(&reg.path, SCOPE_SESSION),
        NOT_REGISTERED,
        "a permanent activation must not be in the session scope"
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
        "a second process does not find {family} through CoreText"
    );
    // Criterion 1: the registration is in the session scope, not the user one.
    assert_eq!(
        unregister_code(&reg.path, SCOPE_USER),
        NOT_REGISTERED,
        "a session activation must not be in the user scope"
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

#[test]
fn a_file_coretext_cannot_read_fails_without_a_ledger_entry() {
    let fixture = Fixture::new("broken");
    std::fs::create_dir_all(&fixture.dir).expect("scratch folder");
    let broken = fixture.dir.join("Broken.ttf");
    std::fs::write(&broken, b"not a font").expect("scratch file");
    let activator = fixture.activator();

    let outcomes = activator
        .activate(
            &[FaceFile {
                path: broken,
                postscript_name: "Broken".to_owned(),
            }],
            Mode::Session,
        )
        .expect("activator works");
    assert!(
        matches!(outcomes[0].status, OutcomeStatus::Failed(_)),
        "{:?}",
        outcomes[0].status
    );
    assert!(activator.list_active().expect("activator works").is_empty());
}
