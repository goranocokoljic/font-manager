//! The MCP tools over the real core (E27.2): search_fonts, get_font and
//! check_coverage on a library scanned from `fixtures/fonts/`; export_web
//! writes real web fonts and answers with their paths and the CSS only; and
//! deactivate_fonts only reaches faces the app registered (rule 3). The
//! resources (E27.5) read a collection's families and a set's faces from the
//! core. The workflow tools (E27.4) shortlist, prepare and write a specimen
//! on the core, and the app's gate refuses them until they are granted.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use fm_api::activation::{ActivateRequest, ActivationMode, ConflictPolicy, Target};
use fm_api::library::{AddSourceRequest, ImportMode, ScanRequest};
use fm_api::organization::{CollectionFacesRequest, CreateCollectionRequest, CreateSetRequest};
use fm_api::{
    ActivationService as _, Api, Event, EventSource as _, FaceId, LibraryService as _,
    OrganizationService as _, SearchService as _,
};
use fm_core::{CoreApi, startup};
use fm_mcp::{AllowAll, McpServer};
use fm_platform::{AppPaths, MockActivator};
use rmcp::model::ResourceContents;
use serde_json::{Value, json};

use common::{Client, assert_no_font_bytes, connect, png_size};

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
async fn preview_tools_render_real_pngs_on_the_core() {
    let home = Home::new("preview");
    let api = scanned_core(&home, &["derived/agdasima", "google-repo/barlow"]);
    let client = client_over(&api).await;
    let barlow = id(&client
        .ok("search_fonts", json!({ "query": "barlow" }))
        .await["families"][0]["faces"][0]);
    let agdasima = id(&client
        .ok("search_fonts", json!({ "query": "agdasima" }))
        .await["families"][0]["faces"][0]);
    let args = |face: u32| json!({ "face_id": face, "text": "Hamburg", "size": 40 });

    let (one, caption) = client.png("render_preview", args(barlow)).await;
    let (w1, h1) = png_size(&one);
    assert!(w1 > 40 && h1 > 20, "{w1}x{h1}");
    assert!(caption.contains("Barlow"), "{caption}");
    let (two, _) = client.png("render_preview", args(agdasima)).await;
    let (w2, h2) = png_size(&two);
    let (again, _) = client.png("render_preview", args(barlow)).await;
    assert_eq!(again, one, "the same request, the same (cached) PNG");

    let (both, caption) = client
        .png(
            "compare_preview",
            json!({ "face_ids": [agdasima, barlow], "text": "Hamburg", "size": 40 }),
        )
        .await;
    assert_eq!(png_size(&both), (w1.max(w2), h1 + h2), "one row per face");
    let rows = (
        caption.find("Agdasima").expect("agdasima"),
        caption.find("Barlow").expect("barlow"),
    );
    assert!(rows.0 < rows.1, "top to bottom in request order: {caption}");
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

#[tokio::test(flavor = "multi_thread")]
async fn collection_and_set_resources_read_the_core() {
    let home = Home::new("resources");
    let api = scanned_core(&home, &["derived/agdasima", "google-repo/barlow"]);
    let client = client_over(&api).await;
    let face_of = |name: &str| {
        let found = api
            .search(fm_api::search::SearchRequest {
                query: name.to_owned(),
                ..Default::default()
            })
            .expect("search");
        found.families[0].faces[0]
    };
    let barlow = face_of("barlow");
    let collection = api
        .create_collection(CreateCollectionRequest {
            name: "Picks".into(),
            parent: None,
            rules: None,
            rule_group: None,
        })
        .expect("collection");
    api.add_to_collection(CollectionFacesRequest {
        collection: collection.id,
        faces: vec![barlow],
        families: vec![],
    })
    .expect("added");
    let set = api
        .create_set(CreateSetRequest {
            name: "Brand".into(),
            faces: vec![barlow, face_of("agdasima")],
        })
        .expect("set");

    let read = async |uri: String| -> Value {
        let result = client.read_resource(&uri).await.expect("read");
        let ResourceContents::TextResourceContents { text, .. } = &result.contents[0] else {
            panic!("{uri}: text");
        };
        let value: Value = serde_json::from_str(text).expect("JSON");
        assert_no_font_bytes(&value);
        value
    };
    let held = read(format!("collection://{}", collection.id.0)).await;
    let names: Vec<&str> = held["families"]
        .as_array()
        .expect("families")
        .iter()
        .map(|f| f["name"].as_str().expect("name"))
        .collect();
    assert_eq!(names, ["Barlow"], "only the family in the collection");
    let brand = read(format!("set://{}", set.id.0)).await;
    let rows = brand["faces"].as_array().expect("faces");
    let row_ids: Vec<&Value> = rows.iter().map(|f| &f["id"]).collect();
    let set_ids: Vec<&Value> = brand["set"]["faces"]
        .as_array()
        .expect("ids")
        .iter()
        .collect();
    assert_eq!(row_ids, set_ids, "one row per face, in set order");
    let mut families: Vec<&str> = rows
        .iter()
        .map(|f| f["familyName"].as_str().expect("family"))
        .collect();
    families.sort_unstable();
    assert_eq!(families, ["Agdasima", "Barlow"]);
    let face = read(format!("font://{}", barlow.0)).await;
    assert_eq!(face["face"]["id"], json!(barlow.0));
    client.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn workflow_tools_run_on_the_core_and_are_refused_until_granted() {
    let home = Home::new("workflows");
    let api = scanned_core(&home, &["google-repo/barlow", "google-static/glory"]);
    let web = home.0.join("web");
    let specimens = home.0.join("Specimens ü");
    std::fs::create_dir_all(&web).expect("web");
    std::fs::create_dir_all(&specimens).expect("specimens");

    // The app's own gate, no grants given: the read-only shortlist runs.
    let dyn_api: Arc<dyn Api> = api.clone();
    let gated = connect(McpServer::new(dyn_api)).await;
    let picked = gated
        .ok(
            "pick_fonts_for_brief",
            json!({ "brief": "glory", "license": "web", "limit": 5 }),
        )
        .await;
    let top = &picked["candidates"][0];
    assert_eq!(top["family"]["name"], "Glory", "{picked}");
    let reasons: Vec<&str> = top["reasons"]
        .as_array()
        .expect("reasons")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(reasons[0], "No languages asked");
    assert!(reasons[1].starts_with("Allows web use"), "{reasons:?}");
    let glory = id(&top["family"]["id"]);
    let prepare = json!({
        "family_id": glory,
        "out_dir": web.display().to_string(),
        "subset_presets": ["latin"],
    });
    let specimen = json!({ "family_id": glory, "out_dir": specimens.display().to_string() });
    for (tool, args) in [
        ("prepare_for_web", prepare.clone()),
        ("build_specimen", specimen.clone()),
    ] {
        let e = gated.err(tool, args).await;
        assert_eq!(e.code, fm_api::ErrorCode::PermissionDenied, "{tool}");
    }
    assert!(api.list_active().expect("active").is_empty());
    assert_eq!(std::fs::read_dir(&web).expect("web").count(), 0);
    assert_eq!(std::fs::read_dir(&specimens).expect("specimens").count(), 0);
    gated.close().await;

    // Granted (here: every grant): the family is activated and exported.
    let client = client_over(&api).await;
    let prepared = client.ok("prepare_for_web", prepare).await;
    assert_no_font_bytes(&prepared);
    let faces = api
        .get_family(fm_api::FamilyId(glory))
        .expect("glory")
        .faces;
    assert_eq!(
        prepared["activated"].as_array().expect("activated").len(),
        faces.len()
    );
    assert_eq!(api.list_active().expect("active").len(), faces.len());
    let files: Vec<PathBuf> = prepared["export"]["files"]
        .as_array()
        .expect("files")
        .iter()
        .filter_map(Value::as_str)
        .map(PathBuf::from)
        .collect();
    assert_eq!(files.len(), faces.len() + 1, "{prepared}");
    for f in &files {
        assert!(f.exists(), "{}", f.display());
        assert!(
            f.extension().is_some_and(|e| e == "woff2") || f.ends_with("fonts.css"),
            "{}",
            f.display()
        );
    }

    // The specimen is written into the folder given, and its path returned.
    let written = client.ok("build_specimen", specimen.clone()).await;
    let path = PathBuf::from(written["path"].as_str().expect("path"));
    assert_eq!(path, specimens.join(fm_mcp::tools::specimen_file(glory)));
    let page = std::fs::read_to_string(&path).expect("the specimen");
    assert!(page.contains("Glory"));
    let e = client.err("build_specimen", specimen.clone()).await;
    assert_eq!(
        e.code,
        fm_api::ErrorCode::Conflict,
        "an existing file is kept"
    );
    let mut again = specimen;
    again["overwrite"] = json!(true);
    client.ok("build_specimen", again).await;
    client.close().await;
}
