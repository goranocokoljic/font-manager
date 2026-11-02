//! `CoreApi` over a copy of fixture fonts, one group of tests per service.

use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use fm_api::activation::{
    ActivationMode, ConflictPolicy, DocumentClosedRequest, NeededFont, NeededFontRule,
    OutcomeStatus,
};
use fm_api::common::{AxisValue, Embedding, FeatureSetting, FontFormat, Health, Page};
use fm_api::export::{Framework, FrameworkSnippetRequest, Subset, WebFormat};
use fm_api::library::{
    FamilyActivation, FamilySort, FontMatchKind, GlyphRequest, GlyphsRequest, LicenseType,
    OpenLicense, ProblemKind, TrashMoveStatus,
};
use fm_api::search::{Filters, Supports};
use fm_platform::{MockActivator, MockTrash};

use super::*;
use crate::catalog::google::GoogleFonts;
use crate::catalog::{CATALOGS_DIR, Fetcher};
use crate::index::INDEX_FILE;
use crate::test_support::TempDir;

/// How long a test waits for a scan's events.
const WAIT: Duration = Duration::from_secs(60);

/// Font folders copied into the test library: a static TTF, a variable TTF,
/// a TTC with a truncated copy, and two WOFF2 files.
const FOLDERS: &[&str] = &[
    "google-repo/bebasneue",
    "google-repo/asapsharp",
    "google-repo/barlow",
    "derived/agdasima",
    "fontshare/archivo",
];

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts")
}

/// A catalog fetcher that fails on any call and counts the calls.
#[derive(Debug, Default)]
struct OfflineFetcher {
    calls: std::sync::atomic::AtomicU32,
}

impl OfflineFetcher {
    fn calls(&self) -> u32 {
        self.calls.load(Ordering::Relaxed)
    }
}

impl Fetcher for OfflineFetcher {
    fn get(&self, url: &str) -> FmResult<Vec<u8>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Err(FmError::new(ErrorCode::Io, "offline").with_detail(url.to_owned()))
    }
}

/// A `CoreApi` over scratch app folders, a mock activator and a library
/// folder holding copies of [`FOLDERS`] (scans write sidecars into the
/// source, so the fixtures themselves are never a source).
struct Lib {
    dir: TempDir,
    api: CoreApi,
    activator: Arc<MockActivator>,
    /// The catalogs' fetcher: it fails on any call.
    fetcher: Arc<OfflineFetcher>,
    root: PathBuf,
    paths: AppPaths,
}

impl Lib {
    fn new(label: &str) -> Self {
        let dir = TempDir::new(label);
        let root = dir.path().join("Library");
        for folder in FOLDERS {
            let to = root.join(folder.rsplit('/').next().unwrap_or(folder));
            std::fs::create_dir_all(&to).expect("folder");
            for entry in std::fs::read_dir(fixtures().join(folder)).expect("fixture folder") {
                let path = entry.expect("entry").path();
                if crate::scanner::is_font_file(&path) {
                    std::fs::copy(&path, to.join(path.file_name().expect("name"))).expect("copy");
                }
            }
        }
        let base = dir.path();
        let paths = AppPaths {
            data: base.join("data"),
            config: base.join("config"),
            cache: base.join("cache"),
            logs: base.join("logs"),
            library: base.join("Fonts"),
        };
        crate::startup::prepare(&paths).expect("app folders");
        let index = Index::open(&paths.data.join(INDEX_FILE))
            .expect("index")
            .index;
        let activator = Arc::new(MockActivator::new());
        let previews = PreviewCache::open(&cache::cache_dir(&paths), cache::limit_from_mb(50))
            .expect("preview cache");
        let api = CoreApi::new(
            index,
            Arc::clone(&activator) as Arc<dyn FontActivator>,
            SettingsStore::new(&paths),
            Some(previews),
        )
        .with_layouts_in(paths.layouts_dir());
        let fetcher = Arc::new(OfflineFetcher::default());
        let api = api.with_catalogs(Catalogs::new(
            paths.data.join(CATALOGS_DIR),
            Arc::clone(&fetcher) as Arc<dyn Fetcher>,
            vec![Arc::new(GoogleFonts::with_base_url("http://127.0.0.1:9"))],
        ));
        Self {
            dir,
            api,
            activator,
            fetcher,
            root,
            paths,
        }
    }

    /// A library whose folder is added and scanned.
    fn scanned(label: &str) -> (Self, Source, Vec<Event>) {
        Self::scanned_with(label, &[])
    }

    /// As [`Lib::scanned`], with the fixture files `extra` (paths under
    /// `fixtures/fonts/`) copied into the library folder too.
    fn scanned_with(label: &str, extra: &[&str]) -> (Self, Source, Vec<Event>) {
        let lib = Self::new(label);
        let to = lib.root.join("extra");
        std::fs::create_dir_all(&to).expect("folder");
        for file in extra {
            let from = fixtures().join(file);
            std::fs::copy(&from, to.join(from.file_name().expect("name"))).expect("copy");
        }
        let source = lib
            .api
            .add_source(AddSourceRequest {
                path: lib.root.display().to_string(),
                mode: ImportMode::IndexInPlace,
            })
            .expect("source");
        let events = lib.scan(Some(source.id), true);
        (lib, source, events)
    }

    /// Scans and returns the events up to the scan's last one: its
    /// `LibraryChanged` when `changes`, else its `ScanFinished`.
    fn scan(&self, source: Option<SourceId>, changes: bool) -> Vec<Event> {
        let rx = self.api.subscribe();
        let job = self.api.scan(ScanRequest { source }).expect("scan");
        let mut events = Vec::new();
        let deadline = Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let event = match rx.recv_timeout(left) {
                Ok(e) => e,
                Err(RecvTimeoutError::Timeout) => panic!("scan {job} did not end: {events:?}"),
                Err(RecvTimeoutError::Disconnected) => panic!("event stream closed"),
            };
            let last = match &event {
                Event::JobFailed { job: j, .. } => *j == job,
                Event::ScanFinished { job: j, .. } => *j == job && !changes,
                Event::LibraryChanged { .. } => changes,
                _ => false,
            };
            events.push(event);
            if last {
                return events;
            }
        }
    }

    fn family(&self, name: &str) -> FamilySummary {
        self.api
            .list_families(ListFamiliesRequest {
                page: Page {
                    offset: 0,
                    limit: 1000,
                },
                ..ListFamiliesRequest::default()
            })
            .expect("families")
            .items
            .into_iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("no family {name}"))
    }

    fn names(&self) -> Vec<String> {
        self.api
            .list_families(ListFamiliesRequest::default())
            .expect("families")
            .items
            .into_iter()
            .map(|f| f.name)
            .collect()
    }
}

fn is_unsupported<T: std::fmt::Debug>(result: ApiResult<T>) {
    let e = result.expect_err("not built yet");
    assert_eq!(e.code, ErrorCode::Unsupported, "{e:?}");
    assert!(
        e.message.ends_with("is not available in this version yet."),
        "{e:?}"
    );
}

// --- library -------------------------------------------------------------

#[test]
fn a_scan_reports_progress_then_the_library_change() {
    let (lib, source, events) = Lib::scanned("api-scan");
    let Some(Event::ScanStarted { job, source: from }) = events.first().cloned() else {
        panic!("first event: {events:?}");
    };
    assert_eq!(from, Some(source.id));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::ScanProgress { job: j, done, total } if *j == job && done == total && *total > 0)),
        "{events:?}"
    );
    let finished = events
        .iter()
        .position(
            |e| matches!(e, Event::ScanFinished { job: j, added, .. } if *j == job && *added > 0),
        )
        .expect("finished");
    let Some(Event::LibraryChanged {
        source: changed,
        added,
        removed,
        ..
    }) = events.last()
    else {
        panic!("last event: {events:?}");
    };
    assert!(events.len() - 1 > finished, "the change follows the finish");
    assert_eq!(*changed, Some(source.id));
    assert!(added.len() >= 6, "{added:?}");
    assert!(removed.is_empty());
    let sources = lib.api.list_sources().expect("sources");
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].face_count as usize, added.len());
}

#[test]
fn the_faces_of_a_scan_are_in_the_index_when_it_finishes() {
    let lib = Lib::new("api-scan-written");
    let source = lib
        .api
        .add_source(AddSourceRequest {
            path: lib.root.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    let rx = lib.api.subscribe();
    let job = lib
        .api
        .scan(ScanRequest {
            source: Some(source.id),
        })
        .expect("scan");
    // Stop at the finish: `fm scan` exits there, before `LibraryChanged`.
    let added = loop {
        match rx.recv_timeout(WAIT).expect("scan events") {
            Event::ScanFinished { job: j, added, .. } if j == job => break added,
            Event::JobFailed { job: j, error } if j == job => panic!("{error:?}"),
            _ => {}
        }
    };
    assert!(added >= 6, "{added}");
    let sources = lib.api.list_sources().expect("sources");
    assert_eq!(sources[0].face_count, added, "written before the finish");
}

/// Whether the API still holds the files its last scan read.
fn holds_scan_files(lib: &Lib) -> bool {
    lock(&lib.api.files).is_some()
}

#[test]
fn a_finished_scan_lets_the_files_it_read_go() {
    let (lib, source, events) = Lib::scanned("api-scan-release");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::ScanFinished { added, .. } if *added >= 6)),
        "{events:?}"
    );
    assert!(!holds_scan_files(&lib), "released after the first scan");

    // The next scan loads them back from the index: nothing is read again,
    // nothing changes, and they are released again.
    let quiet = lib.scan(Some(source.id), false);
    let Some(Event::ScanFinished {
        added,
        changed,
        removed,
        ..
    }) = quiet.last()
    else {
        panic!("{quiet:?}");
    };
    assert_eq!((*added, *changed, *removed), (0, 0, 0), "{quiet:?}");
    assert!(!holds_scan_files(&lib), "released after the rescan");
    assert!(lib.names().iter().any(|n| n == "Bebas Neue"));
}

#[test]
fn a_scan_of_every_source_releases_the_files_after_its_last_job() {
    let lib = Lib::new("api-scan-release-all");
    let second = lib.root.join("second");
    std::fs::create_dir_all(&second).expect("folder");
    let from = fixtures().join("google-repo/bebasneue");
    for entry in std::fs::read_dir(&from).expect("fixtures") {
        let path = entry.expect("entry").path();
        if path.is_file() {
            std::fs::copy(&path, second.join(path.file_name().expect("name"))).expect("copy");
        }
    }
    for path in [lib.root.join("barlow"), second] {
        lib.api
            .add_source(AddSourceRequest {
                path: path.display().to_string(),
                mode: ImportMode::IndexInPlace,
            })
            .expect("source");
    }
    let rx = lib.api.subscribe();
    let first = lib.api.scan(ScanRequest { source: None }).expect("scan");
    let last = JobId(first.0 + 1);
    loop {
        match rx.recv_timeout(WAIT).expect("scan events") {
            Event::ScanFinished { job, .. } if job == last => break,
            Event::JobFailed { error, .. } => panic!("{error:?}"),
            _ => {}
        }
    }
    assert!(!holds_scan_files(&lib));
    assert!(lib.names().iter().any(|n| n == "Barlow"));
    assert!(lib.names().iter().any(|n| n == "Bebas Neue"));
}

#[test]
fn a_failed_scan_lets_the_files_it_loaded_go() {
    let (lib, source, _) = Lib::scanned("api-scan-release-failed");
    std::fs::remove_dir_all(&lib.root).expect("remove");
    let events = lib.scan(Some(source.id), false);
    assert!(
        matches!(events.last(), Some(Event::JobFailed { .. })),
        "{events:?}"
    );
    assert!(!holds_scan_files(&lib));
}

#[test]
fn the_list_shows_the_real_families_of_the_folder() {
    let (lib, source, _) = Lib::scanned("api-families");
    let names = lib.names();
    for name in ["Bebas Neue", "Asap Sharp", "Barlow", "Archivo"] {
        assert!(names.iter().any(|n| n == name), "{name} in {names:?}");
    }
    let mut sorted = names.clone();
    sorted.sort_by_key(|n| n.to_lowercase());
    assert_eq!(names, sorted, "name order by default");

    let asap = lib.family("Asap Sharp");
    assert!(asap.is_variable);
    assert_eq!(asap.formats, [FontFormat::Ttf]);
    assert!(asap.style_count > 1, "named instances count as styles");
    let archivo = lib.family("Archivo");
    assert_eq!(archivo.formats, [FontFormat::Woff2]);
    let range = archivo.weight_range.expect("weights");
    assert_eq!((range.min, range.max), (600, 700));
    assert_eq!(archivo.styles.len(), 2);
    assert!(
        archivo
            .location
            .as_deref()
            .is_some_and(|l| l.ends_with("archivo"))
    );
    assert_eq!(archivo.open_license, Some(OpenLicense::Ofl));

    let by_styles = lib
        .api
        .list_families(ListFamiliesRequest {
            sort: FamilySort::StyleCount,
            ..ListFamiliesRequest::default()
        })
        .expect("by styles");
    assert_eq!(by_styles.items[0].name, "Asap Sharp");
    let in_source = lib
        .api
        .list_families(ListFamiliesRequest {
            source: Some(source.id),
            page: Page {
                offset: 1,
                limit: 2,
            },
            ..ListFamiliesRequest::default()
        })
        .expect("page");
    assert_eq!(in_source.total as usize, names.len());
    assert_eq!(in_source.items.len(), 2);
    assert_eq!(in_source.items[0].name, names[1]);
    let other = lib
        .api
        .list_families(ListFamiliesRequest {
            source: Some(SourceId(source.id.0 + 1)),
            ..ListFamiliesRequest::default()
        })
        .expect("other source");
    assert_eq!(other.total, 0);
}

#[test]
fn families_filter_by_a_manual_or_smart_collection_and_an_unknown_one_is_not_found() {
    let (lib, _, _) = Lib::scanned("api-families-collection");
    let in_collection = |id| -> Vec<String> {
        let mut names: Vec<String> = lib
            .api
            .list_families(ListFamiliesRequest {
                collection: Some(id),
                page: Page {
                    offset: 0,
                    limit: 1000,
                },
                ..ListFamiliesRequest::default()
            })
            .expect("families")
            .items
            .into_iter()
            .map(|f| f.name)
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names
    };
    let manual = lib
        .api
        .create_collection(CreateCollectionRequest {
            name: "Picks".into(),
            parent: None,
            rules: None,
            rule_group: None,
        })
        .expect("manual");
    assert!(in_collection(manual.id).is_empty(), "an empty collection");
    // One face of each family is enough to list the family.
    let barlow = lib.family("Barlow");
    let archivo = lib.family("Archivo");
    lib.api
        .add_to_collection(CollectionFacesRequest {
            collection: manual.id,
            faces: vec![FaceId(barlow.id.0), FaceId(archivo.id.0)],
            families: Vec::new(),
        })
        .expect("added");
    assert_eq!(in_collection(manual.id), ["Archivo", "Barlow"]);

    let smart = lib
        .api
        .create_collection(CreateCollectionRequest {
            name: "Variable".into(),
            parent: None,
            rules: Some(Filters {
                variable: Some(true),
                ..Filters::default()
            }),
            rule_group: None,
        })
        .expect("smart");
    let all = lib
        .api
        .list_families(ListFamiliesRequest {
            page: Page {
                offset: 0,
                limit: 1000,
            },
            ..ListFamiliesRequest::default()
        })
        .expect("all");
    let mut variable: Vec<String> = all
        .items
        .into_iter()
        .filter(|f| f.is_variable)
        .map(|f| f.name)
        .collect();
    variable.sort_by_key(|n| n.to_lowercase());
    assert!(!variable.is_empty(), "the fixtures have a variable family");
    assert_eq!(in_collection(smart.id), variable);
    assert!(variable.len() < all.total as usize);

    let unknown = lib
        .api
        .list_families(ListFamiliesRequest {
            collection: Some(CollectionId(smart.id.0 + 100)),
            ..ListFamiliesRequest::default()
        })
        .expect_err("unknown collection");
    assert_eq!(unknown.code, ErrorCode::NotFound);
}

#[test]
fn a_family_lists_its_faces_and_the_inspector_reads_one() {
    let (lib, _, _) = Lib::scanned("api-family");
    let archivo = lib.family("Archivo");
    let family = lib.api.get_family(archivo.id).expect("family");
    assert_eq!(family.summary, archivo);
    let weights: Vec<u16> = family.faces.iter().map(|f| f.weight).collect();
    assert_eq!(weights, [600, 700]);
    assert!(family.faces.iter().all(|f| f.family == archivo.id));

    let detail = lib.api.get_face(family.faces[1].id).expect("detail");
    assert_eq!(detail.face, family.faces[1]);
    assert!(
        detail.path.ends_with("Archivo-Bold.woff2"),
        "{}",
        detail.path
    );
    assert_eq!(detail.hash.len(), 64);
    assert!(detail.file_size > 0);
    assert!(detail.metrics.units_per_em > 0, "{:?}", detail.metrics);
    assert!(detail.metrics.ascender > 0 && detail.metrics.descender < 0);
    assert!(detail.version.is_some());
    assert_eq!(detail.license.open_license, Some(OpenLicense::Ofl));
    assert_ne!(detail.license.embedding, Embedding::Restricted);
    assert!(detail.code_point_count.is_some_and(|n| n > 0));
    assert!(!detail.features.is_empty());
    // get_face names registered features and flags shaper and default ones (E10.3).
    let feature = |tag: &str| detail.features.iter().find(|f| f.tag == tag);
    let kern = feature("kern").expect("kern");
    assert_eq!(kern.name.as_deref(), Some("Kerning"));
    assert!(kern.default_on && !kern.automatic);
    for f in &detail.features {
        assert_eq!(f, &crate::features::feature_info(&f.tag), "{}", f.tag);
    }

    let asap = lib
        .api
        .get_face(FaceId(lib.family("Asap Sharp").id.0))
        .expect("variable");
    assert!(!asap.axes.is_empty());
    assert!(!asap.instances.is_empty());

    let face_id = family
        .faces
        .iter()
        .map(|f| f.id)
        .find(|f| f.0 != archivo.id.0)
        .expect("a second face");
    let err = lib
        .api
        .get_family(FamilyId(face_id.0))
        .expect_err("not a family id");
    assert_eq!(err.code, ErrorCode::NotFound);
    let err = lib.api.get_face(FaceId(99_999)).expect_err("unknown face");
    assert_eq!(err.code, ErrorCode::NotFound);
}

/// E34.1 criterion 1: the drag payload is the face's names and its file as a
/// `file://` URL, read from the index (the file is gone when it is asked for).
#[test]
fn font_switch_payload_comes_from_the_index() {
    let (lib, _, _) = Lib::scanned("api-font-switch");
    let archivo = lib
        .api
        .get_family(lib.family("Archivo").id)
        .expect("family");
    let bold = &archivo.faces[1];
    let detail = lib.api.get_face(bold.id).expect("detail");
    std::fs::remove_file(&detail.path).expect("remove the file");

    let p = lib.api.font_switch_payload(bold.id).expect("payload");
    assert_eq!(p.postscript_name, bold.postscript_name);
    assert_eq!(p.family, "Archivo");
    assert_eq!(p.style, bold.style);
    assert_eq!(
        p.file_url,
        crate::font_switch::file_url(Path::new(&detail.path))
    );
    assert!(p.file_url.starts_with("file://"), "{}", p.file_url);
    assert!(
        p.file_url.ends_with("/Archivo-Bold.woff2"),
        "{}",
        p.file_url
    );

    let err = lib
        .api
        .font_switch_payload(FaceId(99_999))
        .expect_err("unknown face");
    assert_eq!(err.code, ErrorCode::NotFound);
}

