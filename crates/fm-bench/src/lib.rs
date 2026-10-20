//! Font Manager performance driver.
//!
//! Each function runs one thing the performance budgets cover against a bench
//! home folder (the app folders of a throwaway user) and says how long it
//! took. `scripts/bench.sh` runs the `fm-bench` binary once per measurement,
//! so cold start and idle memory are taken from a fresh process; budgets,
//! memory sampling and the JSON report live in the script.
//!
//! The library folder is only read. The bench home is the only place written.

use std::collections::HashSet;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fm_api::activation::{
    ActivateRequest, ActivationMode, ConflictPolicy, DeactivateRequest, OutcomeStatus, Target,
};
use fm_api::common::Page;
use fm_api::ids::FaceId;
use fm_api::library::{LibraryService, ListFamiliesRequest, SourceKind};
use fm_api::preview::PreviewRequest;
use fm_api::search::SearchRequest;
use fm_api::{ActivationService, SearchService, SourceId};
use fm_core::index::INDEX_FILE;
use fm_core::preview::cache::{CacheKey, DEFAULT_LIMIT_BYTES, PreviewCache, cache_dir};
use fm_core::preview::render_preview_file;
use fm_core::scanner::{FileIndex, discover};
use fm_core::{ActivationManager, CoreApi, Index, SettingsStore};
use fm_core::{logging, startup};
use fm_platform::{AppPaths, FontActivator, MockActivator};
use serde::Serialize;

/// Errors end the run; the script reports the metric as failed.
pub type BenchResult<T> = Result<T, Box<dyn Error>>;

/// The queries the search measurement times: words, prefixes, a typo, style
/// words, two letters and the empty query (the whole list by name).
pub const QUERIES: &[&str] = &[
    "sans",
    "serif",
    "mono",
    "ro",
    "noto sans",
    "bold italic",
    "robto",
    "display",
    "",
];

/// Rows of the first page, as the font list asks for at start.
const FIRST_PAGE: u32 = 100;

/// The app folders of a bench home: `<home>/{data,config,cache,logs}`.
pub fn home_paths(home: &Path) -> AppPaths {
    AppPaths {
        data: home.join("data"),
        config: home.join("config"),
        cache: home.join("cache"),
        logs: home.join("logs"),
        library: home.join("Fonts"),
    }
}

/// What a start served.
#[derive(Debug, Serialize)]
pub struct Started {
    /// Milliseconds from the call to the first page, inside the process.
    pub ms: f64,
    /// Family groups in the library.
    pub families: u32,
    /// Rows on the first page.
    pub page: usize,
}

/// What the app does first ([`startup::init`] without the per-user
/// folders): app folders and settings, logging, the index with its
/// migrations, the real API over it ([`CoreApi`], cleaning up the last
/// session), then the first page of the family list, as the UI asks for it.
///
/// The session cleanup runs on a [`MockActivator`]: the OS activator keeps
/// its records per user, not per home (the Windows ledger is one registry
/// key), so cleaning up with it would deregister the session fonts of the
/// user's running app.
///
/// Keep the returned guard alive while the process runs: it flushes the log.
pub fn start(home: &Path) -> BenchResult<(Started, logging::LogGuard)> {
    start_with(home, Arc::new(MockActivator::new()))
}

/// [`start`], cleaning up the last session through `activator`.
pub fn start_with(
    home: &Path,
    activator: Arc<dyn FontActivator>,
) -> BenchResult<(Started, logging::LogGuard)> {
    let begin = Instant::now();
    let paths = home_paths(home);
    let config = startup::prepare(&paths)?;
    let log = logging::init(&paths.logs, config.log_level)?;
    let index = startup::open_index(&paths)?.index;
    let api = CoreApi::open(&paths, index, activator);
    api.start()?;
    let res = api.list_families(ListFamiliesRequest {
        page: Page {
            offset: 0,
            limit: FIRST_PAGE,
        },
        ..ListFamiliesRequest::default()
    })?;
    let started = Started {
        ms: ms(begin.elapsed()),
        families: res.total,
        page: res.items.len(),
    };
    Ok((started, log))
}

