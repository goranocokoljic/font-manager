//! E17.1 against the real activator of each OS: after the main window's
//! close handler hides it to the tray, a second process still sees a
//! session-activated fixture font; after Quit (the exit handler, which runs
//! `ActivationManager::shutdown`) it no longer does, and a permanent one stays.
//!
//! The second process is this test binary again, running only `probe_child`
//! with `FM_PROBE_FAMILY` set; it asks the OS font list the way other apps
//! do: DirectWrite on Windows, CoreText on macOS, `fc-list` on Linux. The
//! activator's records are scratch (a test ledger key or file, a scratch
//! `XDG_DATA_HOME`), so the app's own activations are never read or changed.
//!
//! Fonts: Nosifer (session) and Cagliostro (permanent) from `fixtures/fonts/`,
//! indexed in place; no runner image ships them, so "before" is a real
//! negative. The fm-platform activation tests use other families, so the two
//! test binaries can run at the same time.
// The probes are FFI. The whole file is test code: its helpers fail the test
// by panicking, which clippy's in-test allowance only sees inside `#[test]`.
#![allow(unsafe_code, clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use fm_api::activation::{ActivateRequest, ActivationMode, ConflictPolicy, Target};
use fm_api::library::{AddSourceRequest, ImportMode, ListFamiliesRequest, ScanRequest};
use fm_api::settings::SettingsUpdate;
use fm_api::{ActivationService, Event, EventSource, FamilyId, LibraryService, SettingsService};
use fm_app_lib::tray::{TrayState, on_close_requested};
use fm_app_lib::{AppState, on_exit, serve, setup_state};
use fm_core::CoreApi;
use fm_core::lifecycle::CloseAction;
use fm_platform::{AppPaths, FontActivator, MockActivator, TraySupport};
use tauri::Manager;
use tauri::test::{mock_builder, mock_context, noop_assets};

const PROBE_ENV: &str = "FM_PROBE_FAMILY";
const PROBE_TAG: &str = "FM_PROBE_FOUND=";
/// A new process sees a change at once (ADR 0008); the margin is for slow runners.
const WAIT: Duration = Duration::from_secs(10);
const SCAN_WAIT: Duration = Duration::from_secs(60);

const SESSION_FAMILY: &str = "Nosifer";
const PERMANENT_FAMILY: &str = "Cagliostro";

fn fixtures() -> PathBuf {
    // The repository root without `..`: `absolute` removes it on Windows only,
    // and a source path with `..` in it indexes nothing on Linux.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("repository root")
        .join("fixtures")
        .join("fonts")
}

#[cfg(windows)]
mod os {
    use super::*;
    use fm_platform::WindowsActivator;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Graphics::DirectWrite::{
        DWRITE_FACTORY_TYPE_ISOLATED, DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection,
    };
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RegDeleteTreeW};
    use windows::core::{BOOL, PCWSTR};

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    /// DirectWrite's answer, in this process, to "is `family` installed?".
    pub fn has(family: &str) -> bool {
        let name = wide(family);
        // SAFETY: COM calls on interfaces DirectWrite returned; out pointers
        // are locals; `name` is NUL-terminated and outlives the call.
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

    fn ledger(root: &Path) -> String {
        format!(
            r"Software\FontManager-test\tray-{}-{}",
            std::process::id(),
            root.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        )
    }

    pub fn activator(root: &Path, _paths: &AppPaths) -> Arc<dyn FontActivator> {
        Arc::new(WindowsActivator::with_keys(
            fm_platform::FONTS_KEY,
            &ledger(root),
            true,
        ))
    }

    pub fn probe_env(_root: &Path, cmd: &mut Command) {
        let _ = cmd;
    }

    pub fn forget(root: &Path) {
        let key = wide(&ledger(root));
        // SAFETY: `key` is NUL-terminated and outlives the call.
        let _ = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(key.as_ptr())) };
    }
}

