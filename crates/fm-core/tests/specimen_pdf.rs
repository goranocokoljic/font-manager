//! E21.2: a fixture family's specimen printed to PDF by the OS webview
//! (ADR 0013), on every CI runner (Linux under `xvfb-run`).
//!
//! AppKit needs the process's main thread, which libtest never gives a
//! test, so this file has its own `main` (`harness = false`): the main
//! thread runs the UI loop ([`fm_platform::pdf::run_ui_loop`]) and the test
//! runs on a worker, the way the app calls the API. It answers the libtest
//! arguments nextest uses (`--list --format terse`, `--exact <name>`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // a test binary
#![allow(missing_debug_implementations)]

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use fm_api::Event;
use fm_api::common::Page;
use fm_api::export::{CollectionSpecimenRequest, SpecimenFormat, SpecimenRequest};
use fm_api::library::{AddSourceRequest, ImportMode, ListFamiliesRequest, ScanRequest};
use fm_api::organization::{CollectionFacesRequest, CreateCollectionRequest};
use fm_api::{
    EventSource as _, ExportService as _, FamilyId, LibraryService as _, OrganizationService as _,
};
use fm_core::{CoreApi, Index, SettingsStore};
use fm_platform::{AppPaths, FontActivator, MockActivator};

/// The tests of this binary, by name.
const TESTS: [(&str, fn()); 2] = [
    (
        "a_fixture_family_prints_to_a_paginated_pdf_that_embeds_its_fonts",
        run,
    ),
    (
        "a_three_family_collection_prints_a_cover_and_a_page_per_family",
        run_collection,
    ),
];
/// The fixture family: two static faces, one TrueType and one CFF.
const FIXTURE: &str = "fontshare/public-sans";
/// The fixture collection (E21.3): three families of one static face each.
const COLLECTION_FIXTURES: [&str; 3] = [
    "google-repo/asul/Asul-Bold.ttf",
    "google-repo/basic/Basic-Regular.ttf",
    "fontshare/teko/Teko-SemiBold.ttf",
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    if has("--list") {
        if !has("--ignored") {
            for (name, _) in TESTS {
                println!("{name}: test");
            }
        }
        return ExitCode::SUCCESS;
    }
    let filters: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();
    let selected: Vec<(&str, fn())> = TESTS
        .into_iter()
        .filter(|(name, _)| {
            filters.is_empty()
                || filters.iter().any(|f| {
                    if has("--exact") {
                        *f == name
                    } else {
                        name.contains(f.as_str())
                    }
                })
        })
        .collect();
    if selected.is_empty() || has("--ignored") {
        println!("running 0 tests");
        return ExitCode::SUCCESS;
    }
    println!("running {} test(s)", selected.len());
    let passed = fm_platform::pdf::run_ui_loop(move || {
        let mut passed = true;
        for (name, test) in selected {
            let ok = std::panic::catch_unwind(test).is_ok();
            println!("test {name} ... {}", if ok { "ok" } else { "FAILED" });
            passed &= ok;
        }
        passed
    });
    if passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// A scratch folder, removed at the end.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An API over a fresh library in `scratch`, printing through the OS printer.
fn printing_api(scratch: &Scratch) -> CoreApi {
    let base = &scratch.0;
    let paths = AppPaths {
        data: base.join("data"),
        config: base.join("config"),
        cache: base.join("cache"),
        logs: base.join("logs"),
        library: base.join("Fonts"),
    };
    fm_core::startup::prepare(&paths).expect("app folders");
    let index = Index::open(&paths.data.join(fm_core::index::INDEX_FILE))
        .expect("index")
        .index;
    let printer = fm_platform::pdf::system(&paths).expect("the OS printer");
    CoreApi::new(
        index,
        Arc::new(MockActivator::new()) as Arc<dyn FontActivator>,
        SettingsStore::new(&paths),
        None,
    )
    .with_pdf_printer(printer)
}

/// Copies `files` into a library folder in `scratch` and scans it.
fn scan_library(api: &CoreApi, scratch: &Scratch, files: &[PathBuf]) {
    let library = scratch.0.join("Library");
    std::fs::create_dir_all(&library).expect("library");
    for path in files {
        std::fs::copy(path, library.join(path.file_name().expect("name"))).expect("copy");
    }
    let source = api
        .add_source(AddSourceRequest {
            path: library.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    scan(api, source.id);
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts")
}

fn run() {
    let scratch = Scratch(std::env::temp_dir().join(format!("fm-pdf-it-{}", std::process::id())));
    let base = &scratch.0;
    let api = printing_api(&scratch);

    // The fixture family, copied into a library folder and scanned.
    let files: Vec<PathBuf> = std::fs::read_dir(fixtures().join(FIXTURE))
        .expect("fixture")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "ttf" || e == "otf"))
        .collect();
    scan_library(&api, &scratch, &files);
    let families = api
        .list_families(ListFamiliesRequest {
            page: Page {
                offset: 0,
                limit: 100,
            },
            ..ListFamiliesRequest::default()
        })
        .expect("families")
        .items;
    let family = families
        .iter()
        .max_by_key(|f| f.style_count)
        .expect("a family");
    let faces = api.get_family(family.id).expect("family").faces;
    assert!(
        faces.len() >= 2,
        "the fixture family has two faces: {faces:?}"
    );

    let out = base
        .join("Specimens ü")
        .join(format!("{}.pdf", family.name));
    std::fs::create_dir_all(out.parent().expect("folder")).expect("out folder");
    let started = Instant::now();
    let res = api
        .export_specimen(SpecimenRequest {
            family: family.id,
            format: SpecimenFormat::Pdf,
            out_path: out.display().to_string(),
        })
        .unwrap_or_else(|e| panic!("the PDF export failed: {e:?}"));
    println!("printed {} in {:?}", family.name, started.elapsed());
    assert_eq!(Path::new(&res.path), out);

    let pdf = std::fs::read(&out).expect("the PDF");
    assert!(pdf.starts_with(b"%PDF"), "the file starts with %PDF");
    let text = pdf_text(&pdf);
    let pages = count_pages(&text);
    assert!(pages >= 1, "at least one page, found {pages}");
    let (w, h) = media_box(&text).expect("a MediaBox");
    assert!(
        (w - 595.3).abs() < 2.0 && (h - 841.9).abs() < 2.0,
        "A4 pages, found {w} × {h} pt"
    );
    println!(
        "{pages} page(s) of {w} × {h} pt, {} bytes, {} face(s)",
        pdf.len(),
        faces.len()
    );
    assert!(
        text.contains("/FontFile"),
        "a FontFile entry embeds a font program"
    );
    for face in &faces {
        assert!(
            names_font(&text, &face.postscript_name),
            "the PDF embeds {} ({})",
            face.postscript_name,
            face.style
        );
    }
    // Only the PDF is left next to it: no partial file.
    let left: Vec<_> = std::fs::read_dir(out.parent().expect("folder"))
        .expect("out folder")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(left.len(), 1, "{left:?}");
}

/// E21.3: a collection of three fixture families prints to a cover and one
/// page per family, the page break before each family doing the work.
fn run_collection() {
    let scratch =
        Scratch(std::env::temp_dir().join(format!("fm-pdf-it-collection-{}", std::process::id())));
    let base = &scratch.0;
    let api = printing_api(&scratch);
    let files: Vec<PathBuf> = COLLECTION_FIXTURES
        .iter()
        .map(|f| fixtures().join(f))
        .collect();
    scan_library(&api, &scratch, &files);
    let families: Vec<FamilyId> = api
        .list_families(ListFamiliesRequest {
            page: Page {
                offset: 0,
                limit: 100,
            },
            ..ListFamiliesRequest::default()
        })
        .expect("families")
        .items
        .into_iter()
        .map(|f| f.id)
        .collect();
    assert_eq!(families.len(), 3, "three fixture families");
    let collection = api
        .create_collection(CreateCollectionRequest {
            name: "Client sign-off".into(),
            parent: None,
            rules: None,
            rule_group: None,
        })
        .expect("collection");
    api.add_to_collection(CollectionFacesRequest {
        collection: collection.id,
        faces: Vec::new(),
        families,
    })
    .expect("added");

    let out = base.join("Specimens ü").join("Client sign-off.pdf");
    std::fs::create_dir_all(out.parent().expect("folder")).expect("out folder");
    let started = Instant::now();
    let res = api
        .export_collection_specimen(CollectionSpecimenRequest {
            collection: collection.id,
            format: SpecimenFormat::Pdf,
            out_path: out.display().to_string(),
        })
        .unwrap_or_else(|e| panic!("the PDF export failed: {e:?}"));
    println!("printed the collection in {:?}", started.elapsed());
    assert_eq!(Path::new(&res.path), out);
    let pdf = std::fs::read(&out).expect("the PDF");
    assert!(pdf.starts_with(b"%PDF"), "the file starts with %PDF");
    let text = pdf_text(&pdf);
    assert_eq!(
        count_pages(&text),
        4,
        "a cover and one page per family for three families"
    );
    let (w, h) = media_box(&text).expect("a MediaBox");
    assert!(
        (w - 595.3).abs() < 2.0 && (h - 841.9).abs() < 2.0,
        "A4 pages, found {w} × {h} pt"
    );
}

/// Scans `source` and waits for its `LibraryChanged`.
fn scan(api: &CoreApi, source: fm_api::SourceId) {
    let rx = api.subscribe();
    let job = api
        .scan(ScanRequest {
            source: Some(source),
        })
        .expect("scan");
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(Event::LibraryChanged { .. }) => return,
            Ok(Event::JobFailed { job: j, .. }) if j == job => panic!("scan failed"),
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => panic!("scan {job} did not end"),
            Err(RecvTimeoutError::Disconnected) => panic!("event stream closed"),
        }
    }
}