/// What a scan or rescan did.
#[derive(Debug, Serialize)]
pub struct Scanned {
    /// Milliseconds, discovery to the last committed write.
    pub ms: f64,
    /// Of which: walking the folder.
    pub discover_ms: f64,
    /// Of which: reading, hashing and parsing (stat only for unchanged files).
    pub read_ms: f64,
    /// Of which: writing the index.
    pub write_ms: f64,
    /// Font files found.
    pub files: usize,
    /// The sum of the sizes of the font files found, in bytes: with
    /// `read_ms` of a first scan, how fast the disk gave them.
    pub bytes: u64,
    /// Faces written.
    pub faces: usize,
    /// Files read for the first time.
    pub added: usize,
    /// Files read again because their size or time changed.
    pub changed: usize,
    /// Files with the same size and time, not read.
    pub unchanged: usize,
    /// Files no longer found.
    pub gone: usize,
    /// Files that could not be read.
    pub failed: usize,
}

/// The first scan of `library` into a fresh index in `home`: the index file
/// of an earlier run is removed first (it belongs to the bench home).
pub fn first_scan(library: &Path, home: &Path) -> BenchResult<Scanned> {
    let paths = home_paths(home);
    startup::prepare(&paths)?;
    let db = paths.data.join(INDEX_FILE);
    for stale in [
        db.clone(),
        with_suffix(&db, "-wal"),
        with_suffix(&db, "-shm"),
    ] {
        if stale.exists() {
            std::fs::remove_file(&stale)?;
        }
    }
    let mut index = Index::open(&db)?.index;
    let source = index
        .add_source(&library.to_string_lossy(), SourceKind::Watched)?
        .id;
    rescan(&mut index, source, library, FileIndex::new())
}

/// The rescan the app runs at start and after watcher events, on the index a
/// [`first_scan`] of `library` left in `home`: the files indexed so far are
/// loaded back, the folder is walked again and what changed is written.
pub fn incremental(library: &Path, home: &Path) -> BenchResult<Scanned> {
    let paths = home_paths(home);
    let mut index = startup::open_index(&paths)?.index;
    let wanted = library.to_string_lossy();
    let source = index
        .sources()?
        .into_iter()
        .find(|s| s.path == wanted)
        .ok_or_else(|| format!("{wanted} is not a source of the bench index: run scan first"))?
        .id;
    let begin = Instant::now();
    let files = index.load_files()?;
    let mut done = rescan(&mut index, source, library, files)?;
    done.ms = ms(begin.elapsed());
    Ok(done)
}

fn rescan(
    index: &mut Index,
    source: SourceId,
    library: &Path,
    mut files: FileIndex,
) -> BenchResult<Scanned> {
    let begin = Instant::now();
    let found = discover(library)?;
    let discovered = Instant::now();
    let report = files.rescan(library, &found);
    let read = Instant::now();
    let persisted = index.apply_rescan(source, &files, &report)?;
    let written = Instant::now();
    Ok(Scanned {
        ms: ms(written - begin),
        discover_ms: ms(discovered - begin),
        read_ms: ms(read - discovered),
        write_ms: ms(written - read),
        files: files.len(),
        bytes: found.total_size,
        faces: persisted.faces,
        added: report.added.len(),
        changed: report.changed.len(),
        unchanged: report.unchanged.len(),
        gone: report.gone.len(),
        failed: report.failed.len(),
    })
}

/// Search latency over [`QUERIES`].
#[derive(Debug, Serialize)]
pub struct Searched {
    /// Timed searches.
    pub samples: usize,
    /// Median, in milliseconds.
    pub p50_ms: f64,
    /// 95th percentile, in milliseconds.
    pub p95_ms: f64,
    /// Slowest, in milliseconds.
    pub max_ms: f64,
    /// The slowest query.
    pub slowest: String,
    /// Family groups in the library (the empty query).
    pub families: u32,
}

