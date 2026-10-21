//! E18.2: an update installs only when it is signed with the release key
//! whose public half ships in `tauri.conf.json`, and only after the normal
//! quit path has deregistered the session fonts.
//!
//! The signature checks run through Tauri's updater on a mock app that has
//! the shipped `plugins.updater` config. The package in
//! `tests/fixtures/updater/` was signed for version 1.1.0 with a throwaway
//! test key (its private half was deleted); `test-key.pub` is that key's
//! public half, the positive control. No network: an update is restored from
//! a manifest the test writes, the bytes are the fixture's.
// Test code: its helpers fail the test by panicking.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use fm_api::activation::{ActivateRequest, ActivationMode, ConflictPolicy, Target};
use fm_api::library::{AddSourceRequest, ImportMode, ScanRequest};
use fm_api::settings::SettingsUpdate;
use fm_api::{ActivationService, ApiError, ErrorCode, Event, EventSource, LibraryService};
use fm_api::{FaceId, SettingsService};
use fm_app_lib::updater::{
    available, install_after_quit, install_error, is_signature_error, verify,
};
use fm_app_lib::{AppState, serve};
use fm_core::CoreApi;
use fm_platform::{AppPaths, MockActivator};
use serde_json::{Value, json};
use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};
use tauri_plugin_updater::{Update, UpdaterExt};

const SCAN_WAIT: Duration = Duration::from_secs(60);

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn fixture(name: &str) -> PathBuf {
    manifest_dir().join("tests/fixtures/updater").join(name)
}

fn read_text(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .trim()
        .to_owned()
}

/// `plugins.updater` of the shipped `tauri.conf.json`.
fn shipped_config() -> Value {
    let conf: Value =
        serde_json::from_str(&read_text(&manifest_dir().join("tauri.conf.json"))).expect("json");
    conf["plugins"]["updater"].clone()
}

/// The shipped config with another public key.
fn config_with_key(pubkey: &str) -> Value {
    let mut config = shipped_config();
    config["pubkey"] = json!(pubkey);
    config
}

fn app_with(config: Value) -> tauri::App<MockRuntime> {
    let mut context = mock_context(noop_assets());
    context
        .config_mut()
        .plugins
        .0
        .insert("updater".into(), config);
    mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)
        .expect("app builds")
}

/// The update a release server announcing `version` with `signature` offers
/// to the mock app (version 0.1.0).
fn offered(app: &tauri::App<MockRuntime>, version: &str, signature: &str) -> Update {
    app.updater_builder()
        .target("test-target")
        .build()
        .expect("updater")
        .restore_update(json!({
            "version": version,
            "notes": "Faster scans.",
            "pub_date": "2026-10-01T12:00:00Z",
            "platforms": {
                "test-target": { "url": "https://example.invalid/package", "signature": signature }
            }
        }))
        .expect("a valid manifest")
        .expect("newer than 0.1.0")
}

fn package() -> Vec<u8> {
    std::fs::read(fixture("package.bin")).expect("fixture package")
}

fn signature() -> String {
    read_text(&fixture("package.bin.sig"))
}

fn refused(result: Result<(), ApiError>) -> ApiError {
    let e = result.expect_err("the package must be refused");
    assert_eq!(e.code, ErrorCode::PermissionDenied, "{e:?}");
    assert!(
        e.message
            .contains("not signed with Font Manager's release key"),
        "not a user-facing refusal: {e:?}"
    );
    e
}

#[test]
fn the_shipped_config_has_a_release_key_an_endpoint_and_requires_signed_versions() {
    let config = shipped_config();
    let key = config["pubkey"].as_str().expect("pubkey");
    assert!(!key.is_empty());
    assert_ne!(
        key,
        read_text(&fixture("test-key.pub")),
        "the test key must not ship"
    );
    assert_eq!(config["requireSignedVersion"], true);
    let endpoint = config["endpoints"][0].as_str().expect("endpoint");
    assert!(
        endpoint.starts_with("https://") && endpoint.ends_with("/latest.json"),
        "{endpoint}"
    );
}

#[test]
fn a_package_signed_with_the_release_key_is_accepted_positive_control() {
    // The same check with the test key as the trusted key: the fixture is
    // well formed, so the refusals below are about the key and the bytes.
    let app = app_with(config_with_key(&read_text(&fixture("test-key.pub"))));
    let update = offered(&app, "1.1.0", &signature());
    verify(&update, &package()).expect("signed with the trusted key");
}

#[test]
fn a_package_signed_with_another_key_is_refused() {
    let app = app_with(shipped_config());
    let update = offered(&app, "1.1.0", &signature());
    refused(verify(&update, &package()));
    // install checks again, and installs nothing.
    let e = update.install(package()).expect_err("refused");
    assert!(is_signature_error(&e), "{e:?}");
}

#[test]
fn a_tampered_package_is_refused() {
    let app = app_with(config_with_key(&read_text(&fixture("test-key.pub"))));
    let update = offered(&app, "1.1.0", &signature());
    let mut bytes = package();
    bytes[0] ^= 0x01;
    refused(verify(&update, &bytes));
    let e = update.install(&bytes).expect_err("refused");
    assert_eq!(install_error(e).code, ErrorCode::PermissionDenied);
}