/// E10.4 criteria 1 and 2: `get_face` names stylistic sets and character
/// variants with the font's own names, read from the index: the file is gone
/// when `get_face` runs, so it cannot have been opened again.
#[test]
fn css_values_come_from_the_faces_own_axes_and_features() {
    let (lib, _, _) = Lib::scanned("api-css-values");
    let face = FaceId(lib.family("Asap Sharp").id.0);
    let detail = lib.api.get_face(face).expect("face");
    let wdth = detail.axes.iter().find(|a| a.tag == "wdth").expect("wdth");
    let css = lib
        .api
        .css_values(CssValuesRequest {
            face,
            variations: vec![
                AxisValue {
                    tag: "wght".into(),
                    value: 650.0001,
                },
                AxisValue {
                    tag: "wdth".into(),
                    value: wdth.default,
                },
            ],
            features: vec![
                FeatureSetting {
                    tag: "liga".into(),
                    on: false,
                },
                FeatureSetting {
                    tag: "kern".into(),
                    on: true,
                },
            ],
        })
        .expect("css");
    // wdth sits at its default and kern is on by default: neither is written.
    assert_eq!(
        css.variation_settings,
        r#"font-variation-settings: "wght" 650;"#
    );
    assert_eq!(css.feature_settings, r#"font-feature-settings: "liga" 0;"#);
    let err = lib
        .api
        .css_values(CssValuesRequest {
            face: FaceId(u32::MAX),
            variations: Vec::new(),
            features: Vec::new(),
        })
        .expect_err("unknown face");
    assert_eq!(err.code, ErrorCode::NotFound);
}

#[test]
fn get_face_names_stylistic_sets_from_the_index() {
    let (lib, source, _) = Lib::scanned("api-feature-names");
    let file = lib.root.join("calsans").join("CalSans-Regular.ttf");
    std::fs::create_dir_all(file.parent().expect("folder")).expect("folder");
    std::fs::copy(
        fixtures().join("google-repo/calsans/CalSans-Regular.ttf"),
        &file,
    )
    .expect("copy");
    lib.scan(Some(source.id), true);
    let family = lib
        .api
        .get_family(lib.family("Cal Sans").id)
        .expect("family");
    std::fs::remove_file(&file).expect("remove the file");

    let detail = lib.api.get_face(family.faces[0].id).expect("detail");
    let feature = |tag: &str| {
        detail
            .features
            .iter()
            .find(|f| f.tag == tag)
            .unwrap_or_else(|| panic!("no {tag}: {:?}", detail.features))
    };
    assert_eq!(feature("ss01").name.as_deref(), Some("Futura Alternates"));
    assert_eq!(feature("cv01").name.as_deref(), Some("Futura-style C"));
    assert_eq!(feature("cv05").name.as_deref(), Some("Futura-style zero"));
    let ss01 = feature("ss01");
    assert!(!ss01.automatic && !ss01.default_on);
    // Registered features keep their registry names.
    assert_eq!(feature("kern").name.as_deref(), Some("Kerning"));

    // A face whose ss01 the font does not name keeps the generic name.
    let asap = lib
        .api
        .get_face(FaceId(lib.family("Asap Sharp").id.0))
        .expect("asap");
    let ss01 = asap
        .features
        .iter()
        .find(|f| f.tag == "ss01")
        .expect("ss01");
    assert_eq!(ss01.name.as_deref(), Some("Stylistic set 1"));
}

#[test]
fn get_face_reports_the_palettes_of_a_color_face_and_none_for_a_plain_one() {
    let (lib, source, _) = Lib::scanned("api-palettes");
    let file = lib.root.join("cairoplay").join("CairoPlay[slnt,wght].ttf");
    std::fs::create_dir_all(file.parent().expect("folder")).expect("folder");
    std::fs::copy(
        fixtures().join("google-repo/cairoplay/CairoPlay[slnt,wght].ttf"),
        &file,
    )
    .expect("copy");
    lib.scan(Some(source.id), true);
    let cairo = FaceId(lib.family("Cairo Play").id.0);

    let detail = lib.api.get_face(cairo).expect("detail");
    assert_eq!(detail.palettes.len(), 1, "{:?}", detail.palettes);
    assert_eq!(detail.palettes[0].index, 0);
    assert_eq!(detail.palettes[0].colors[1], "#e53a35");

    let plain = lib
        .api
        .get_face(FaceId(lib.family("Archivo").id.0))
        .expect("plain");
    assert_eq!(plain.palettes, Vec::new());

    // A file gone since the scan still answers, without palettes.
    std::fs::remove_file(&file).expect("remove the file");
    let gone = lib.api.get_face(cairo).expect("detail of a missing file");
    assert_eq!(gone.palettes, Vec::new());
}

#[test]
fn problems_list_the_truncated_file_and_duplicates_answer() {
    let (lib, _, _) = Lib::scanned("api-problems");
    let problems = lib.api.list_problems().expect("problems");
    assert!(
        problems
            .iter()
            .any(|p| p.kind == ProblemKind::Broken && p.path.ends_with("agdasima-truncated.ttf")),
        "{problems:?}"
    );
    lib.api.list_duplicates().expect("duplicates");
}

#[test]
fn keep_this_one_moves_the_other_copy_to_the_trash_and_undo_brings_it_back() {
    let lib = Lib::new("api-keep-copy");
    let original = lib.root.join("bebasneue").join("BebasNeue-Regular.ttf");
    let copy = lib.root.join("copy").join("BebasNeue-Regular.ttf");
    std::fs::create_dir_all(copy.parent().expect("folder")).expect("folder");
    std::fs::copy(&original, &copy).expect("copy");
    let trash = lib.dir.path().join("trash");
    let lib = Lib {
        api: lib.api.with_trash(Arc::new(MockTrash::new(&trash))),
        ..lib
    };
    let source = lib
        .api
        .add_source(AddSourceRequest {
            path: lib.root.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    lib.scan(Some(source.id), true);
    let group = lib
        .api
        .list_duplicates()
        .expect("duplicates")
        .into_iter()
        .find(|g| g.copies.iter().any(|c| Path::new(&c.path) == copy))
        .expect("the copy is a duplicate");
    let keep = group
        .copies
        .iter()
        .find(|c| Path::new(&c.path) == original)
        .expect("the original is in the group")
        .path
        .clone();
    let req = |dry_run| KeepCopyRequest {
        keep: keep.clone(),
        trash: vec![copy.display().to_string()],
        dry_run,
    };

    let plan = lib.api.keep_copy(req(true)).expect("dry run");
    assert_eq!(plan.moves.len(), 1, "{plan:?}");
    assert_eq!(plan.moves[0].status, TrashMoveStatus::Planned);
    assert!(copy.is_file(), "a dry run moves nothing");
    assert!(lib.api.list_resolutions().expect("history").is_empty());

    let rx = lib.api.subscribe();
    let done = lib.api.keep_copy(req(false)).expect("keep");
    assert_eq!(done.moves[0].status, TrashMoveStatus::Trashed, "{done:?}");
    assert!(!copy.exists() && original.is_file());
    assert!(trash.read_dir().expect("trash").next().is_some());
    assert!(matches!(
        rx.recv_timeout(WAIT),
        Ok(Event::LibraryChanged { ref removed, .. }) if removed.len() == 1
    ));
    let history = lib.api.list_resolutions().expect("history");
    assert_eq!(history.len(), 1);
    let id = history[0].id.expect("logged");

    let undone = lib.api.undo_resolution(id).expect("undo");
    assert!(undone.undone, "{undone:?}");
    assert!(copy.is_file(), "the copy is back");
    // A scan after the undo finds the copy where it was, not a new file.
    lib.scan(Some(source.id), false);
    assert!(
        lib.api
            .list_duplicates()
            .expect("duplicates")
            .iter()
            .any(|g| g.copies.iter().any(|c| Path::new(&c.path) == copy))
    );
}

#[test]
fn keep_this_one_without_a_trash_answers_unsupported_and_moves_nothing() {
    let lib = Lib::new("api-no-trash");
    let file = lib.root.join("bebasneue").join("BebasNeue-Regular.ttf");
    is_unsupported(lib.api.keep_copy(KeepCopyRequest {
        keep: file.display().to_string(),
        trash: vec![file.display().to_string()],
        dry_run: false,
    }));
    is_unsupported(lib.api.undo_resolution(ResolutionId(1)));
    assert!(file.is_file());
    assert!(lib.api.list_resolutions().expect("history").is_empty());
}

#[test]
fn a_rescan_reports_removed_files_and_nothing_for_an_unchanged_folder() {
    let (lib, source, _) = Lib::scanned("api-rescan");
    let quiet = lib.scan(Some(source.id), false);
    assert!(
        !quiet
            .iter()
            .any(|e| matches!(e, Event::LibraryChanged { .. })),
        "{quiet:?}"
    );
    assert!(lib.names().iter().any(|n| n == "Bebas Neue"));
    std::fs::remove_dir_all(lib.root.join("bebasneue")).expect("remove");
    let events = lib.scan(Some(source.id), true);
    let Some(Event::LibraryChanged { removed, added, .. }) = events.last() else {
        panic!("{events:?}");
    };
    assert_eq!(removed.len(), 1, "{removed:?}");
    assert!(added.is_empty());
    assert!(!lib.names().iter().any(|n| n == "Bebas Neue"));
}

#[test]
fn sources_are_added_paused_and_removed() {
    let (lib, source, _) = Lib::scanned("api-sources");
    let rx = lib.api.subscribe();
    let paused = lib
        .api
        .set_source_paused(SetSourcePausedRequest {
            source: source.id,
            paused: true,
        })
        .expect("paused");
    assert!(paused.paused);
    assert!(lib.names().is_empty());
    assert!(matches!(
        rx.recv_timeout(WAIT),
        Ok(Event::LibraryChanged { removed, .. }) if !removed.is_empty()
    ));
    lib.api
        .set_source_paused(SetSourcePausedRequest {
            source: source.id,
            paused: false,
        })
        .expect("resumed");
    assert!(!lib.names().is_empty());
    let _ = rx.recv_timeout(WAIT);

    lib.api.remove_source(source.id).expect("removed");
    assert!(lib.names().is_empty());
    assert!(lib.api.list_sources().expect("sources").is_empty());
    assert!(matches!(
        rx.recv_timeout(WAIT),
        Ok(Event::LibraryChanged { removed, .. }) if !removed.is_empty()
    ));
    assert!(
        lib.root.join("bebasneue/BebasNeue-Regular.ttf").is_file(),
        "files stay"
    );
}

#[test]
fn bad_source_requests_are_errors_not_panics() {
    let lib = Lib::new("api-bad-sources");
    let missing = lib.dir.path().join("no-such-folder");
    let err = lib
        .api
        .add_source(AddSourceRequest {
            path: missing.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect_err("missing folder");
    assert_eq!(err.code, ErrorCode::NotFound);
    is_unsupported(lib.api.add_source(AddSourceRequest {
        path: lib.root.display().to_string(),
        mode: ImportMode::CopyTo {
            folder: lib.paths.library.display().to_string(),
        },
    }));
    let err = lib
        .api
        .scan(ScanRequest {
            source: Some(SourceId(42)),
        })
        .expect_err("unknown source");
    assert_eq!(err.code, ErrorCode::NotFound);
    // Nothing to scan still ends its job.
    let events = lib.scan(None, false);
    assert!(matches!(
        events.last(),
        Some(Event::ScanFinished { added: 0, .. })
    ));
    let info = lib.api.app_info().expect("info");
    assert_eq!(info.name, "Font Manager");
    assert_eq!(info.api_version, API_VERSION);
}

#[test]
fn app_info_notes_how_updates_arrive_where_the_app_does_not_update_itself() {
    let lib = Lib::new("api-update-notes");
    let notes = lib.api.app_info().expect("info").platform_notes;
    let note = crate::update::support(&fm_platform::updater::distribution()).reason;
    match note {
        Some(note) => assert!(notes.contains(&note), "{notes:?}"),
        None => assert!(
            !notes.iter().any(|n| n.contains("Check for updates")),
            "{notes:?}"
        ),
    }
}

#[test]
fn app_info_says_when_closing_the_window_quits() {
    let lib = Lib::new("api-tray-notes");
    let quits = |notes: &[String]| notes.iter().any(|n| n.contains("closing the window quits"));
    assert!(!quits(&lib.api.app_info().expect("info").platform_notes));
    lib.api.set_tray_support(TraySupport::Available);
    assert!(!quits(&lib.api.app_info().expect("info").platform_notes));
    lib.api
        .set_tray_support(TraySupport::Unavailable("no StatusNotifier host".into()));
    assert!(quits(&lib.api.app_info().expect("info").platform_notes));
}

/// E29.4 criterion 4: Settings → About says plugins need the window open
/// where closing it quits, the "Keep running in the tray" setting included.
#[test]
fn app_info_says_plugins_need_the_window_where_closing_quits() {
    use crate::lifecycle::PLUGINS_NEED_THE_WINDOW;
    let lib = Lib::new("api-plugin-notes");
    let says = |lib: &Lib| {
        lib.api
            .app_info()
            .expect("info")
            .platform_notes
            .iter()
            .any(|n| n == PLUGINS_NEED_THE_WINDOW)
    };
    lib.api.set_tray_support(TraySupport::Available);
    assert!(
        !says(&lib),
        "a tray and the default setting keep the app serving"
    );
    lib.api
        .update_settings(SettingsUpdate {
            keep_in_tray: Some(false),
            ..SettingsUpdate::default()
        })
        .expect("saved");
    assert!(says(&lib), "the setting is off: closing the window quits");
    lib.api
        .update_settings(SettingsUpdate {
            keep_in_tray: Some(true),
            ..SettingsUpdate::default()
        })
        .expect("saved");
    lib.api
        .set_tray_support(TraySupport::Unavailable("no StatusNotifier host".into()));
    assert!(says(&lib), "no tray item: closing the window quits");
}

const FLATPAK_CHILD: &str = "FM_FLATPAK_CHILD";

/// Run by [`inside_a_flatpak_start_at_login_is_hidden_and_the_notes_say_why`]
/// in a new process of this test binary with `FLATPAK_ID` set.
#[test]
#[ignore = "run as a child process with FLATPAK_ID set"]
fn child_reports_start_at_login_inside_a_flatpak() {
    assert!(
        std::env::var_os(FLATPAK_CHILD).is_some(),
        "run by the parent test"
    );
    let lib = Lib::new("api-flatpak-child");
    let notes = lib.api.app_info().expect("info").platform_notes;
    assert!(
        notes.iter().any(|n| n.contains("Flatpak sandbox")),
        "{notes:?}"
    );
    // Flatpak installs the updates: the note says so (E18.2).
    assert!(
        notes
            .iter()
            .any(|n| n.contains("Flatpak installs the updates")),
        "{notes:?}"
    );
    let store = SettingsStore::new(&lib.paths).with_system_autostart();
    assert_eq!(store.get_settings().expect("read").start_at_login, None);
}

#[test]
fn inside_a_flatpak_start_at_login_is_hidden_and_the_notes_say_why() {
    // Only Linux has a Flatpak; elsewhere `FLATPAK_ID` means nothing.
    if Os::current() != Os::Linux {
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "api::tests::child_reports_start_at_login_inside_a_flatpak",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env(FLATPAK_CHILD, "1")
        .env("FLATPAK_ID", "org.fontmanager.app")
        .output()
        .expect("the child runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("1 passed"),
        "{stdout}{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn rebuilding_the_index_scans_again_and_keeps_ids() {
    let (lib, _, _) = Lib::scanned("api-rebuild");
    let before = lib.family("Barlow");
    let rx = lib.api.subscribe();
    let job = lib.api.rebuild_index().expect("rebuild");
    let deadline = Instant::now() + WAIT;
    loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Event::ScanFinished { job: j, .. }) if j == job => break,
            Ok(Event::JobFailed { error, .. }) => panic!("{error:?}"),
            Ok(_) => {}
            Err(e) => panic!("{e:?}"),
        }
    }
    // The write follows the finish; wait for the index lock to be free.
    let _ = rx.recv_timeout(Duration::from_secs(5));
    assert_eq!(lib.family("Barlow").id, before.id);
}

/// Rebuilds the index and waits until the rebuild's scan has finished.
fn rebuild(lib: &Lib) {
    let rx = lib.api.subscribe();
    let job = lib.api.rebuild_index().expect("rebuild");
    let deadline = Instant::now() + WAIT;
    loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Event::ScanFinished { job: j, .. }) if j == job => break,
            Ok(Event::JobFailed { error, .. }) => panic!("{error:?}"),
            Ok(_) => {}
            Err(e) => panic!("{e:?}"),
        }
    }
    let _ = rx.recv_timeout(Duration::from_secs(5));
}

#[test]
fn pinned_styles_survive_a_rebuild_and_reach_the_sidecar() {
    let (lib, _, _) = Lib::scanned("api-virtual-styles");
    let face = FaceId(lib.family("Asap Sharp").id.0);
    let font = lib.root.join("asapsharp").join("AsapSharp[wdth,wght].ttf");
    let bytes = std::fs::read(&font).expect("font");
    let brand = lib
        .api
        .create_collection(CreateCollectionRequest {
            rule_group: None,
            name: "Brand".into(),
            parent: None,
            rules: None,
        })
        .expect("collection")
        .id;
    let client = lib
        .api
        .create_set(CreateSetRequest {
            name: "Client".into(),
            faces: Vec::new(),
        })
        .expect("set")
        .id;
    let semibold = lib
        .api
        .pin_instance(PinInstanceRequest {
            face,
            name: "Condensed Semibold".into(),
            coordinates: vec![AxisValue {
                tag: "wght".into(),
                value: 600.0,
            }],
            collection: Some(brand),
            set: None,
        })
        .expect("pin");
    // The real font's axes, the left-out width at its default.
    let axes = lib.api.get_face(face).expect("face").axes;
    assert_eq!(semibold.coordinates.len(), axes.len());
    for (c, a) in semibold.coordinates.iter().zip(&axes) {
        assert_eq!(c.tag, a.tag);
        let want = if a.tag == "wght" { 600.0 } else { a.default };
        assert!((c.value - want).abs() < f32::EPSILON, "{c:?} vs {a:?}");
    }
    let heavy = lib
        .api
        .pin_instance(PinInstanceRequest {
            face,
            name: "Heavy".into(),
            coordinates: Vec::new(),
            collection: None,
            set: Some(client),
        })
        .expect("pin");
    let bebas = FaceId(lib.family("Bebas Neue").id.0);
    let err = lib
        .api
        .pin_instance(PinInstanceRequest {
            face: bebas,
            name: "Wide".into(),
            coordinates: Vec::new(),
            collection: Some(brand),
            set: None,
        })
        .expect_err("a static face");
    assert_eq!(err.code, ErrorCode::InvalidRequest);
    assert!(err.message.contains("not a variable font"), "{err:?}");

    let in_brand = VirtualStylesRequest {
        collection: Some(brand),
        set: None,
    };
    let in_client = VirtualStylesRequest {
        collection: None,
        set: Some(client),
    };
    rebuild(&lib);
    assert_eq!(
        lib.api.list_virtual_styles(in_brand.clone()).expect("list"),
        vec![semibold.clone()]
    );
    assert_eq!(
        lib.api
            .list_virtual_styles(in_client.clone())
            .expect("list"),
        vec![heavy.clone()]
    );

    // Mirrored to the family's sidecar.
    let sidecars: Vec<PathBuf> = std::fs::read_dir(lib.root.join(crate::sidecar::SIDECAR_DIR))
        .expect("sidecar folder")
        .map(|e| e.expect("entry").path())
        .collect();
    let text: String = sidecars
        .iter()
        .map(|p| std::fs::read_to_string(p).expect("sidecar"))
        .collect();
    assert!(
        text.contains("Condensed Semibold") && text.contains("Heavy"),
        "{text}"
    );

    lib.api.delete_virtual_style(heavy.id).expect("delete");
    assert_eq!(
        lib.api.list_virtual_styles(in_client).expect("list"),
        vec![]
    );
    lib.api.delete_collection(brand).expect("delete");
    let err = lib
        .api
        .list_virtual_styles(in_brand)
        .expect_err("collection gone");
    assert_eq!(err.code, ErrorCode::NotFound);
    // Rule 2: the font file is never written.
    assert_eq!(std::fs::read(&font).expect("font"), bytes);
}

// --- search --------------------------------------------------------------

#[test]
fn search_and_filters_run_on_the_real_index() {
    let (lib, _, _) = Lib::scanned("api-search");
    let found = lib
        .api
        .search(SearchRequest {
            query: "bebas".into(),
            ..SearchRequest::default()
        })
        .expect("search");
    assert_eq!(found.families[0].family.name, "Bebas Neue");
    let variable = lib
        .api
        .search(SearchRequest {
            filters: Filters {
                variable: Some(true),
                ..Filters::default()
            },
            ..SearchRequest::default()
        })
        .expect("filter");
    let names: Vec<&str> = variable
        .families
        .iter()
        .map(|h| h.family.name.as_str())
        .collect();
    assert_eq!(names, ["Asap Sharp"]);
    assert!(variable.counts.is_some());
}

#[test]
fn coverage_and_languages_come_from_the_stored_cmap() {
    let (lib, _, _) = Lib::scanned("api-coverage");
    let languages = lib.api.list_languages().expect("languages");
    assert!(languages.iter().any(|l| l.code == "de"), "{languages:?}");
    let sr = languages
        .iter()
        .find(|l| l.code == "sr-Cyrl")
        .expect("sr-Cyrl");
    assert!(
        sr.pangram.as_deref().is_some_and(|p| p.contains('џ')),
        "{sr:?}"
    );
    let face = FaceId(lib.family("Bebas Neue").id.0);
    let text = lib
        .api
        .check_coverage(CoverageRequest {
            face,
            target: Supports::Text {
                text: "AB \u{2603}".into(),
            },
        })
        .expect("text");
    assert_eq!(text.missing, ["\u{2603}"]);
    assert!(!text.complete);
    let german = lib
        .api
        .check_coverage(CoverageRequest {
            face,
            target: Supports::Languages {
                codes: vec!["de".into()],
            },
        })
        .expect("german");
    assert_eq!(german.languages.len(), 1);
    assert!(german.languages[0].covered > 0);
    let err = lib
        .api
        .check_coverage(CoverageRequest {
            face: FaceId(99_999),
            target: Supports::Text { text: "a".into() },
        })
        .expect_err("unknown face");
    assert_eq!(err.code, ErrorCode::NotFound);
    // E23.2: similar faces come from the index, never the face itself.
    let similar = lib
        .api
        .find_similar(SimilarRequest { face, limit: 5 })
        .expect("similar");
    assert!(!similar.is_empty() && similar.len() <= 5, "{similar:?}");
    assert!(similar.iter().all(|s| s.face.id != face));
    assert!(similar.iter().all(|s| (0.0..=1.0).contains(&s.score)));
    let err = lib
        .api
        .find_similar(SimilarRequest {
            face: FaceId(99_999),
            limit: 5,
        })
        .expect_err("unknown face");
    assert_eq!(err.code, ErrorCode::NotFound);
}

#[test]
fn pairings_come_from_the_index_by_rule() {
    let (lib, _, _) = Lib::scanned_with(
        "api-pairings",
        &["google-repo/librebaskerville/LibreBaskerville[wght].ttf"],
    );
    let serif = FaceId(lib.family("Libre Baskerville").id.0);
    let got = lib
        .api
        .suggest_pairings(PairingRequest {
            face: serif,
            limit: 3,
        })
        .expect("pairings");
    assert_eq!(got.len(), 3);
    assert!(
        got[0].reasons[0].starts_with("contrast: serif with "),
        "{got:?}"
    );
    assert!(
        got.iter()
            .all(|s| s.face.family_name != "Libre Baskerville")
    );
    let err = lib
        .api
        .suggest_pairings(PairingRequest {
            face: FaceId(99_999),
            limit: 3,
        })
        .expect_err("unknown face");
    assert_eq!(err.code, ErrorCode::NotFound);
}

// --- preview -------------------------------------------------------------

#[test]
fn previews_render_from_the_file_and_are_cached() {
    let (lib, _, _) = Lib::scanned("api-preview");
    let face = FaceId(lib.family("Barlow").id.0);
    let source = lib.api.font_source(face).expect("source");
    assert!(
        source.path.ends_with("Barlow-BoldItalic.ttf"),
        "{}",
        source.path
    );
    assert_eq!(source.hash.len(), 64);
    let req = PreviewRequest {
        face,
        text: "Hamburg".into(),
        size_px: 32.0,
        variations: Vec::new(),
        features: Vec::new(),
        foreground: None,
        background: None,
    };
    let image = lib.api.render_preview(req.clone()).expect("preview");
    assert!(image.width > 0 && !image.png.is_empty());
    let cached = lib.api.previews.as_ref().map(PreviewCache::len);
    assert_eq!(cached, Some(1));
    assert_eq!(lib.api.render_preview(req).expect("again"), image);
    let samples = lib
        .api
        .sample_texts(SampleTextRequest {
            faces: vec![face],
            text: "Ab".into(),
        })
        .expect("samples");
    assert_eq!(samples[0].text, "Ab");
    let broken = lib
        .api
        .list_problems()
        .expect("problems")
        .into_iter()
        .find(|p| p.kind == ProblemKind::Broken)
        .and_then(|p| p.face);
    if let Some(face) = broken {
        let err = lib
            .api
            .render_preview(PreviewRequest {
                face,
                text: "a".into(),
                size_px: 20.0,
                variations: Vec::new(),
                features: Vec::new(),
                foreground: None,
                background: None,
            })
            .expect_err("broken font");
        assert_ne!(err.code, ErrorCode::Internal);
    }
}

#[test]
fn a_comparison_renders_one_row_per_face_through_the_cache() {
    let (lib, _, _) = Lib::scanned("api-compare");
    let face = |name: &str| {
        lib.api
            .get_family(lib.family(name).id)
            .expect("family")
            .faces[0]
            .id
    };
    let (barlow, bebas, archivo) = (face("Barlow"), face("Bebas Neue"), face("Archivo"));
    let req = |faces: Vec<FaceId>| ComparePreviewRequest {
        faces,
        text: "Hamburg".into(),
        size_px: 32.0,
        variations: Vec::new(),
        features: Vec::new(),
        foreground: None,
        background: None,
    };
    let image = lib
        .api
        .compare_preview(req(vec![barlow, bebas, archivo]))
        .expect("comparison");
    let rows: Vec<PreviewImage> = [barlow, bebas, archivo]
        .into_iter()
        .map(|f| lib.api.render_preview(req(vec![f]).row(f)).expect("row"))
        .collect();
    assert_eq!(image.height, rows.iter().map(|r| r.height).sum::<u32>());
    assert_eq!(image.width, rows.iter().map(|r| r.width).max().unwrap_or(0));
    let cached = lib.api.previews.as_ref().map(PreviewCache::len);
    assert_eq!(cached, Some(4), "the comparison and its three rows");
    assert_eq!(
        lib.api
            .compare_preview(req(vec![barlow, bebas, archivo]))
            .expect("again"),
        image
    );
    assert_eq!(lib.api.previews.as_ref().map(PreviewCache::len), Some(4));
    let swapped = lib
        .api
        .compare_preview(req(vec![bebas, barlow, archivo]))
        .expect("swapped");
    assert_ne!(swapped.png, image.png, "row order is part of the image");

    for faces in [vec![barlow], vec![barlow, bebas, archivo, barlow, bebas]] {
        let n = faces.len();
        let err = lib.api.compare_preview(req(faces)).expect_err("count");
        assert_eq!(err.code, ErrorCode::InvalidRequest, "{n} faces");
    }
    let mut long = req(vec![barlow, bebas]);
    long.text = "a".repeat(501);
    assert_eq!(
        lib.api.compare_preview(long).expect_err("long").code,
        ErrorCode::InvalidRequest
    );
    let mut big = req(vec![barlow, bebas]);
    big.size_px = 301.0;
    assert_eq!(
        lib.api.compare_preview(big).expect_err("size").code,
        ErrorCode::InvalidRequest
    );
    let missing = lib
        .api
        .compare_preview(req(vec![barlow, FaceId(999_999)]))
        .expect_err("missing face");
    assert_eq!(missing.code, ErrorCode::NotFound);
}

// --- activation ----------------------------------------------------------

#[test]
fn activation_goes_through_the_activator_and_is_announced() {
    let (lib, _, _) = Lib::scanned("api-activation");
    let family = lib.family("Bebas Neue");
    let rx = lib.api.subscribe();
    let outcomes = lib
        .api
        .activate(ActivateRequest {
            target: Target::Family { family: family.id },
            mode: ActivationMode::Session,
            on_conflict: ConflictPolicy::default(),
        })
        .expect("activate");
    assert!(
        outcomes.iter().all(|o| o.outcome == OutcomeStatus::Done),
        "{outcomes:?}"
    );
    assert!(matches!(
        rx.recv_timeout(WAIT),
        Ok(Event::ActivationChanged { activated, .. }) if activated.len() == outcomes.len()
    ));
    assert_eq!(lib.activator.list_active().expect("os list").len(), 1);
    assert_eq!(lib.api.list_active().expect("active").len(), outcomes.len());
    assert_eq!(
        lib.family("Bebas Neue").activation,
        FamilyActivation::Active
    );
    lib.api.list_conflicts().expect("conflicts");
    lib.api
        .conflicts(Target::Family { family: family.id })
        .expect("conflicts of a target");

    lib.api
        .deactivate(DeactivateRequest {
            target: None,
            except: None,
        })
        .expect("deactivate");
    assert!(matches!(
        rx.recv_timeout(WAIT),
        Ok(Event::ActivationChanged { deactivated, .. }) if !deactivated.is_empty()
    ));
    assert!(lib.api.list_active().expect("active").is_empty());
}

// --- organization --------------------------------------------------------

#[test]
fn collections_tags_metadata_and_sets_use_the_index() {
    let (lib, _, _) = Lib::scanned("api-organization");
    let family = lib.family("Barlow");
    let face = FaceId(family.id.0);
    let collection = lib
        .api
        .create_collection(CreateCollectionRequest {
            rule_group: None,
            name: "Headlines".into(),
            parent: None,
            rules: None,
        })
        .expect("collection");
    let filled = lib
        .api
        .add_to_collection(CollectionFacesRequest {
            collection: collection.id,
            faces: Vec::new(),
            families: vec![family.id],
        })
        .expect("added");
    assert_eq!(filled.face_count, 1);
    assert_eq!(lib.api.list_collections().expect("collections").len(), 1);

    let rx = lib.api.subscribe();
    let tags = lib
        .api
        .tag_faces(TagFacesRequest {
            families: Vec::new(),
            faces: vec![face],
            tags: vec!["Poster".into()],
        })
        .expect("tagged");
    assert!(matches!(
        rx.recv_timeout(WAIT),
        Ok(Event::LibraryChanged { updated, .. }) if updated == vec![face]
    ));
    assert_eq!(lib.family("Barlow").tags, ["Poster"]);
    lib.api
        .update_user_meta(UserMetaUpdate {
            faces: vec![face],
            favorite: Some(true),
            rating: Some(4),
            notes: Some("for the poster".into()),
        })
        .expect("meta");
    let detail = lib.api.get_face(face).expect("detail");
    assert!(detail.favorite);
    assert_eq!(detail.rating, Some(4));
    assert_eq!(detail.tags, ["Poster"]);
    lib.api
        .untag_faces(UntagFacesRequest {
            faces: vec![face],
            tags: vec![tags[0].id],
        })
        .expect("untagged");
    assert!(lib.api.get_face(face).expect("detail").tags.is_empty());

    let set = lib
        .api
        .create_set(CreateSetRequest {
            name: "Brochure".into(),
            faces: vec![face],
        })
        .expect("set");
    assert_eq!(lib.api.list_sets().expect("sets").len(), 1);
    let export = lib.api.export_set(set.id).expect("export");
    lib.api.delete_set(set.id).expect("deleted");
    lib.api.import_set(export).expect("imported");
    lib.api.delete_collection(collection.id).expect("deleted");
}

// --- settings, export, workflows, events ---------------------------------

#[test]
fn settings_live_in_the_config_file() {
    let lib = Lib::new("api-settings");
    assert_eq!(
        lib.api.get_settings().expect("settings"),
        Settings::default()
    );
    let changed = lib
        .api
        .update_settings(SettingsUpdate {
            cache_limit_mb: Some(64),
            ..SettingsUpdate::default()
        })
        .expect("updated");
    assert_eq!(changed.cache_limit_mb, 64);
    assert_eq!(
        lib.api.previews.as_ref().map(PreviewCache::limit),
        Some(cache::limit_from_mb(64))
    );
    assert_eq!(
        SettingsStore::new(&lib.paths)
            .get_settings()
            .expect("reread"),
        changed
    );
    let folders = lib.api.app_folders().expect("folders");
    assert_eq!(folders.data, lib.paths.data.display().to_string());
}

#[test]
fn every_subscriber_sees_every_event_and_a_dropped_one_is_forgotten() {
    let events = Broadcast::default();
    let a = events.subscribe();
    let b = events.subscribe();
    drop(b);
    let event = Event::ScanProgress {
        job: JobId(1),
        done: 1,
        total: 2,
    };
    events.send(&event);
    assert_eq!(a.try_recv(), Ok(event));
    assert_eq!(lock(&events.subscribers).len(), 1);
}

#[test]
fn start_and_shutdown_follow_the_settings() {
    let (lib, _, _) = Lib::scanned("api-session");
    let family = lib.family("Bebas Neue");
    lib.api
        .activate(ActivateRequest {
            target: Target::Family { family: family.id },
            mode: ActivationMode::Session,
            on_conflict: ConflictPolicy::default(),
        })
        .expect("activate");
    lib.api.shutdown().expect("shutdown");
    assert!(lib.api.list_active().expect("active").is_empty());
    lib.api
        .update_settings(SettingsUpdate {
            reactivate_on_launch: Some(true),
            ..SettingsUpdate::default()
        })
        .expect("setting");
    let again = lib.api.start().expect("start");
    assert!(!again.is_empty());
    assert_eq!(lib.api.list_active().expect("active").len(), again.len());
}

#[test]
fn changes_name_added_removed_gone_and_reread_faces() {
    let path = |p: &str| PathBuf::from(p);
    let before: HashMap<FaceId, PathBuf> = [
        (FaceId(1), path("a.ttf")),
        (FaceId(2), path("b.ttf")),
        (FaceId(3), path("c.ttf")),
    ]
    .into();
    let after: HashMap<FaceId, PathBuf> = [
        (FaceId(1), path("a.ttf")),
        (FaceId(3), path("c.ttf")),
        (FaceId(4), path("d.ttf")),
    ]
    .into();
    let rescan = Rescan {
        changed: vec![path("a.ttf")],
        gone: vec![path("c.ttf")],
        ..Rescan::default()
    };
    let Some(Event::LibraryChanged {
        source,
        added,
        removed,
        updated,
    }) = changes(SourceId(7), &before, &after, &rescan)
    else {
        panic!("a change");
    };
    assert_eq!(source, Some(SourceId(7)));
    assert_eq!(added, [FaceId(4)]);
    assert_eq!(removed, [FaceId(2), FaceId(3)]);
    assert_eq!(updated, [FaceId(1)]);
    assert!(changes(SourceId(7), &after, &after, &Rescan::default()).is_none());
}

#[test]
fn health_of_a_broken_face_reaches_the_family() {
    let (lib, _, _) = Lib::scanned("api-health");
    let broken = lib
        .api
        .list_problems()
        .expect("problems")
        .into_iter()
        .find(|p| p.kind == ProblemKind::Broken)
        .and_then(|p| p.face);
    if let Some(face) = broken {
        let detail = lib.api.get_face(face).expect("detail of a broken face");
        assert!(
            matches!(detail.face.health, Health::Broken(_)),
            "{:?}",
            detail.face.health
        );
    }
}

#[test]
fn the_glyph_map_reads_the_face_file_by_block_and_search() {
    let (lib, _, _) = Lib::scanned("api-glyphs");
    let archivo = lib.family("Archivo");
    let face = lib.api.get_family(archivo.id).expect("family").faces[0].id;
    let all = lib
        .api
        .list_glyphs(GlyphsRequest {
            face,
            block: None,
            query: None,
            page: Page::default(),
        })
        .expect("glyphs");
    assert_eq!(all.health, Health::Ok);
    assert_eq!(all.items.len(), 100, "the default page");
    assert!(all.total > 100);
    assert_eq!(all.blocks.iter().map(|b| b.count).sum::<u32>(), all.total);
    let found = lib
        .api
        .list_glyphs(GlyphsRequest {
            face,
            block: Some("Basic Latin".into()),
            query: Some("U+0041".into()),
            page: Page::default(),
        })
        .expect("search");
    assert_eq!(found.total, 1);
    assert_eq!(found.items[0].code_point, 0x41);
    assert_eq!(found.items[0].name.as_deref(), Some("A"));
    let unknown = lib.api.list_glyphs(GlyphsRequest {
        face: FaceId(9999),
        block: None,
        query: None,
        page: Page::default(),
    });
    assert_eq!(unknown.expect_err("unknown face").code, ErrorCode::NotFound);
}

#[test]
fn the_glyph_detail_reads_the_face_file() {
    let (lib, _, _) = Lib::scanned("api-glyph-detail");
    let archivo = lib.family("Archivo");
    let face = lib.api.get_family(archivo.id).expect("family").faces[0].id;
    let fi = lib
        .api
        .get_glyph(GlyphRequest {
            face,
            code_point: 0xFB01,
        })
        .expect("ﬁ");
    assert_eq!(fi.health, Health::Ok);
    assert_eq!(fi.name.as_deref(), Some("fi"));
    assert!(fi.advance_width.is_some_and(|w| w > 0));
    assert_eq!(
        fi.produced_by
            .iter()
            .map(|f| f.tag.as_str())
            .collect::<Vec<_>>(),
        ["liga"]
    );
    let unmapped = lib.api.get_glyph(GlyphRequest {
        face,
        code_point: 0x10_FFFD,
    });
    assert_eq!(unmapped.expect_err("unmapped").code, ErrorCode::NotFound);
    let unknown = lib.api.get_glyph(GlyphRequest {
        face: FaceId(9999),
        code_point: 0x41,
    });
    assert_eq!(unknown.expect_err("unknown face").code, ErrorCode::NotFound);
}

/// Every file under `dir`, relative, sorted.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path.strip_prefix(dir).expect("under dir").to_path_buf());
            }
        }
    }
    out.sort();
    out
}

