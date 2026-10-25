//! The built `fm` binary against a scratch home (`FM_HOME`): the index is
//! made and scanned through `fm-core` here, then `fm` reads it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use fm_api::library::{AddSourceRequest, ImportMode, ScanRequest};
use fm_api::search::{FamilyHit, Filters, SearchRequest, SimilarFace, Supports};
use fm_api::{API_VERSION, Event, EventSource, LibraryService, SearchService};
use fm_core::{CoreApi, startup};
use fm_platform::{AppPaths, MockActivator};
use serde_json::Value;

/// A folder under the system temp folder, removed on drop.
struct Home(PathBuf);

impl Home {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("fm-cli-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp home");
        Self(dir)
    }

    /// A home whose library holds copies of `folders` (under
    /// `fixtures/fonts/`), added as a source and scanned.
    fn scanned(label: &str, folders: &[&str]) -> Self {
        let home = Self::new(label);
        let library = home.library(folders);
        let paths = AppPaths::under(&home.0);
        startup::prepare(&paths).expect("app folders");
        let index = startup::open_index(&paths).expect("index").index;
        let api = CoreApi::open(&paths, index, Arc::new(MockActivator::new()));
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
        home
    }

    /// A `Library` folder holding copies of `folders` (under `fixtures/fonts/`).
    fn library(&self, folders: &[&str]) -> PathBuf {
        let library = self.0.join("Library");
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts");
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
        library
    }

    fn fm(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_fm"))
            .args(args)
            .env("FM_HOME", &self.0)
            .output()
            .expect("fm runs")
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn code(out: &Output) -> i32 {
    out.status.code().expect("an exit code")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn version_runs_on_a_fresh_index_with_and_without_json() {
    let home = Home::new("version");
    let out = home.fm(&["version"]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains(&format!("API {API_VERSION}")));

    let out = home.fm(&["version", "--json"]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    assert!(out.stderr.is_empty());
    let v: Value = serde_json::from_slice(&out.stdout).expect("one JSON value");
    assert_eq!(v["apiVersion"]["major"], API_VERSION.major);
    assert_eq!(v["apiVersion"]["minor"], API_VERSION.minor);
    // The index was made in the scratch home, not the user's app folders.
    assert!(home.0.join("data").is_dir());
}

#[test]
fn the_help_lists_the_exit_codes() {
    let home = Home::new("help");
    let out = home.fm(&["--help"]);
    assert_eq!(code(&out), 0);
    let help = text(&out.stdout);
    assert!(help.contains(fm_cli::EXIT_CODES), "{help}");
}

#[test]
fn a_usage_error_exits_2() {
    let home = Home::new("usage");
    let out = home.fm(&["no-such-command"]);
    assert_eq!(code(&out), 2);
    assert!(out.stdout.is_empty());

    let out = home.fm(&["--json", "no-such-command"]);
    assert_eq!(code(&out), 2);
    assert!(out.stdout.is_empty());
    let e: Value = serde_json::from_slice(&out.stderr).expect("JSON error");
    assert_eq!(e["code"], "invalidRequest");
}

#[test]
fn a_font_argument_is_resolved_in_the_scanned_index() {
    let home = Home::scanned("resolve", &["derived/agdasima", "google-repo/barlow"]);

    let out = home.fm(&["--json", "resolve", "Barlow Bold Italic"]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    let v: Value = serde_json::from_slice(&out.stdout).expect("JSON");
    assert_eq!(v["postscriptName"], "Barlow-BoldItalic");
    let id = v["face"].as_u64().expect("face id").to_string();

    let out = home.fm(&["resolve", &id]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("Barlow-BoldItalic"));

    // Agdasima.ttc holds two faces of the family: ambiguous, exit 2.
    let out = home.fm(&["resolve", "agdasima"]);
    assert_eq!(code(&out), 2);
    assert!(out.stdout.is_empty());
    let err = text(&out.stderr);
    assert!(err.contains("matches 2 fonts"), "{err}");
    assert!(
        err.contains("Agdasima-Bold") && err.contains("Agdasima-Regular"),
        "{err}"
    );

    let out = home.fm(&["--json", "resolve", "Inter"]);
    assert_eq!(code(&out), 3);
    assert!(out.stdout.is_empty());
    let e: Value = serde_json::from_slice(&out.stderr).expect("JSON error");
    assert_eq!(e["code"], "notFound");
}

fn json(out: &Output) -> Value {
    assert!(out.stderr.is_empty(), "{}", text(&out.stderr));
    serde_json::from_slice(&out.stdout).expect("one JSON value")
}

#[test]
fn scan_adds_the_folder_and_list_shows_what_it_found() {
    let home = Home::new("scan");
    let library = home.library(&["derived/agdasima", "google-repo/barlow"]);
    let folder = library.display().to_string();

    let out = home.fm(&["scan", &folder]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    let progress = text(&out.stderr);
    assert!(
        progress.contains(&format!("Scanning {folder}: ")),
        "{progress}"
    );
    let printed = text(&out.stdout);
    assert!(
        printed.contains("ADDED") && printed.contains(&folder),
        "{printed}"
    );

    // The faces are in the index when `fm scan` exits.
    let out = home.fm(&["--json", "list"]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    let v = json(&out);
    let names: Vec<&str> = v
        .as_array()
        .expect("families")
        .iter()
        .map(|f| f["name"].as_str().expect("name"))
        .collect();
    // The folder holds a truncated copy too: listed, as broken.
    assert_eq!(names, ["Agdasima", "agdasima-truncated", "Barlow"]);

    let out = home.fm(&["--json", "list", "--family", "Agdasima"]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    assert_eq!(json(&out).as_array().expect("faces").len(), 2);

    // Scanned again: the folder is a source already, nothing is new.
    let out = home.fm(&["--json", "scan", &folder]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    let v = json(&out);
    assert_eq!(v[0]["source"]["path"], folder.as_str());
    assert_eq!(v[0]["added"], 0);
    // Every source, with no folder given.
    let out = home.fm(&["--json", "scan"]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    assert_eq!(json(&out).as_array().expect("scans").len(), 1);
}

#[test]
fn scan_of_a_missing_folder_exits_3_and_an_empty_one_finds_nothing() {
    let home = Home::new("scan-missing");
    let gone = home.0.join("No such folder").display().to_string();
    let out = home.fm(&["--json", "scan", &gone]);
    assert_eq!(code(&out), 3);
    assert!(out.stdout.is_empty());
    let e: Value = serde_json::from_slice(&out.stderr).expect("JSON error");
    assert_eq!(e["code"], "notFound");

    let empty = home.0.join("Empty");
    std::fs::create_dir_all(&empty).expect("folder");
    let out = home.fm(&["--json", "scan", &empty.display().to_string()]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    assert_eq!(json(&out)[0]["added"], 0);
}

#[test]
fn search_prints_the_hits_of_the_core_search() {
    let home = Home::scanned("search", &["derived/agdasima", "google-repo/barlow"]);
    let out = home.fm(&["--json", "search", "barlow", "--supports", "sr-Latn"]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    let printed: Vec<FamilyHit> = serde_json::from_value(json(&out)).expect("hits");

    let paths = AppPaths::under(&home.0);
    let index = startup::open_index(&paths).expect("index").index;
    let api = CoreApi::open(&paths, index, Arc::new(MockActivator::new()));
    let direct = api
        .search(SearchRequest {
            query: "barlow".into(),
            filters: Filters {
                supports: Some(Supports::Languages {
                    codes: vec!["sr-Latn".into()],
                }),
                ..Filters::default()
            },
            ..SearchRequest::default()
        })
        .expect("search");
    assert_eq!(printed, direct.families);
    assert_eq!(printed.len(), 1);
    assert_eq!(printed[0].family.name, "Barlow");
    assert!(printed[0].missing.is_some(), "with --supports");
}

#[test]
fn info_coverage_and_similar_run_on_the_core() {
    let home = Home::scanned("info", &["derived/agdasima", "google-repo/barlow"]);

    let out = home.fm(&["info", "Barlow-BoldItalic"]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    let info = text(&out.stdout);
    for label in [
        "PostScript name   Barlow-BoldItalic",
        "Family            Barlow",
        "Embedding ",
    ] {
        assert!(info.contains(label), "{label:?} in {info}");
    }
    let out = home.fm(&["--json", "info", "Barlow-BoldItalic"]);
    assert_eq!(code(&out), 0);
    let v = json(&out);
    assert_eq!(v["face"]["postscriptName"], "Barlow-BoldItalic");
    assert!(v["features"].is_array() && v["scripts"].is_array());

    let out = home.fm(&["coverage", "Barlow-BoldItalic", "--text", "Barlow"]);
    assert_eq!(code(&out), 0, "{}", text(&out.stdout));
    let out = home.fm(&[
        "--json",
        "coverage",
        "Barlow-BoldItalic",
        "--text",
        "a\u{6F22}",
    ]);
    assert_eq!(code(&out), 5);
    let v = json(&out);
    assert_eq!(v["complete"], false);
    assert_eq!(v["missing"], serde_json::json!(["\u{6F22}"]));

    // E23.2 is in: the core ranks faces of the other families.
    let out = home.fm(&["--json", "similar", "Barlow-BoldItalic", "--limit", "2"]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    let similar: Vec<SimilarFace> = serde_json::from_value(json(&out)).expect("similar");
    assert!(!similar.is_empty() && similar.len() <= 2, "{similar:?}");
    assert!(
        similar.iter().all(|s| s.face.family_name != "Barlow"),
        "{similar:?}"
    );

    let out = home.fm(&["similar", "No Such Font"]);
    assert_eq!(code(&out), 3);
    assert!(out.stdout.is_empty());
}
