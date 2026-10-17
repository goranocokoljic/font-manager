//! A scan job: discovers one source folder, rescans it, and reports through
//! the typed [`Event`] stream.
//!
//! A job emits, in this order and all under its [`JobId`]:
//!
//! 1. [`Event::ScanStarted`];
//! 2. [`Event::ScanProgress`] with `done: 0` once the folder walk knows the
//!    total, then at most one per [`PROGRESS_INTERVAL`], then one with
//!    `done == total`;
//! 3. [`Event::ScanFinished`] with face counts, or [`Event::JobFailed`] when
//!    the folder cannot be walked.
//!
//! Progress is throttled so a 10k-font scan sends a few dozen events, not
//! 10,000: the UI repaints at its own pace instead of once per file.
//! [`ScanJob::spawn`] runs the job on its own thread, so the caller (an API
//! method that returns a [`JobId`] at once, ADR 0006) never waits for it.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use fm_api::common::Health;
use fm_api::{Event, JobId, SourceId};

use crate::scanner::{Discovery, FileIndex, Rescan, discover};
use crate::{FmError, FmResult};

/// The shortest time between two [`Event::ScanProgress`] events of one job.
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// One scan of one source folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanJob {
    job: JobId,
    source: Option<SourceId>,
    root: PathBuf,
    interval: Duration,
}

impl ScanJob {
    /// A scan of the folder `root` as job `job`. `source` is the source the
    /// folder belongs to, passed on in [`Event::ScanStarted`].
    pub fn new(job: JobId, source: Option<SourceId>, root: impl Into<PathBuf>) -> Self {
        Self {
            job,
            source,
            root: root.into(),
            interval: PROGRESS_INTERVAL,
        }
    }

    /// The same job with another time between progress events.
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// The job's id.
    pub fn job(&self) -> JobId {
        self.job
    }

    /// Runs the scan on this thread, updating `index` and sending every event
    /// to `emit`. A folder that cannot be walked is reported as
    /// [`Event::JobFailed`] and returned as the error.
    pub fn run(&self, index: &mut FileIndex, emit: &mut dyn FnMut(Event)) -> FmResult<Rescan> {
        let job = self.job;
        emit(Event::ScanStarted {
            job,
            source: self.source,
        });
        let discovery = match discover(&self.root) {
            Ok(d) => d,
            Err(e) => return Err(self.fail(e, emit)),
        };
        let total = count(discovery.files.len());
        emit(Event::ScanProgress {
            job,
            done: 0,
            total,
        });
        let mut throttle = Throttle::new(self.interval, Instant::now());
        let rescan = index.rescan_with(&self.root, &discovery, &mut |done| {
            let done = count(done);
            if done < total && throttle.ready(Instant::now()) {
                emit(Event::ScanProgress { job, done, total });
            }
        });
        if total > 0 {
            emit(Event::ScanProgress {
                job,
                done: total,
                total,
            });
        }
        emit(finished(job, index, &discovery, &rescan));
        Ok(rescan)
    }

    /// Runs the scan on a new thread named `fm-scan` and returns at once.
    /// The thread holds the `index` lock while it scans.
    pub fn spawn(
        self,
        index: Arc<Mutex<FileIndex>>,
        mut emit: impl FnMut(Event) + Send + 'static,
    ) -> FmResult<JoinHandle<FmResult<Rescan>>> {
        std::thread::Builder::new()
            .name("fm-scan".into())
            .spawn(move || {
                let Ok(mut index) = index.lock() else {
                    let e = FmError::internal("the file index lock is poisoned");
                    return Err(self.fail(e, &mut emit));
                };
                self.run(&mut index, &mut emit)
            })
            .map_err(|e| FmError::internal(format!("could not start the scan thread: {e}")))
    }

    fn fail(&self, error: FmError, emit: &mut dyn FnMut(Event)) -> FmError {
        emit(Event::JobFailed {
            job: self.job,
            error: error.clone().into(),
        });
        error
    }
}