#[test]
fn an_unsigned_package_is_refused() {
    let app = app_with(config_with_key(&read_text(&fixture("test-key.pub"))));
    for signature in ["", "bm90IGEgc2lnbmF0dXJl"] {
        let update = offered(&app, "1.1.0", signature);
        refused(verify(&update, &package()));
    }
}

#[test]
fn a_signed_package_announced_as_another_version_is_refused() {
    // A tampered manifest pairs a newer version number with an older package.
    let app = app_with(config_with_key(&read_text(&fixture("test-key.pub"))));
    let update = offered(&app, "2.0.0", &signature());
    refused(verify(&update, &package()));
}

#[test]
fn the_offer_shows_version_notes_date_and_kind() {
    let app = app_with(shipped_config());
    let shown = available(&offered(&app, "1.1.0", &signature()))
        .expect("valid")
        .expect("newer");
    assert_eq!(shown.current_version, "0.1.0");
    assert_eq!(shown.version, "1.1.0");
    assert_eq!(shown.notes.as_deref(), Some("Faster scans."));
    assert_eq!(shown.date.as_deref(), Some("2026-10-01T12:00:00Z"));
    // 0.1 → 1.1 is a major step: it asks a second time.
    assert!(shown.needs_second_confirmation());
}

// ---- the quit path before an install ----

fn fixtures() -> PathBuf {
    manifest_dir()
        .ancestors()
        .nth(3)
        .expect("repository root")
        .join("fixtures")
        .join("fonts")
}

struct Home {
    root: PathBuf,
    paths: AppPaths,
}

impl Home {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!("fm-update-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = AppPaths {
            data: root.join("data"),
            config: root.join("config"),
            cache: root.join("cache"),
            logs: root.join("logs"),
            library: root.join("Fonts"),
        };
        fm_core::startup::prepare(&paths).expect("app folders");
        Self { root, paths }
    }

    /// The app as startup serves it, over this home's index.
    fn serve(&self) -> (AppState, Arc<CoreApi>) {
        let index = fm_core::startup::open_index(&self.paths);
        let (state, core) = serve(&self.paths, index, Arc::new(MockActivator::new()));
        (state, core.expect("the real library"))
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Indexes one fixture family and activates it for the session.
fn session_font(core: &CoreApi) -> Vec<FaceId> {
    let events = core.subscribe();
    let source = core
        .add_source(AddSourceRequest {
            path: fixtures().join("google-repo/nosifer").display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source added");
    core.scan(ScanRequest {
        source: Some(source.id),
    })
    .expect("scan starts");
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
    let family = core
        .list_families(Default::default())
        .expect("families")
        .items[0]
        .id;
    core.activate(ActivateRequest {
        target: Target::Family { family },
        mode: ActivationMode::Session,
        on_conflict: ConflictPolicy::KeepExisting,
    })
    .expect("activates");
    let active = core.list_active().expect("active");
    assert!(!active.is_empty());
    active.into_iter().map(|a| a.face).collect()
}

#[test]
fn installing_runs_the_quit_path_first_so_session_fonts_are_deregistered() {
    let home = Home::new("quit-first");
    let (state, core) = home.serve();
    session_font(&core);
    let mut active_at_install = None;
    install_after_quit(&state, || {
        active_at_install = Some(core.list_active().expect("active"));
        Ok(())
    })
    .expect("installed");
    assert_eq!(
        active_at_install,
        Some(Vec::new()),
        "session fonts were still registered when the installer ran"
    );
}

#[test]
fn the_quit_at_restart_after_an_install_keeps_the_last_session() {
    // "Reactivate on launch" must bring back the fonts of the session the
    // update ended, so the quit path must not run a second time at exit.
    let home = Home::new("quit-once");
    let (state, core) = home.serve();
    core.update_settings(SettingsUpdate {
        reactivate_on_launch: Some(true),
        ..SettingsUpdate::default()
    })
    .expect("settings");
    let faces = session_font(&core);
    install_after_quit(&state, || Ok(())).expect("installed");
    // The restart's RunEvent::Exit.
    state.run_quit_path();
    drop((state, core));

    let (_state, next) = home.serve();
    let mut back: Vec<FaceId> = next
        .list_active()
        .expect("active")
        .into_iter()
        .map(|a| a.face)
        .collect();
    back.sort_by_key(|f| f.0);
    let mut faces = faces;
    faces.sort_by_key(|f| f.0);
    assert_eq!(
        back, faces,
        "the next launch did not reactivate the last session"
    );
}

#[test]
fn a_failed_install_keeps_the_app_running_and_quit_still_ends_new_session_fonts() {
    let home = Home::new("install-fails");
    let (state, core) = home.serve();
    session_font(&core);
    let failed = install_after_quit(&state, || {
        Err(ApiError::new(
            ErrorCode::Io,
            "The update could not be installed.",
        ))
    });
    assert_eq!(failed.expect_err("failed").code, ErrorCode::Io);
    // The user activates again, then quits: the quit path runs again.
    let family = core
        .list_families(Default::default())
        .expect("families")
        .items[0]
        .id;
    core.activate(ActivateRequest {
        target: Target::Family { family },
        mode: ActivationMode::Session,
        on_conflict: ConflictPolicy::KeepExisting,
    })
    .expect("activates");
    assert!(!core.list_active().expect("active").is_empty());
    state.run_quit_path();
    assert!(core.list_active().expect("active").is_empty());
}