/// Times every query of [`QUERIES`] `rounds` times on the index in `home`,
/// after one untimed run each to warm the caches, as a running app has them.
pub fn search(home: &Path, rounds: usize) -> BenchResult<Searched> {
    let paths = home_paths(home);
    let index = startup::open_index(&paths)?.index;
    // Through the real API, as the app serves it; the mock activator keeps
    // the OS out of a search measurement.
    let api = CoreApi::new(
        index,
        Arc::new(MockActivator::new()),
        SettingsStore::new(&paths),
        None,
    );
    let mut samples = Vec::with_capacity(QUERIES.len() * rounds);
    let mut families = 0;
    let mut slowest = (0.0, String::new());
    for &query in QUERIES {
        let req = SearchRequest {
            query: query.to_owned(),
            ..SearchRequest::default()
        };
        let res = api.search(req.clone())?;
        if query.is_empty() {
            families = res.total;
        }
        for _ in 0..rounds {
            let begin = Instant::now();
            api.search(req.clone())?;
            let took = ms(begin.elapsed());
            if took > slowest.0 {
                slowest = (took, query.to_owned());
            }
            samples.push(took);
        }
    }
    samples.sort_by(f64::total_cmp);
    Ok(Searched {
        samples: samples.len(),
        p50_ms: percentile(&samples, 50),
        p95_ms: percentile(&samples, 95),
        max_ms: samples.last().copied().unwrap_or_default(),
        slowest: slowest.1,
        families,
    })
}

/// Faces one activation request registers in the budget (100-face activation).
pub const ACTIVATION_FACES: u32 = 100;

/// What an activation round did.
#[derive(Debug, Default, Serialize)]
pub struct Activated {
    /// Milliseconds for the one request that activates the faces, conflict
    /// check included (the default policy keeps existing fonts).
    pub ms: f64,
    /// Milliseconds to deactivate them again (not budgeted).
    pub deactivate_ms: f64,
    /// Faces in the request.
    pub faces: usize,
    /// Faces activated.
    pub done: usize,
    /// Faces left inactive because they clash with an installed font.
    pub conflicts: usize,
    /// Faces skipped or failed.
    pub other: usize,
}

/// Activates [`ACTIVATION_FACES`] faces of the index in `home` in one
/// session request through this OS's activator, as the Activate button does,
/// then deactivates them. The fonts stay where they are (rule 2); only the
/// app's own per-user registrations are written and removed.
pub fn activate(home: &Path) -> BenchResult<Activated> {
    let paths = home_paths(home);
    let index = startup::open_index(&paths)?.index;
    let faces = index.activatable_faces(ACTIVATION_FACES)?;
    if faces.len() < ACTIVATION_FACES as usize {
        return Err(format!(
            "the index has {} activatable faces, {ACTIVATION_FACES} needed: run the scan first",
            faces.len()
        )
        .into());
    }
    let activator = fm_platform::activator::system(&paths)?;
    let service = ActivationManager::new(Arc::new(Mutex::new(index)), activator, |_| {});
    let begin = Instant::now();
    let outcomes = service.activate(ActivateRequest {
        target: Target::Faces {
            faces: faces.clone(),
        },
        mode: ActivationMode::Session,
        on_conflict: ConflictPolicy::KeepExisting,
    })?;
    let mut report = Activated {
        ms: ms(begin.elapsed()),
        faces: faces.len(),
        ..Activated::default()
    };
    for o in &outcomes {
        match o.outcome {
            OutcomeStatus::Done => report.done += 1,
            OutcomeStatus::Conflict { .. } => report.conflicts += 1,
            _ => report.other += 1,
        }
    }
    let begin = Instant::now();
    service.deactivate(DeactivateRequest {
        target: Some(Target::Faces { faces }),
        except: None,
    })?;
    report.deactivate_ms = ms(begin.elapsed());
    Ok(report)
}