/// Lets an event through at most once per interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Throttle {
    interval: Duration,
    last: Instant,
}

impl Throttle {
    /// A throttle whose last event was at `now`.
    pub fn new(interval: Duration, now: Instant) -> Self {
        Self {
            interval,
            last: now,
        }
    }

    /// Whether an event may go out at `now`; if so, `now` becomes the last one.
    pub fn ready(&mut self, now: Instant) -> bool {
        if now.saturating_duration_since(self.last) < self.interval {
            return false;
        }
        self.last = now;
        true
    }
}

/// [`Event::ScanFinished`] for a rescan: added, changed and removed count
/// faces; problems count unreadable entries, files that could not be read,
/// and faces (or whole files) of this scan that are not healthy.
fn finished(job: JobId, index: &FileIndex, discovery: &Discovery, rescan: &Rescan) -> Event {
    let faces = |paths: &[PathBuf]| {
        paths
            .iter()
            .filter_map(|p| index.get(p))
            .map(|f| f.parsed.faces.len())
            .sum::<usize>()
    };
    let unhealthy = |p: &PathBuf| {
        index.get(p).map_or(0, |f| {
            let bad = f
                .parsed
                .faces
                .iter()
                .filter(|face| face.health != Health::Ok)
                .count();
            if bad == 0 && f.parsed.health != Health::Ok {
                1
            } else {
                bad
            }
        })
    };
    let problems = discovery.unreadable.len()
        + rescan.failed.len()
        + rescan
            .added
            .iter()
            .chain(&rescan.changed)
            .map(unhealthy)
            .sum::<usize>();
    Event::ScanFinished {
        job,
        added: count(faces(&rescan.added)),
        changed: count(faces(&rescan.changed)),
        removed: count(faces(&rescan.gone)),
        problems: count(problems),
    }
}

