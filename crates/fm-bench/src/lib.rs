//! Font Manager performance driver.
//!
//! Each function runs one thing the performance budgets cover against a bench
//! home folder (the app folders of a throwaway user) and says how long it
//! took. `scripts/bench.sh` runs the `fm-bench` binary once per measurement,
//! so cold start and idle memory are taken from a fresh process; budgets,
//! memory sampling and the JSON report live in the script.
//!
//! The library folder is only read. The bench home is the only place written.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use fm_api::SourceId;
use fm_api::common::Page;
use fm_api::library::SourceKind;
use fm_api::search::SearchRequest;
use fm_core::Index;
use fm_core::index::INDEX_FILE;
use fm_core::scanner::{FileIndex, discover};
use fm_core::{logging, startup};
use fm_platform::AppPaths;
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

/// What every entry point does first ([`startup::init`] without the per-user
/// folders): app folders and settings, logging, the index with its
/// migrations, then the first page of the family list.
///
/// Keep the returned guard alive while the process runs: it flushes the log.
pub fn start(home: &Path) -> BenchResult<(Started, logging::LogGuard)> {
    let begin = Instant::now();
    let paths = home_paths(home);
    let config = startup::prepare(&paths)?;
    let log = logging::init(&paths.logs, config.log_level)?;
    let index = startup::open_index(&paths)?.index;
    let res = index.search(&SearchRequest {
        page: Page {
            offset: 0,
            limit: FIRST_PAGE,
        },
        ..SearchRequest::default()
    })?;
    let started = Started {
        ms: ms(begin.elapsed()),
        families: res.total,
        page: res.families.len(),
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
    let index = startup::open_index(&home_paths(home))?.index;
    let mut samples = Vec::with_capacity(QUERIES.len() * rounds);
    let mut families = 0;
    let mut slowest = (0.0, String::new());
    for &query in QUERIES {
        let req = SearchRequest {
            query: query.to_owned(),
            ..SearchRequest::default()
        };
        let res = index.search(&req)?;
        if query.is_empty() {
            families = res.total;
        }
        for _ in 0..rounds {
            let begin = Instant::now();
            index.search(&req)?;
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
