//! The app over the real library: startup's folders and index serve
//! `fm-core`'s `CoreApi`, and the Tauri commands the UI calls work against a
//! folder of fixture fonts, with the events the UI listens to.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use fm_api::activation::{ActivationOutcome, ActiveFace, OutcomeStatus};
use fm_api::common::Paged;
use fm_api::library::{FaceDetail, Family, FamilySummary, Source};
use fm_api::search::SearchResponse;
use fm_api::{ApiError, ErrorCode, JobId};
use fm_app_lib::{Backend, EVENT_CHANNEL, serve, setup_state};
use fm_platform::{AppPaths, MockActivator};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{
    INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
};
use tauri::webview::InvokeRequest;
use tauri::{App, Listener, WebviewWindow};

type TestResult = Result<(), Box<dyn Error>>;

const WAIT: Duration = Duration::from_secs(60);

/// A scratch folder with the app folders and a library folder holding
/// copies of a few fixture families; removed on drop.
struct Home {
    root: PathBuf,
    paths: AppPaths,
    library: PathBuf,
}

impl Home {
    fn new(label: &str) -> Result<Self, Box<dyn Error>> {
        let root = std::env::temp_dir().join(format!("fm-app-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let library = root.join("Library");
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/fonts");
        for folder in [
            "google-repo/bebasneue",
            "google-repo/barlow",
            "google-repo/asapsharp",
        ] {
            let to = library.join(folder.rsplit('/').next().unwrap_or(folder));
            std::fs::create_dir_all(&to)?;
            for entry in std::fs::read_dir(fixtures.join(folder))? {
                let path = entry?.path();
                if path.extension().is_some_and(|e| e == "ttf") {
                    std::fs::copy(&path, to.join(path.file_name().unwrap_or_default()))?;
                }
            }
        }
        let paths = AppPaths {
            data: root.join("data"),
            config: root.join("config"),
            cache: root.join("cache"),
            logs: root.join("logs"),
            library: root.join("Fonts"),
        };
        fm_core::startup::prepare(&paths)?;
        Ok(Self {
            root,
            paths,
            library,
        })
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A built app, its main window, and the events it forwards to the UI.
type Running = (
    App<MockRuntime>,
    WebviewWindow<MockRuntime>,
    Receiver<Value>,
);

/// The app as `run` builds it from `home`, with a mock activator.
fn app(home: &Home) -> Result<Running, Box<dyn Error>> {
    let index = fm_core::startup::open_index(&home.paths);
    let (state, core) = serve(&home.paths, index, Arc::new(MockActivator::new()));
    assert_eq!(state.backend(), Backend::Core);
    assert!(core.is_some());
    let mut app = setup_state(mock_builder(), state).build(mock_context(noop_assets()))?;
    // `setup` hooks run on the first turn of the event loop; one turn starts
    // the event forwarder as `run` would.
    #[allow(deprecated)]
    app.run_iteration(|_, _| {});
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build()?;
    let (tx, rx) = mpsc::channel();
    app.listen(EVENT_CHANNEL, move |e| {
        if let Ok(v) = serde_json::from_str(e.payload()) {
            let _ = tx.send(v);
        }
    });
    Ok((app, webview, rx))
}

/// Invokes `cmd` as the webview does and decodes the answer.
fn call<T: DeserializeOwned>(
    webview: &WebviewWindow<MockRuntime>,
    cmd: &str,
    args: Value,
) -> Result<T, Box<dyn Error>> {
    let url = if cfg!(windows) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    };
    let response = get_ipc_response(
        webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: url.parse()?,
            body: InvokeBody::Json(args),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    );
    match response {
        Ok(body) => Ok(body.deserialize()?),
        Err(e) => Err(format!("{cmd} failed: {e}").into()),
    }
}

/// Events until one of type `last` arrives, that one included.
fn events_until(rx: &Receiver<Value>, last: &str) -> Result<Vec<Value>, Box<dyn Error>> {
    let deadline = Instant::now() + WAIT;
    let mut seen = Vec::new();
    loop {
        let event = rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))?;
        let kind = event["type"].as_str().unwrap_or_default().to_owned();
        seen.push(event);
        if kind == last {
            return Ok(seen);
        }
        if kind == "jobFailed" {
            return Err(format!("a job failed: {seen:?}").into());
        }
    }
}

fn kinds(events: &[Value]) -> Vec<&str> {
    events.iter().filter_map(|e| e["type"].as_str()).collect()
}

#[test]
fn the_app_serves_the_core_api_when_the_folders_were_found() -> TestResult {
    let home = Home::new("serve-core")?;
    let index = fm_core::startup::open_index(&home.paths);
    let (state, core) = serve(&home.paths, index, Arc::new(MockActivator::new()));
    assert_eq!(state.backend(), Backend::Core);
    assert!(core.is_some());
    Ok(())
}

#[test]
fn the_app_wires_the_os_pdf_printer_into_the_core_api() -> TestResult {
    use fm_api::ExportService as _;
    use fm_api::export::{SpecimenFormat, SpecimenRequest};
    let home = Home::new("serve-pdf")?;
    let index = fm_core::startup::open_index(&home.paths);
    let (_state, core) = serve(&home.paths, index, Arc::new(MockActivator::new()));
    let core = core.ok_or("no core api")?;
    // Without a printer a PDF is Unsupported before the family is looked
    // up; with the app's printer an unknown family is NotFound, and
    // nothing is printed.
    let e = core
        .export_specimen(SpecimenRequest {
            family: fm_api::FamilyId(999_999),
            format: SpecimenFormat::Pdf,
            out_path: home.root.join("specimen.pdf").display().to_string(),
        })
        .err()
        .ok_or("an unknown family must not print")?;
    assert_eq!(e.code, ErrorCode::NotFound, "{e:?}");
    assert!(!home.root.join("specimen.pdf").exists());
    Ok(())
}

#[test]
fn the_app_serves_sample_data_only_without_an_index() -> TestResult {
    let home = Home::new("serve-mock")?;
    let (state, core) = serve(
        &home.paths,
        Err(ApiError::new(ErrorCode::Unsupported, "made by a newer app")),
        Arc::new(MockActivator::new()),
    );
    assert_eq!(state.backend(), Backend::Other);
    assert!(core.is_none());
    Ok(())
}

#[test]
fn adding_a_folder_scans_it_and_the_ui_commands_work_on_the_real_index() -> TestResult {
    let home = Home::new("real-commands")?;
    let (_app, webview, events) = app(&home)?;

    let info: Value = call(&webview, "get_app_info", json!({}))?;
    assert_eq!(info["name"], "Font Manager");

    // Add a folder, scan it: progress and the library change arrive as events.
    let source: Source = call(
        &webview,
        "add_source",
        json!({ "req": { "path": home.library.display().to_string(), "mode": { "kind": "indexInPlace" } } }),
    )?;
    let job: JobId = call(&webview, "scan", json!({ "req": { "source": source.id } }))?;
    let seen = events_until(&events, "libraryChanged")?;
    let order = kinds(&seen);
    assert_eq!(order.first(), Some(&"scanStarted"), "{order:?}");
    assert!(order.contains(&"scanProgress"), "{order:?}");
    let finished = order.iter().position(|k| *k == "scanFinished");
    assert!(finished.is_some_and(|i| i + 1 < order.len()), "{order:?}");
    assert!(seen.iter().all(|e| e.get("job").is_none_or(|j| j == job.0)));
    let changed = seen.last().ok_or("no change")?;
    assert_eq!(changed["source"], source.id.0);
    assert!(changed["added"].as_array().is_some_and(|a| a.len() >= 3));

    // The list shows the real families.
    let page: Paged<FamilySummary> = call(&webview, "list_families", json!({ "req": {} }))?;
    let names: Vec<&str> = page.items.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Asap Sharp", "Barlow", "Bebas Neue"]);
    let sources: Vec<Source> = call(&webview, "list_sources", json!({}))?;
    assert!(sources[0].face_count >= 3);

    // Search and filters.
    let found: SearchResponse = call(&webview, "search", json!({ "req": { "query": "bebas" } }))?;
    assert_eq!(found.families[0].family.name, "Bebas Neue");
    let variable: SearchResponse = call(
        &webview,
        "search",
        json!({ "req": { "filters": { "variable": true } } }),
    )?;
    assert_eq!(variable.families.len(), 1);
    assert_eq!(variable.families[0].family.name, "Asap Sharp");

    // Coverage: the `supports` filter and the per-face sample text.
    let snowman: SearchResponse = call(
        &webview,
        "search",
        json!({ "req": { "filters": { "supports": { "kind": "text", "text": "\u{2603}\u{2604}\u{2605}\u{2606}\u{2607}\u{2608}" } } } }),
    )?;
    assert_eq!(snowman.total, 0, "no fixture draws these symbols");
    let languages: Vec<Value> = call(&webview, "list_languages", json!({}))?;
    assert!(!languages.is_empty());
    let bebas = &found.families[0];
    let samples: Vec<Value> = call(
        &webview,
        "sample_texts",
        json!({ "req": { "faces": [bebas.faces[0]], "text": "Ab\u{4E2D}" } }),
    )?;
    assert!(
        samples[0]["missing"]
            .as_array()
            .is_some_and(|m| !m.is_empty()),
        "{samples:?}"
    );

    // The inspector Info tab.
    let family: Family = call(&webview, "get_family", json!({ "family": bebas.family.id }))?;
    let detail: FaceDetail = call(&webview, "get_face", json!({ "face": family.faces[0].id }))?;
    assert!(
        detail.path.ends_with("BebasNeue-Regular.ttf"),
        "{}",
        detail.path
    );
    assert!(detail.metrics.units_per_em > 0);
    assert_eq!(detail.face.family_name, "Bebas Neue");

    // Activate and deactivate: outcomes, and an event each.
    let done: Vec<ActivationOutcome> = call(
        &webview,
        "activate",
        json!({ "req": { "target": { "kind": "family", "family": bebas.family.id } } }),
    )?;
    assert!(
        done.iter().all(|o| o.outcome == OutcomeStatus::Done),
        "{done:?}"
    );
    let activated = events_until(&events, "activationChanged")?;
    assert!(
        activated
            .last()
            .is_some_and(|e| e["activated"].as_array().is_some_and(|a| !a.is_empty()))
    );
    let active: Vec<ActiveFace> = call(&webview, "list_active", json!({}))?;
    assert_eq!(active.len(), done.len());
    let _: Vec<ActivationOutcome> = call(&webview, "deactivate", json!({ "req": {} }))?;
    events_until(&events, "activationChanged")?;
    let active: Vec<ActiveFace> = call(&webview, "list_active", json!({}))?;
    assert!(active.is_empty());
    Ok(())
}
