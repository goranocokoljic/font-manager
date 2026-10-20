//! `CoreApi` over a copy of fixture fonts, one group of tests per service.

use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use fm_api::activation::{ActivationMode, ConflictPolicy, OutcomeStatus};
use fm_api::common::{AxisValue, Embedding, FontFormat, Health, Page};
use fm_api::library::{
    FamilyActivation, FamilySort, GlyphRequest, GlyphsRequest, OpenLicense, ProblemKind,
    TrashMoveStatus,
};
use fm_api::search::{Filters, Supports};
use fm_platform::{MockActivator, MockTrash};

use super::*;
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

/// A `CoreApi` over scratch app folders, a mock activator and a library
/// folder holding copies of [`FOLDERS`] (scans write sidecars into the
/// source, so the fixtures themselves are never a source).
struct Lib {
    dir: TempDir,
    api: CoreApi,
    activator: Arc<MockActivator>,
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
        );
        Self {
            dir,
            api,
            activator,
            root,
            paths,
        }
    }

    /// A library whose folder is added and scanned.
    fn scanned(label: &str) -> (Self, Source, Vec<Event>) {
        let lib = Self::new(label);
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

/// E10.4 criteria 1 and 2: `get_face` names stylistic sets and character
/// variants with the font's own names, read from the index: the file is gone
/// when `get_face` runs, so it cannot have been opened again.
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
    is_unsupported(lib.api.find_similar(SimilarRequest { face, limit: 5 }));
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
fn services_not_built_yet_answer_unsupported() {
    let lib = Lib::new("api-not-built");
    is_unsupported(lib.api.export_web(ExportWebRequest {
        faces: vec![FaceId(1)],
        formats: Vec::new(),
        subset: None,
        out_dir: "out".into(),
        css: false,
    }));
    is_unsupported(
        lib.api
            .license_inventory(LicenseInventoryRequest { collection: None }),
    );
    is_unsupported(lib.api.pick_fonts_for_brief(BriefRequest {
        brief: "a poster".into(),
        languages: Vec::new(),
        license: None,
        limit: None,
    }));
    is_unsupported(lib.api.prepare_for_web(PrepareForWebRequest {
        family: FamilyId(1),
        subset: None,
        out_dir: "out".into(),
    }));
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