/// The thumbnail the preview-cache measurement caches per face: one word at
/// a grid-cell size.
pub const THUMBNAIL_TEXT: &str = "Hamburgefonstiv";
/// The thumbnail size, in pixels.
pub const THUMBNAIL_PX: f32 = 32.0;
/// The faces the preview-cache measurement caches: a screenful of grid cells
/// and more.
pub const CACHE_FACES: usize = 200;

/// Preview-cache hit latency.
#[derive(Debug, Serialize)]
pub struct CacheHits {
    /// Thumbnails cached.
    pub entries: usize,
    /// Their bytes on disk.
    pub bytes: u64,
    /// Files the rasterizer could not draw, skipped.
    pub failed: usize,
    /// Files with the same bytes as one already cached: the same face, so
    /// the same entry (the cache keys by content, not by path).
    pub duplicates: usize,
    /// Timed hits.
    pub samples: usize,
    /// Median, in milliseconds.
    pub p50_ms: f64,
    /// 95th percentile, in milliseconds.
    pub p95_ms: f64,
    /// Slowest, in milliseconds.
    pub max_ms: f64,
}

/// Renders a thumbnail of the first face of up to [`CACHE_FACES`] files of
/// the index in `home` into a fresh preview cache (`<home>/cache/previews`),
/// then times `rounds` hits on every entry, as a grid scrolling back does.
/// A copy of a file already cached shares its entry and is counted, not
/// cached twice.
pub fn preview_cache(home: &Path, rounds: usize) -> BenchResult<CacheHits> {
    let paths = home_paths(home);
    let files = startup::open_index(&paths)?.index.load_files()?;
    let dir = cache_dir(&paths);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    let cache = PreviewCache::open(&dir, DEFAULT_LIMIT_BYTES)?;
    let req = PreviewRequest {
        face: FaceId(0),
        text: THUMBNAIL_TEXT.to_owned(),
        size_px: THUMBNAIL_PX,
        variations: Vec::new(),
        features: Vec::new(),
        foreground: None,
        background: None,
    };
    let mut keys = Vec::with_capacity(CACHE_FACES);
    let mut seen = HashSet::with_capacity(CACHE_FACES);
    let (mut failed, mut duplicates) = (0, 0);
    for file in files.files().filter(|f| !f.gone) {
        if keys.len() == CACHE_FACES {
            break;
        }
        let key = CacheKey::new(&file.hash, 0, &req);
        if !seen.insert(key) {
            duplicates += 1;
            continue;
        }
        match cache.get_or_render(&key, || render_preview_file(&file.path, 0, &req)) {
            Ok(_) => keys.push(key),
            Err(_) => failed += 1,
        }
    }
    if cache.len() != keys.len() {
        return Err(format!("{} of {} thumbnails were cached", cache.len(), keys.len()).into());
    }
    let mut samples = Vec::with_capacity(keys.len() * rounds);
    for _ in 0..rounds {
        for key in &keys {
            let begin = Instant::now();
            let hit = cache.get(key);
            samples.push(ms(begin.elapsed()));
            if hit.is_none() {
                return Err("a cached thumbnail was missed".into());
            }
        }
    }
    samples.sort_by(f64::total_cmp);
    Ok(CacheHits {
        entries: keys.len(),
        bytes: cache.total_bytes(),
        failed,
        duplicates,
        samples: samples.len(),
        p50_ms: percentile(&samples, 50),
        p95_ms: percentile(&samples, 95),
        max_ms: samples.last().copied().unwrap_or_default(),
    })
}

/// The `p`th percentile (nearest rank) of sorted `samples`; 0 when empty.
pub fn percentile(samples: &[f64], p: usize) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let rank = (p * samples.len()).div_ceil(100).clamp(1, samples.len());
    samples[rank - 1]
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

#[cfg(test)]
mod tests;