#[cfg(target_os = "macos")]
mod os {
    use super::*;
    use fm_platform::MacosActivator;
    use std::ffi::{c_char, c_void};

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
    }

    #[link(name = "CoreText", kind = "framework")]
    unsafe extern "C" {
        fn CTFontManagerCopyAvailableFontFamilyNames() -> CFTypeRef;
    }

    /// CoreText's answer, in this process, to "is `family` available?".
    pub fn has(family: &str) -> bool {
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

    pub fn activator(root: &Path, _paths: &AppPaths) -> Arc<dyn FontActivator> {
        Arc::new(MacosActivator::with_ledger(
            root.join(fm_platform::LEDGER_FILE),
        ))
    }

    pub fn probe_env(_root: &Path, cmd: &mut Command) {
        let _ = cmd;
    }

    pub fn forget(_root: &Path) {}
}

#[cfg(target_os = "linux")]
mod os {
    use super::*;
    use fm_platform::LinuxActivator;

    /// `fc-list`'s answer, in a new process, to "is `family` available?",
    /// with the `XDG_DATA_HOME` the parent process gave this one.
    pub fn has(family: &str) -> bool {
        let out = Command::new("fc-list")
            .args(["--format", "%{file}\n", &format!(":family={family}")])
            .output()
            .expect("fc-list runs (install the fontconfig package)");
        assert!(out.status.success(), "fc-list failed: {out:?}");
        out.stdout.contains(&b'\n')
    }

    /// The links go to `$XDG_DATA_HOME/fonts/font-manager`, with
    /// `XDG_DATA_HOME` the parent of the scratch data folder.
    pub fn activator(_root: &Path, paths: &AppPaths) -> Arc<dyn FontActivator> {
        Arc::new(LinuxActivator::new(paths))
    }

    pub fn probe_env(root: &Path, cmd: &mut Command) {
        cmd.env("XDG_DATA_HOME", root)
            .env("XDG_CACHE_HOME", root.join("fc-cache"));
    }

    pub fn forget(_root: &Path) {}
}

/// Not a check of its own: the child half of [`seen_by_second_process`].
#[test]
fn probe_child() {
    if let Some(family) = std::env::var_os(PROBE_ENV) {
        let family = family.to_string_lossy();
        println!("{PROBE_TAG}{}", os::has(&family));
    }
}