/// A count for the API, which carries `u32` (ADR 0006).
fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::mpsc;

    use fm_api::ErrorCode;

    use super::*;
    use crate::test_support::TempDir;

    const REGULAR: &str = "google-static/asap/Asap_Condensed-ExtraLightItalic.ttf";
    const OTHER: &str = "google-static/anybody/Anybody_Expanded-MediumItalic.ttf";
    /// Long enough that only the first and last progress events go out.
    const NEVER: Duration = Duration::from_secs(3600);

    fn fixture(rel: &str) -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts")
            .join(rel);
        fs::read(path).expect("read fixture")
    }

    fn put(path: &Path, data: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create folder");
        }
        fs::write(path, data).expect("write font");
    }

    fn run(job: &ScanJob, index: &mut FileIndex) -> (FmResult<Rescan>, Vec<Event>) {
        let mut events = Vec::new();
        let result = job.run(index, &mut |e| events.push(e));
        (result, events)
    }

    fn progress(events: &[Event]) -> Vec<(u32, u32)> {
        events
            .iter()
            .filter_map(|e| match e {
                Event::ScanProgress { done, total, .. } => Some((*done, *total)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_scan_starts_progresses_and_finishes_under_one_job() {
        let tmp = TempDir::new("job-events");
        let root = tmp.path();
        put(&root.join("a.ttf"), &fixture(REGULAR));
        put(&root.join("sub/b.ttf"), &fixture(OTHER));
        put(&root.join("Schrift-ü.ttf"), &fixture(OTHER));

        let job = ScanJob::new(JobId(7), Some(SourceId(3)), root).with_interval(NEVER);
        let mut index = FileIndex::new();
        let (result, events) = run(&job, &mut index);

        assert_eq!(result.expect("scan").added.len(), 3);
        assert_eq!(
            events,
            [
                Event::ScanStarted {
                    job: JobId(7),
                    source: Some(SourceId(3)),
                },
                Event::ScanProgress {
                    job: JobId(7),
                    done: 0,
                    total: 3,
                },
                Event::ScanProgress {
                    job: JobId(7),
                    done: 3,
                    total: 3,
                },
                Event::ScanFinished {
                    job: JobId(7),
                    added: 3,
                    changed: 0,
                    removed: 0,
                    problems: 0,
                },
            ]
        );
    }

    #[test]
    fn without_a_throttle_every_file_reports_progress_in_order() {
        let tmp = TempDir::new("job-every");
        let root = tmp.path();
        for name in ["a.ttf", "b.ttf", "c.ttf", "d.ttf"] {
            put(&root.join(name), &fixture(REGULAR));
        }
        let job = ScanJob::new(JobId(1), None, root).with_interval(Duration::ZERO);
        let (_, events) = run(&job, &mut FileIndex::new());
        assert_eq!(progress(&events), [(0, 4), (1, 4), (2, 4), (3, 4), (4, 4)]);
    }

    #[test]
    fn a_rescan_counts_changed_and_removed_faces() {
        let tmp = TempDir::new("job-rescan");
        let root = tmp.path();
        put(&root.join("keep.ttf"), &fixture(REGULAR));
        put(&root.join("edit.ttf"), &fixture(REGULAR));
        put(&root.join("drop.ttf"), &fixture(REGULAR));
        put(&root.join("drop2.ttf"), &fixture(REGULAR));
        let mut index = FileIndex::new();
        let job = ScanJob::new(JobId(1), None, root).with_interval(NEVER);
        run(&job, &mut index).0.expect("first scan");

        // A different size proves the change without waiting on the clock.
        put(&root.join("edit.ttf"), &fixture(OTHER));
        fs::remove_file(root.join("drop.ttf")).expect("remove");
        fs::remove_file(root.join("drop2.ttf")).expect("remove");
        put(&root.join("new.ttf"), &fixture(OTHER));
        put(&root.join("new2.ttf"), &fixture(OTHER));
        put(&root.join("new3.ttf"), &fixture(OTHER));

        let again = ScanJob::new(JobId(2), None, root).with_interval(NEVER);
        let (_, events) = run(&again, &mut index);
        // Distinct counts: a field read from the wrong list cannot pass.
        assert_eq!(
            events.last(),
            Some(&Event::ScanFinished {
                job: JobId(2),
                added: 3,
                changed: 1,
                removed: 2,
                problems: 0,
            })
        );
        assert_eq!(progress(&events), [(0, 5), (5, 5)]);
    }

    #[test]
    fn a_malformed_font_is_a_problem_not_a_failure() {
        let tmp = TempDir::new("job-broken");
        let root = tmp.path();
        put(&root.join("good.ttf"), &fixture(REGULAR));
        put(&root.join("broken.ttf"), b"not a font at all");

        let job = ScanJob::new(JobId(4), None, root).with_interval(NEVER);
        let (result, events) = run(&job, &mut FileIndex::new());

        assert_eq!(result.expect("scan").added.len(), 2);
        assert!(matches!(
            events.last(),
            Some(Event::ScanFinished {
                problems: 1,
                removed: 0,
                ..
            })
        ));
    }

    #[test]
    fn an_empty_folder_starts_and_finishes_with_nothing() {
        let tmp = TempDir::new("job-empty");
        let job = ScanJob::new(JobId(5), None, tmp.path());
        let (_, events) = run(&job, &mut FileIndex::new());
        assert_eq!(progress(&events), [(0, 0)]);
        assert_eq!(
            events.last(),
            Some(&Event::ScanFinished {
                job: JobId(5),
                added: 0,
                changed: 0,
                removed: 0,
                problems: 0,
            })
        );
    }

    #[test]
    fn a_missing_folder_fails_the_job() {
        let tmp = TempDir::new("job-missing");
        let job = ScanJob::new(JobId(6), Some(SourceId(1)), tmp.path().join("gone"));
        let (result, events) = run(&job, &mut FileIndex::new());

        let error = result.expect_err("missing folder");
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0],
            Event::ScanStarted { job: JobId(6), .. }
        ));
        match &events[1] {
            Event::JobFailed { job, error: api } => {
                assert_eq!(*job, JobId(6));
                assert_eq!(api.code, error.code());
                assert_ne!(api.code, ErrorCode::Internal);
            }
            other => panic!("expected JobFailed, got {other:?}"),
        }
    }

    #[test]
    fn the_throttle_lets_one_event_through_per_interval() {
        let start = Instant::now();
        let ms = Duration::from_millis;
        let mut throttle = Throttle::new(ms(100), start);
        assert!(!throttle.ready(start));
        assert!(!throttle.ready(start + ms(99)));
        assert!(throttle.ready(start + ms(100)));
        assert!(!throttle.ready(start + ms(150)));
        assert!(throttle.ready(start + ms(200)));
        // A clock that steps back does not open the throttle.
        assert!(!throttle.ready(start));

        // 10,000 files over one second: about ten events, not ten thousand.
        let mut throttle = Throttle::new(PROGRESS_INTERVAL, start);
        let sent = (1..=10_000u64)
            .filter(|i| throttle.ready(start + Duration::from_micros(i * 100)))
            .count();
        assert_eq!(sent, 10);
    }

    #[test]
    fn spawn_returns_before_the_scan_sends_anything() {
        let tmp = TempDir::new("job-spawn");
        let root = tmp.path();
        put(&root.join("a.ttf"), &fixture(REGULAR));
        put(&root.join("b.ttf"), &fixture(OTHER));
        let index = Arc::new(Mutex::new(FileIndex::new()));

        // A rendezvous channel: the scan thread blocks on its first event
        // until this thread receives it, so `spawn` cannot have waited for it.
        let (tx, rx) = mpsc::sync_channel::<(Event, Option<String>)>(0);
        let job = ScanJob::new(JobId(9), Some(SourceId(2)), root).with_interval(NEVER);
        let handle = job
            .spawn(Arc::clone(&index), move |e| {
                let thread = std::thread::current().name().map(str::to_owned);
                let _ = tx.send((e, thread));
            })
            .expect("spawn");

        let events: Vec<_> = rx.iter().collect();
        assert!(events.iter().all(|(_, t)| t.as_deref() == Some("fm-scan")));
        assert!(matches!(
            events.first(),
            Some((Event::ScanStarted { job: JobId(9), .. }, _))
        ));
        assert!(matches!(
            events.last(),
            Some((Event::ScanFinished { added: 2, .. }, _))
        ));
        assert_eq!(handle.join().expect("join").expect("scan").added.len(), 2);
        assert_eq!(index.lock().expect("lock").len(), 2);
    }

    /// The reference library (10k fonts): the scan runs off this thread,
    /// progress keeps coming, and the event rate stays bounded.
    #[test]
    fn a_reference_library_scan_reports_at_a_bounded_rate() {
        let Some(library) = std::env::var_os("FM_REFERENCE_LIBRARY") else {
            eprintln!("skipped: FM_REFERENCE_LIBRARY is not set");
            return;
        };
        let index = Arc::new(Mutex::new(FileIndex::new()));
        let (tx, rx) = mpsc::channel();
        let start = Instant::now();
        let handle = ScanJob::new(JobId(1), None, PathBuf::from(library))
            .spawn(index, move |e| {
                let _ = tx.send(e);
            })
            .expect("spawn");
        let returned = start.elapsed();

        let events: Vec<_> = rx.iter().collect();
        let elapsed = start.elapsed();
        handle.join().expect("join").expect("scan");
        let progress = events
            .iter()
            .filter(|e| matches!(e, Event::ScanProgress { .. }))
            .count();
        let ceiling = (elapsed.as_millis() / PROGRESS_INTERVAL.as_millis()) as usize + 3;
        eprintln!(
            "spawn returned in {returned:?}; scan took {elapsed:?}; {progress} progress events"
        );
        assert!(returned < Duration::from_millis(50), "{returned:?}");
        assert!(progress <= ceiling, "{progress} > {ceiling}");
        assert!(matches!(events.last(), Some(Event::ScanFinished { .. })));
    }
}