fn web_request(faces: Vec<FaceId>, out_dir: &Path) -> ExportWebRequest {
    ExportWebRequest {
        faces,
        formats: vec![WebFormat::Woff2, WebFormat::Woff],
        subset: None,
        out_dir: out_dir.display().to_string(),
        css: false,
        overwrite: false,
    }
}

#[test]
fn faces_export_as_web_fonts_named_by_postscript_name() {
    let (lib, _, _) = Lib::scanned("api-export-web");
    let first = |name: &str| {
        lib.api
            .get_family(lib.family(name).id)
            .expect("family")
            .faces[0]
            .clone()
    };
    let bebas = first("Bebas Neue");
    let archivo = first("Archivo");
    let ttc = lib
        .api
        .get_family(lib.family("Agdasima").id)
        .expect("family")
        .faces
        .into_iter()
        .find(|f| {
            let d = lib.api.get_face(f.id).expect("face");
            d.path.ends_with(".ttc") && d.collection_index == 1
        })
        .expect("a face of the collection");
    let out = lib.dir.path().join("Web fonts ü");
    std::fs::create_dir_all(&out).expect("out");
    let before = files_under(&lib.root);
    let faces = vec![bebas.id, ttc.id, archivo.id, bebas.id];
    let res = lib
        .api
        .export_web(web_request(faces, &out))
        .expect("export");
    assert_eq!(res.css, None);
    let mut want = Vec::new();
    for f in [&bebas, &ttc, &archivo] {
        for ext in ["woff2", "woff"] {
            let path = out.join(format!("{}.{ext}", f.postscript_name));
            want.push(path.display().to_string());
        }
    }
    assert_eq!(res.files, want);
    for f in &res.files {
        assert!(Path::new(f).is_absolute(), "{f}");
        let bytes = std::fs::read(f).expect("written");
        assert!(
            bytes.starts_with(b"wOF2") || bytes.starts_with(b"wOFF"),
            "{f}"
        );
    }
    assert_eq!(files_under(&lib.root), before, "the sources are only read");

    // A second export into the same folder clashes and writes nothing.
    let e = lib
        .api
        .export_web(web_request(vec![archivo.id], &out))
        .expect_err("clash");
    assert_eq!(e.code, ErrorCode::InvalidRequest);
    let name = format!("{}.woff2", archivo.postscript_name);
    assert!(e.message.contains(&name), "{e:?}");
    assert_eq!(files_under(&out).len(), 6);
}

#[test]
fn web_export_refuses_bad_requests() {
    let (lib, _, _) = Lib::scanned("api-export-web-refused");
    let face = lib
        .api
        .get_family(lib.family("Bebas Neue").id)
        .expect("family")
        .faces[0]
        .id;
    let out = lib.dir.path().join("out");
    std::fs::create_dir_all(&out).expect("out");
    let code = |req| lib.api.export_web(req).expect_err("refused").code;

    assert_eq!(
        code(web_request(Vec::new(), &out)),
        ErrorCode::InvalidRequest
    );
    let no_format = ExportWebRequest {
        formats: Vec::new(),
        ..web_request(vec![face], &out)
    };
    assert_eq!(code(no_format), ErrorCode::InvalidRequest);
    let no_folder = ExportWebRequest {
        out_dir: "  ".into(),
        ..web_request(vec![face], &out)
    };
    assert_eq!(code(no_folder), ErrorCode::InvalidRequest);
    let unknown = web_request(vec![face, FaceId(999_999)], &out);
    assert_eq!(code(unknown), ErrorCode::NotFound);
    assert!(files_under(&out).is_empty());
}

/// The face of `family` whose file name ends with `file`.
fn face_in(lib: &Lib, family: &str, file: &str) -> FaceId {
    lib.api
        .get_family(lib.family(family).id)
        .expect("family")
        .faces
        .into_iter()
        .find(|f| lib.api.get_face(f.id).expect("face").path.ends_with(file))
        .unwrap_or_else(|| panic!("{family}: no face from {file}"))
        .id
}

#[test]
fn web_export_writes_fonts_css_for_static_variable_and_italic_faces() {
    let (lib, _, _) = Lib::scanned_with(
        "api-export-web-css",
        &[
            "google-repo/cairoplay/CairoPlay[slnt,wght].ttf",
            "fontshare/sora/Sora-Italic.woff2",
            // Name ID 1 "Asap SemiBold", name ID 16 "Asap": the CSS family is 16.
            "fontshare/asap/Asap-SemiBold.otf",
        ],
    );
    let faces = vec![
        face_in(&lib, "Bebas Neue", "BebasNeue-Regular.ttf"),
        face_in(&lib, "Asap Sharp", "AsapSharp[wdth,wght].ttf"),
        face_in(&lib, "Cairo Play", "CairoPlay[slnt,wght].ttf"),
        face_in(&lib, "Sora", "Sora-Italic.woff2"),
        face_in(&lib, "Asap", "Asap-SemiBold.otf"),
    ];
    let out = lib.dir.path().join("css");
    std::fs::create_dir_all(&out).expect("out");
    let res = lib
        .api
        .export_web(ExportWebRequest {
            css: true,
            ..web_request(faces, &out)
        })
        .expect("export");
    let css = res.css.clone().expect("css");
    let css_path = out.join("fonts.css");
    assert_eq!(res.files.len(), 11);
    assert_eq!(res.files.last(), Some(&css_path.display().to_string()));
    assert_eq!(std::fs::read_to_string(&css_path).expect("written"), css);
    assert_eq!(css, WEB_CSS_SNAPSHOT, "{css}");

    // A second export clashes on the stylesheet too, and writes nothing.
    let again = lib.dir.path().join("again");
    std::fs::create_dir_all(&again).expect("again");
    std::fs::write(again.join("fonts.css"), "keep me").expect("existing");
    let e = lib
        .api
        .export_web(ExportWebRequest {
            css: true,
            ..web_request(vec![face_in(&lib, "Bebas Neue", ".ttf")], &again)
        })
        .expect_err("clash");
    assert_eq!(e.code, ErrorCode::InvalidRequest);
    assert!(e.message.contains("fonts.css"), "{e:?}");
    assert_eq!(files_under(&again).len(), 1);
    assert_eq!(
        std::fs::read_to_string(again.join("fonts.css")).expect("kept"),
        "keep me"
    );
}

/// `fonts.css` of [`web_export_writes_fonts_css_for_static_variable_and_italic_faces`].
const WEB_CSS_SNAPSHOT: &str = r#"@font-face {
  font-family: "Bebas Neue";
  font-style: normal;
  font-weight: 400;
  font-display: swap;
  src: url("BebasNeue-Regular.woff2") format("woff2"), url("BebasNeue-Regular.woff") format("woff");
}

@font-face {
  font-family: "Asap Sharp";
  font-style: normal;
  font-weight: 100 900;
  font-stretch: 75% 125%;
  font-display: swap;
  src: url("AsapSharp-Regular.woff2") format("woff2"), url("AsapSharp-Regular.woff") format("woff");
}

@font-face {
  font-family: "Cairo Play";
  font-style: oblique -11deg 11deg;
  font-weight: 200 1000;
  font-display: swap;
  src: url("CairoPlay-Regular.woff2") format("woff2"), url("CairoPlay-Regular.woff") format("woff");
}

@font-face {
  font-family: "Sora";
  font-style: italic;
  font-weight: 400;
  font-display: swap;
  src: url("Sora-Italic.woff2") format("woff2"), url("Sora-Italic.woff") format("woff");
}

@font-face {
  font-family: "Asap";
  font-style: normal;
  font-weight: 600;
  font-display: swap;
  src: url("Asap-SemiBold.woff2") format("woff2"), url("Asap-SemiBold.woff") format("woff");
}
"#;

fn snippet(
    lib: &Lib,
    framework: Framework,
    faces: Vec<FaceId>,
    fonts_path: Option<&str>,
) -> String {
    lib.api
        .framework_snippet(FrameworkSnippetRequest {
            faces,
            framework,
            fonts_path: fonts_path.map(str::to_owned),
        })
        .expect("snippet")
        .text
}

#[test]
fn framework_snippets_name_the_fixture_families_with_their_class_fallback() {
    let (lib, _, _) = Lib::scanned_with(
        "api-framework-snippet",
        &[
            "google-repo/castoro/Castoro[wght].ttf",
            "google-repo/inconsolata/Inconsolata-SemiExpandedBold.ttf",
            "google-repo/clickerscript/ClickerScript-Regular.ttf",
        ],
    );
    let faces = vec![
        face_in(&lib, "Castoro", "Castoro[wght].ttf"),
        face_in(&lib, "Inconsolata", "Inconsolata-SemiExpandedBold.ttf"),
        face_in(&lib, "Clicker Script", "ClickerScript-Regular.ttf"),
        face_in(&lib, "Asap Sharp", "AsapSharp[wdth,wght].ttf"),
    ];
    let tailwind = snippet(&lib, Framework::Tailwind, faces.clone(), None);
    assert_eq!(tailwind, TAILWIND_SNAPSHOT, "{tailwind}");
    let css = snippet(&lib, Framework::CssVariables, faces, None);
    assert_eq!(css, CSS_VARIABLES_SNAPSHOT, "{css}");
}

#[test]
fn a_next_local_font_snippet_lists_static_and_variable_faces() {
    let (lib, _, _) = Lib::scanned("api-framework-snippet-next");
    let faces = vec![
        face_in(&lib, "Archivo", "Archivo-Bold.woff2"),
        face_in(&lib, "Asap Sharp", "AsapSharp[wdth,wght].ttf"),
        face_in(&lib, "Archivo", "Archivo-SemiBold.woff2"),
        // Listed twice: one entry.
        face_in(&lib, "Archivo", "Archivo-Bold.woff2"),
    ];
    let next = snippet(&lib, Framework::NextLocalFont, faces.clone(), None);
    assert_eq!(next, NEXT_LOCAL_FONT_SNAPSHOT, "{next}");
    let elsewhere = snippet(
        &lib,
        Framework::NextLocalFont,
        faces,
        Some("../public/fonts/"),
    );
    assert_eq!(
        elsewhere,
        NEXT_LOCAL_FONT_SNAPSHOT.replace("'./fonts/", "'../public/fonts/")
    );
}

#[test]
fn a_framework_snippet_needs_a_known_face() {
    let (lib, _, _) = Lib::scanned("api-framework-snippet-bad");
    for framework in [
        Framework::Tailwind,
        Framework::CssVariables,
        Framework::NextLocalFont,
    ] {
        let empty = lib
            .api
            .framework_snippet(FrameworkSnippetRequest {
                faces: Vec::new(),
                framework,
                fonts_path: None,
            })
            .expect_err("no faces");
        assert_eq!(empty.code, ErrorCode::InvalidRequest);
        let unknown = lib
            .api
            .framework_snippet(FrameworkSnippetRequest {
                faces: vec![FaceId(999_999)],
                framework,
                fonts_path: None,
            })
            .expect_err("unknown face");
        assert_eq!(unknown.code, ErrorCode::NotFound);
    }
}

/// The Tailwind snippet of [`framework_snippets_name_the_fixture_families_with_their_class_fallback`].
const TAILWIND_SNAPSHOT: &str = r#"// tailwind.config.js
export default {
  theme: {
    extend: {
      fontFamily: {
        'castoro': ['"Castoro"', 'serif'],
        'inconsolata': ['"Inconsolata"', 'monospace'],
        'clicker-script': ['"Clicker Script"', 'cursive'],
        'asap-sharp': ['"Asap Sharp"', 'sans-serif'],
      },
    },
  },
};
"#;

/// The CSS variables of [`framework_snippets_name_the_fixture_families_with_their_class_fallback`].
const CSS_VARIABLES_SNAPSHOT: &str = r#":root {
  --font-castoro: "Castoro", serif;
  --font-inconsolata: "Inconsolata", monospace;
  --font-clicker-script: "Clicker Script", cursive;
  --font-asap-sharp: "Asap Sharp", sans-serif;
}
"#;

/// The `localFont` calls of [`a_next_local_font_snippet_lists_static_and_variable_faces`].
const NEXT_LOCAL_FONT_SNAPSHOT: &str = r#"import localFont from 'next/font/local';

export const fontArchivo = localFont({
  src: [
    { path: './fonts/Archivo-SemiBold.woff2', weight: '600', style: 'normal' },
    { path: './fonts/Archivo-Bold.woff2', weight: '700', style: 'normal' },
  ],
  variable: '--font-archivo',
  display: 'swap',
});

export const fontAsapSharp = localFont({
  src: [
    { path: './fonts/AsapSharp-Regular.woff2', weight: '100 900', style: 'normal' },
  ],
  variable: '--font-asap-sharp',
  display: 'swap',
});
"#;

#[test]
fn a_subset_web_export_writes_the_mapped_unicode_range() {
    let (lib, _, _) = Lib::scanned("api-export-web-css-subset");
    let face = face_in(&lib, "Bebas Neue", "BebasNeue-Regular.ttf");
    let out = lib.dir.path().join("out");
    std::fs::create_dir_all(&out).expect("out");
    let res = lib
        .api
        .export_web(ExportWebRequest {
            css: true,
            subset: Some(Subset::Text {
                text: "BA日".into(),
            }),
            formats: vec![WebFormat::Woff],
            ..web_request(vec![face], &out)
        })
        .expect("export");
    let css = res.css.expect("css");
    assert!(
        css.contains(
            "  unicode-range: U+0020, U+0041-0042;
"
        ),
        "only what the written file maps: {css}"
    );
    assert!(
        css.contains(
            "  src: url(\"BebasNeue-Regular.woff\") format(\"woff\");
"
        ),
        "{css}"
    );
}

#[test]
fn web_export_keeps_only_the_subset_and_refuses_a_bad_one() {
    let (lib, _, _) = Lib::scanned("api-export-web-subset");
    let face = lib
        .api
        .get_family(lib.family("Bebas Neue").id)
        .expect("family")
        .faces[0]
        .clone();
    let out = lib.dir.path().join("out");
    std::fs::create_dir_all(&out).expect("out");
    let with = |subset: Subset| ExportWebRequest {
        formats: vec![WebFormat::Woff2],
        subset: Some(subset),
        ..web_request(vec![face.id], &out)
    };
    let refused = |subset: Subset| {
        let e = lib.api.export_web(with(subset)).expect_err("refused");
        assert_eq!(e.code, ErrorCode::InvalidRequest, "{e:?}");
        assert!(files_under(&out).is_empty(), "nothing is written: {e:?}");
        e.message
    };
    let m = refused(Subset::Presets {
        names: vec!["latin".into(), "elvish".into()],
    });
    assert!(m.contains("\"elvish\""), "{m}");
    let m = refused(Subset::Unicode {
        ranges: vec!["U+00FF-0000".into()],
    });
    assert!(m.contains("\"U+00FF-0000\""), "{m}");
    let m = refused(Subset::Text {
        text: "日本".into(),
    });
    assert!(m.contains("none of the characters"), "{m}");

    let res = lib
        .api
        .export_web(with(Subset::Text { text: "AB".into() }))
        .expect("export");
    assert_eq!(res.files.len(), 1);
    let woff2 = std::fs::read(&res.files[0]).expect("written");
    let sfnt = crate::glyphs::sfnt(&woff2).expect("decodes");
    let font = skrifa::FontRef::new(&sfnt).expect("font");
    use skrifa::MetadataProvider as _;
    let mapped: Vec<u32> = font.charmap().mappings().map(|(cp, _)| cp).collect();
    assert_eq!(mapped, [0x20, 0x41, 0x42]);
}

/// Writes a copy of the fixture `file` with `OS/2.fsType` set to `fs_type`
/// into `folder`.
fn with_fs_type(file: &str, fs_type: u16, folder: &Path) {
    let mut data = std::fs::read(fixtures().join(file)).expect("fixture");
    let os2 = read_fonts::FontRef::new(&data)
        .expect("font")
        .table_directory()
        .table_records()
        .iter()
        .find(|r| r.tag() == read_fonts::types::Tag::new(b"OS/2"))
        .map(|r| r.offset() as usize)
        .expect("OS/2");
    data[os2 + 8..os2 + 10].copy_from_slice(&fs_type.to_be_bytes());
    std::fs::create_dir_all(folder).expect("folder");
    let name = Path::new(file).file_name().expect("name");
    std::fs::write(folder.join(name), &data).expect("font");
}

