#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

/// A fresh folder under the system temp folder, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("fm-bench-{label}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts")
}

const FONT: &str = "google-repo/almendradisplay/AlmendraDisplay-Regular.ttf";

/// Every file under `dir` with its size and modification time.
fn listing(dir: &Path) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).expect("read dir") {
            let entry = entry.expect("entry");
            let meta = entry.metadata().expect("meta");
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                out.push((entry.path(), meta.len(), meta.modified().expect("mtime")));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_first_scan_reads_every_file_and_the_rescan_reads_none() {
    let home = TempDir::new("scan");
    let first = first_scan(&fixtures(), home.path()).expect("scan");
    assert!(first.files > 100, "{first:?}");
    assert!(first.faces > 100, "{first:?}");
    assert_eq!(first.added, first.files, "a fresh index reads every file");
    assert_eq!(first.unchanged, 0);
    assert!(first.ms >= first.discover_ms + first.read_ms, "{first:?}");

    let again = incremental(&fixtures(), home.path()).expect("rescan");
    assert_eq!(again.files, first.files);
    assert_eq!(
        (again.added, again.changed, again.gone),
        (0, 0, 0),
        "{again:?}"
    );
    assert_eq!(again.unchanged, first.files - first.failed, "{again:?}");
}

#[test]
fn the_scan_counts_the_bytes_of_the_font_files_it_found() {
    let library = TempDir::new("bytes-library");
    let home = TempDir::new("bytes-home");
    let font = fs::read(fixtures().join(FONT)).expect("fixture");
    fs::write(library.path().join("a.ttf"), &font).expect("write");
    fs::write(library.path().join("b.ttf"), &font[..font.len() / 2]).expect("write");
    fs::write(library.path().join("notes.txt"), b"not a font").expect("write");
    let size = |p: &str| fs::metadata(library.path().join(p)).unwrap().len();

    let done = first_scan(library.path(), home.path()).expect("scan");
    assert_eq!(done.files, 2, "{done:?}");
    assert_eq!(done.bytes, size("a.ttf") + size("b.ttf"), "{done:?}");
}

#[test]
fn a_second_first_scan_starts_from_a_fresh_index() {
    let home = TempDir::new("rescan-fresh");
    let one = first_scan(&fixtures(), home.path()).expect("scan");
    let two = first_scan(&fixtures(), home.path()).expect("scan again");
    assert_eq!(two.added, one.added, "the earlier index is not reused");
    assert_eq!(two.unchanged, 0);
}

#[test]
fn the_bench_never_writes_into_the_library() {
    let home = TempDir::new("read-only-home");
    let before = listing(&fixtures());
    first_scan(&fixtures(), home.path()).expect("scan");
    incremental(&fixtures(), home.path()).expect("rescan");
    search(home.path(), 1).expect("search");
    assert_eq!(listing(&fixtures()), before);
    assert!(home_paths(home.path()).data.join(INDEX_FILE).is_file());
}

#[test]
fn an_incremental_run_without_a_scan_says_what_to_do() {
    let home = TempDir::new("no-scan");
    let err = incremental(&fixtures(), home.path()).expect_err("no source");
    assert!(err.to_string().contains("run scan first"), "{err}");
}

#[test]
fn an_empty_folder_scans_to_nothing() {
    let home = TempDir::new("empty-home");
    let library = TempDir::new("empty-library");
    let done = first_scan(library.path(), home.path()).expect("scan");
    assert_eq!((done.files, done.faces, done.failed), (0, 0, 0));
    let res = search(home.path(), 1).expect("search");
    assert_eq!(res.families, 0);
}

#[test]
fn a_missing_library_is_an_error() {
    let home = TempDir::new("missing-home");
    let gone = home.path().join("no-such-folder");
    assert!(first_scan(&gone, home.path()).is_err());
}

#[test]
fn a_malformed_font_does_not_stop_the_scan() {
    let home = TempDir::new("broken-home");
    let library = TempDir::new("broken-library");
    fs::copy(fixtures().join(FONT), library.path().join("good.ttf")).expect("copy");
    fs::write(library.path().join("junk.ttf"), b"not a font at all").expect("junk");
    let done = first_scan(library.path(), home.path()).expect("scan");
    assert_eq!(done.files + done.failed, 2, "{done:?}");
    assert!(done.faces >= 1, "{done:?}");
}

#[test]
fn a_non_ascii_library_path_is_found_again() {
    let home = TempDir::new("unicode-home");
    let library = TempDir::new("unicode-library");
    let dir = library.path().join("Schriften ü 字体");
    fs::create_dir_all(&dir).expect("dir");
    fs::copy(fixtures().join(FONT), dir.join("a.ttf")).expect("copy");
    first_scan(&dir, home.path()).expect("scan");
    let again = incremental(&dir, home.path()).expect("rescan");
    assert_eq!(again.unchanged, 1, "{again:?}");
}

#[test]
fn search_times_every_query_for_every_round() {
    let home = TempDir::new("search");
    first_scan(&fixtures(), home.path()).expect("scan");
    let res = search(home.path(), 3).expect("search");
    assert_eq!(res.samples, QUERIES.len() * 3);
    assert!(res.families > 10, "{res:?}");
    assert!(
        res.p50_ms <= res.p95_ms && res.p95_ms <= res.max_ms,
        "{res:?}"
    );
    assert!(QUERIES.contains(&res.slowest.as_str()), "{res:?}");
}

#[test]
fn similar_faces_are_timed_for_every_face_and_round() {
    let home = TempDir::new("similar");
    first_scan(&fixtures(), home.path()).expect("scan");
    let res = similar(home.path(), 2).expect("similar");
    assert_eq!(res.faces, SIMILAR_FACES as usize, "{res:?}");
    assert_eq!(res.samples, res.faces * 2);
    assert!(
        res.p50_ms <= res.p95_ms && res.p95_ms <= res.max_ms,
        "{res:?}"
    );
}

#[test]
fn pairings_are_timed_for_the_first_faces() {
    let home = TempDir::new("pairings");
    first_scan(&fixtures(), home.path()).expect("scan");
    let res = pairings(home.path(), 2).expect("pairings");
    assert_eq!(res.faces, PAIRING_FACES as usize, "{res:?}");
    assert_eq!(res.samples, res.faces * 2, "{res:?}");
    assert!(
        res.p50_ms <= res.p95_ms && res.p95_ms <= res.max_ms,
        "{res:?}"
    );
}

#[test]
fn similar_needs_a_scanned_index() {
    let home = TempDir::new("similar-empty");
    assert!(similar(home.path(), 1).is_err());
}

#[test]
fn smart_collections_are_made_once_and_timed() {
    let home = TempDir::new("smart");
    first_scan(&fixtures(), home.path()).expect("scan");
    let res = smart_collections(home.path(), 2).expect("smart");
    assert_eq!(res.collections, SMART_COLLECTIONS, "{res:?}");
    assert!(
        res.p95_ms >= res.search_p95_ms && res.p95_ms >= res.list_p95_ms,
        "{res:?}"
    );
    // A second run finds them: still twenty.
    let again = smart_collections(home.path(), 1).expect("again");
    assert_eq!(again.collections, SMART_COLLECTIONS);
    let index = fm_core::startup::open_index(&home_paths(home.path()))
        .expect("open")
        .index;
    // The library's own default smart collections (E14.3) are listed too.
    let bench = index
        .collections()
        .expect("list")
        .into_iter()
        .filter(|c| c.name.starts_with("Bench smart "))
        .count();
    assert_eq!(bench, SMART_COLLECTIONS);
}

#[test]
fn preview_cache_times_a_hit_on_every_cached_thumbnail() {
    let home = TempDir::new("preview-cache");
    let before = listing(&fixtures());
    first_scan(&fixtures(), home.path()).expect("scan");
    let res = preview_cache(home.path(), 2).expect("preview cache");
    // The fixtures hold about 200 files, a few of them broken on purpose.
    assert!(res.entries > CACHE_FACES * 3 / 4, "{res:?}");
    assert!(res.entries <= CACHE_FACES, "{res:?}");
    assert_eq!(res.samples, res.entries * 2);
    assert!(res.bytes > 0, "{res:?}");
    assert!(
        res.p50_ms <= res.p95_ms && res.p95_ms <= res.max_ms,
        "{res:?}"
    );
    let dir = home_paths(home.path()).cache.join("previews");
    assert_eq!(fs::read_dir(&dir).expect("cache dir").count(), res.entries);
    // A second run starts from an empty cache, not from the first one's.
    let again = preview_cache(home.path(), 1).expect("again");
    assert_eq!(again.entries, res.entries);
    assert_eq!(fs::read_dir(&dir).expect("cache dir").count(), res.entries);
    assert_eq!(listing(&fixtures()), before, "the library is only read");
}

#[test]
fn preview_cache_on_an_empty_library_caches_nothing() {
    let home = TempDir::new("preview-cache-empty");
    let library = TempDir::new("preview-cache-empty-lib");
    first_scan(library.path(), home.path()).expect("scan");
    let res = preview_cache(home.path(), 3).expect("preview cache");
    assert_eq!((res.entries, res.samples, res.failed), (0, 0, 0));
    assert_eq!(res.p95_ms, 0.0);
}

#[test]
fn preview_cache_counts_a_copy_of_a_cached_file_once() {
    let home = TempDir::new("preview-cache-dup");
    let library = TempDir::new("preview-cache-dup-lib");
    fs::copy(fixtures().join(FONT), library.path().join("a.ttf")).expect("copy");
    fs::copy(fixtures().join(FONT), library.path().join("b.ttf")).expect("copy");
    first_scan(library.path(), home.path()).expect("scan");
    let res = preview_cache(home.path(), 3).expect("a duplicate does not fail the run");
    assert_eq!(
        (res.entries, res.duplicates, res.failed),
        (1, 1, 0),
        "{res:?}"
    );
    assert_eq!(res.samples, 3);
}

#[test]
fn preview_cache_skips_a_file_it_cannot_draw() {
    let home = TempDir::new("preview-cache-broken");
    let library = TempDir::new("preview-cache-broken-lib");
    fs::copy(fixtures().join(FONT), library.path().join("good.ttf")).expect("copy");
    first_scan(library.path(), home.path()).expect("scan");
    // The file changes behind the index: still listed, no longer a font.
    fs::write(library.path().join("good.ttf"), b"not a font any more").expect("junk");
    let res = preview_cache(home.path(), 1).expect("preview cache");
    assert_eq!((res.entries, res.failed), (0, 1), "{res:?}");
}

/// The only test that calls [`start_with`] (and so [`start`]): it installs
/// the process-wide logger.
#[test]
fn start_prepares_the_app_folders_and_serves_the_first_page() {
    let home = TempDir::new("start");
    first_scan(&fixtures(), home.path()).expect("scan");
    let families = search(home.path(), 1).expect("search").families;
    // A session font of "another run": the cleanup must reach the activator
    // it is given, never the OS one shared with the user's running app.
    let activator = Arc::new(MockActivator::new());
    let font = fixtures().join("google-repo/bebasneue/BebasNeue-Regular.ttf");
    activator
        .activate(
            &[fm_platform::FaceFile {
                path: font,
                postscript_name: "BebasNeue-Regular".into(),
            }],
            fm_platform::Mode::Session,
        )
        .expect("seed a session font");
    assert_eq!(activator.list_active().expect("listed").len(), 1);
    let (started, _log) = start_with(
        home.path(),
        Arc::clone(&activator) as Arc<dyn FontActivator>,
    )
    .expect("start");
    assert!(
        activator.list_active().expect("listed").is_empty(),
        "start cleans up the session of the activator it was given"
    );
    assert_eq!(started.families, families);
    assert_eq!(started.page, (families as usize).min(FIRST_PAGE as usize));
    let paths = home_paths(home.path());
    assert!(paths.config_file().is_file());
    assert!(paths.logs.is_dir());
    assert!(
        !paths.library.exists(),
        "the library folder is never created"
    );
}

#[test]
fn percentiles_use_the_nearest_rank() {
    let s: Vec<f64> = (1..=20).map(f64::from).collect();
    assert_eq!(percentile(&s, 50), 10.0);
    assert_eq!(percentile(&s, 95), 19.0);
    assert_eq!(percentile(&s, 100), 20.0);
    assert_eq!(percentile(&[7.0], 95), 7.0);
    assert_eq!(percentile(&[], 95), 0.0);
}

#[test]
fn activation_needs_a_scanned_index_with_enough_faces() {
    let library = TempDir::new("activate-library");
    fs::copy(fixtures().join(FONT), library.path().join("One.ttf")).expect("copy");
    let home = TempDir::new("activate");
    first_scan(library.path(), home.path()).expect("scan");
    // Refused before any OS call: one face is not a 100-face round.
    let err = activate(home.path()).expect_err("too few faces");
    assert!(err.to_string().contains("1 activatable faces"), "{err}");
}

#[test]
fn fonts_needed_matches_and_activates_100_fonts_in_one_request() {
    let home = TempDir::new("fonts-needed");
    first_scan(&fixtures(), home.path()).expect("scan");
    let res = fonts_needed_with(home.path(), Arc::new(MockActivator::new())).expect("round");
    assert_eq!(res.fonts, ACTIVATION_FACES as usize, "{res:?}");
    assert_eq!(res.matched, res.fonts, "{res:?}");
    assert_eq!(
        res.done, res.fonts,
        "every font activates its own face: {res:?}"
    );
    assert!(res.ms > 0.0, "{res:?}");
}

#[test]
fn fonts_needed_needs_a_scanned_index_with_enough_faces() {
    let library = TempDir::new("fonts-needed-library");
    fs::copy(fixtures().join(FONT), library.path().join("One.ttf")).expect("copy");
    let home = TempDir::new("fonts-needed-one");
    first_scan(library.path(), home.path()).expect("scan");
    let err =
        fonts_needed_with(home.path(), Arc::new(MockActivator::new())).expect_err("too few faces");
    assert!(err.to_string().contains("1 fonts that name"), "{err}");
}
