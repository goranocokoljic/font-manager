//! The MCP tools over the real core (E27.2): search_fonts, get_font and
//! check_coverage on a library scanned from `fixtures/fonts/`; export_web
//! writes real web fonts and answers with their paths and the CSS only; and
//! deactivate_fonts only reaches faces the app registered (rule 3).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use fm_api::activation::{ActivateRequest, ActivationMode, ConflictPolicy, Target};
use fm_api::library::{AddSourceRequest, ImportMode, ScanRequest};
use fm_api::{ActivationService as _, Api, Event, EventSource as _, FaceId, LibraryService as _};
use fm_core::{CoreApi, startup};
use fm_mcp::{AllowAll, McpServer};
use fm_platform::{AppPaths, MockActivator};
use serde_json::{Value, json};

use common::{Client, assert_no_font_bytes, connect};

/// A scratch home under the temp folder, removed on drop.
struct Home(PathBuf);

impl Home {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("fm-mcp-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp home");
        Self(dir)
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The core over `home` with a library of copies of `folders` (under
/// `fixtures/fonts/`), scanned.
fn scanned_core(home: &Home, folders: &[&str]) -> Arc<CoreApi> {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts");
    let library = home.0.join("Library");
    for folder in folders {
        let to = library.join(folder.rsplit('/').next().unwrap_or(folder));
        std::fs::create_dir_all(&to).expect("folder");
        for entry in std::fs::read_dir(fixtures.join(folder)).expect("fixture folder") {
            let path = entry.expect("entry").path();
            if path.extension().is_some_and(|e| e != "txt") {
                std::fs::copy(&path, to.join(path.file_name().expect("name"))).expect("copy");
            }
        }
    }
    let paths = AppPaths::under(&home.0);
    startup::prepare(&paths).expect("app folders");
    let index = startup::open_index(&paths).expect("index").index;
    let api = Arc::new(CoreApi::open(&paths, index, Arc::new(MockActivator::new())));
    let source = api
        .add_source(AddSourceRequest {
            path: library.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    let events = api.subscribe();
    let job = api
        .scan(ScanRequest {
            source: Some(source.id),
        })
        .expect("scan");
    let deadline = Instant::now() + Duration::from_secs(60);
    let (mut finished, mut changed) = (false, false);
    while !(finished && changed) {
        let left = deadline.saturating_duration_since(Instant::now());
        match events.recv_timeout(left).expect("scan events") {
            Event::ScanFinished { job: j, .. } if j == job => finished = true,
            Event::LibraryChanged { .. } => changed = true,
            Event::JobFailed { job: j, error } if j == job => panic!("scan failed: {error:?}"),
            _ => {}
        }
    }
    api
}

async fn client_over(api: &Arc<CoreApi>) -> Client {
    let dyn_api: Arc<dyn Api> = api.clone();
    connect(McpServer::with_gate(dyn_api, Arc::new(AllowAll))).await
}

fn id(value: &Value) -> u32 {
    value
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or_else(|| panic!("an id: {value}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn search_get_font_and_coverage_run_on_the_core() {
    let home = Home::new("read");
    let api = scanned_core(&home, &["derived/agdasima", "google-repo/barlow"]);
    let client = client_over(&api).await;

    let found = client
        .ok(
            "search_fonts",
            json!({ "query": "barlow", "languages": ["sr-Latn"] }),
        )
        .await;
    assert_eq!(found["total"], 1, "{found}");
    let hit = &found["families"][0];
    assert_eq!(hit["family"]["name"], "Barlow");
    assert!(
        !hit["missing"].is_null(),
        "a supports filter reports what is missing"
    );
    let face = id(&hit["faces"][0]);
    assert_no_font_bytes(&found);

    let font = client.ok("get_font", json!({ "face_id": face })).await;
    let detail = api.get_face(FaceId(face)).expect("face");
    assert_eq!(
        font,
        serde_json::to_value(&detail).unwrap(),
        "the API's own JSON"
    );
    assert_eq!(font["face"]["familyName"], "Barlow");
    assert_no_font_bytes(&font);

    let latin = client
        .ok(
            "check_coverage",
            json!({ "face_id": face, "text": "Barlow" }),
        )
        .await;
    assert_eq!(latin["complete"], true, "{latin}");
    let greek = client
        .ok(
            "check_coverage",
            json!({ "face_id": face, "text": "Ωμέγα" }),
        )
        .await;
    assert_eq!(greek["complete"], false, "Barlow has no Greek: {greek}");
    assert!(!greek["missing"].as_array().expect("missing").is_empty());
    let polish = client
        .ok(
            "check_coverage",
            json!({ "face_id": face, "languages": ["pl"] }),
        )
        .await;
    assert_eq!(polish["languages"][0]["code"], "pl");
    client.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn export_web_over_the_core_returns_paths_and_css_only() {
    let home = Home::new("export");
    let api = scanned_core(&home, &["google-repo/barlow"]);
    let client = client_over(&api).await;
    let found = client
        .ok("search_fonts", json!({ "query": "barlow" }))
        .await;
    let face = id(&found["families"][0]["faces"][0]);
    let out = home.0.join("web");
    std::fs::create_dir_all(&out).expect("the folder the user chose");

    let exported = client
        .ok(
            "export_web",
            json!({ "face_ids": [face], "out_dir": out.display().to_string() }),
        )
        .await;
    let keys: Vec<&String> = exported.as_object().expect("object").keys().collect();
    assert_eq!(keys, ["css", "files"], "paths and CSS only");
    assert_no_font_bytes(&exported);
    let text = exported.to_string();
    let woff2: Vec<PathBuf> = exported["files"]
        .as_array()
        .expect("files")
        .iter()
        .filter_map(Value::as_str)
        .map(PathBuf::from)
        .filter(|p| p.extension().is_some_and(|e| e == "woff2"))
        .collect();
    assert!(!woff2.is_empty(), "{exported}");
    for path in &woff2 {
        let bytes = std::fs::read(path).expect("the export wrote the file");
        assert!(bytes.starts_with(b"wOF2"), "{}", path.display());
        // No run of the file's bytes is in the answer, raw or lossy.
        let head = String::from_utf8_lossy(&bytes[..64]);
        assert!(!text.contains(head.as_ref()));
    }
    assert!(
        exported["css"]
            .as_str()
            .is_some_and(|c| c.contains("@font-face"))
    );
    client.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn deactivate_fonts_only_reaches_faces_the_app_registered_on_the_core() {
    let home = Home::new("deactivate");
    let api = scanned_core(&home, &["derived/agdasima", "google-repo/barlow"]);
    let client = client_over(&api).await;
    let barlow = id(&client
        .ok("search_fonts", json!({ "query": "barlow" }))
        .await["families"][0]["faces"][0]);
    let agdasima = id(&client
        .ok("search_fonts", json!({ "query": "agdasima" }))
        .await["families"][0]["faces"][0]);
    api.activate(ActivateRequest {
        target: Target::Faces {
            faces: vec![FaceId(barlow)],
        },
        mode: ActivationMode::Session,
        on_conflict: ConflictPolicy::KeepExisting,
    })
    .expect("activate");

    let answer = client
        .ok(
            "deactivate_fonts",
            json!({ "face_ids": [barlow, agdasima] }),
        )
        .await;
    let status = |face: u32| {
        answer["outcomes"]
            .as_array()
            .expect("outcomes")
            .iter()
            .find(|o| o["face"] == face)
            .map(|o| o["outcome"]["status"].clone())
            .unwrap_or_else(|| panic!("no outcome for {face}: {answer}"))
    };
    assert_eq!(status(barlow), "done");
    assert_eq!(status(agdasima), "skipped", "the app never registered it");
    assert!(api.list_active().expect("active").is_empty());
    client.close().await;
}