/// The PDF's bytes as text, with every Flate stream inflated next to it, so
/// dictionaries packed into object streams are searched too.
fn pdf_text(pdf: &[u8]) -> String {
    let mut text = String::from_utf8_lossy(pdf).into_owned();
    let mut rest = pdf;
    while let Some(at) = find(rest, b"stream") {
        let mut body = &rest[at + b"stream".len()..];
        body = body.strip_prefix(b"\r").unwrap_or(body);
        body = body.strip_prefix(b"\n").unwrap_or(body);
        let end = find(body, b"endstream").unwrap_or(body.len());
        let mut inflated = Vec::new();
        if flate2::read::ZlibDecoder::new(&body[..end])
            .read_to_end(&mut inflated)
            .is_ok()
        {
            text.push('\n');
            text.push_str(&String::from_utf8_lossy(&inflated));
        }
        rest = &body[end..];
        if let Some(stripped) = rest.strip_prefix(b"endstream") {
            rest = stripped;
        }
    }
    text
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// The `/Type /Page` objects (not `/Pages`).
fn count_pages(text: &str) -> usize {
    let mut n = 0;
    for (i, _) in text.match_indices("/Type") {
        let after = text[i + "/Type".len()..].trim_start();
        if let Some(rest) = after.strip_prefix("/Page")
            && !rest.starts_with(|c: char| c.is_ascii_alphanumeric())
        {
            n += 1;
        }
    }
    n
}

/// The width and height of the first `/MediaBox [x0 y0 x1 y1]`, in points.
fn media_box(text: &str) -> Option<(f64, f64)> {
    let at = text.find("/MediaBox")?;
    let rest = text[at + "/MediaBox".len()..]
        .trim_start()
        .strip_prefix('[')?;
    let nums: Vec<f64> = rest[..rest.find(']')?]
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    match nums[..] {
        [x0, y0, x1, y1] => Some((x1 - x0, y1 - y0)),
        _ => None,
    }
}

/// Whether a font name in the PDF is `postscript`, with or without a
/// subset tag (`/ABCDEF+DMSans-Regular`).
fn names_font(text: &str, postscript: &str) -> bool {
    text.match_indices(postscript).any(|(i, _)| {
        let before = &text[..i];
        let tagged = before.len() >= 8
            && before.ends_with('+')
            && before[before.len() - 8..before.len() - 1]
                .chars()
                .all(|c| c == '/' || c.is_ascii_uppercase());
        let after = text[i + postscript.len()..].chars().next();
        let ends = after.is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-');
        (tagged || before.ends_with('/')) && ends
    })
}