#[test]
fn the_license_guard_refuses_the_whole_export_on_embedding_bits() {
    let lib = Lib::new("api-export-web-guard");
    let folder = lib.dir.path().join("Patched");
    // Restricted (0x0002), preview & print (0x0004), installable (0x0000)
    // and editable (0x0008), each its own family. All four name the OFL:
    // an open license does not lift an embedding restriction.
    with_fs_type(
        "google-repo/bebasneue/BebasNeue-Regular.ttf",
        0x0002,
        &folder,
    );
    with_fs_type("google-repo/asul/Asul-Bold.ttf", 0x0004, &folder);
    with_fs_type("google-repo/basic/Basic-Regular.ttf", 0x0000, &folder);
    with_fs_type("google-repo/castoro/Castoro[wght].ttf", 0x0008, &folder);
    let source = lib
        .api
        .add_source(AddSourceRequest {
            path: folder.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    lib.scan(Some(source.id), true);
    let face = |family: &str| {
        lib.api
            .get_family(lib.family(family).id)
            .expect("family")
            .faces[0]
            .id
    };
    let (restricted, preview, installable, editable) = (
        face("Bebas Neue"),
        face("Asul"),
        face("Basic"),
        face("Castoro"),
    );
    let out = lib.dir.path().join("out");
    std::fs::create_dir_all(&out).expect("out");
    let export = |faces: Vec<FaceId>| lib.api.export_web(web_request(faces, &out));

    // One blocked face refuses the whole request: nothing is written, and
    // the message names every blocked face with its reason.
    let e = export(vec![installable, restricted, editable, preview]).expect_err("blocked");
    assert_eq!(e.code, ErrorCode::PermissionDenied, "{e:?}");
    assert_eq!(
        e.message,
        "Bebas Neue Regular: Embedding restricted, no web license declared; \
         Asul Bold: Embedding preview & print only, no web license declared. \
         Declare a web license for the family to export it."
    );
    assert_eq!(
        e.detail.as_deref(),
        Some(format!("blocked faces: {}, {}", restricted.0, preview.0).as_str())
    );
    assert!(files_under(&out).is_empty());
    // The same with CSS: no fonts.css either.
    let e = lib
        .api
        .export_web(ExportWebRequest {
            css: true,
            ..web_request(vec![preview], &out)
        })
        .expect_err("preview & print");
    assert_eq!(e.code, ErrorCode::PermissionDenied);
    assert!(files_under(&out).is_empty());

    // Installable and editable faces with no declaration export.
    let res = export(vec![installable, editable]).expect("installable and editable");
    assert_eq!(res.files.len(), 4);
    for f in &res.files {
        std::fs::remove_file(f).expect("clean up");
    }

    // A declared license that is not Web does not lift the block.
    let declare = |family: &str, license_type| {
        lib.api
            .declare_license(LicenseDeclaration {
                family: lib.family(family).id,
                license_type,
                seats: None,
                purchase_date: None,
                receipt: ReceiptChange::Keep,
            })
            .expect("declare");
    };
    declare("Bebas Neue", Some(LicenseType::Desktop));
    let e = export(vec![restricted]).expect_err("desktop");
    assert!(e.message.contains("Embedding restricted"), "{e:?}");
    assert!(files_under(&out).is_empty());

    // Declaring Web for both families lets the same export through.
    declare("Bebas Neue", Some(LicenseType::Web));
    let e = export(vec![installable, restricted, editable, preview]).expect_err("asul left");
    assert!(e.message.starts_with("Asul Bold:"), "{e:?}");
    assert_eq!(e.detail, Some(format!("blocked faces: {}", preview.0)));
    declare("Asul", Some(LicenseType::Web));
    let res = export(vec![installable, restricted, editable, preview]).expect("declared web");
    assert_eq!(res.files.len(), 8);
}

#[test]
fn a_glyph_exports_as_svg_text_without_writing_a_file() {
    let (lib, _, _) = Lib::scanned("api-glyph-svg");
    let archivo = lib.family("Archivo");
    let face = lib.api.get_family(archivo.id).expect("family").faces[0].id;
    let before = files_under(&lib.root);
    let svg = lib
        .api
        .export_glyph_svg(GlyphSvgRequest {
            face,
            code_point: 'A' as u32,
            variations: Vec::new(),
        })
        .expect("A");
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 "));
    assert!(svg.contains("<path d=\"M"), "{svg}");
    assert_eq!(files_under(&lib.root), before);

    let unmapped = lib.api.export_glyph_svg(GlyphSvgRequest {
        face,
        code_point: 0x10_FFFD,
        variations: Vec::new(),
    });
    assert_eq!(unmapped.expect_err("unmapped").code, ErrorCode::NotFound);
    let unknown = lib.api.export_glyph_svg(GlyphSvgRequest {
        face: FaceId(9999),
        code_point: 0x41,
        variations: Vec::new(),
    });
    assert_eq!(unknown.expect_err("unknown face").code, ErrorCode::NotFound);
}

#[test]
fn face_and_family_notes_survive_a_rescan_and_a_rebuild() {
    let (lib, _, _) = Lib::scanned("api-notes");
    let barlow = lib.family("Barlow");
    let face = FaceId(barlow.id.0);
    let rx = lib.api.subscribe();
    lib.api
        .update_family_notes(FamilyNotesUpdate {
            family: barlow.id,
            notes: "Licensed for the 2026 rebrand".into(),
        })
        .expect("family notes");
    // The list hears of it: every face of the family is updated.
    let family_faces = lib.api.get_family(barlow.id).expect("family").faces;
    assert!(matches!(
        rx.recv_timeout(WAIT),
        Ok(Event::LibraryChanged { updated, .. }) if updated.len() == family_faces.len()
            && updated.contains(&face)
    ));
    lib.api
        .update_user_meta(UserMetaUpdate {
            faces: vec![face],
            notes: Some("Use for headlines only".into()),
            ..UserMetaUpdate::default()
        })
        .expect("face notes");

    let check = |when: &str| {
        assert_eq!(
            lib.family("Barlow").notes.as_deref(),
            Some("Licensed for the 2026 rebrand"),
            "{when}"
        );
        assert_eq!(lib.family("Bebas Neue").notes, None, "{when}");
        let detail = lib.api.get_face(face).expect("detail");
        assert_eq!(
            detail.notes.as_deref(),
            Some("Use for headlines only"),
            "{when}"
        );
        assert_eq!(
            detail.family_notes.as_deref(),
            Some("Licensed for the 2026 rebrand"),
            "{when}"
        );
    };
    check("set");
    lib.scan(None, false);
    check("after a rescan");
    rebuild(&lib);
    check("after a rebuild");

    // Empty strings clear both.
    lib.api
        .update_family_notes(FamilyNotesUpdate {
            family: barlow.id,
            notes: String::new(),
        })
        .expect("cleared");
    lib.api
        .update_user_meta(UserMetaUpdate {
            faces: vec![face],
            notes: Some(String::new()),
            ..UserMetaUpdate::default()
        })
        .expect("cleared");
    assert_eq!(lib.family("Barlow").notes, None);
    let detail = lib.api.get_face(face).expect("detail");
    assert_eq!((detail.notes, detail.family_notes), (None, None));
    assert_eq!(
        lib.api
            .update_family_notes(FamilyNotesUpdate {
                family: FamilyId(9999),
                notes: "x".into(),
            })
            .map_err(|e| e.code),
        Err(ErrorCode::NotFound)
    );
}

/// E11.3: the coverage list comes from the stored `cmap`: its blocks agree
/// with the glyph map read from the file, in Unicode order; languages are
/// complete first, then partial by share, and carry `locl` from the scan.
#[test]
fn the_coverage_list_counts_blocks_and_languages_of_a_face() {
    let (lib, _, _) = Lib::scanned("api-face-coverage");
    let face_of = |name: &str| {
        lib.api
            .get_family(lib.family(name).id)
            .expect("family")
            .faces[0]
            .id
    };
    for name in ["Archivo", "Asap Sharp", "Bebas Neue"] {
        let face = face_of(name);
        let got = lib.api.face_coverage(face).expect("coverage");
        assert_eq!(got.face, face);
        let glyphs = lib
            .api
            .list_glyphs(GlyphsRequest {
                face,
                block: None,
                query: None,
                page: Page::default(),
            })
            .expect("glyphs");
        let from_file: Vec<(String, u32)> = glyphs
            .blocks
            .iter()
            .filter(|b| b.name != crate::glyphs::NO_BLOCK)
            .map(|b| (b.name.clone(), b.count))
            .collect();
        let listed: Vec<(String, u32)> = got
            .blocks
            .iter()
            .map(|b| (b.name.clone(), b.covered))
            .collect();
        assert_eq!(listed, from_file, "{name}: the blocks of the glyph map");
        assert!(
            got.blocks.windows(2).all(|w| w[0].last < w[1].first),
            "{name}: Unicode order"
        );
        assert!(
            got.blocks
                .iter()
                .all(|b| b.covered > 0 && b.covered <= b.assigned),
            "{name}"
        );
        assert_eq!(got.blocks[0].name, "Basic Latin");
        assert_eq!(got.blocks[0].assigned, 128);
        let complete = |l: &fm_api::search::FaceLanguageCoverage| l.covered == l.total;
        let first_partial = got
            .languages
            .iter()
            .position(|l| !complete(l))
            .unwrap_or(got.languages.len());
        assert!(first_partial > 0, "{name}: English is complete");
        assert!(
            got.languages[first_partial..].iter().all(|l| !complete(l)),
            "{name}: complete first"
        );
        let shares: Vec<f64> = got.languages[first_partial..]
            .iter()
            .map(|l| f64::from(l.covered) / f64::from(l.total))
            .collect();
        assert!(
            shares.windows(2).all(|w| w[0] >= w[1]),
            "{name}: {shares:?}"
        );
    }
    let bebas = lib.api.face_coverage(face_of("Bebas Neue")).expect("bebas");
    let vi = bebas.languages.iter().find(|l| l.code == "vi").expect("vi");
    assert!(
        vi.covered > 0 && vi.covered < vi.total,
        "Bebas Neue lacks Vietnamese tones: {vi:?}"
    );
    assert_eq!(vi.name, "Vietnamese");
    let asap = lib.api.face_coverage(face_of("Asap Sharp")).expect("asap");
    let ro = asap.languages.iter().find(|l| l.code == "ro").expect("ro");
    assert!(ro.has_locl && ro.covered == ro.total, "{ro:?}");
    let unknown = lib.api.face_coverage(FaceId(99_999));
    assert_eq!(unknown.expect_err("unknown face").code, ErrorCode::NotFound);
}

/// A [`Lib`] scanned, with a trash and an app-data receipts folder.
fn licensed(label: &str) -> (Lib, PathBuf, PathBuf) {
    let (lib, _, _) = Lib::scanned(label);
    let trash = lib.dir.path().join("trash");
    let receipts = lib.paths.data.join(crate::receipt::RECEIPTS_DIR);
    let lib = Lib {
        api: lib
            .api
            .with_trash(Arc::new(MockTrash::new(&trash)))
            .with_receipts_in(receipts.clone()),
        ..lib
    };
    (lib, trash, receipts)
}

/// A receipt the user picked, outside the library.
fn picked(lib: &Lib, name: &str, text: &str) -> PathBuf {
    let dir = lib.dir.path().join("Downloads");
    std::fs::create_dir_all(&dir).expect("downloads");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("receipt");
    path
}

/// The contents of every file in the mock trash, sorted.
fn trashed(trash: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(trash)
        .map(|dir| {
            dir.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_none_or(|x| x != "info"))
                .map(|p| std::fs::read_to_string(p).expect("trashed"))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn declaration(family: FamilyId) -> LicenseDeclaration {
    LicenseDeclaration {
        family,
        license_type: Some(LicenseType::Desktop),
        seats: Some(3),
        purchase_date: Some("2026-03-01".into()),
        receipt: ReceiptChange::Keep,
    }
}

#[test]
fn a_declared_license_survives_a_rescan_and_a_rebuild() {
    let (lib, _, _) = licensed("api-license");
    let barlow = lib.family("Barlow");
    let rx = lib.api.subscribe();
    lib.api
        .declare_license(declaration(barlow.id))
        .expect("declare");
    let family_faces = lib.api.get_family(barlow.id).expect("family").faces;
    assert!(matches!(
        rx.recv_timeout(WAIT),
        Ok(Event::LibraryChanged { updated, .. }) if updated.len() == family_faces.len()
    ));
    let check = |when: &str| {
        let barlow = lib.family("Barlow");
        assert_eq!(
            barlow.declared_license,
            Some(LicenseType::Desktop),
            "{when}"
        );
        assert_eq!(lib.family("Bebas Neue").declared_license, None, "{when}");
        for face in &lib.api.get_family(barlow.id).expect("family").faces {
            let license = lib.api.get_face(face.id).expect("detail").license;
            assert_eq!(license.declared, Some(LicenseType::Desktop), "{when}");
            assert_eq!(license.seats, Some(3), "{when}");
            assert_eq!(
                license.purchase_date.as_deref(),
                Some("2026-03-01"),
                "{when}"
            );
        }
    };
    check("declared");
    lib.scan(None, false);
    check("after a rescan");
    rebuild(&lib);
    check("after a rebuild");

    // Every field clears.
    lib.api
        .declare_license(LicenseDeclaration {
            family: lib.family("Barlow").id,
            license_type: None,
            seats: None,
            purchase_date: Some(String::new()),
            receipt: ReceiptChange::Keep,
        })
        .expect("cleared");
    let barlow = lib.family("Barlow");
    assert_eq!(barlow.declared_license, None);
    let license = lib
        .api
        .get_face(FaceId(barlow.id.0))
        .expect("detail")
        .license;
    assert_eq!(
        (
            license.declared,
            license.seats,
            license.purchase_date,
            license.receipt
        ),
        (None, None, None, None)
    );
    assert_eq!(
        lib.api
            .declare_license(declaration(FamilyId(9999)))
            .map_err(|e| e.code),
        Err(ErrorCode::NotFound)
    );
}

#[test]
fn a_receipt_is_copied_to_the_place_picked_and_a_replaced_copy_goes_to_the_trash() {
    let (lib, trash, receipts) = licensed("api-receipt");
    let barlow = lib.family("Barlow").id;
    let invoice = picked(&lib, "invoice.pdf", "first invoice");
    lib.api
        .declare_license(LicenseDeclaration {
            receipt: ReceiptChange::Attach {
                path: invoice.display().to_string(),
                place: ReceiptPlace::NextToFont,
            },
            ..declaration(barlow)
        })
        .expect("attach next to the font");
    let next_to_font = lib
        .root
        .join(crate::sidecar::SIDECAR_DIR)
        .join(crate::receipt::RECEIPTS_DIR)
        .join("invoice.pdf");
    let receipt = |lib: &Lib| {
        lib.api
            .get_face(FaceId(barlow.0))
            .expect("detail")
            .license
            .receipt
            .map(PathBuf::from)
    };
    assert_eq!(receipt(&lib), Some(next_to_font.clone()));
    assert_eq!(
        std::fs::read_to_string(&next_to_font).expect("copy"),
        "first invoice"
    );
    // Copied, never moved.
    assert_eq!(
        std::fs::read_to_string(&invoice).expect("picked"),
        "first invoice"
    );
    // The family's sidecar carries it, relative to its folder.
    let sidecar = std::fs::read_to_string(
        lib.root
            .join(crate::sidecar::SIDECAR_DIR)
            .join(crate::sidecar::file_name("Barlow")),
    )
    .expect("sidecar");
    assert!(
        sidecar.contains("\"receipt\": \"receipts/invoice.pdf\""),
        "{sidecar}"
    );

    // Changing the rest keeps the receipt.
    lib.api
        .declare_license(LicenseDeclaration {
            license_type: Some(LicenseType::Web),
            ..declaration(barlow)
        })
        .expect("keep");
    assert_eq!(receipt(&lib), Some(next_to_font.clone()));
    assert!(trashed(&trash).is_empty());

    // A new receipt in the data folder replaces it: the old copy goes to
    // the trash.
    let second = picked(&lib, "invoice-2026.pdf", "second invoice");
    lib.api
        .declare_license(LicenseDeclaration {
            receipt: ReceiptChange::Attach {
                path: second.display().to_string(),
                place: ReceiptPlace::AppData,
            },
            ..declaration(barlow)
        })
        .expect("attach in app data");
    let in_app_data = receipts.join("invoice-2026.pdf");
    assert_eq!(receipt(&lib), Some(in_app_data.clone()));
    assert!(!next_to_font.exists());
    assert_eq!(trashed(&trash), ["first invoice"]);

    // Clearing moves the copy to the trash too; the picked files stay.
    lib.api
        .declare_license(LicenseDeclaration {
            receipt: ReceiptChange::Clear,
            ..declaration(barlow)
        })
        .expect("clear");
    assert_eq!(receipt(&lib), None);
    assert!(!in_app_data.exists());
    assert_eq!(trashed(&trash), ["first invoice", "second invoice"]);
    assert!(invoice.exists() && second.exists());
}

#[test]
fn a_bad_date_negative_seats_or_a_missing_receipt_is_an_invalid_request() {
    let (lib, _, receipts) = licensed("api-license-invalid");
    let barlow = lib.family("Barlow").id;
    let missing = lib.dir.path().join("Downloads").join("nope.pdf");
    let attempts = [
        LicenseDeclaration {
            purchase_date: Some("2026-02-30".into()),
            ..declaration(barlow)
        },
        LicenseDeclaration {
            purchase_date: Some("March 1st".into()),
            ..declaration(barlow)
        },
        LicenseDeclaration {
            seats: Some(-1),
            ..declaration(barlow)
        },
        LicenseDeclaration {
            receipt: ReceiptChange::Attach {
                path: missing.display().to_string(),
                place: ReceiptPlace::AppData,
            },
            ..declaration(barlow)
        },
    ];
    for req in attempts {
        let err = lib.api.declare_license(req.clone()).expect_err("invalid");
        assert_eq!(err.code, ErrorCode::InvalidRequest, "{req:?}");
        assert!(!err.message.is_empty());
    }
    // Nothing was stored or copied.
    assert_eq!(lib.family("Barlow").declared_license, None);
    assert!(!receipts.exists());
}

#[test]
fn a_receipt_whose_old_copy_cannot_be_trashed_changes_nothing() {
    let (lib, _, receipts) = licensed("api-receipt-refused");
    let trash = Arc::new(MockTrash::new(lib.dir.path().join("trash")));
    let lib = Lib {
        api: lib.api.with_trash(Arc::clone(&trash) as Arc<dyn Trash>),
        ..lib
    };
    let barlow = lib.family("Barlow").id;
    let first = picked(&lib, "a.pdf", "a");
    lib.api
        .declare_license(LicenseDeclaration {
            receipt: ReceiptChange::Attach {
                path: first.display().to_string(),
                place: ReceiptPlace::AppData,
            },
            ..declaration(barlow)
        })
        .expect("attach");
    let kept = receipts.join("a.pdf");
    trash.refuse(&kept);
    let second = picked(&lib, "b.pdf", "b");
    let err = lib
        .api
        .declare_license(LicenseDeclaration {
            license_type: Some(LicenseType::Trial),
            receipt: ReceiptChange::Attach {
                path: second.display().to_string(),
                place: ReceiptPlace::AppData,
            },
            ..declaration(barlow)
        })
        .expect_err("refused");
    assert!(err.message.contains("trash"), "{err:?}");
    let license = lib.api.get_face(FaceId(barlow.0)).expect("detail").license;
    assert_eq!(license.declared, Some(LicenseType::Desktop));
    assert_eq!(license.receipt.map(PathBuf::from), Some(kept.clone()));
    assert!(kept.exists());
    // The copy just made went to the trash, not left behind.
    assert!(!receipts.join("b.pdf").exists());
    assert!(second.exists());
}

#[test]
fn a_receipt_cannot_be_kept_next_to_a_font_in_a_folder_that_is_not_there() {
    let (lib, _, _) = licensed("api-receipt-offline");
    let barlow = lib.family("Barlow").id;
    let invoice = picked(&lib, "invoice.pdf", "x");
    // The library folder goes offline (renamed away): the index keeps it.
    let away = lib.dir.path().join("Library-offline");
    std::fs::rename(&lib.root, &away).expect("offline");
    let err = lib
        .api
        .declare_license(LicenseDeclaration {
            receipt: ReceiptChange::Attach {
                path: invoice.display().to_string(),
                place: ReceiptPlace::NextToFont,
            },
            ..declaration(barlow)
        })
        .expect_err("offline");
    std::fs::rename(&away, &lib.root).expect("back");
    assert_eq!(err.code, ErrorCode::InvalidRequest);
    assert!(err.message.contains("data folder"), "{err:?}");
}

#[test]
fn layouts_are_the_core_presets_and_come_back_clamped() {
    let lib = Lib::new("api-layout");
    assert_eq!(
        lib.api.layout_presets().expect("presets"),
        layout::presets()
    );
    let mut wild = layout::preset(fm_api::layout::LayoutPreset::Quote);
    wild.blocks[0].size_pt = 2_000.0;
    wild.blocks[1].tracking = f32::NAN;
    let got = lib.api.clamp_layout(wild).expect("clamp");
    assert_eq!(got.blocks[0].size_pt, 600.0);
    assert_eq!(got.blocks[1].tracking, 0.0);
    assert_eq!(lib.api.layout_bounds().expect("bounds"), layout::bounds());
}

#[test]
fn layout_bounds_are_the_core_ranges_and_the_mock_matches_them() {
    let lib = Lib::new("api-layout-bounds");
    let bounds = lib.api.layout_bounds().expect("bounds");
    assert_eq!(bounds, layout::bounds());
    assert_eq!(bounds.size_pt.max, 600.0);
    // The UI's MockApi (browser mode, e2e) must offer the same ranges as the core.
    let mock = fm_api::mock::MockApi::new();
    assert_eq!(mock.layout_bounds().expect("mock bounds"), bounds);
}

#[test]
fn layout_width_is_the_core_width_and_the_mock_matches_it() {
    let lib = Lib::new("api-layout-width");
    let mock = fm_api::mock::MockApi::new();
    let presets = lib.api.layout_presets().expect("presets");
    // The UI's MockApi (browser mode, e2e) must lay out at the same widths as the core.
    for p in &presets {
        let want = layout::layout_width(p);
        assert!(want.is_some(), "{} starts locked", p.name);
        assert_eq!(lib.api.layout_width(p.clone()).expect("width"), want);
        let got = mock.layout_width(p.clone()).expect("mock width");
        assert!((got.unwrap() - want.unwrap()).abs() < 0.01, "{}", p.name);
    }
    let mut edge = presets[0].clone();
    for w in [None, Some(5.0), Some(900.0), Some(1.0e9), Some(f32::NAN)] {
        edge.width_px = w;
        assert_eq!(
            mock.clamp_layout(edge.clone()).expect("mock").width_px,
            lib.api.clamp_layout(edge.clone()).expect("core").width_px,
            "{w:?}"
        );
        assert_eq!(
            mock.layout_width(edge.clone()).expect("mock"),
            lib.api.layout_width(edge.clone()).expect("core"),
            "{w:?}"
        );
    }
    edge.width_locked = false;
    assert_eq!(lib.api.layout_width(edge.clone()).expect("core"), None);
    assert_eq!(mock.layout_width(edge).expect("mock"), None);
}

#[test]
fn a_locked_width_stays_with_the_saved_layout_file() {
    use fm_api::layout::{LayoutRef, SaveLayoutRequest};
    let lib = Lib::new("api-layout-width-file");
    let mut book = lib
        .api
        .get_layout(LayoutRef { id: "book".into() })
        .expect("book");
    assert!(book.layout.width_locked);
    assert_eq!(book.layout.width_px, None);
    book.layout.width_px = Some(812.0);
    lib.api
        .save_layout(SaveLayoutRequest {
            id: "book".into(),
            layout: book.layout.clone(),
        })
        .expect("save");
    let again = lib
        .api
        .get_layout(LayoutRef { id: "book".into() })
        .expect("read");
    assert!(again.layout.width_locked);
    assert_eq!(again.layout.width_px, Some(812.0));
    book.layout.width_locked = false;
    lib.api
        .save_layout(SaveLayoutRequest {
            id: "book".into(),
            layout: book.layout,
        })
        .expect("save unlocked");
    let unlocked = lib
        .api
        .get_layout(LayoutRef { id: "book".into() })
        .expect("read");
    assert!(!unlocked.layout.width_locked);
    assert_eq!(unlocked.layout.width_px, Some(812.0));
    let raw = std::fs::read_to_string(lib.paths.layouts_dir().join("book.json")).expect("file");
    assert!(
        raw.contains("\"widthLocked\": false") || raw.contains("\"widthLocked\":false"),
        "{raw}"
    );
}

#[test]
fn layout_files_live_in_the_app_layouts_folder_through_the_api() {
    use fm_api::layout::{
        DuplicateLayoutRequest, LayoutRef, RestoreLayoutRequest, RevertLayoutRequest,
        SaveLayoutRequest,
    };
    let lib = Lib::new("api-layout-files");
    let listed = lib.api.list_layouts().expect("list");
    assert_eq!(listed.len(), 6);
    assert!(lib.paths.layouts_dir().join("book.json").is_file());

    let book = lib
        .api
        .get_layout(LayoutRef { id: "book".into() })
        .expect("get")
        .layout;
    let mut bigger = book.clone();
    bigger.blocks[0].size_pt = 9_000.0;
    let saved = lib
        .api
        .save_layout(SaveLayoutRequest {
            id: "book".into(),
            layout: bigger,
        })
        .expect("save");
    assert_eq!(
        saved.layout.blocks[0].size_pt, 600.0,
        "clamped before writing"
    );
    let history = lib
        .api
        .layout_history(LayoutRef { id: "book".into() })
        .expect("history");
    assert_eq!(history.len(), 1);
    let restored = lib
        .api
        .restore_layout(RestoreLayoutRequest {
            id: "book".into(),
            number: history[0].number,
        })
        .expect("restore");
    assert_eq!(restored.layout, book);

    let copy = lib
        .api
        .duplicate_layout(DuplicateLayoutRequest {
            id: "book".into(),
            name: None,
        })
        .expect("duplicate");
    assert!(
        lib.paths
            .layouts_dir()
            .join(format!("{}.json", copy.id))
            .is_file()
    );
    let reset = lib
        .api
        .revert_layout(RevertLayoutRequest {
            id: copy.id,
            to_original: true,
        })
        .expect("reset");
    assert_eq!(
        reset.layout.blocks,
        layout::preset(fm_api::layout::LayoutPreset::Book).blocks
    );
}

// --- license inventory (E15.4) -------------------------------------------

/// The inventory of `collection` read back: the header, then one map per
/// family row from column name to cell.
fn inventory(lib: &Lib, collection: Option<CollectionId>) -> Vec<HashMap<String, String>> {
    let inventory = lib
        .api
        .license_inventory(LicenseInventoryRequest { collection })
        .expect("inventory");
    let records = crate::inventory::tests::parse(&inventory.csv);
    assert_eq!(records[0], crate::inventory::HEADER);
    let rows: Vec<HashMap<String, String>> = records[1..]
        .iter()
        .map(|r| {
            assert_eq!(r.len(), crate::inventory::HEADER.len(), "{r:?}");
            crate::inventory::HEADER
                .iter()
                .map(|h| (*h).to_owned())
                .zip(r.iter().cloned())
                .collect()
        })
        .collect();
    assert_eq!(inventory.rows as usize, rows.len());
    rows
}

#[test]
fn the_license_inventory_lists_every_family_with_its_license_facts() {
    let (lib, _, _) = licensed("api-inventory");
    let barlow = lib.family("Barlow");
    // A receipt whose name needs quoting in a CSV cell.
    let invoice = picked(&lib, "invoice, \u{201c}Barlow\u{201d}.pdf", "paid");
    lib.api
        .declare_license(LicenseDeclaration {
            receipt: ReceiptChange::Attach {
                path: invoice.display().to_string(),
                place: ReceiptPlace::NextToFont,
            },
            ..declaration(barlow.id)
        })
        .expect("declare");

    let rows = inventory(&lib, None);
    let families = lib
        .api
        .list_families(ListFamiliesRequest {
            page: Page {
                offset: 0,
                limit: 1000,
            },
            ..ListFamiliesRequest::default()
        })
        .expect("families");
    assert_eq!(rows.len() as u32, families.total);
    let mut names: Vec<&str> = rows.iter().map(|r| r["Family"].as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_by_key(|n| n.to_lowercase());
    assert_eq!(names, sorted, "A to Z");
    names.sort_unstable();
    let mut listed: Vec<&str> = families.items.iter().map(|f| f.name.as_str()).collect();
    listed.sort_unstable();
    assert_eq!(names, listed);

    for family in &families.items {
        let row = rows
            .iter()
            .find(|r| r["Family"] == family.name)
            .expect("listed");
        assert_eq!(
            row["Styles"],
            family.style_count.to_string(),
            "{}",
            family.name
        );
        assert_eq!(row["Foundry"], family.foundry.clone().unwrap_or_default());
        assert_eq!(
            row["Source folder"],
            family.location.clone().unwrap_or_default()
        );
        assert!(!row["Formats"].is_empty(), "{}", family.name);
        assert!(!row["Embedding"].is_empty(), "{}", family.name);
        let first = lib.api.get_face(FaceId(family.id.0)).expect("detail");
        assert!(
            row["Embedding"].contains(match first.license.embedding {
                Embedding::Installable => "Installable",
                Embedding::Editable => "Editable",
                Embedding::PreviewAndPrint => "Preview and print",
                Embedding::Restricted => "Restricted",
            }),
            "{}: {}",
            family.name,
            row["Embedding"]
        );
        if family.open_license == Some(OpenLicense::Ofl) {
            assert_eq!(row["Open license"], "SIL Open Font License");
        }
    }

    let barlow_row = rows
        .iter()
        .find(|r| r["Family"] == "Barlow")
        .expect("Barlow");
    let detail = lib
        .api
        .get_face(FaceId(barlow.id.0))
        .expect("detail")
        .license;
    assert_eq!(barlow_row["Declared license"], "Desktop");
    assert_eq!(barlow_row["Seats"], "3");
    assert_eq!(barlow_row["Purchase date"], "2026-03-01");
    assert_eq!(Some(barlow_row["Receipt"].clone()), detail.receipt);
    assert!(barlow_row["Receipt"].ends_with("invoice, \u{201c}Barlow\u{201d}.pdf"));
    assert_eq!(barlow_row["License URL"], detail.url.unwrap_or_default());
    assert_eq!(barlow_row["Formats"], "TTF");
    let bebas = rows
        .iter()
        .find(|r| r["Family"] == "Bebas Neue")
        .expect("Bebas");
    assert_eq!(
        (
            bebas["Declared license"].as_str(),
            bebas["Seats"].as_str(),
            bebas["Receipt"].as_str()
        ),
        ("", "", "")
    );
}

#[test]
fn the_license_inventory_of_a_collection_lists_only_its_families() {
    let (lib, _, _) = Lib::scanned("api-inventory-collection");
    let barlow = lib.family("Barlow");
    let manual = lib
        .api
        .create_collection(CreateCollectionRequest {
            name: "Headlines".into(),
            parent: None,
            rules: None,
            rule_group: None,
        })
        .expect("collection");
    let names = |c| -> Vec<String> {
        inventory(&lib, Some(c))
            .into_iter()
            .map(|r| r["Family"].clone())
            .collect()
    };
    assert!(names(manual.id).is_empty(), "an empty collection");
    // One face of the family: the family is listed, with all its styles.
    lib.api
        .add_to_collection(CollectionFacesRequest {
            collection: manual.id,
            faces: vec![FaceId(barlow.id.0)],
            families: Vec::new(),
        })
        .expect("added");
    let rows = inventory(&lib, Some(manual.id));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["Family"], "Barlow");
    assert_eq!(rows[0]["Styles"], barlow.style_count.to_string());

    let smart = lib
        .api
        .create_collection(CreateCollectionRequest {
            name: "Variable".into(),
            parent: None,
            rules: Some(Filters {
                variable: Some(true),
                ..Filters::default()
            }),
            rule_group: None,
        })
        .expect("smart");
    let mut variable: Vec<String> = lib
        .api
        .search(SearchRequest {
            query: String::new(),
            filters: Filters {
                variable: Some(true),
                ..Filters::default()
            },
            page: Page {
                offset: 0,
                limit: 1000,
            },
            rule: None,
        })
        .expect("search")
        .families
        .into_iter()
        .map(|h| h.family.name)
        .collect();
    variable.sort_by_key(|n| n.to_lowercase());
    assert!(!variable.is_empty(), "the fixtures have a variable family");
    assert_eq!(names(smart.id), variable);
    assert!(names(smart.id).len() < inventory(&lib, None).len());

    assert_eq!(
        lib.api
            .license_inventory(LicenseInventoryRequest {
                collection: Some(CollectionId(9999)),
            })
            .map_err(|e| e.code),
        Err(ErrorCode::NotFound)
    );
}

#[test]
fn the_license_inventory_of_an_empty_library_is_the_header_alone() {
    let lib = Lib::new("api-inventory-empty");
    assert!(inventory(&lib, None).is_empty());
}

/// E16.2 criteria 1 and 2: the scan stores each face's x-height and cap
/// height, so `get_face` and `size_factors` answer with the files gone; one
/// `size_factors` call answers many faces, in order, with the factor the
/// stored metrics give; a face without metrics keeps factor 1.
#[test]
fn metrics_are_stored_at_scan_and_size_factors_answer_many_faces() {
    use fm_api::preview::{SizeBasis, SizeFactorRequest};

    let (lib, _, _) = Lib::scanned("api-size-factors");
    let families = lib
        .api
        .list_families(ListFamiliesRequest {
            page: Page {
                offset: 0,
                limit: 1000,
            },
            ..ListFamiliesRequest::default()
        })
        .expect("families")
        .items;
    let faces: Vec<FaceId> = families
        .iter()
        .flat_map(|f| lib.api.get_family(f.id).expect("family").faces)
        .map(|f| f.id)
        .collect();
    assert!(faces.len() >= 6, "{faces:?}");
    let before: Vec<_> = faces
        .iter()
        .map(|&f| lib.api.get_face(f).expect("face").metrics)
        .collect();
    std::fs::remove_dir_all(&lib.root).expect("remove the library folder");

    let mut scaled = 0;
    for basis in [SizeBasis::XHeight, SizeBasis::CapHeight] {
        let got = lib
            .api
            .size_factors(SizeFactorRequest {
                faces: faces.clone(),
                basis,
            })
            .expect("factors");
        assert_eq!(got.iter().map(|f| f.face).collect::<Vec<_>>(), faces);
        for ((&face, m), factor) in faces.iter().zip(&before).zip(&got) {
            let detail = lib.api.get_face(face).expect("face from the index");
            assert_eq!(&detail.metrics, m, "{face}: stored metrics");
            assert_eq!(factor.factor, crate::metrics::size_factor(m, basis));
            if factor.factor != 1.0 {
                scaled += 1;
            }
        }
    }
    assert!(scaled > 0, "some face is scaled");
    let bebas = lib
        .api
        .get_face(FaceId(lib.family("Bebas Neue").id.0))
        .expect("Bebas");
    assert!(
        bebas.metrics.cap_height.is_some_and(|c| c > 0),
        "{:?}",
        bebas.metrics
    );

    let err = lib
        .api
        .size_factors(SizeFactorRequest {
            faces: vec![faces[0], FaceId(u32::MAX)],
            basis: SizeBasis::XHeight,
        })
        .expect_err("unknown face");
    assert_eq!(err.code, ErrorCode::NotFound);
}

// --- comparisons ---------------------------------------------------------

/// The first face of the family `name`.
fn first_face(lib: &Lib, name: &str) -> fm_api::library::FaceSummary {
    let family = lib.family(name);
    lib.api
        .get_family(family.id)
        .expect("family")
        .faces
        .into_iter()
        .next()
        .expect("a face")
}

/// E13.2: a named comparison of real fonts survives a rescan, a rebuild and
/// a restart; when a font's file is removed, reopening it gives the faces
/// left and names the missing one.
#[test]
fn named_comparisons_survive_a_rescan_a_restart_and_a_removed_font() {
    use fm_api::comparison::{CompareMode, ComparisonRef, MissingFace, SaveComparisonRequest};

    let (lib, source, _) = Lib::scanned("api-comparisons");
    let picked: Vec<_> = ["Bebas Neue", "Barlow", "Agdasima"]
        .iter()
        .map(|name| first_face(&lib, name))
        .collect();
    let ids: Vec<_> = picked.iter().map(|f| f.id).collect();
    let saved = lib
        .api
        .save_comparison(SaveComparisonRequest {
            id: None,
            name: "Display options".into(),
            faces: ids.clone(),
            mode: CompareMode::Overlay,
            sample_text: "Ångström 2026".into(),
        })
        .expect("save");
    assert_eq!(saved.faces.iter().map(|f| f.id).collect::<Vec<_>>(), ids);
    let reopen = |api: &CoreApi| {
        api.get_comparison(ComparisonRef { id: saved.id })
            .expect("reopen")
    };

    lib.scan(None, false);
    assert_eq!(reopen(&lib.api), saved, "after a rescan");
    rebuild(&lib);
    assert_eq!(
        reopen(&lib.api)
            .faces
            .iter()
            .map(|f| f.id)
            .collect::<Vec<_>>(),
        ids,
        "after a rebuild"
    );

    // A restart: a new core over the same index file.
    let index = Index::open(&lib.paths.data.join(INDEX_FILE))
        .expect("index")
        .index;
    let restarted = CoreApi::new(
        index,
        Arc::clone(&lib.activator) as Arc<dyn FontActivator>,
        SettingsStore::new(&lib.paths),
        None,
    );
    let after = reopen(&restarted);
    assert_eq!(after.name, "Display options");
    assert_eq!(after.mode, CompareMode::Overlay);
    assert_eq!(after.sample_text, "Ångström 2026");
    assert_eq!(after.faces.iter().map(|f| f.id).collect::<Vec<_>>(), ids);
    assert_eq!(restarted.list_comparisons().expect("list").len(), 1);
    drop(restarted);

    // Bebas Neue's file is removed from the folder and the folder rescanned.
    std::fs::remove_dir_all(lib.root.join("bebasneue")).expect("remove");
    lib.scan(Some(source.id), true);
    let left = reopen(&lib.api);
    assert_eq!(
        left.faces.iter().map(|f| f.id).collect::<Vec<_>>(),
        ids[1..],
        "the faces left, in order"
    );
    assert_eq!(
        left.missing,
        [MissingFace {
            id: ids[0],
            name: format!("{} {}", picked[0].family_name, picked[0].style),
        }]
    );
}

fn specimen_request(family: FamilyId, out: &Path) -> SpecimenRequest {
    SpecimenRequest {
        family,
        format: SpecimenFormat::Html,
        out_path: out.display().to_string(),
    }
}

/// Every `data:` URI of `page`'s `@font-face` rules, decoded, in order.
fn embedded_fonts(page: &str) -> Vec<(String, Vec<u8>)> {
    page.split("src: url(\"data:")
        .skip(1)
        .map(|rest| {
            let uri = &rest[..rest.find('"').expect("end of the URI")];
            let (mime, data) = uri.split_once(";base64,").expect("base64 URI");
            (mime.to_owned(), decode_base64(data))
        })
        .collect()
}

fn decode_base64(text: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => panic!("not base64: {c}"),
    };
    assert_eq!(text.len() % 4, 0);
    let mut out = Vec::new();
    for quad in text.as_bytes().chunks(4) {
        let pad = quad.iter().rev().take_while(|&&c| c == b'=').count();
        let n = quad[..4 - pad]
            .iter()
            .fold(0u32, |n, &c| n << 6 | u32::from(value(c)))
            << (6 * pad);
        out.extend_from_slice(&[(n >> 16) as u8, (n >> 8) as u8, n as u8][..3 - pad]);
    }
    out
}

#[test]
fn a_family_exports_as_a_self_contained_html_specimen() {
    let (lib, _, _) = Lib::scanned("api-specimen-html");
    let out_dir = lib.dir.path().join("Specimens ü");
    std::fs::create_dir_all(&out_dir).expect("out");
    for name in ["Archivo", "Asap Sharp"] {
        let family = lib.api.get_family(lib.family(name).id).expect("family");
        let out = out_dir.join(format!("{name}.html"));
        let res = lib
            .api
            .export_specimen(specimen_request(family.summary.id, &out))
            .expect("specimen");
        assert_eq!(Path::new(&res.path), out);
        let page = std::fs::read_to_string(&out).expect("page");
        // Self-contained: nothing loaded from anywhere, no script.
        let lower = page.to_lowercase();
        for bad in ["http:", "https:", "file:", "<script", "javascript"] {
            assert!(!lower.contains(bad), "{name}: {bad}");
        }
        // Every face is embedded, in the family's order, and decoding its
        // data URI gives the face's file bytes.
        let fonts = embedded_fonts(&page);
        assert_eq!(fonts.len(), family.faces.len(), "{name}");
        for ((mime, bytes), face) in fonts.iter().zip(&family.faces) {
            let detail = lib.api.get_face(face.id).expect("face");
            let file = std::fs::read(&detail.path).expect("font file");
            assert!(
                *bytes == file,
                "{name} {}: embedded bytes differ",
                face.style
            );
            assert_eq!(
                Some(mime.as_str()),
                crate::font_file::font_mime(&file),
                "{name}"
            );
        }
        assert!(page.contains("<h1 class=\"f"));
        assert!(page.contains("<h2>Waterfall</h2>"));
        assert!(page.contains("<h3>Basic Latin</h3>"));
        // The same family gives byte-identical HTML.
        let again = out_dir.join(format!("{name} again.html"));
        lib.api
            .export_specimen(specimen_request(family.summary.id, &again))
            .expect("again");
        assert!(
            std::fs::read(&again).expect("again") == page.as_bytes(),
            "{name}"
        );
    }
    // A variable family shows its named instances as styles.
    let asap = lib
        .api
        .get_family(lib.family("Asap Sharp").id)
        .expect("asap");
    let detail = lib.api.get_face(asap.faces[0].id).expect("face");
    assert!(!detail.instances.is_empty());
    let page = std::fs::read_to_string(out_dir.join("Asap Sharp.html")).expect("page");
    for inst in &detail.instances {
        assert!(
            page.contains(&format!(
                "<p class=\"label\">{}</p>",
                specimen::escape(&inst.name)
            )),
            "{}",
            inst.name
        );
    }
    assert!(page.contains("font-variation-settings: &quot;wght&quot;"));
}

#[test]
fn a_specimen_creates_a_missing_output_folder() {
    let (lib, _, _) = Lib::scanned("api-specimen-new-folder");
    let out = lib
        .dir
        .path()
        .join("specimens")
        .join("archivo")
        .join("out.html");
    let archivo = lib.family("Archivo").id;
    let got = lib
        .api
        .export_specimen(specimen_request(archivo, &out))
        .expect("specimen into a new folder");
    assert!(out.is_file(), "{}", got.path);
}

#[test]
fn a_specimen_refuses_an_unknown_family_a_pdf_and_no_path() {
    let (lib, _, _) = Lib::scanned("api-specimen-refusals");
    let out = lib.dir.path().join("out.html");
    let e = lib
        .api
        .export_specimen(specimen_request(FamilyId(999_999), &out))
        .expect_err("unknown");
    assert_eq!(e.code, ErrorCode::NotFound);
    let archivo = lib.family("Archivo").id;
    let e = lib
        .api
        .export_specimen(SpecimenRequest {
            format: SpecimenFormat::Pdf,
            ..specimen_request(archivo, &out)
        })
        .expect_err("pdf");
    assert_eq!(e.code, ErrorCode::Unsupported);
    assert_eq!(e.message, "PDF export needs the Font Manager app running.");
    let e = lib
        .api
        .export_specimen(SpecimenRequest {
            out_path: "  ".into(),
            ..specimen_request(archivo, &out)
        })
        .expect_err("no path");
    assert_eq!(e.code, ErrorCode::InvalidRequest);
    assert!(!out.exists());
}

#[test]
fn the_specimen_guard_refuses_restricted_embedding_with_no_license() {
    let lib = Lib::new("api-specimen-guard");
    let folder = lib.dir.path().join("Patched");
    with_fs_type(
        "google-repo/bebasneue/BebasNeue-Regular.ttf",
        0x0002,
        &folder,
    );
    with_fs_type("google-repo/asul/Asul-Bold.ttf", 0x0004, &folder);
    let source = lib
        .api
        .add_source(AddSourceRequest {
            path: folder.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    lib.scan(Some(source.id), true);
    let out = lib.dir.path().join("specimen.html");
    let bebas = lib.family("Bebas Neue").id;
    let bebas_face = lib.api.get_family(bebas).expect("family").faces[0].id;
    let e = lib
        .api
        .export_specimen(specimen_request(bebas, &out))
        .expect_err("restricted");
    assert_eq!(e.code, ErrorCode::PermissionDenied, "{e:?}");
    assert_eq!(
        e.message,
        "Bebas Neue Regular: Embedding restricted and no license declared, so the font \
         may not be embedded in a specimen. Declare a license for the family to export it."
    );
    assert_eq!(e.detail, Some(format!("blocked faces: {}", bebas_face.0)));
    assert!(!out.exists());

    // Preview & print embeds: a specimen is a preview.
    lib.api
        .export_specimen(specimen_request(lib.family("Asul").id, &out))
        .expect("preview & print");
    assert!(out.exists());
    std::fs::remove_file(&out).expect("clean up");

    // Declaring a license for the family, any type, lets it through.
    lib.api
        .declare_license(LicenseDeclaration {
            family: bebas,
            license_type: Some(LicenseType::Desktop),
            seats: None,
            purchase_date: None,
            receipt: ReceiptChange::Keep,
        })
        .expect("declare");
    lib.api
        .export_specimen(specimen_request(bebas, &out))
        .expect("declared");
    assert!(out.exists());
}

/// What [`FakePrinter`] does with a print.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PrintOutcome {
    /// Writes a small PDF.
    Pdf,
    /// Writes half a file, then fails.
    FailAfterWriting,
    /// Reports success but writes something that is not a PDF.
    NotAPdf,
}

/// A printer that records the page it was handed and the files it wrote.
#[derive(Debug)]
struct FakePrinter {
    outcome: PrintOutcome,
    /// (html path, html text, partial path, printing thread) per print.
    calls: Mutex<Vec<(PathBuf, String, PathBuf, std::thread::ThreadId)>>,
}

impl FakePrinter {
    fn new(outcome: PrintOutcome) -> Arc<Self> {
        Arc::new(Self {
            outcome,
            calls: Mutex::new(Vec::new()),
        })
    }

    fn calls(&self) -> Vec<(PathBuf, String, PathBuf, std::thread::ThreadId)> {
        self.calls.lock().expect("calls").clone()
    }
}

impl PdfPrinter for FakePrinter {
    fn print_html(&self, html: &Path, out: &Path) -> Result<(), fm_platform::PrintError> {
        let text = std::fs::read_to_string(html).expect("the page is there while printing");
        self.calls.lock().expect("calls").push((
            html.to_path_buf(),
            text,
            out.to_path_buf(),
            std::thread::current().id(),
        ));
        match self.outcome {
            PrintOutcome::Pdf => {
                std::fs::write(
                    out,
                    b"%PDF-1.7
%fake
",
                )
                .expect("write");
                Ok(())
            }
            PrintOutcome::FailAfterWriting => {
                std::fs::write(
                    out,
                    b"%PDF-1.7
1 0 obj",
                )
                .expect("write");
                Err(fm_platform::PrintError::new("the webview crashed"))
            }
            PrintOutcome::NotAPdf => {
                std::fs::write(out, b"<html>").expect("write");
                Ok(())
            }
        }
    }
}

fn pdf_request(family: FamilyId, out: &Path) -> SpecimenRequest {
    SpecimenRequest {
        format: SpecimenFormat::Pdf,
        ..specimen_request(family, out)
    }
}

/// The names in `dir`, sorted.
fn dir_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .expect("dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_pdf_specimen_prints_the_html_page_through_the_injected_printer() {
    let (lib, _, _) = Lib::scanned("api-specimen-pdf");
    let printer = FakePrinter::new(PrintOutcome::Pdf);
    let lib = Lib {
        api: lib
            .api
            .with_pdf_printer(Arc::clone(&printer) as Arc<dyn PdfPrinter>),
        ..lib
    };
    let out_dir = lib.dir.path().join("Specimens ü");
    std::fs::create_dir_all(&out_dir).expect("out");
    let out = out_dir.join("Archivo.pdf");
    let archivo = lib.family("Archivo").id;
    let res = lib
        .api
        .export_specimen(pdf_request(archivo, &out))
        .expect("pdf");
    assert_eq!(Path::new(&res.path), out);
    assert!(std::fs::read(&out).expect("pdf").starts_with(b"%PDF-"));

    let calls = printer.calls();
    assert_eq!(calls.len(), 1);
    let (html, page, partial, thread) = &calls[0];
    // The printer was handed the E21.1 page, the faces embedded.
    let mut html_req = specimen_request(archivo, &out_dir.join("Archivo.html"));
    html_req.format = SpecimenFormat::Html;
    lib.api.export_specimen(html_req).expect("html");
    let same = std::fs::read_to_string(out_dir.join("Archivo.html")).expect("html");
    assert_eq!(page, &same, "the PDF prints the HTML specimen");
    assert!(!embedded_fonts(page).is_empty());
    // On the caller's thread; the source HTML is removed afterwards.
    assert_eq!(*thread, std::thread::current().id());
    assert!(!html.exists(), "the temporary page is removed");
    // The print went to a hidden partial file next to out_path, now renamed.
    assert_eq!(partial.parent(), Some(out_dir.as_path()));
    assert_ne!(partial, &out);
    assert!(!partial.exists());
    assert_eq!(dir_names(&out_dir), ["Archivo.html", "Archivo.pdf"]);
}

#[test]
fn a_failed_pdf_specimen_print_leaves_no_partial_file_and_no_temporary_page() {
    for outcome in [PrintOutcome::FailAfterWriting, PrintOutcome::NotAPdf] {
        let (lib, _, _) = Lib::scanned("api-specimen-pdf-fail");
        let printer = FakePrinter::new(outcome);
        let lib = Lib {
            api: lib
                .api
                .with_pdf_printer(Arc::clone(&printer) as Arc<dyn PdfPrinter>),
            ..lib
        };
        let out_dir = lib.dir.path().join("Out");
        std::fs::create_dir_all(&out_dir).expect("out");
        let out = out_dir.join("Archivo.pdf");
        let e = lib
            .api
            .export_specimen(pdf_request(lib.family("Archivo").id, &out))
            .expect_err("print fails");
        assert_eq!(e.code, ErrorCode::Io, "{outcome:?}");
        assert_eq!(e.message, "The PDF specimen could not be printed.");
        assert!(!out.exists(), "{outcome:?}: nothing at out_path");
        assert_eq!(dir_names(&out_dir), Vec::<String>::new(), "{outcome:?}");
        let calls = printer.calls();
        assert_eq!(calls.len(), 1);
        assert!(
            !calls[0].0.exists(),
            "{outcome:?}: the temporary page is removed"
        );

        // A file already at out_path is left as it was.
        std::fs::write(&out, b"the user's old file").expect("old");
        lib.api
            .export_specimen(pdf_request(lib.family("Archivo").id, &out))
            .expect_err("print fails again");
        assert_eq!(std::fs::read(&out).expect("old"), b"the user's old file");
        assert_eq!(dir_names(&out_dir), ["Archivo.pdf"]);
    }
}

#[test]
fn a_pdf_specimen_keeps_the_html_checks_and_never_prints_a_refused_family() {
    let lib = Lib::new("api-specimen-pdf-guard");
    let folder = lib.dir.path().join("Patched");
    with_fs_type(
        "google-repo/bebasneue/BebasNeue-Regular.ttf",
        0x0002,
        &folder,
    );
    let source = lib
        .api
        .add_source(AddSourceRequest {
            path: folder.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    lib.scan(Some(source.id), true);
    let printer = FakePrinter::new(PrintOutcome::Pdf);
    let lib = Lib {
        api: lib
            .api
            .with_pdf_printer(Arc::clone(&printer) as Arc<dyn PdfPrinter>),
        ..lib
    };
    let out = lib.dir.path().join("specimen.pdf");
    let e = lib
        .api
        .export_specimen(pdf_request(lib.family("Bebas Neue").id, &out))
        .expect_err("restricted");
    assert_eq!(e.code, ErrorCode::PermissionDenied);
    let e = lib
        .api
        .export_specimen(pdf_request(FamilyId(999_999), &out))
        .expect_err("unknown");
    assert_eq!(e.code, ErrorCode::NotFound);
    assert!(printer.calls().is_empty(), "nothing was printed");
    assert!(!out.exists());
}

fn collection_request(collection: CollectionId, out: &Path) -> CollectionSpecimenRequest {
    CollectionSpecimenRequest {
        collection,
        format: SpecimenFormat::Html,
        out_path: out.display().to_string(),
    }
}

fn new_collection(lib: &Lib, name: &str, rules: Option<Filters>) -> CollectionId {
    lib.api
        .create_collection(CreateCollectionRequest {
            name: name.into(),
            parent: None,
            rules,
            rule_group: None,
        })
        .expect("collection")
        .id
}

/// The family names of a collection specimen, in the order of its pages.
fn page_families(page: &str) -> Vec<String> {
    page.split("<section class=\"family\">\n<h2 class=\"f")
        .skip(1)
        .map(|rest| {
            let name = &rest[rest.find('>').expect("h2") + 1..];
            name[..name.find("</h2>").expect("end")].to_owned()
        })
        .collect()
}

/// The family names `search` lists for `collection`, in its order.
fn searched_families(lib: &Lib, collection: CollectionId) -> Vec<String> {
    lib.api
        .search(SearchRequest {
            query: String::new(),
            filters: Filters {
                collection: Some(collection),
                ..Filters::default()
            },
            page: Page {
                offset: 0,
                limit: 1000,
            },
            rule: None,
        })
        .expect("search")
        .families
        .into_iter()
        .map(|h| h.family.name)
        .collect()
}

#[test]
fn a_collection_exports_as_one_html_specimen_with_a_page_per_family() {
    let (lib, _, _) = Lib::scanned("api-collection-specimen");
    let manual = new_collection(&lib, "Client brief", None);
    let archivo = lib
        .api
        .get_family(lib.family("Archivo").id)
        .expect("archivo");
    assert!(archivo.faces.len() >= 2);
    // A whole family, and one face of another: that family shows every style.
    lib.api
        .add_to_collection(CollectionFacesRequest {
            collection: manual,
            faces: vec![archivo.faces[0].id],
            families: vec![lib.family("Bebas Neue").id, lib.family("Agdasima").id],
        })
        .expect("added");
    let out_dir = lib.dir.path().join("Specimens ü");
    std::fs::create_dir_all(&out_dir).expect("out");
    let out = out_dir.join("Client brief.html");
    let res = lib
        .api
        .export_collection_specimen(collection_request(manual, &out))
        .expect("specimen");
    assert_eq!(Path::new(&res.path), out);
    let page = std::fs::read_to_string(&out).expect("page");

    // One self-contained file.
    let lower = page.to_lowercase();
    for bad in ["http:", "https:", "file:", "<script", "javascript"] {
        assert!(!lower.contains(bad), "{bad}");
    }
    // The cover, then the families in the collection's order.
    assert!(page.contains("<h1>Client brief</h1>"));
    assert!(page.contains(&format!("<dt>Date</dt><dd>{}</dd>", specimen::today())));
    assert!(page.contains("<dt>Families</dt><dd>3</dd>"));
    let order = searched_families(&lib, manual);
    assert_eq!(order, ["Agdasima", "Archivo", "Bebas Neue"]);
    assert_eq!(page_families(&page), order);
    // Every face of every family is embedded, family by family.
    let mut faces = Vec::new();
    for name in &order {
        faces.extend(
            lib.api
                .get_family(lib.family(name).id)
                .expect("family")
                .faces,
        );
    }
    let fonts = embedded_fonts(&page);
    assert_eq!(fonts.len(), faces.len());
    for ((_, bytes), face) in fonts.iter().zip(&faces) {
        let file = std::fs::read(lib.api.get_face(face.id).expect("face").path).expect("file");
        assert!(*bytes == file, "{} {}", face.family_name, face.style);
    }
    // Archivo's page lists every style, not only the face in the collection.
    let archivo_page = page
        .split("<section class=\"family\">")
        .find(|s| s.contains(">Archivo</h2>"))
        .expect("Archivo's page");
    for face in &archivo.faces {
        assert!(
            archivo_page.contains(&format!(
                "<p class=\"label\">{}</p>",
                specimen::escape(&face.style)
            )),
            "{}",
            face.style
        );
    }
    assert_eq!(archivo_page.matches(" px</p>").count(), 3);
    assert!(archivo_page.contains("<dt>License</dt>"));
    assert!(archivo_page.contains("<dt>Web use</dt>"));

    // A smart collection exports the families its rule finds.
    let smart = new_collection(
        &lib,
        "Variable",
        Some(Filters {
            variable: Some(true),
            ..Filters::default()
        }),
    );
    let out = out_dir.join("Variable.html");
    lib.api
        .export_collection_specimen(collection_request(smart, &out))
        .expect("smart");
    let page = std::fs::read_to_string(&out).expect("page");
    let order = searched_families(&lib, smart);
    assert!(order.contains(&"Asap Sharp".to_owned()), "{order:?}");
    assert!(!order.contains(&"Bebas Neue".to_owned()), "{order:?}");
    assert_eq!(page_families(&page), order);
    assert!(page.contains(&format!("<dt>Families</dt><dd>{}</dd>", order.len())));
}

#[test]
fn a_collection_specimen_refuses_empty_unknown_restricted_and_a_pdf_without_the_app() {
    let lib = Lib::new("api-collection-specimen-refusals");
    let folder = lib.dir.path().join("Patched");
    with_fs_type(
        "google-repo/bebasneue/BebasNeue-Regular.ttf",
        0x0002,
        &folder,
    );
    let source = lib
        .api
        .add_source(AddSourceRequest {
            path: folder.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    lib.scan(Some(source.id), true);
    let out = lib.dir.path().join("collection.html");

    let empty = new_collection(&lib, "Empty", None);
    let e = lib
        .api
        .export_collection_specimen(collection_request(empty, &out))
        .expect_err("empty");
    assert_eq!(e.code, ErrorCode::InvalidRequest, "{e:?}");
    assert_eq!(
        e.message,
        "The collection Empty is empty. Add fonts to it to export a specimen."
    );
    // A smart collection whose rule finds nothing is empty too.
    let nothing = new_collection(
        &lib,
        "No such foundry",
        Some(Filters {
            foundry: Some("No Such Foundry".into()),
            ..Filters::default()
        }),
    );
    let e = lib
        .api
        .export_collection_specimen(collection_request(nothing, &out))
        .expect_err("empty smart");
    assert_eq!(e.code, ErrorCode::InvalidRequest, "{e:?}");

    let e = lib
        .api
        .export_collection_specimen(collection_request(CollectionId(999_999), &out))
        .expect_err("unknown");
    assert_eq!(e.code, ErrorCode::NotFound, "{e:?}");

    // The restricted Bebas Neue refuses the whole collection, as E21.1.
    let bebas = lib.family("Bebas Neue").id;
    let bebas_face = lib.api.get_family(bebas).expect("family").faces[0].id;
    let refused = new_collection(&lib, "Refused", None);
    lib.api
        .add_to_collection(CollectionFacesRequest {
            collection: refused,
            faces: Vec::new(),
            families: vec![bebas],
        })
        .expect("added");
    let e = lib
        .api
        .export_collection_specimen(collection_request(refused, &out))
        .expect_err("restricted");
    assert_eq!(e.code, ErrorCode::PermissionDenied, "{e:?}");
    assert_eq!(
        e.message,
        "Bebas Neue Regular: Embedding restricted and no license declared, so the font \
         may not be embedded in a specimen. Declare a license for the family to export it."
    );
    assert_eq!(e.detail, Some(format!("blocked faces: {}", bebas_face.0)));

    let e = lib
        .api
        .export_collection_specimen(CollectionSpecimenRequest {
            format: SpecimenFormat::Pdf,
            ..collection_request(refused, &out)
        })
        .expect_err("pdf");
    assert_eq!(e.code, ErrorCode::Unsupported);
    assert_eq!(e.message, "PDF export needs the Font Manager app running.");
    let e = lib
        .api
        .export_collection_specimen(CollectionSpecimenRequest {
            out_path: " ".into(),
            ..collection_request(refused, &out)
        })
        .expect_err("no path");
    assert_eq!(e.code, ErrorCode::InvalidRequest);
    assert!(!out.exists());

    // Declaring a license lets it through.
    lib.api
        .declare_license(LicenseDeclaration {
            family: bebas,
            license_type: Some(LicenseType::Desktop),
            seats: None,
            purchase_date: None,
            receipt: ReceiptChange::Keep,
        })
        .expect("declare");
    lib.api
        .export_collection_specimen(collection_request(refused, &out))
        .expect("declared");
    assert!(out.exists());
}

#[test]
fn a_pdf_collection_specimen_prints_the_collection_page_through_the_printer() {
    let (lib, _, _) = Lib::scanned("api-collection-specimen-pdf");
    let printer = FakePrinter::new(PrintOutcome::Pdf);
    let lib = Lib {
        api: lib
            .api
            .with_pdf_printer(Arc::clone(&printer) as Arc<dyn PdfPrinter>),
        ..lib
    };
    let c = new_collection(&lib, "Sign-off", None);
    lib.api
        .add_to_collection(CollectionFacesRequest {
            collection: c,
            faces: Vec::new(),
            families: vec![lib.family("Archivo").id, lib.family("Agdasima").id],
        })
        .expect("added");
    let out_dir = lib.dir.path().join("Out");
    std::fs::create_dir_all(&out_dir).expect("out");
    let out = out_dir.join("Sign-off.pdf");
    let res = lib
        .api
        .export_collection_specimen(CollectionSpecimenRequest {
            format: SpecimenFormat::Pdf,
            ..collection_request(c, &out)
        })
        .expect("pdf");
    assert_eq!(Path::new(&res.path), out);
    assert!(std::fs::read(&out).expect("pdf").starts_with(b"%PDF-"));
    let calls = printer.calls();
    assert_eq!(calls.len(), 1);
    let (html, page, _, _) = &calls[0];
    // The printer was handed the collection page, the HTML export's.
    let same = out_dir.join("Sign-off.html");
    lib.api
        .export_collection_specimen(collection_request(c, &same))
        .expect("html");
    assert_eq!(page, &std::fs::read_to_string(&same).expect("html"));
    assert_eq!(page_families(page), ["Agdasima", "Archivo"]);
    assert!(!html.exists(), "the temporary page is removed");
    assert_eq!(dir_names(&out_dir), ["Sign-off.html", "Sign-off.pdf"]);
}

// ---- catalogs ----

#[test]
fn catalogs_never_fetch_outside_refresh() {
    let (lib, source, _) = Lib::scanned("catalog-no-fetch");
    let id = fm_api::catalog::CatalogId::GoogleFonts;
    let catalogs = lib.api.list_catalogs().expect("catalogs");
    assert_eq!(catalogs.len(), 1);
    assert!(!catalogs[0].enabled, "off by default");
    let e = lib.api.refresh_catalog(CatalogRef { id }).expect_err("off");
    assert_eq!(e.code, ErrorCode::Conflict);

    let on = lib
        .api
        .set_catalog_enabled(SetCatalogEnabledRequest { id, enabled: true })
        .expect("on");
    assert!(on.enabled);
    lib.scan(Some(source.id), false);
    lib.api
        .search(SearchRequest {
            query: "bebas".into(),
            ..SearchRequest::default()
        })
        .expect("search");
    lib.api
        .list_families(ListFamiliesRequest::default())
        .expect("families");
    let page = lib
        .api
        .list_catalog_families(ListCatalogFamiliesRequest {
            id,
            query: "lato".into(),
            page: Page::default(),
        })
        .expect("catalog families");
    assert_eq!(page.total, 0, "never refreshed");
    assert_eq!(
        lib.fetcher.calls(),
        0,
        "scan, search and listing fetch nothing"
    );

    let e = lib
        .api
        .refresh_catalog(CatalogRef { id })
        .expect_err("offline");
    assert_eq!(e.code, ErrorCode::Io);
    assert_eq!(
        lib.fetcher.calls(),
        1,
        "refresh is the one call that fetches"
    );
    assert_eq!(
        lib.api.list_catalogs().expect("catalogs")[0].family_count,
        0
    );
}

/// A scanned library whose catalogs reach the Lato fixture server over
/// HTTP, with Google Fonts on and refreshed.
fn downloading(label: &str) -> (Lib, Source, crate::test_support::LocalServer) {
    downloading_from(
        label,
        crate::catalog::tests::lato_server((200, crate::catalog::tests::lato_font("bold"))),
    )
}

/// As [`downloading`], over `server`.
fn downloading_from(
    label: &str,
    server: crate::test_support::LocalServer,
) -> (Lib, Source, crate::test_support::LocalServer) {
    let (lib, source, _) = Lib::scanned(label);
    let catalogs = Catalogs::new(
        lib.paths.data.join(CATALOGS_DIR),
        Arc::new(crate::catalog::HttpFetcher::new()),
        vec![Arc::new(GoogleFonts::with_base_url(server.base()))],
    );
    let lib = Lib {
        api: lib.api.with_catalogs(catalogs),
        ..lib
    };
    let id = fm_api::catalog::CatalogId::GoogleFonts;
    lib.api
        .set_catalog_enabled(SetCatalogEnabledRequest { id, enabled: true })
        .expect("on");
    lib.api.refresh_catalog(CatalogRef { id }).expect("refresh");
    (lib, source, server)
}

/// Waits for the `LibraryChanged` that ends scan `job`; the events up to it.
fn until_library_changed(rx: &Receiver<Event>, job: JobId) -> Vec<Event> {
    let mut events = Vec::new();
    let mut finished = false;
    let deadline = Instant::now() + WAIT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let event = match rx.recv_timeout(left) {
            Ok(e) => e,
            Err(e) => panic!("scan {job} did not end ({e:?}): {events:?}"),
        };
        match &event {
            Event::JobFailed { job: j, .. } if *j == job => panic!("scan failed: {event:?}"),
            Event::ScanFinished { job: j, .. } if *j == job => finished = true,
            Event::LibraryChanged { .. } if finished => {
                events.push(event);
                return events;
            }
            _ => {}
        }
        events.push(event);
    }
}

fn download_lato(lib: &Lib, source: SourceId) -> (DownloadFamilyResponse, Vec<Event>) {
    let rx = lib.api.subscribe();
    let got = lib
        .api
        .download_family(DownloadFamilyRequest {
            id: fm_api::catalog::CatalogId::GoogleFonts,
            family: "Lato".into(),
            kind: fm_api::catalog::CatalogFileKind::Static,
            source,
        })
        .expect("download");
    let events = until_library_changed(&rx, got.scan);
    (got, events)
}

#[test]
fn a_download_is_rescanned_and_its_faces_join_the_library() {
    let (lib, source, _server) = downloading("catalog-download-scan");
    assert!(!lib.names().iter().any(|n| n == "Asul" || n == "Cagliostro"));

    let (got, events) = download_lato(&lib, source.id);
    assert_eq!(got.download.folder, lib.root.join("Lato").to_string_lossy());
    assert_eq!(got.download.source, source.id);
    assert!(got.skipped.is_empty());
    assert!(
        matches!(events.first(), Some(Event::ScanStarted { job, source: Some(s) }) if *job == got.scan && *s == source.id),
        "{events:?}"
    );
    let Some(Event::LibraryChanged { added, .. }) = events.last() else {
        panic!("{events:?}");
    };
    assert_eq!(added.len(), 2, "the two static files: {events:?}");
    let names = lib.names();
    for family in ["Asul", "Cagliostro"] {
        assert!(names.iter().any(|n| n == family), "{family} in {names:?}");
    }
    assert!(
        !names.iter().any(|n| n == "Castoro"),
        "the variable file was not asked for"
    );
}

#[test]
fn provenance_survives_a_rescan_and_follows_a_moved_file_by_hash() {
    let (lib, source, _server) = downloading("catalog-download-provenance");
    let (got, _) = download_lato(&lib, source.id);
    let folder = lib.root.join("Lato");
    let listed = lib.api.list_catalog_downloads().expect("downloads");
    assert_eq!(listed.len(), 1);
    let d = &listed[0];
    assert_eq!(d.catalog, fm_api::catalog::CatalogId::GoogleFonts);
    assert_eq!(d.family, "Lato");
    assert_eq!(d.catalog_family_id, "Lato");
    assert_eq!(d.version.as_deref(), Some("2025-09-16"));
    assert_eq!(d.kind, fm_api::catalog::CatalogFileKind::Static);
    assert_eq!(d.downloaded_at, got.download.downloaded_at);
    assert_eq!(d.source, source.id);
    let bold_hash = blake3::hash(&crate::catalog::tests::lato_font("bold"))
        .to_hex()
        .to_string();
    let files: Vec<(&str, &str, Option<&str>)> = d
        .files
        .iter()
        .map(|f| (f.name.as_str(), f.hash.as_str(), f.path.as_deref()))
        .collect();
    let bold_at = folder.join("Lato-Bold.ttf").to_string_lossy().into_owned();
    let regular_at = folder
        .join("Lato-Regular.ttf")
        .to_string_lossy()
        .into_owned();
    assert_eq!(
        files[0],
        ("Lato-Bold.ttf", bold_hash.as_str(), Some(bold_at.as_str()))
    );
    assert_eq!(files[1].0, "Lato-Regular.ttf");
    assert_eq!(files[1].2, Some(regular_at.as_str()));
    assert_eq!(d, &got.download, "the answer is what is recorded");

    // A rescan and a rebuild of the index keep it.
    lib.scan(Some(source.id), false);
    assert_eq!(lib.api.list_catalog_downloads().expect("downloads"), listed);
    let rx = lib.api.subscribe();
    let job = lib.api.rebuild_index().expect("rebuild");
    loop {
        match rx.recv_timeout(WAIT).expect("rebuild scan") {
            Event::ScanFinished { job: j, .. } if j == job => break,
            _ => {}
        }
    }
    // `ScanFinished` comes once the rescan is written.
    lib.scan(Some(source.id), false);
    assert_eq!(lib.api.list_catalog_downloads().expect("downloads"), listed);

    // The user moves the bold file elsewhere in the library and renames it.
    let moved = lib.root.join("Sorted").join("Bold one.ttf");
    std::fs::create_dir_all(moved.parent().expect("parent")).expect("folder");
    std::fs::rename(folder.join("Lato-Bold.ttf"), &moved).expect("move");
    lib.scan(Some(source.id), true);
    let after = lib.api.list_catalog_downloads().expect("downloads");
    let bold = &after[0].files[0];
    assert_eq!(bold.name, "Lato-Bold.ttf", "the name it was downloaded as");
    assert_eq!(bold.hash, bold_hash);
    assert_eq!(
        bold.path.as_deref(),
        Some(moved.to_string_lossy().as_ref()),
        "followed by its hash"
    );
    assert_eq!(after[0].files[1].path.as_deref(), Some(regular_at.as_str()));

    // Gone from the library: still recorded, with no path.
    std::fs::remove_file(&moved).expect("remove");
    lib.scan(Some(source.id), true);
    let after = lib.api.list_catalog_downloads().expect("downloads");
    assert_eq!(after[0].files[0].path, None);
    assert_eq!(after[0].files.len(), 2);
}

#[test]
fn a_stopped_download_records_and_rescans_what_it_wrote_and_says_why() {
    let (lib, source, _server) = downloading_from(
        "catalog-download-stopped",
        crate::catalog::tests::stopping_server(),
    );
    let (got, events) = download_lato(&lib, source.id);
    let error = got.error.as_ref().expect("the download says it stopped");
    assert_eq!(error.code, ErrorCode::Io);
    assert!(
        error.message.starts_with("Lato could not be downloaded."),
        "{error:?}"
    );
    let names: Vec<&str> = got.download.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        ["Lato-Regular.ttf"],
        "the file written before it stopped"
    );

    // Recorded and rescanned like a finished download.
    let listed = lib.api.list_catalog_downloads().expect("downloads");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].files.len(), 1);
    assert_eq!(listed[0].files[0].name, "Lato-Regular.ttf");
    let Some(Event::LibraryChanged { added, .. }) = events.last() else {
        panic!("{events:?}");
    };
    assert_eq!(added.len(), 1, "the one file written: {events:?}");
    assert!(lib.names().iter().any(|n| n == "Cagliostro"));
}

#[test]
fn a_download_goes_only_into_a_watched_source() {
    let (lib, source, server) = downloading("catalog-download-target");
    let system = lib
        .api
        .lock()
        .add_source(
            &lib.dir.path().join("System").to_string_lossy(),
            SourceKind::System,
        )
        .expect("system source");
    std::fs::create_dir_all(lib.dir.path().join("System")).expect("folder");
    let req = |source| DownloadFamilyRequest {
        id: fm_api::catalog::CatalogId::GoogleFonts,
        family: "Lato".into(),
        kind: fm_api::catalog::CatalogFileKind::Static,
        source,
    };
    let hits = server.hits();
    let e = lib
        .api
        .download_family(req(system.id))
        .expect_err("system fonts");
    assert_eq!(e.code, ErrorCode::InvalidRequest);
    let e = lib
        .api
        .download_family(req(SourceId(999)))
        .expect_err("unknown source");
    assert_eq!(e.code, ErrorCode::NotFound);
    assert_eq!(server.hits(), hits, "refused before any request");
    assert!(!lib.dir.path().join("System").join("Lato").exists());
    assert!(lib.api.list_catalog_downloads().expect("list").is_empty());
    let _ = source;
}

/// Sets the download folder of `lib` to `folder`.
fn set_download_folder(lib: &Lib, folder: &Path) {
    lib.api
        .update_settings(SettingsUpdate {
            download_folder: Some(folder.to_string_lossy().into_owned()),
            ..SettingsUpdate::default()
        })
        .expect("download folder");
}

fn install_lato(lib: &Lib) -> ApiResult<InstallFamilyResponse> {
    lib.api.install_family(InstallFamilyRequest {
        id: fm_api::catalog::CatalogId::GoogleFonts,
        family: "Lato".into(),
        kind: fm_api::catalog::CatalogFileKind::Static,
    })
}

/// The watched sources of `lib`, by path.
fn watched_paths(lib: &Lib) -> Vec<String> {
    lib.api
        .list_sources()
        .expect("sources")
        .into_iter()
        .filter(|s| s.kind == SourceKind::Watched)
        .map(|s| s.path)
        .collect()
}

/// The names in `dir`, sorted.
fn listing_of(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("folder")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn an_install_downloads_into_the_download_folder_added_once_and_activates_its_faces() {
    let (lib, _source, server) = downloading("catalog-install");
    let folder = lib.dir.path().join("Font Manager Fonts");
    set_download_folder(&lib, &folder);
    assert!(!folder.exists(), "made only by the install");

    let got = install_lato(&lib).expect("install");
    assert!(!got.already_installed);
    assert_eq!(got.error, None);
    assert_eq!(
        got.download.folder,
        folder.join("Lato").to_string_lossy(),
        "a subfolder named after the family"
    );
    let names: Vec<&str> = got.download.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Lato-Bold.ttf", "Lato-Regular.ttf"]);
    // The folder is a watched source now, indexed in place.
    let watched = watched_paths(&lib);
    assert_eq!(watched.len(), 2, "{watched:?}");
    assert!(watched.contains(&folder.to_string_lossy().into_owned()));
    // The rescan is done when the call returns: both faces are in the library and active.
    assert_eq!(got.faces.len(), 2, "{got:?}");
    let names = lib.names();
    assert!(names.iter().any(|n| n == "Asul") && names.iter().any(|n| n == "Cagliostro"));
    assert_eq!(got.outcomes.len(), 2);
    assert!(
        got.outcomes
            .iter()
            .all(|o| o.outcome == OutcomeStatus::Done),
        "{:?}",
        got.outcomes
    );
    let active = lib.api.list_active().expect("active");
    for face in &got.faces {
        let a = active.iter().find(|a| a.face == *face).expect("active");
        assert_eq!(a.mode, ActivationMode::Session, "the mode of the settings");
    }
    // Provenance is recorded as for a download.
    let listed = lib.api.list_catalog_downloads().expect("downloads");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0], got.download);

    // A second install downloads nothing, adds no source and leaves the
    // active faces as they are.
    let hits = server.hits();
    let again = install_lato(&lib).expect("again");
    assert!(again.already_installed);
    assert_eq!(server.hits(), hits, "no request");
    assert_eq!(watched_paths(&lib).len(), 2, "the source is added once");
    assert_eq!(again.faces, got.faces);
    assert!(again.outcomes.is_empty(), "nothing to activate: {again:?}");
    assert_eq!(
        lib.api.list_catalog_downloads().expect("downloads").len(),
        1
    );
}

#[test]
fn an_install_of_a_family_in_the_library_activates_what_is_there_in_the_configured_mode() {
    let (lib, _source, server) = downloading("catalog-install-present");
    let folder = lib.dir.path().join("Installed");
    set_download_folder(&lib, &folder);
    let first = install_lato(&lib).expect("install");
    lib.api
        .deactivate(DeactivateRequest::default())
        .expect("deactivate");
    lib.api
        .update_settings(SettingsUpdate {
            activation_mode: Some(ActivationMode::Permanent),
            ..SettingsUpdate::default()
        })
        .expect("permanent");
    let hits = server.hits();
    let again = install_lato(&lib).expect("again");
    assert!(again.already_installed);
    assert_eq!(server.hits(), hits, "nothing downloaded again");
    assert_eq!(again.download, first.download, "the recorded provenance");
    assert_eq!(again.outcomes.len(), 2);
    let active = lib.api.list_active().expect("active");
    assert_eq!(active.len(), 2);
    assert!(active.iter().all(|a| a.mode == ActivationMode::Permanent));
}

#[test]
fn files_already_in_the_family_folder_are_activated_and_not_downloaded() {
    let (lib, _source, server) = downloading("catalog-install-by-hand");
    let folder = lib.dir.path().join("Installed");
    let family = folder.join("Lato");
    std::fs::create_dir_all(&family).expect("folder");
    std::fs::write(
        family.join("Lato-Bold.ttf"),
        crate::catalog::tests::lato_font("bold"),
    )
    .expect("file");
    let source = lib
        .api
        .add_source(AddSourceRequest {
            path: folder.to_string_lossy().into_owned(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    lib.scan(Some(source.id), true);
    set_download_folder(&lib, &folder);

    let hits = server.hits();
    let got = install_lato(&lib).expect("install");
    assert!(got.already_installed);
    assert_eq!(server.hits(), hits, "no request");
    assert_eq!(got.faces.len(), 1);
    assert_eq!(got.download.source, source.id);
    assert_eq!(got.download.files.len(), 1);
    assert_eq!(got.download.files[0].name, "Lato-Bold.ttf");
    assert_eq!(
        got.download.files[0].hash,
        blake3::hash(&crate::catalog::tests::lato_font("bold"))
            .to_hex()
            .to_string()
    );
    assert_eq!(got.outcomes.len(), 1);
    assert_eq!(listing_of(&family), ["Lato-Bold.ttf"], "nothing written");
}

#[test]
fn family_files_on_disk_but_not_in_the_library_are_indexed_activated_and_not_fetched() {
    // The download folder holds the family (an earlier download, or its
    // source was removed), but no source has it: nothing in the library.
    let (lib, _source, server) = downloading("catalog-install-on-disk");
    let folder = lib.dir.path().join("Installed");
    let family = folder.join("Lato");
    std::fs::create_dir_all(&family).expect("folder");
    for (name, which) in [("Lato-Bold.ttf", "bold"), ("Lato-Regular.ttf", "regular")] {
        std::fs::write(family.join(name), crate::catalog::tests::lato_font(which)).expect("file");
    }
    set_download_folder(&lib, &folder);

    let hits = server.hits();
    let got = install_lato(&lib).expect("install");
    assert_eq!(server.hits(), hits, "not fetched again");
    assert!(got.already_installed);
    assert_eq!(got.faces.len(), 2, "{got:?}");
    assert_eq!(got.outcomes.len(), 2);
    assert!(
        got.outcomes
            .iter()
            .all(|o| o.outcome == OutcomeStatus::Done),
        "{:?}",
        got.outcomes
    );
    assert_eq!(lib.api.list_active().expect("active").len(), 2);
    let watched = watched_paths(&lib);
    assert!(
        watched.contains(&folder.to_string_lossy().into_owned()),
        "{watched:?}"
    );
    assert_eq!(
        listing_of(&family),
        ["Lato-Bold.ttf", "Lato-Regular.ttf"],
        "nothing written"
    );
}

#[test]
fn a_download_folder_inside_a_watched_source_uses_that_source() {
    let (lib, source, _server) = downloading("catalog-install-nested");
    let folder = lib.root.join("Downloads");
    set_download_folder(&lib, &folder);
    let got = install_lato(&lib).expect("install");
    assert_eq!(got.download.source, source.id, "no nested source");
    assert_eq!(watched_paths(&lib).len(), 1);
    assert_eq!(got.faces.len(), 2);
    assert!(folder.join("Lato").join("Lato-Regular.ttf").is_file());
}

#[test]
fn an_install_is_refused_without_a_folder_while_off_or_into_system_fonts_before_any_request() {
    let (lib, _source, server) = downloading("catalog-install-refused");
    let hits = server.hits();
    let e = install_lato(&lib).expect_err("no folder");
    assert_eq!(e.code, ErrorCode::Conflict);
    assert_eq!(e.message, crate::api::install::NO_DOWNLOAD_FOLDER);

    // A system font folder: one the OS reads, and a system source of the library.
    let os_fonts = fm_platform::paths::system_font_folders();
    let system = lib.dir.path().join("System");
    std::fs::create_dir_all(&system).expect("folder");
    lib.api
        .lock()
        .add_source(&system.to_string_lossy(), SourceKind::System)
        .expect("system source");
    // The same folders written with `..` on the way, or through a folder
    // that does not exist yet, are the same folders.
    let parent = os_fonts[0].parent().expect("parent");
    let name = os_fonts[0].file_name().expect("name");
    let dotted = parent.join("Not There").join("..").join(name).join("Sub");
    let system_dotted = lib.dir.path().join("Elsewhere").join("..").join("System");
    for folder in [
        os_fonts[0].clone(),
        os_fonts[0].join("Sub"),
        dotted,
        system.clone(),
        system_dotted,
    ] {
        set_download_folder(&lib, &folder);
        let e = install_lato(&lib).expect_err("system fonts");
        assert_eq!(e.code, ErrorCode::Conflict, "{folder:?}");
        assert_eq!(e.message, crate::api::install::SYSTEM_DOWNLOAD_FOLDER);
    }
    assert!(!system.join("Lato").exists());
    // On Windows a path with a root but no drive (`\Windows\Fonts`) is not a
    // full path: refused before it is compared.
    let text = os_fonts[0].to_string_lossy().into_owned();
    if text.as_bytes().get(1) == Some(&b':') {
        set_download_folder(&lib, Path::new(&text[2..]));
        let e = install_lato(&lib).expect_err("no drive");
        assert_eq!(e.code, ErrorCode::Conflict);
        assert_eq!(e.message, crate::api::install::RELATIVE_DOWNLOAD_FOLDER);
    }

    let folder = lib.dir.path().join("Installed");
    set_download_folder(&lib, &folder);
    lib.api
        .set_catalog_enabled(SetCatalogEnabledRequest {
            id: fm_api::catalog::CatalogId::GoogleFonts,
            enabled: false,
        })
        .expect("off");
    let e = install_lato(&lib).expect_err("off");
    assert_eq!(e.code, ErrorCode::Conflict);
    assert!(e.message.contains("turned off"), "{e:?}");
    assert_eq!(server.hits(), hits, "refused before any request");
    assert!(!folder.exists(), "no folder made");
    assert_eq!(watched_paths(&lib).len(), 1, "no source added");
    assert!(lib.api.list_active().expect("active").is_empty());
}

#[test]
fn the_update_check_compares_the_recorded_download_with_a_refreshed_list() {
    let (lib, source, server) = downloading("catalog-updates-api");
    download_lato(&lib, source.id);
    let hits = server.hits();
    let updates = lib
        .api
        .check_updates(CheckUpdatesRequest::default())
        .expect("check");
    assert!(updates.is_empty(), "the same version: {updates:?}");
    assert_eq!(server.hits(), hits + 1, "one refresh of the index");
}

#[test]
fn nothing_checks_for_updates_at_startup() {
    let (lib, source, _server) = downloading("catalog-updates-startup");
    download_lato(&lib, source.id);
    // A restart: the same folders, over a fetcher that fails on any call.
    let fetcher = Arc::new(OfflineFetcher::default());
    let catalogs = Catalogs::new(
        lib.paths.data.join(CATALOGS_DIR),
        Arc::clone(&fetcher) as Arc<dyn Fetcher>,
        vec![Arc::new(GoogleFonts::with_base_url("http://127.0.0.1:9"))],
    );
    let lib = Lib {
        api: lib.api.with_catalogs(catalogs),
        fetcher,
        ..lib
    };
    let id = fm_api::catalog::CatalogId::GoogleFonts;
    let listed = lib.api.list_catalogs().expect("catalogs");
    assert!(listed[0].enabled && listed[0].refreshed_at.is_some());
    lib.api
        .list_catalog_families(ListCatalogFamiliesRequest {
            id,
            query: String::new(),
            page: Page::default(),
        })
        .expect("families");
    assert_eq!(
        lib.api.list_catalog_downloads().expect("downloads").len(),
        1
    );
    let cached = lib
        .api
        .check_updates(CheckUpdatesRequest { cached_only: true })
        .expect("compared with the cache");
    assert!(cached.is_empty(), "{cached:?}");
    assert_eq!(lib.fetcher.calls(), 0, "the start fetches nothing");

    let e = lib
        .api
        .check_updates(CheckUpdatesRequest::default())
        .expect_err("offline");
    assert_eq!(e.code, ErrorCode::Io);
    assert_eq!(lib.fetcher.calls(), 1, "only the user's check fetches");
    assert_eq!(
        lib.api.list_catalogs().expect("catalogs")[0].family_count,
        listed[0].family_count,
        "a failed check keeps the cache"
    );
}

#[test]
fn catalogs_without_a_data_folder_are_not_available() {
    let dir = TempDir::new("catalog-none");
    let index = Index::open(&dir.path().join(INDEX_FILE))
        .expect("index")
        .index;
    let paths = AppPaths {
        data: dir.path().join("data"),
        config: dir.path().join("config"),
        cache: dir.path().join("cache"),
        logs: dir.path().join("logs"),
        library: dir.path().join("Fonts"),
    };
    let api = CoreApi::new(
        index,
        Arc::new(MockActivator::new()),
        SettingsStore::new(&paths),
        None,
    );
    assert_eq!(
        api.list_catalogs().expect_err("no folder").code,
        ErrorCode::Unsupported
    );
}

#[test]
fn the_local_api_needs_a_data_folder_and_then_publishes_and_regenerates() {
    let lib = Lib::new("endpoint");
    // Without `with_endpoint_in` (the tests' API, like no data folder).
    let e = lib.api.endpoint_info().expect_err("no data folder");
    assert_eq!(e.code, fm_api::ErrorCode::Unsupported);
    assert_eq!(
        lib.api
            .regenerate_endpoint_token()
            .expect_err("no data folder")
            .code,
        fm_api::ErrorCode::Unsupported
    );
    assert!(lib.api.unpublish_endpoint().is_ok(), "nothing to remove");

    let api = Lib::new("endpoint-on")
        .api
        .with_endpoint_in(lib.paths.data.clone());
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 40001));
    let before = api.endpoint_info().expect("info");
    assert_eq!(before.url, None);
    let published = api.publish_endpoint(addr).expect("publish");
    assert_eq!(published.url.as_deref(), Some("http://127.0.0.1:40001"));
    assert_eq!(published.token, before.token);
    let file = lib.paths.data.join(crate::endpoint::DISCOVERY_FILE);
    assert!(file.exists());
    let next = api.regenerate_endpoint_token().expect("regenerate");
    assert_ne!(next.token, before.token);
    assert_eq!(api.endpoint_info().expect("info"), next);
    api.unpublish_endpoint().expect("unpublish");
    assert!(!file.exists());
}

#[test]
fn the_mcp_server_is_off_by_default_and_its_switch_applies_without_a_restart() {
    let lib = Lib::new("mcp");
    let e = lib.api.mcp_status().expect_err("no data folder");
    assert_eq!(e.code, fm_api::ErrorCode::Unsupported);

    let api = Lib::new("mcp-on")
        .api
        .with_endpoint_in(lib.paths.data.clone());
    let off = api.mcp_status().expect("status");
    assert!(!off.enabled, "off by default");
    assert_eq!(off.url, None, "no URL before the endpoint runs");
    assert!(off.snippets.is_empty());

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 40002));
    let token = api.publish_endpoint(addr).expect("publish").token;
    let on = api
        .set_mcp_enabled(fm_api::settings::McpUpdate { enabled: true })
        .expect("on");
    assert!(on.enabled);
    assert_eq!(on.url.as_deref(), Some("http://127.0.0.1:40002/mcp"));
    assert_eq!(on.snippets.len(), 2);
    assert!(on.snippets.iter().all(|s| s.text.contains(&token)));
    assert_eq!(api.mcp_status().expect("status"), on);

    // A new token reaches the snippets at once.
    let next = api.regenerate_endpoint_token().expect("regenerate").token;
    let status = api.mcp_status().expect("status");
    assert!(status.snippets.iter().all(|s| s.text.contains(&next)));

    let off = api
        .set_mcp_enabled(fm_api::settings::McpUpdate { enabled: false })
        .expect("off");
    assert!(!off.enabled);
    assert!(!api.mcp_status().expect("status").enabled);
    api.unpublish_endpoint().expect("unpublish");
}

// --- MCP grants and confirmations (E27.6) --------------------------------

fn mcp_call(tool: &str, target: Option<Target>) -> fm_api::settings::McpToolCall {
    fm_api::settings::McpToolCall {
        tool: tool.into(),
        target,
    }
}

fn grant(api: &CoreApi, tool: &str, granted: bool) -> fm_api::settings::McpStatus {
    api.set_mcp_grant(fm_api::settings::McpGrantUpdate {
        tool: tool.into(),
        granted,
    })
    .expect("grant saved")
}

#[test]
fn mcp_grants_are_off_until_turned_on_and_apply_without_a_restart() {
    let lib = Lib::new("mcp-grants");
    let api = lib.api.with_endpoint_in(lib.paths.data.clone());
    let status = api.mcp_status().expect("status");
    let tools: Vec<&str> = status.grants.iter().map(|g| g.tool.as_str()).collect();
    assert_eq!(tools, fm_api::settings::MCP_MUTATING_TOOLS);
    assert!(status.grants.iter().all(|g| !g.granted), "all off");

    for tool in fm_api::settings::MCP_MUTATING_TOOLS {
        let e = api
            .authorize_mcp_tool(mcp_call(tool, None))
            .expect_err("not granted");
        assert_eq!(e.code, ErrorCode::PermissionDenied, "{tool}");
        assert!(
            e.message.contains("Settings → AI assistants"),
            "{}",
            e.message
        );
    }
    // Read-only tools need no grant.
    api.authorize_mcp_tool(mcp_call("search_fonts", None))
        .expect("read tools always run");

    // On: the same API allows the next call, without a restart.
    let on = grant(&api, "tag_fonts", true);
    assert!(on.grants.iter().any(|g| g.tool == "tag_fonts" && g.granted));
    assert_eq!(api.mcp_status().expect("status"), on);
    api.authorize_mcp_tool(mcp_call("tag_fonts", None))
        .expect("granted");
    assert!(
        SettingsStore::new(&lib.paths)
            .mcp_grants()
            .expect("grants")
            .contains("tag_fonts"),
        "the grant is saved in config.json"
    );
    let still = api
        .authorize_mcp_tool(mcp_call("activate_fonts", None))
        .expect_err("other grants stay off");
    assert!(
        still.message.contains("\"Activate fonts\""),
        "{}",
        still.message
    );

    // Off again: refused at once.
    grant(&api, "tag_fonts", false);
    let e = api
        .authorize_mcp_tool(mcp_call("tag_fonts", None))
        .expect_err("off again");
    assert_eq!(e.code, ErrorCode::PermissionDenied);

    let e = api
        .set_mcp_grant(fm_api::settings::McpGrantUpdate {
            tool: "search_fonts".into(),
            granted: true,
        })
        .expect_err("read tools have no grant");
    assert_eq!(e.code, ErrorCode::InvalidRequest);
}

#[test]
fn a_destructive_mcp_call_waits_for_the_users_allow_and_deny_changes_nothing() {
    let (lib, _, _) = Lib::scanned("mcp-confirm");
    let family = lib.family("Bebas Neue");
    let api = lib
        .api
        .with_endpoint_in(lib.paths.data.clone())
        .with_mcp_confirm_timeout(Duration::from_secs(20));
    let target = Target::Family { family: family.id };
    api.activate(ActivateRequest {
        target: target.clone(),
        mode: ActivationMode::Session,
        on_conflict: ConflictPolicy::default(),
    })
    .expect("activate");
    let active = api.list_active().expect("active");
    assert!(!active.is_empty());

    // Granted, but deactivate_fonts still asks every time.
    grant(&api, "deactivate_fonts", true);
    let rx = api.subscribe();
    for allow in [false, true] {
        let result = std::thread::scope(|s| {
            let call = s.spawn(|| {
                api.authorize_mcp_tool(mcp_call("deactivate_fonts", Some(target.clone())))
            });
            let id = loop {
                match rx.recv_timeout(WAIT).expect("a confirmation") {
                    Event::McpConfirmationRequested {
                        id,
                        tool,
                        faces,
                        timeout_secs,
                    } => {
                        assert_eq!(tool, "deactivate_fonts");
                        assert_eq!(faces, Some(u32::try_from(active.len()).expect("small")));
                        assert_eq!(timeout_secs, 20);
                        break id;
                    }
                    _ => continue,
                }
            };
            api.answer_mcp_confirmation(fm_api::settings::McpConfirmationAnswer { id, allow })
                .expect("answered");
            assert!(matches!(
                rx.recv_timeout(WAIT),
                Ok(Event::McpConfirmationClosed { id: closed, allowed }) if closed == id && allowed == allow
            ));
            call.join().expect("joined")
        });
        if allow {
            result.expect("allowed");
        } else {
            let e = result.expect_err("denied");
            assert_eq!(e.code, ErrorCode::PermissionDenied);
            assert!(e.message.contains("Nothing was changed"), "{}", e.message);
        }
        assert_eq!(
            api.list_active().expect("active"),
            active,
            "asking changes nothing"
        );
    }

    // An unknown target is refused before the user is asked.
    let e = api
        .authorize_mcp_tool(mcp_call(
            "deactivate_fonts",
            Some(Target::Faces {
                faces: vec![FaceId(999_999)],
            }),
        ))
        .expect_err("unknown face");
    assert_eq!(e.code, ErrorCode::NotFound);
    assert!(rx.try_recv().is_err(), "nobody was asked");
}

#[test]
fn an_unanswered_mcp_confirmation_times_out_as_a_deny() {
    let lib = Lib::new("mcp-timeout");
    let api = lib
        .api
        .with_endpoint_in(lib.paths.data.clone())
        .with_mcp_confirm_timeout(Duration::from_millis(50));
    grant(&api, "deactivate_fonts", true);
    let rx = api.subscribe();
    let started = Instant::now();
    let e = api
        .authorize_mcp_tool(mcp_call("deactivate_fonts", None))
        .expect_err("timed out");
    assert_eq!(e.code, ErrorCode::PermissionDenied);
    assert!(started.elapsed() >= Duration::from_millis(50));
    let Ok(Event::McpConfirmationRequested { id, faces, .. }) = rx.recv_timeout(WAIT) else {
        panic!("no confirmation asked");
    };
    assert_eq!(faces, Some(0), "no target: every face the app activated");
    assert_eq!(
        rx.recv_timeout(WAIT).expect("closed"),
        Event::McpConfirmationClosed { id, allowed: false }
    );
    let late = api
        .answer_mcp_confirmation(fm_api::settings::McpConfirmationAnswer { id, allow: true })
        .expect_err("too late");
    assert_eq!(late.code, ErrorCode::NotFound);
}

// --- font arguments (E26.1) --------------------------------------------

fn resolved(api: &CoreApi, font: &str) -> ApiResult<(FontMatchKind, Vec<String>)> {
    api.resolve_font(ResolveFontRequest { font: font.into() })
        .map(|m| {
            let names = m.faces.iter().map(|f| f.postscript_name.clone()).collect();
            (m.matched_by, names)
        })
}

#[test]
fn a_font_argument_names_faces_by_id_postscript_full_or_family_name() {
    let (lib, source, _) = Lib::scanned("api-resolve-font");
    let api = &lib.api;
    let barlow = api
        .resolve_font(ResolveFontRequest {
            font: "barlow-bolditalic".into(),
        })
        .expect("by PostScript name");
    assert_eq!(barlow.matched_by, FontMatchKind::PostscriptName);
    let face = barlow.faces[0].face;
    assert_eq!(
        barlow.faces[0].full_name.as_deref(),
        Some("Barlow Bold Italic")
    );
    let by_id = api
        .resolve_font(ResolveFontRequest {
            font: face.0.to_string(),
        })
        .expect("by id");
    assert_eq!(
        (by_id.matched_by, by_id.faces[0].face),
        (FontMatchKind::Id, face)
    );
    // Archivo SemiBold's name ID 1 is "Archivo SemiBold": its full name
    // matches before its family does.
    assert_eq!(
        resolved(api, "Archivo SemiBold"),
        Ok((FontMatchKind::FullName, vec!["Archivo-SemiBold".to_owned()]))
    );
    assert_eq!(
        resolved(api, "asap sharp regular"),
        Ok((
            FontMatchKind::FullName,
            vec!["AsapSharp-Regular".to_owned()]
        ))
    );
    assert_eq!(
        resolved(api, "Bebas Neue"),
        Ok((
            FontMatchKind::FamilyStyle,
            vec!["BebasNeue-Regular".to_owned()]
        ))
    );
    // A family of two faces in a TTC is ambiguous; the truncated copy has
    // no names and is not "Agdasima".
    let both = api
        .resolve_font(ResolveFontRequest {
            font: "Agdasima".into(),
        })
        .expect("two faces");
    assert_eq!(both.faces.len(), 2);
    let err = both.single("Agdasima").expect_err("ambiguous");
    assert_eq!(err.code, ErrorCode::InvalidRequest);
    let detail = err.detail.unwrap_or_default();
    assert!(detail.contains("Agdasima-Bold	Agdasima Bold"), "{detail}");
    assert!(
        detail.contains("Agdasima-Regular	Agdasima Regular"),
        "{detail}"
    );
    assert_eq!(
        resolved(api, "Inter").expect_err("unknown").code,
        ErrorCode::NotFound
    );
    // A paused source's faces are not in the library the user sees.
    api.set_source_paused(SetSourcePausedRequest {
        source: source.id,
        paused: true,
    })
    .expect("pause");
    assert_eq!(
        resolved(api, "Bebas Neue").expect_err("paused").code,
        ErrorCode::NotFound
    );
}

#[test]
fn the_app_offers_claude_desktop_its_stdio_shim() {
    let lib = Lib::new("mcp-shim");
    let shim = lib.paths.data.join("Font Manager").join("fm-mcp-stdio");
    let api = Lib::new("mcp-shim-on")
        .api
        .with_endpoint_in(lib.paths.data.clone())
        .with_mcp_shim(fm_platform::mcp_shim::ShimCommand {
            program: shim.clone(),
            args: Vec::new(),
        });
    api.publish_endpoint(std::net::SocketAddr::from(([127, 0, 0, 1], 40003)))
        .expect("publish");
    let status = api.mcp_status().expect("status");
    let desktop = status
        .snippets
        .iter()
        .find(|s| s.client == fm_api::settings::McpClient::ClaudeDesktop)
        .expect("a Claude Desktop snippet");
    let json: serde_json::Value = serde_json::from_str(&desktop.text).expect("JSON");
    assert_eq!(
        json["mcpServers"]["font-manager"]["command"],
        shim.to_string_lossy().as_ref()
    );
    api.unpublish_endpoint().expect("unpublish");
}

// --- set files across machines (E28.2) ----------------------------------

/// Every face of the library but those in a truncated file.
fn healthy_faces(lib: &Lib) -> Vec<FaceId> {
    let mut faces = Vec::new();
    for name in lib.names() {
        let family = lib.api.get_family(lib.family(&name).id).expect("family");
        for face in family.faces {
            let path = lib.api.get_face(face.id).expect("detail").path;
            if !path.contains("truncated") {
                faces.push(face.id);
            }
        }
    }
    faces
}

/// `(family, style, hash)` of each face, sorted.
fn entries(lib: &Lib, faces: &[FaceId]) -> Vec<(String, String, String)> {
    let mut out: Vec<_> = faces
        .iter()
        .map(|f| {
            let d = lib.api.get_face(*f).expect("detail");
            (d.face.family_name, d.face.style, d.hash)
        })
        .collect();
    out.sort();
    out
}

/// The second machine: the same fixture copies, but Barlow renamed, the
/// collection file (.ttc) in another source and Bebas Neue not there.
fn second_machine() -> Lib {
    let lib = Lib::new("api-set-file-b");
    let barlow = lib.root.join("barlow");
    std::fs::rename(
        barlow.join("Barlow-BoldItalic.ttf"),
        barlow.join("my-barlow-copy.ttf"),
    )
    .expect("rename");
    let other = lib.dir.path().join("Elsewhere");
    std::fs::create_dir_all(&other).expect("folder");
    std::fs::rename(lib.root.join("agdasima"), other.join("agdasima")).expect("move");
    std::fs::remove_dir_all(lib.root.join("bebasneue")).expect("remove");
    for folder in [&lib.root, &other] {
        let source = lib
            .api
            .add_source(AddSourceRequest {
                path: folder.display().to_string(),
                mode: ImportMode::IndexInPlace,
            })
            .expect("source");
        lib.scan(Some(source.id), true);
    }
    lib
}

#[test]
fn save_set_file_writes_the_set_export_and_nothing_else() {
    let (lib, _, _) = Lib::scanned("api-set-file-save");
    let faces = healthy_faces(&lib);
    let set = lib
        .api
        .create_set(CreateSetRequest {
            name: "Brochure".into(),
            faces: faces.clone(),
        })
        .expect("set");
    let out = lib.dir.path().join("Out");
    std::fs::create_dir_all(&out).expect("folder");
    let saved = lib
        .api
        .save_set_file(SaveSetFileRequest {
            set: set.id,
            path: out.join("Brochure").display().to_string(),
        })
        .expect("saved");
    assert_eq!(PathBuf::from(&saved.path), out.join("Brochure.fmset.json"));
    assert_eq!(saved.fonts as usize, faces.len());
    let written: Vec<_> = std::fs::read_dir(&out)
        .expect("dir")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(written, ["Brochure.fmset.json"]);
    let text = std::fs::read_to_string(&saved.path).expect("read");
    let parsed: SetExport = serde_json::from_str(&text).expect("SetExport JSON");
    assert_eq!(parsed, lib.api.export_set(set.id).expect("export"));

    let relative = lib.api.save_set_file(SaveSetFileRequest {
        set: set.id,
        path: "Brochure.fmset.json".into(),
    });
    assert_eq!(
        relative.expect_err("relative").code,
        ErrorCode::InvalidRequest
    );
    let unknown = lib.api.save_set_file(SaveSetFileRequest {
        set: SetId(9_999),
        path: out.join("x").display().to_string(),
    });
    assert_eq!(unknown.expect_err("unknown").code, ErrorCode::NotFound);
    assert_eq!(std::fs::read_dir(&out).expect("dir").count(), 1);
}

#[test]
fn a_set_file_matches_by_hash_on_a_second_machine_and_activates_the_same_files() {
    let (a, _, _) = Lib::scanned("api-set-file-a");
    let faces = healthy_faces(&a);
    let set = a
        .api
        .create_set(CreateSetRequest {
            name: "Brochure".into(),
            faces: faces.clone(),
        })
        .expect("set");
    let file = a.dir.path().join("Brochure.fmset.json");
    a.api
        .save_set_file(SaveSetFileRequest {
            set: set.id,
            path: file.display().to_string(),
        })
        .expect("saved");
    let on_a = entries(&a, &faces);
    let ttc_on_a: Vec<_> = on_a
        .iter()
        .filter(|(family, ..)| family == "Agdasima")
        .cloned()
        .collect();
    assert_eq!(ttc_on_a.len(), 2, "both faces of the .ttc: {on_a:?}");
    assert_eq!(ttc_on_a[0].2, ttc_on_a[1].2, "one file, one hash");

    let b = second_machine();
    let imported = b
        .api
        .load_set_file(LoadSetFileRequest {
            path: file.display().to_string(),
        })
        .expect("imported");

    // Bebas Neue is not on B: it is listed with its family and style.
    let missing: Vec<_> = imported
        .missing
        .iter()
        .map(|m| (m.family.as_str(), m.style.as_str()))
        .collect();
    assert_eq!(missing, [("Bebas Neue", "Regular")]);
    // Every other font matched by hash, renamed, moved to another source,
    // and each face of the .ttc by family and style.
    let expected: Vec<_> = on_a
        .iter()
        .filter(|(family, ..)| family != "Bebas Neue")
        .cloned()
        .collect();
    assert_eq!(entries(&b, &imported.set.faces), expected);
    let paths: Vec<String> = imported
        .set
        .faces
        .iter()
        .map(|f| b.api.get_face(*f).expect("detail").path)
        .collect();
    assert!(
        paths.iter().any(|p| p.ends_with("my-barlow-copy.ttf")),
        "{paths:?}"
    );
    assert!(paths.iter().any(|p| p.contains("Elsewhere")), "{paths:?}");

    // Activating the set activates the same files, by hash, on A and on B
    // (less the font B does not have; web fonts are skipped on both).
    let active_on_a = activate_set(&a, set.id);
    assert!(active_on_a.len() >= 3, "{active_on_a:?}");
    let expected_active: Vec<_> = active_on_a
        .into_iter()
        .filter(|(family, ..)| family != "Bebas Neue")
        .collect();
    assert!(
        expected_active
            .iter()
            .any(|(family, ..)| family == "Agdasima")
    );
    assert_eq!(activate_set(&b, imported.set.id), expected_active);
}

/// Activates a set for the session; `(family, style, hash)` of what is active.
fn activate_set(lib: &Lib, set: SetId) -> Vec<(String, String, String)> {
    lib.api
        .activate(ActivateRequest {
            target: Target::Set { set },
            mode: ActivationMode::Session,
            on_conflict: ConflictPolicy::default(),
        })
        .expect("activate");
    let active: Vec<FaceId> = lib
        .api
        .list_active()
        .expect("active")
        .into_iter()
        .map(|a| a.face)
        .collect();
    entries(lib, &active)
}

// --- fonts needed (E29.2) ----------------------------------------------

fn needed_ps(name: &str) -> NeededFont {
    NeededFont {
        postscript_name: Some(name.into()),
        ..NeededFont::default()
    }
}

fn needed_family(family: &str, style: Option<&str>) -> NeededFont {
    NeededFont {
        family: Some(family.into()),
        style: style.map(str::to_owned),
        ..NeededFont::default()
    }
}

fn needed(fonts: Vec<NeededFont>, activate: bool) -> FontsNeededRequest {
    FontsNeededRequest {
        fonts,
        document: None,
        activate,
        on_conflict: Default::default(),
    }
}

/// The first face of the file `relative` under the library folder.
fn face_at(lib: &Lib, relative: &str) -> FaceId {
    let path = lib.root.join(relative);
    (1..1000)
        .map(FaceId)
        .find(|&id| {
            lib.api
                .get_face(id)
                .is_ok_and(|f| Path::new(&f.path) == path)
        })
        .unwrap_or_else(|| panic!("no face at {relative}"))
}

/// The face and rule of each match.
fn hits(res: &FontsNeededResponse) -> Vec<(FaceId, NeededFontRule)> {
    res.matched.iter().map(|m| (m.face, m.matched_by)).collect()
}

/// Criterion 2: every rule on real names. Archivo SemiBold has a typographic
/// family ("Archivo" / "SemiBold") that differs from its legacy one
/// ("Archivo SemiBold" / "Regular") and from its full name.
#[test]
fn fonts_needed_matches_each_rule_on_the_fixtures_and_reports_the_rest() {
    let (lib, _, _) = Lib::scanned("api-needed-rules");
    let barlow = face_at(&lib, "barlow/Barlow-BoldItalic.ttf");
    let semibold = face_at(&lib, "archivo/Archivo-SemiBold.woff2");
    let bebas = face_at(&lib, "bebasneue/BebasNeue-Regular.ttf");
    let asap = face_at(&lib, "asapsharp/AsapSharp[wdth,wght].ttf");
    let full = NeededFont {
        postscript_name: Some("NotInTheLibrary".into()),
        full_name: Some(" BEBAS NEUE regular ".into()),
        family: Some("Barlow".into()),
        style: Some("Bold Italic".into()),
    };
    let res = lib
        .api
        .fonts_needed(needed(
            vec![
                needed_ps("Helvetica-Bold"),
                needed_ps("barlow-bolditalic"),
                full.clone(),
                needed_family("archivo", Some("SEMIBOLD")),
                needed_family("Archivo SemiBold", None),
                needed_family("Inter", Some("Bold")),
                needed_family("Asap Sharp", Some("Regular")),
                needed_family("Archivo", Some("Black")),
            ],
            false,
        ))
        .expect("fonts needed");
    assert_eq!(
        hits(&res),
        [
            (barlow, NeededFontRule::PostscriptName),
            // The PostScript name is not there: the full name decides
            // before the family does.
            (bebas, NeededFontRule::FullName),
            (semibold, NeededFontRule::TypographicFamily),
            (semibold, NeededFontRule::Family),
            (asap, NeededFontRule::Family),
        ]
    );
    assert_eq!(res.matched[1].font, full, "the font as asked");
    assert_eq!(
        res.unmatched,
        [
            needed_ps("Helvetica-Bold"),
            needed_family("Inter", Some("Bold")),
            needed_family("Archivo", Some("Black")),
        ]
    );
    assert_eq!(res.missing, 3);
    assert!(res.outcomes.is_empty(), "activate false activates nothing");
    assert!(lib.api.list_active().expect("active").is_empty());
    assert!(lib.activator.list_active().expect("os list").is_empty());
}

/// A library with the fixtures, a healthy copy of Bebas Neue in `extra/`
/// (a higher face id) and a broken one (the file cut short, its names
/// intact) in `0-cut/` (a lower id than both).
fn bebas_copies(label: &str) -> (Lib, [FaceId; 3]) {
    let lib = Lib::new(label);
    let bytes = std::fs::read(fixtures().join("google-repo/bebasneue/BebasNeue-Regular.ttf"))
        .expect("fixture");
    for (folder, data) in [
        ("extra", &bytes[..]),
        ("0-cut", &bytes[..bytes.len() * 99 / 100]),
    ] {
        let to = lib.root.join(folder);
        std::fs::create_dir_all(&to).expect("folder");
        std::fs::write(to.join("BebasNeue-Regular.ttf"), data).expect("copy");
    }
    lib.api
        .add_source(AddSourceRequest {
            path: lib.root.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    lib.scan(None, true);
    let ids = [
        face_at(&lib, "0-cut/BebasNeue-Regular.ttf"),
        face_at(&lib, "bebasneue/BebasNeue-Regular.ttf"),
        face_at(&lib, "extra/BebasNeue-Regular.ttf"),
    ];
    (lib, ids)
}

/// Criterion 4: of several faces with the name, an active one wins, then a
/// healthy one, then the lowest id; the broken copy never does.
#[test]
fn of_duplicate_faces_the_active_then_the_lowest_healthy_one_is_matched() {
    let (lib, [broken, first, copy]) = bebas_copies("api-needed-dups");
    assert!(broken < first && first < copy, "{broken} {first} {copy}");
    let detail = lib.api.get_face(broken).expect("broken face");
    assert!(matches!(detail.face.health, Health::Broken(_)));
    let ask = || {
        lib.api
            .fonts_needed(needed(
                vec![
                    needed_ps("BebasNeue-Regular"),
                    needed_family("Bebas Neue", None),
                ],
                false,
            ))
            .expect("fonts needed")
    };
    assert_eq!(
        hits(&ask()),
        [
            (first, NeededFontRule::PostscriptName),
            (first, NeededFontRule::Family)
        ]
    );

    lib.api
        .activate(ActivateRequest {
            target: Target::Faces { faces: vec![copy] },
            mode: ActivationMode::Session,
            on_conflict: ConflictPolicy::default(),
        })
        .expect("activate the copy");
    assert_eq!(
        hits(&ask()),
        [
            (copy, NeededFontRule::PostscriptName),
            (copy, NeededFontRule::Family)
        ]
    );
}

/// Criterion 5: with `activate` the matched faces are session-activated
/// through the activator, one outcome per face, a clash included, and the
/// document is recorded; the match report is the one without `activate`.
#[test]
fn fonts_needed_activates_the_matched_faces_for_the_session() {
    let (lib, _, _) = Lib::scanned("api-needed-activate");
    let barlow = face_at(&lib, "barlow/Barlow-BoldItalic.ttf");
    let bebas = face_at(&lib, "bebasneue/BebasNeue-Regular.ttf");
    lib.activator.install(
        "Agdasima-Regular",
        "/System/Fonts/Agdasima.ttf",
        fm_platform::ConflictKind::System,
    );
    let fonts = vec![
        needed_ps("Barlow-BoldItalic"),
        needed_ps("Missing-Font"),
        needed_family("Bebas Neue", Some("Regular")),
        needed_ps("Agdasima-Regular"),
        // The same face again, by another name: one outcome.
        NeededFont {
            full_name: Some("Barlow Bold Italic".into()),
            ..NeededFont::default()
        },
    ];
    let dry = lib
        .api
        .fonts_needed(needed(fonts.clone(), false))
        .expect("match only");
    let agdasima = dry.matched[2].face;
    assert_eq!(dry.missing, 1);
    let rx = lib.api.subscribe();
    let res = lib
        .api
        .fonts_needed(FontsNeededRequest {
            fonts,
            document: Some("poster.indd".into()),
            activate: true,
            on_conflict: Default::default(),
        })
        .expect("activate");
    assert_eq!(
        (&res.matched, &res.unmatched, res.missing),
        (&dry.matched, &dry.unmatched, dry.missing),
        "the same report either way"
    );
    assert_eq!(
        res.outcomes,
        [
            ActivationOutcome {
                face: barlow,
                outcome: OutcomeStatus::Done
            },
            ActivationOutcome {
                face: bebas,
                outcome: OutcomeStatus::Done
            },
            ActivationOutcome {
                face: agdasima,
                outcome: OutcomeStatus::Conflict {
                    with: fm_api::activation::ConflictWith::System {
                        path: "/System/Fonts/Agdasima.ttf".into()
                    }
                }
            },
        ]
    );
    assert!(matches!(
        rx.recv_timeout(WAIT),
        Ok(Event::ActivationChanged { activated, .. }) if activated == [barlow, bebas]
    ));
    assert_eq!(lib.activator.list_active().expect("os list").len(), 2);
    let active = lib.api.list_active().expect("active");
    assert_eq!(
        active
            .iter()
            .map(|a| (a.face, a.mode, a.document.as_deref()))
            .collect::<Vec<_>>(),
        [
            (barlow, ActivationMode::Session, Some("poster.indd")),
            (bebas, ActivationMode::Session, Some("poster.indd")),
        ]
    );

    // Asked again by another document: nothing new, and the face shows the
    // document that asked last (E29.3).
    let again = lib
        .api
        .fonts_needed(FontsNeededRequest {
            fonts: vec![needed_ps("Barlow-BoldItalic")],
            document: Some("other.indd".into()),
            activate: true,
            on_conflict: Default::default(),
        })
        .expect("again");
    assert_eq!(again.outcomes[0].outcome, OutcomeStatus::Unchanged);
    assert_eq!(
        lib.api.list_active().expect("active")[0]
            .document
            .as_deref(),
        Some("other.indd")
    );
    // Through the API: closing one document offers only Bebas Neue, which
    // the other does not need.
    let offer = lib
        .api
        .document_closed(DocumentClosedRequest {
            document: "poster.indd".into(),
        })
        .expect("closed")
        .offer;
    assert_eq!(offer, [bebas]);
    assert_eq!(lib.activator.list_active().expect("os list").len(), 2);
}

/// Review cycle 1 (COR-1): a face the user activated permanently stays
/// permanent when a document asks for it; it is reported unchanged and
/// does not take the document.
#[test]
fn fonts_needed_leaves_a_permanently_active_face_permanent() {
    let (lib, _, _) = Lib::scanned("api-needed-permanent");
    let barlow = face_at(&lib, "barlow/Barlow-BoldItalic.ttf");
    let bebas = face_at(&lib, "bebasneue/BebasNeue-Regular.ttf");
    lib.api
        .activate(ActivateRequest {
            target: Target::Faces {
                faces: vec![barlow],
            },
            mode: ActivationMode::Permanent,
            on_conflict: ConflictPolicy::default(),
        })
        .expect("activate permanently");
    let res = lib
        .api
        .fonts_needed(FontsNeededRequest {
            fonts: vec![
                needed_ps("Barlow-BoldItalic"),
                needed_ps("BebasNeue-Regular"),
                needed_family("Barlow", Some("Bold Italic")),
            ],
            document: Some("poster.indd".into()),
            activate: true,
            on_conflict: Default::default(),
        })
        .expect("fonts needed");
    assert_eq!(
        res.outcomes,
        [
            ActivationOutcome {
                face: barlow,
                outcome: OutcomeStatus::Unchanged
            },
            ActivationOutcome {
                face: bebas,
                outcome: OutcomeStatus::Done
            },
        ]
    );
    let active: Vec<_> = lib
        .api
        .list_active()
        .expect("active")
        .into_iter()
        .map(|a| (a.face, a.mode, a.document))
        .collect();
    assert_eq!(
        active,
        [
            (barlow, ActivationMode::Permanent, None),
            (
                bebas,
                ActivationMode::Session,
                Some("poster.indd".to_owned())
            ),
        ]
    );
    let os = lib.activator.list_active().expect("os list");
    assert!(
        os.iter()
            .any(|r| r.mode == fm_platform::Mode::Permanent
                && r.path.ends_with("Barlow-BoldItalic.ttf")),
        "the permanent registration is kept: {os:?}"
    );
}

/// Review cycle 1 (TST-1): a broken face (the file cut short, its names
/// intact) is never matched, also when it is the only face with the name.
#[test]
fn a_broken_face_alone_is_not_matched() {
    let lib = Lib::new("api-needed-broken");
    let bytes =
        std::fs::read(fixtures().join("google-static/asap/Asap_Condensed-ExtraLightItalic.ttf"))
            .expect("fixture");
    let to = lib.root.join("0-cut");
    std::fs::create_dir_all(&to).expect("folder");
    std::fs::write(to.join("Cut.ttf"), &bytes[..bytes.len() * 99 / 100]).expect("cut copy");
    lib.api
        .add_source(AddSourceRequest {
            path: lib.root.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    lib.scan(None, true);
    let cut = lib
        .api
        .get_face(face_at(&lib, "0-cut/Cut.ttf"))
        .expect("cut face");
    assert!(
        matches!(cut.face.health, Health::Broken(_)),
        "{:?}",
        cut.face.health
    );
    let ps = cut.face.postscript_name.clone();
    assert!(!ps.is_empty(), "the cut copy keeps its names");
    let font = NeededFont {
        postscript_name: Some(ps),
        family: Some(cut.face.family_name.clone()),
        style: Some(cut.face.style.clone()),
        ..NeededFont::default()
    };
    let res = lib
        .api
        .fonts_needed(needed(vec![font.clone()], true))
        .expect("fonts needed");
    assert!(res.matched.is_empty(), "{:?}", res.matched);
    assert_eq!(res.unmatched, [font]);
    assert_eq!(res.missing, 1);
    assert!(res.outcomes.is_empty());
    assert!(lib.activator.list_active().expect("os list").is_empty());
}

// --- workflows (E27.4) ---------------------------------------------------

fn brief(text: &str, languages: &[&str], limit: Option<u32>) -> BriefRequest {
    BriefRequest {
        brief: text.into(),
        languages: languages.iter().map(|l| (*l).to_owned()).collect(),
        license: None,
        limit,
    }
}

/// The families with a usable face that covers every language in `codes`.
fn families_covering(lib: &Lib, codes: &[&str]) -> Vec<String> {
    let families = lib
        .api
        .list_families(ListFamiliesRequest {
            page: Page {
                offset: 0,
                limit: 1000,
            },
            ..ListFamiliesRequest::default()
        })
        .expect("families")
        .items;
    let mut out = Vec::new();
    for fam in families {
        let faces = lib.api.get_family(fam.id).expect("family").faces;
        let covers = faces.iter().filter(|f| f.health == Health::Ok).any(|f| {
            lib.api
                .check_coverage(CoverageRequest {
                    face: f.id,
                    target: Supports::Languages {
                        codes: codes.iter().map(|c| (*c).to_owned()).collect(),
                    },
                })
                .expect("coverage")
                .complete
        });
        if covers {
            out.push(fam.name);
        }
    }
    out.sort();
    out
}

#[test]
fn a_serbian_brief_returns_only_families_covering_sr_cyrl_and_sr_latn() {
    let (lib, _, _) = Lib::scanned_with(
        "api-brief-serbian",
        &[
            "google-repo/comfortaa/Comfortaa[wght].ttf",
            "fontshare/montserrat/Montserrat-LightItalic.woff2",
        ],
    );
    let serbian = ["sr-Cyrl", "sr-Latn"];
    let expected = families_covering(&lib, &serbian);
    assert!(!expected.is_empty(), "a fixture covers Serbian");
    let all = lib.names();
    assert!(
        expected.len() < all.len(),
        "a Latin-only family is left out"
    );

    let res = lib
        .api
        .pick_fonts_for_brief(brief("a calm bilingual brand", &serbian, Some(50)))
        .expect("brief");
    let mut names: Vec<String> = res
        .candidates
        .iter()
        .map(|c| c.family.name.clone())
        .collect();
    names.sort();
    assert_eq!(names, expected);
    for c in &res.candidates {
        assert_eq!(c.reasons[0], "Covers sr-Cyrl and sr-Latn", "{c:?}");
        assert!(c.reasons[1].starts_with("No license use asked"), "{c:?}");
    }
    // Deterministic, and `limit` defaults to 5.
    let again = lib
        .api
        .pick_fonts_for_brief(brief("a calm bilingual brand", &serbian, Some(50)))
        .expect("again");
    assert_eq!(again, res);
    let five = lib
        .api
        .pick_fonts_for_brief(brief("anything", &[], None))
        .expect("default limit");
    assert_eq!(five.candidates.len(), all.len().min(5));
    assert_eq!(lib.fetcher.calls(), 0, "no network");
}

#[test]
fn brief_words_rank_by_class_tags_and_names_with_license_reasons() {
    let (lib, _, _) = Lib::scanned("api-brief-rank");
    let barlow = lib.family("Barlow");
    lib.api
        .tag_faces(TagFacesRequest {
            families: vec![barlow.id],
            faces: Vec::new(),
            tags: vec!["Wayfinding".into()],
        })
        .expect("tagged");
    let res = lib
        .api
        .pick_fonts_for_brief(BriefRequest {
            license: Some(LicenseType::Web),
            ..brief("signage for wayfinding", &[], Some(3))
        })
        .expect("brief");
    assert_eq!(res.candidates.len(), 3);
    let top = &res.candidates[0];
    assert_eq!(top.family.id, barlow.id);
    assert_eq!(top.reasons[0], "No languages asked");
    assert!(
        top.reasons[1].starts_with("Allows web use: Open license"),
        "{top:?}"
    );
    assert!(top.reasons.contains(&"Tagged \"Wayfinding\"".to_owned()));

    // A name word ranks that family first.
    let res = lib
        .api
        .pick_fonts_for_brief(brief("something like archivo", &[], Some(1)))
        .expect("name");
    assert_eq!(res.candidates[0].family.name, "Archivo");
    assert!(
        res.candidates[0]
            .reasons
            .contains(&"Name \"Archivo\" matches \"archivo\"".to_owned())
    );

    // The user's declaration beats the open license in the file, as in the
    // license panel's web rule: declared desktop is not OK for web.
    let archivo = lib.family("Archivo");
    lib.api
        .declare_license(LicenseDeclaration {
            family: archivo.id,
            license_type: Some(LicenseType::Desktop),
            seats: None,
            purchase_date: None,
            receipt: ReceiptChange::Keep,
        })
        .expect("declare");
    let pick = |license: LicenseType| {
        lib.api
            .pick_fonts_for_brief(BriefRequest {
                license: Some(license),
                ..brief("archivo", &[], Some(50))
            })
            .expect("brief")
            .candidates
    };
    let web = pick(LicenseType::Web);
    assert!(!web.is_empty());
    assert!(web.iter().all(|c| c.family.id != archivo.id), "{web:?}");
    let face = lib.api.get_family(archivo.id).expect("family").faces[0].id;
    let info = lib.api.get_face(face).expect("face").license;
    assert_eq!(info.web_use.map(|w| w.ok), Some(false), "the panel agrees");
    let desktop = pick(LicenseType::Desktop);
    let ours = desktop
        .iter()
        .find(|c| c.family.id == archivo.id)
        .expect("declared desktop allows desktop");
    assert_eq!(
        ours.reasons[1],
        "Allows desktop use: declared desktop license"
    );
}

#[test]
fn a_brief_refuses_an_unknown_language_and_a_bad_limit() {
    let (lib, _, _) = Lib::scanned("api-brief-refusals");
    let e = lib
        .api
        .pick_fonts_for_brief(brief("x", &["xx-Nope"], None))
        .expect_err("unknown preset");
    assert_eq!(e.code, ErrorCode::InvalidRequest);
    for limit in [0, 51] {
        let e = lib
            .api
            .pick_fonts_for_brief(brief("x", &[], Some(limit)))
            .expect_err("limit");
        assert_eq!(e.code, ErrorCode::InvalidRequest, "{limit}");
    }
}

#[test]
fn prepare_for_web_activates_the_family_and_writes_woff2_with_css() {
    let (lib, _, _) = Lib::scanned_with(
        "api-prepare-web",
        &[
            "google-static/glory/Glory-Bold.ttf",
            "google-static/glory/Glory-ExtraBoldItalic.ttf",
        ],
    );
    let family = lib.api.get_family(lib.family("Glory").id).expect("family");
    let faces: Vec<FaceId> = family
        .faces
        .iter()
        .filter(|f| f.health == Health::Ok)
        .map(|f| f.id)
        .collect();
    assert!(faces.len() > 1);
    // A face already active for good stays so (it is not resent).
    let permanent = faces[0];
    lib.api
        .activate(ActivateRequest {
            target: Target::Faces {
                faces: vec![permanent],
            },
            mode: ActivationMode::Permanent,
            on_conflict: ConflictPolicy::KeepExisting,
        })
        .expect("permanent");
    let out = lib.dir.path().join("web");
    std::fs::create_dir_all(&out).expect("out");
    let res = lib
        .api
        .prepare_for_web(PrepareForWebRequest {
            family: family.summary.id,
            subset: Some(Subset::Presets {
                names: vec!["latin".into()],
            }),
            out_dir: out.display().to_string(),
        })
        .expect("prepare");
    assert_eq!(
        res.activated.iter().map(|o| o.face).collect::<Vec<_>>(),
        faces
    );
    assert_eq!(res.activated[0].outcome, OutcomeStatus::Unchanged);
    assert!(
        res.activated[1..]
            .iter()
            .all(|o| o.outcome == OutcomeStatus::Done),
        "{:?}",
        res.activated
    );
    let active = lib.api.list_active().expect("active");
    assert_eq!(active.len(), faces.len());
    for a in &active {
        let want = if a.face == permanent {
            ActivationMode::Permanent
        } else {
            ActivationMode::Session
        };
        assert_eq!(a.mode, want, "{a:?}");
    }
    let written = files_under(&out);
    assert_eq!(written.len(), faces.len() + 1, "{written:?}");
    assert!(written.iter().any(|p| p == Path::new("fonts.css")));
    assert!(
        written
            .iter()
            .filter(|p| *p != Path::new("fonts.css"))
            .all(|p| p.extension().is_some_and(|e| e == "woff2")),
        "{written:?}"
    );
    assert!(
        res.export
            .css
            .as_deref()
            .is_some_and(|c| c.contains("@font-face"))
    );
}

#[test]
fn prepare_for_web_checks_the_license_first_and_activates_nothing_when_refused() {
    let lib = Lib::new("api-prepare-web-guard");
    let folder = lib.dir.path().join("Patched");
    with_fs_type(
        "google-repo/bebasneue/BebasNeue-Regular.ttf",
        0x0002,
        &folder,
    );
    let source = lib
        .api
        .add_source(AddSourceRequest {
            path: folder.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    lib.scan(Some(source.id), true);
    let out = lib.dir.path().join("web");
    std::fs::create_dir_all(&out).expect("out");
    let e = lib
        .api
        .prepare_for_web(PrepareForWebRequest {
            family: lib.family("Bebas Neue").id,
            subset: None,
            out_dir: out.display().to_string(),
        })
        .expect_err("refused");
    assert_eq!(e.code, ErrorCode::PermissionDenied, "{e:?}");
    assert!(lib.api.list_active().expect("active").is_empty());
    assert!(lib.activator.list_active().expect("os list").is_empty());
    assert!(files_under(&out).is_empty());

    let e = lib
        .api
        .prepare_for_web(PrepareForWebRequest {
            family: FamilyId(999_999),
            subset: None,
            out_dir: out.display().to_string(),
        })
        .expect_err("unknown");
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[test]
fn prepare_for_web_deactivates_what_it_activated_when_the_export_fails() {
    let (lib, _, _) = Lib::scanned("api-prepare-web-rollback");
    let archivo = lib.family("Archivo").id;
    let out = lib.dir.path().join("web");
    std::fs::create_dir_all(&out).expect("out");
    let req = PrepareForWebRequest {
        family: archivo,
        subset: None,
        out_dir: out.display().to_string(),
    };
    lib.api.prepare_for_web(req.clone()).expect("first");
    lib.api
        .deactivate(DeactivateRequest {
            target: None,
            except: None,
        })
        .expect("deactivate");
    // The files are there now: the export refuses to replace them.
    let e = lib.api.prepare_for_web(req).expect_err("exists");
    assert_ne!(e.code, ErrorCode::PermissionDenied, "{e:?}");
    assert!(lib.api.list_active().expect("active").is_empty());
    assert!(lib.activator.list_active().expect("os list").is_empty());
}

/// E32.1: `document_fonts` through the core API reads the fixture document
/// (and only reads it); an unreadable one is the plain-message error.
#[test]
fn document_fonts_lists_a_documents_fonts_and_leaves_it_alone() {
    use fm_api::workflows::{DocumentFontsRequest, DocumentFormat};
    let lib = Lib::new("api-document-fonts");
    let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/documents");
    let path = lib.dir.path().join("Brand deck.pptx");
    std::fs::copy(docs.join("sample.pptx"), &path).expect("copy");
    let before = std::fs::read(&path).expect("bytes");
    let res = lib
        .api
        .document_fonts(DocumentFontsRequest {
            path: path.to_string_lossy().into_owned(),
        })
        .expect("fonts");
    assert_eq!(res.format, DocumentFormat::Pptx);
    let families: Vec<_> = res
        .fonts
        .iter()
        .filter_map(|f| f.family.as_deref())
        .collect();
    assert_eq!(
        families,
        ["Montserrat", "Noto Sans KR", "Open Sans", "Roboto"]
    );
    assert_eq!(std::fs::read(&path).expect("bytes"), before);

    let e = lib
        .api
        .document_fonts(DocumentFontsRequest {
            path: docs.join("zstd.fig").to_string_lossy().into_owned(),
        })
        .expect_err("zstd");
    assert_eq!(e.code, ErrorCode::Unsupported);
    assert_eq!(e.message, "This document's fonts could not be read.");
}

/// E32.2: `check_document_fonts` matches a document's fonts the way
/// `fonts_needed` does and marks each active, in the library or missing;
/// it activates nothing and only reads the document. Session-activating
/// `to_activate` turns those rows active.
#[test]
fn check_document_fonts_marks_active_in_library_and_missing_fonts() {
    use fm_api::workflows::{DocumentFontStatus, DocumentFontsRequest, DocumentFormat};
    let (lib, _, _) = Lib::scanned("api-check-document-fonts");
    let barlow = face_at(&lib, "barlow/Barlow-BoldItalic.ttf");
    let bebas = face_at(&lib, "bebasneue/BebasNeue-Regular.ttf");
    lib.api
        .activate(ActivateRequest {
            target: Target::Faces { faces: vec![bebas] },
            mode: ActivationMode::Session,
            on_conflict: ConflictPolicy::KeepExisting,
        })
        .expect("activate");
    let path = lib.dir.path().join("Poster ü.ai");
    let doc = b"%!PS-Adobe-3.0
/BaseFont /Barlow-BoldItalic
/BaseFont /BebasNeue-Regular
/BaseFont /Missing-Font
";
    std::fs::write(&path, doc).expect("write");
    let req = || DocumentFontsRequest {
        path: path.to_string_lossy().into_owned(),
    };
    let res = lib.api.check_document_fonts(req()).expect("check");
    assert_eq!(res.format, DocumentFormat::Ai);
    let rows: Vec<_> = res
        .fonts
        .iter()
        .map(|f| (f.font.postscript_name.as_deref(), f.status, f.face))
        .collect();
    assert_eq!(
        rows,
        [
            (
                Some("Barlow-BoldItalic"),
                DocumentFontStatus::InLibrary,
                Some(barlow)
            ),
            (
                Some("BebasNeue-Regular"),
                DocumentFontStatus::Active,
                Some(bebas)
            ),
            (Some("Missing-Font"), DocumentFontStatus::Missing, None),
        ]
    );
    assert_eq!(res.to_activate, [barlow]);
    assert_eq!(lib.api.list_active().expect("active").len(), 1);

    let rx = lib.api.subscribe();
    lib.api
        .activate(ActivateRequest {
            target: Target::Faces {
                faces: res.to_activate.clone(),
            },
            mode: ActivationMode::Session,
            on_conflict: ConflictPolicy::KeepExisting,
        })
        .expect("activate");
    assert!(
        rx.try_iter().any(|e| matches!(
            e,
            Event::ActivationChanged { ref activated, .. } if activated.contains(&barlow)
        )),
        "the activation event the panel follows"
    );
    let after = lib.api.check_document_fonts(req()).expect("again");
    assert_eq!(after.fonts[0].status, DocumentFontStatus::Active);
    assert_eq!(after.fonts[2].status, DocumentFontStatus::Missing);
    assert!(after.to_activate.is_empty());
    assert_eq!(std::fs::read(&path).expect("bytes"), doc);

    let other = lib.dir.path().join("notes.txt");
    std::fs::write(&other, b"x").expect("write");
    let e = lib
        .api
        .check_document_fonts(DocumentFontsRequest {
            path: other.to_string_lossy().into_owned(),
        })
        .expect_err("not a document");
    assert_eq!(e.code, ErrorCode::Unsupported);
    assert!(e.message.contains(".indd"), "{}", e.message);
    let e = lib
        .api
        .check_document_fonts(DocumentFontsRequest {
            path: lib
                .dir
                .path()
                .join("gone.psd")
                .to_string_lossy()
                .into_owned(),
        })
        .expect_err("missing");
    assert_eq!(e.message, "This document's fonts could not be read.");
    assert_eq!(std::fs::read(&other).expect("bytes"), b"x");
}

/// E33.1: `find_by_image` through the core API reads the screenshot (and
/// only reads it) and ranks the library's faces against it; a file that is
/// not a PNG and an image without text are plain errors.
#[test]
fn find_by_image_ranks_the_faces_like_the_screenshot() {
    use fm_api::search::ImageMatchRequest;
    let (lib, _, _) = Lib::scanned_with(
        "api-find-by-image",
        &[
            "google-repo/crimsonpro/CrimsonPro[wght].ttf",
            "google-repo/clickerscript/ClickerScript-Regular.ttf",
            "google-repo/barlow/Barlow-BoldItalic.ttf",
        ],
    );
    let shot = lib.dir.path().join("Screenshot 1.png");
    let png = crate::image_match::tests::render(
        "google-repo/crimsonpro/CrimsonPro[wght].ttf",
        crate::fingerprint::SAMPLE,
        48.0,
        "#000000",
        "#ffffff",
    )
    .to_png();
    std::fs::write(&shot, &png).expect("write");
    let req = |path: &Path, limit| ImageMatchRequest {
        path: path.to_string_lossy().into_owned(),
        limit,
    };
    let found = lib.api.find_by_image(req(&shot, 2)).expect("found");
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0].face.family_name, "Crimson Pro", "{found:?}");
    assert!(found[0].score >= found[1].score);
    assert!(
        found[0]
            .reasons
            .iter()
            .any(|r| r == "similar letter shapes")
    );
    assert_eq!(std::fs::read(&shot).expect("bytes"), png);

    let text = lib.dir.path().join("notes.png");
    std::fs::write(&text, b"not an image").expect("write");
    let e = lib.api.find_by_image(req(&text, 2)).expect_err("not a png");
    assert_eq!(e.code, ErrorCode::Unsupported);
    assert_eq!(e.message, "Only PNG images can be read.");

    let blank = crate::preview::Raster {
        width: 64,
        height: 32,
        baseline: 0,
        rgba: [255, 255, 255, 255].repeat(64 * 32),
    };
    std::fs::write(&text, blank.to_png()).expect("write");
    let e = lib.api.find_by_image(req(&text, 2)).expect_err("blank");
    assert_eq!(e.code, ErrorCode::InvalidRequest);
    assert_eq!(e.message, "No text was found in this image.");

    let e = lib
        .api
        .find_by_image(req(&lib.dir.path().join("gone.png"), 2))
        .expect_err("missing");
    assert_eq!(e.code, ErrorCode::NotFound);
}

// --- fetch a missing document font (E29.5) ------------------------------

/// Turns "Download fonts a document needs" on or off.
fn set_fetch_missing(lib: &Lib, on: bool) {
    lib.api
        .update_settings(SettingsUpdate {
            fetch_missing_fonts: Some(on),
            ..SettingsUpdate::default()
        })
        .expect("fetch switch");
}

/// The Lato server's static fonts, as a document names them: the regular
/// file is Cagliostro, the bold one Asul, both of the catalog family Lato.
fn lato_fonts() -> Vec<NeededFont> {
    vec![
        NeededFont {
            postscript_name: Some("Cagliostro-Regular".into()),
            family: Some("Lato".into()),
            ..NeededFont::default()
        },
        NeededFont {
            postscript_name: Some("Asul-Bold".into()),
            family: Some("Lato".into()),
            ..NeededFont::default()
        },
    ]
}

fn lato_ok() -> crate::test_support::LocalServer {
    crate::catalog::tests::lato_server((200, crate::catalog::tests::lato_font("bold")))
}

/// `lib` with Google Fonts on, refreshed from `server`, the download folder
/// set and the fetch switch `on`.
fn fetching(
    label: &str,
    server: crate::test_support::LocalServer,
    on: bool,
) -> (Lib, crate::test_support::LocalServer) {
    let (lib, _source, server) = downloading_from(label, server);
    set_download_folder(&lib, &lib.dir.path().join("Font Manager Fonts"));
    set_fetch_missing(&lib, on);
    (lib, server)
}

/// The same library with its catalogs over `fetcher`, on the cache the
/// refresh wrote.
fn with_fetcher(lib: Lib, fetcher: Arc<dyn Fetcher>) -> Lib {
    let catalogs = Catalogs::new(
        lib.paths.data.join(CATALOGS_DIR),
        fetcher,
        vec![Arc::new(GoogleFonts::with_base_url("http://127.0.0.1:9"))],
    );
    Lib {
        api: lib.api.with_catalogs(catalogs),
        ..lib
    }
}

/// The names in `dir`, sorted; none when it is not there.
fn listing_of_or_none(dir: &Path) -> Vec<String> {
    if dir.is_dir() {
        listing_of(dir)
    } else {
        Vec::new()
    }
}

#[test]
fn with_the_fetch_switch_off_a_missing_font_stays_unmatched_and_nothing_is_fetched() {
    let (lib, server) = fetching("fetch-off", lato_ok(), false);
    let hits = server.hits();
    let res = lib
        .api
        .fonts_needed(needed(lato_fonts(), true))
        .expect("needed");
    assert_eq!(res.unmatched, lato_fonts());
    assert!(res.fetched.is_empty());
    assert_eq!(server.hits(), hits, "no request");
    assert!(listing_of_or_none(&lib.dir.path().join("Font Manager Fonts").join("Lato")).is_empty());
}

#[test]
fn a_missing_font_whose_family_is_cached_is_downloaded_once_activated_and_matched() {
    let (lib, server) = fetching("fetch-on", lato_ok(), true);
    let hits = server.hits();
    let mut fonts = lato_fonts();
    // A font the library has: matched as before, never looked up.
    fonts.push(needed_family("Bebas Neue", None));
    let res = lib
        .api
        .fonts_needed(FontsNeededRequest {
            document: Some("doc-1".into()),
            ..needed(fonts.clone(), true)
        })
        .expect("needed");

    // One family, fetched once: its file list and its two static files.
    assert_eq!(server.hits() - hits, 3, "one download of Lato");
    assert!(res.unmatched.is_empty(), "{res:?}");
    assert_eq!(res.missing, 0);
    assert_eq!(res.matched.len(), 3);
    for (m, font) in res.matched.iter().zip(&fonts) {
        assert_eq!(&m.font, font);
    }
    // The normal rule found them: the PostScript name.
    assert_eq!(res.matched[0].matched_by, NeededFontRule::PostscriptName);
    assert_eq!(res.matched[1].matched_by, NeededFontRule::PostscriptName);

    assert_eq!(
        res.fetched.len(),
        2,
        "both fonts of the family: {:?}",
        res.fetched
    );
    for (f, font) in res.fetched.iter().zip(&lato_fonts()) {
        assert_eq!(&f.font, font);
        assert_eq!(f.catalog, fm_api::catalog::CatalogId::GoogleFonts);
        assert_eq!(f.family, "Lato");
        assert_eq!(f.faces.len(), 2);
    }
    let fetched_faces = &res.fetched[0].faces;
    assert!(fetched_faces.contains(&res.matched[0].face));
    assert!(fetched_faces.contains(&res.matched[1].face));

    // Activated by `fonts_needed` itself: session mode, for the document.
    let active = lib.api.list_active().expect("active");
    for m in &res.matched[..2] {
        let a = active.iter().find(|a| a.face == m.face).expect("active");
        assert_eq!(a.mode, ActivationMode::Session);
        let o = res
            .outcomes
            .iter()
            .find(|o| o.face == m.face)
            .expect("outcome");
        assert_eq!(o.outcome, OutcomeStatus::Done);
    }
    let offer = lib
        .api
        .document_closed(DocumentClosedRequest {
            document: "doc-1".into(),
        })
        .expect("closed")
        .offer;
    assert!(offer.contains(&res.matched[0].face), "{offer:?}");
    assert!(offer.contains(&res.matched[1].face), "{offer:?}");

    // The files went into the download folder, recorded as a download.
    let folder = lib.dir.path().join("Font Manager Fonts").join("Lato");
    assert_eq!(listing_of(&folder), ["Lato-Bold.ttf", "Lato-Regular.ttf"]);
    assert_eq!(
        lib.api.list_catalog_downloads().expect("downloads").len(),
        1
    );

    // Asked again: in the library now, so no request and nothing fetched.
    let hits = server.hits();
    let again = lib
        .api
        .fonts_needed(needed(lato_fonts(), true))
        .expect("again");
    assert_eq!(server.hits(), hits);
    assert!(again.fetched.is_empty());
    assert!(again.unmatched.is_empty());
}

#[test]
fn a_font_whose_family_is_not_cached_stays_unmatched_and_makes_no_request() {
    let (lib, _server) = fetching("fetch-not-cached", lato_ok(), true);
    let offline = Arc::clone(&lib.fetcher);
    let lib = with_fetcher(lib, Arc::clone(&offline) as Arc<dyn Fetcher>);
    let calls = offline.calls();
    let unknown = vec![
        needed_family("Not In Any Catalog", Some("Bold")),
        NeededFont {
            full_name: Some("Lateral Bold".into()),
            ..NeededFont::default()
        },
        needed_ps("Nobody-Regular"),
    ];
    let res = lib
        .api
        .fonts_needed(needed(unknown.clone(), true))
        .expect("needed");
    assert_eq!(res.unmatched, unknown);
    assert!(res.fetched.is_empty());
    assert_eq!(offline.calls(), calls, "the lookup reads only the cache");
}

#[test]
fn a_dry_run_never_fetches() {
    let (lib, _server) = fetching("fetch-dry", lato_ok(), true);
    let offline = Arc::clone(&lib.fetcher);
    let lib = with_fetcher(lib, Arc::clone(&offline) as Arc<dyn Fetcher>);
    let calls = offline.calls();
    let res = lib
        .api
        .fonts_needed(needed(lato_fonts(), false))
        .expect("needed");
    assert_eq!(res.unmatched, lato_fonts());
    assert!(res.fetched.is_empty());
    assert_eq!(offline.calls(), calls, "activate: false never fetches");
    // With activate, the same cached family is fetched (and fails here).
    let res = lib
        .api
        .fonts_needed(needed(lato_fonts(), true))
        .expect("needed");
    assert!(
        offline.calls() > calls,
        "the switch is on and Lato is cached"
    );
    assert_eq!(res.unmatched, lato_fonts());
}

#[test]
fn a_failing_download_leaves_its_font_unmatched_and_the_others_are_fine() {
    let lato = crate::catalog::tests::lato_server((500, Vec::new()));
    let (lib, _server) = fetching("fetch-failing", lato, true);
    let mut fonts = lato_fonts();
    fonts.push(needed_family("Bebas Neue", None));
    let res = lib
        .api
        .fonts_needed(needed(fonts, true))
        .expect("no error for the others");
    assert_eq!(res.unmatched, lato_fonts());
    assert_eq!(res.missing, 2);
    assert!(res.fetched.is_empty());
    assert_eq!(res.matched.len(), 1);
    assert_eq!(res.matched[0].font, needed_family("Bebas Neue", None));
    assert_eq!(res.outcomes.len(), 1);
    assert_eq!(res.outcomes[0].outcome, OutcomeStatus::Done);
    assert!(
        listing_of_or_none(&lib.dir.path().join("Font Manager Fonts").join("Lato")).is_empty(),
        "a failed request writes nothing"
    );
}

/// A fetcher whose every call takes `delay`, then fails.
#[derive(Debug)]
struct SlowFetcher {
    delay: Duration,
    calls: AtomicU32,
}

impl Fetcher for SlowFetcher {
    fn get(&self, url: &str) -> FmResult<Vec<u8>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        std::thread::sleep(self.delay);
        Err(FmError::new(ErrorCode::Io, "slow").with_detail(url.to_owned()))
    }
}

#[test]
fn the_fetch_work_of_one_request_ends_with_its_budget() {
    let (lib, _server) = fetching("fetch-budget", lato_ok(), true);
    let slow = Arc::new(SlowFetcher {
        delay: Duration::from_secs(5),
        calls: AtomicU32::new(0),
    });
    let lib = with_fetcher(lib, Arc::clone(&slow) as Arc<dyn Fetcher>);
    let lib = Lib {
        api: lib.api.with_fetch_budget(Duration::from_millis(200)),
        ..lib
    };
    let started = Instant::now();
    let res = lib
        .api
        .fonts_needed(needed(lato_fonts(), true))
        .expect("needed");
    let took = started.elapsed();
    assert!(
        took < Duration::from_secs(3),
        "bounded by the budget: {took:?}"
    );
    assert_eq!(
        res.unmatched,
        lato_fonts(),
        "what did not finish stays unmatched"
    );
    assert!(res.fetched.is_empty());
    assert_eq!(
        slow.calls.load(Ordering::Relaxed),
        1,
        "one request started, then out of time"
    );
}

#[test]
fn the_default_fetch_budget_is_sixty_seconds() {
    assert_eq!(fetch_missing::FETCH_BUDGET, Duration::from_secs(60));
}