/// Starts a new process that asks the OS font list for `family`.
fn seen_by_second_process(root: &Path, family: &str) -> bool {
    let mut cmd = Command::new(std::env::current_exe().expect("test binary"));
    cmd.args(["probe_child", "--exact", "--nocapture", "--test-threads=1"])
        .env(PROBE_ENV, family);
    os::probe_env(root, &mut cmd);
    let out = cmd.output().expect("second process runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let answer = stdout
        .lines()
        .find_map(|l| l.split_once(PROBE_TAG).map(|(_, answer)| answer.trim()))
        .unwrap_or_else(|| panic!("probe gave no answer: {stdout}"));
    answer == "true"
}

/// Polls new processes until they agree with `expected`, up to [`WAIT`].
fn second_process_agrees(root: &Path, family: &str, expected: bool) -> bool {
    let start = Instant::now();
    loop {
        if seen_by_second_process(root, family) == expected {
            return true;
        }
        if start.elapsed() > WAIT {
            return false;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Scratch app folders; on drop, deactivates whatever the activator still
/// lists, forgets the scratch ledger and removes the folder.
struct Home {
    root: PathBuf,
    paths: AppPaths,
    activator: Arc<dyn FontActivator>,
}

impl Home {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!("fm-tray-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = AppPaths {
            data: root.join("data"),
            config: root.join("config"),
            cache: root.join("cache"),
            logs: root.join("logs"),
            library: root.join("Fonts"),
        };
        fm_core::startup::prepare(&paths).expect("app folders");
        let activator = os::activator(&root, &paths);
        Self {
            root,
            paths,
            activator,
        }
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        if let Ok(left) = self.activator.list_active() {
            let _ = self.activator.deactivate(&left);
        }
        os::forget(&self.root);
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Indexes the fixture folder `rel` in place and returns the family named `name`.
fn index_family(core: &CoreApi, rel: &str, name: &str) -> FamilyId {
    let events = core.subscribe();
    let source = core
        .add_source(AddSourceRequest {
            path: fixtures().join(rel).display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source added");
    core.scan(ScanRequest {
        source: Some(source.id),
    })
    .expect("scan starts");
    // `ScanFinished` comes before the index is written; the source's
    // `LibraryChanged` comes after, so the family can be listed from then on.
    let deadline = Instant::now() + SCAN_WAIT;
    loop {
        match events.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Event::LibraryChanged {
                source: Some(s), ..
            }) if s == source.id => break,
            Ok(Event::JobFailed { error, .. }) => panic!("scan failed: {error:?}"),
            Ok(_) => {}
            Err(e) => panic!("no scan result: {e}"),
        }
    }
    let families = core
        .list_families(ListFamiliesRequest {
            source: Some(source.id),
            ..ListFamiliesRequest::default()
        })
        .expect("families")
        .items;
    let names: Vec<String> = families.iter().map(|f| f.name.clone()).collect();
    families
        .into_iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("{name} not indexed from {}: {names:?}", source.path))
        .id
}

fn activate(core: &CoreApi, family: FamilyId, mode: ActivationMode) {
    let outcomes = core
        .activate(ActivateRequest {
            target: Target::Family { family },
            mode,
            on_conflict: ConflictPolicy::KeepExisting,
        })
        .expect("activates");
    assert!(
        outcomes
            .iter()
            .all(|o| o.outcome == fm_api::activation::OutcomeStatus::Done),
        "{outcomes:?}"
    );
}

/// A mock app over `state`, with a main window and a tray item.
fn app(
    state: AppState,
    tray: TraySupport,
) -> (
    tauri::App<tauri::test::MockRuntime>,
    tauri::WebviewWindow<tauri::test::MockRuntime>,
) {
    let app = setup_state(mock_builder(), state)
        .build(mock_context(noop_assets()))
        .expect("app builds");
    app.state::<TrayState>().set(tray);
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("main window");
    (app, window)
}

#[test]
fn closing_the_window_keeps_session_fonts_and_quit_ends_them() {
    for family in [SESSION_FAMILY, PERMANENT_FAMILY] {
        assert!(
            !seen_by_second_process(Path::new("."), family),
            "{family} is installed on this machine already; the test cannot prove anything"
        );
    }
    let home = Home::new("lifecycle");
    let index = fm_core::startup::open_index(&home.paths);
    let (state, core) = serve(&home.paths, index, Arc::clone(&home.activator));
    let core = core.expect("the real library");
    let session = index_family(&core, "google-repo/nosifer", SESSION_FAMILY);
    let permanent = index_family(&core, "google-repo/cagliostro", PERMANENT_FAMILY);
    activate(&core, session, ActivationMode::Session);
    activate(&core, permanent, ActivationMode::Permanent);
    assert!(
        second_process_agrees(&home.root, SESSION_FAMILY, true),
        "a second process does not see the session font"
    );

    // Close the main window: the default settings keep the app in the tray.
    let (_app, window) = app(state, TraySupport::Available);
    assert_eq!(
        on_close_requested(&window.as_ref().window()),
        CloseAction::HideToTray
    );
    assert!(
        second_process_agrees(&home.root, SESSION_FAMILY, true),
        "the session font went away when the window closed"
    );
    assert_eq!(core.list_active().expect("active").len(), 2);

    // Quit: the exit handler every quit path ends in.
    on_exit(Some(&core));
    assert!(
        second_process_agrees(&home.root, SESSION_FAMILY, false),
        "a second process still sees the session font after Quit"
    );
    assert!(
        second_process_agrees(&home.root, PERMANENT_FAMILY, true),
        "the permanent font went away at Quit"
    );
    let active = core.list_active().expect("active");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].mode, ActivationMode::Permanent);
}

#[test]
fn closing_quits_when_the_setting_is_off_or_there_is_no_tray() {
    let home = Home::new("close-off");
    let index = fm_core::startup::open_index(&home.paths);
    let (state, core) = serve(&home.paths, index, Arc::new(MockActivator::new()));
    let core = core.expect("the real library");

    // No tray item (a Linux desktop without a StatusNotifier host).
    let (_app, window) = app(state, TraySupport::Unavailable("no host".into()));
    assert_eq!(
        on_close_requested(&window.as_ref().window()),
        CloseAction::Quit
    );

    // A tray item, but "Keep running in the tray" off.
    let state = AppState::core(Arc::clone(&core));
    core.update_settings(SettingsUpdate {
        keep_in_tray: Some(false),
        ..SettingsUpdate::default()
    })
    .expect("saved");
    let (_app, window) = app(state, TraySupport::Available);
    assert_eq!(
        on_close_requested(&window.as_ref().window()),
        CloseAction::Quit
    );
}
