//! Watched folders: keeps the index current while the app runs.
//!
//! A [`Watcher`] runs one background thread. It subscribes to the OS
//! file-system events (`notify`: ReadDirectoryChangesW, FSEvents, inotify) for
//! every source folder and applies them to the shared [`FileIndex`]:
//!
//! - Create, modify, remove and rename events name paths. The paths are
//!   collected by a [`Debounce`] and applied in one [`FileIndex::refresh`] per
//!   source when the burst is over: after [`WatchSettings::quiet`] without a
//!   new event, and never later than [`WatchSettings::max_wait`] after the
//!   first. With the defaults a change lands in the index well within 1 s.
//! - A periodic full rescan ([`WatchSettings::rescan_interval`], default 1 h)
//!   catches what the OS did not report: an event queue that overflowed, a
//!   network share that sends no events, a change behind a symbolic link. An
//!   event that says "rescan" triggers it at once.
//! - A paused source is not watched and not rescanned. Resuming watches it
//!   again and rescans it at once, so changes made while it was paused land.
//!
//! The watcher only reads (ADR 0004). Every change it makes to the index is
//! reported to the caller as a [`WatchUpdate`].

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use fm_api::ErrorCode;
use notify::{EventKind, RecommendedWatcher, RecursiveMode};

use crate::scanner::{FileIndex, Rescan, discover};
use crate::{Config, FmError, FmResult};

/// How the watcher batches events and how often it rescans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchSettings {
    /// A burst ends after this long without a new event.
    pub quiet: Duration,
    /// A burst is applied at the latest this long after its first event, even
    /// while events keep coming.
    pub max_wait: Duration,
    /// The time between two full rescans of a source.
    pub rescan_interval: Duration,
}

impl Default for WatchSettings {
    fn default() -> Self {
        Self {
            quiet: Duration::from_millis(200),
            max_wait: Duration::from_millis(500),
            rescan_interval: Duration::from_secs(60 * 60),
        }
    }
}

impl WatchSettings {
    /// The defaults with the rescan interval the user set.
    pub fn from_config(config: &Config) -> Self {
        let minutes = u64::from(config.rescan_interval_minutes.max(1));
        Self {
            rescan_interval: Duration::from_secs(minutes * 60),
            ..Self::default()
        }
    }
}

/// Coalesces a burst of events into one batch of distinct paths.
#[derive(Debug, Clone)]
pub struct Debounce {
    quiet: Duration,
    max_wait: Duration,
    first: Option<Instant>,
    last: Option<Instant>,
    paths: BTreeSet<PathBuf>,
}

impl Debounce {
    /// An empty debounce with the timings of `settings`.
    pub fn new(settings: &WatchSettings) -> Self {
        Self {
            quiet: settings.quiet,
            max_wait: settings.max_wait,
            first: None,
            last: None,
            paths: BTreeSet::new(),
        }
    }

    /// Records an event for `path` at `now`.
    pub fn push(&mut self, path: PathBuf, now: Instant) {
        self.first.get_or_insert(now);
        self.last = Some(now);
        self.paths.insert(path);
    }

    /// When the pending batch is due; `None` when nothing is pending.
    pub fn deadline(&self) -> Option<Instant> {
        let (first, last) = (self.first?, self.last?);
        Some((last + self.quiet).min(first + self.max_wait))
    }

    /// The pending paths, sorted and distinct, when the batch is due at `now`.
    pub fn take_due(&mut self, now: Instant) -> Option<Vec<PathBuf>> {
        if self.deadline()? > now {
            return None;
        }
        self.first = None;
        self.last = None;
        Some(std::mem::take(&mut self.paths).into_iter().collect())
    }

    /// Drops the pending paths under `root`.
    fn forget(&mut self, root: &Path) {
        self.paths.retain(|p| !p.starts_with(root));
        if self.paths.is_empty() {
            self.first = None;
            self.last = None;
        }
    }
}

/// Why the index changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// The source was added; this is its first scan.
    Added,
    /// File-system events named paths in the source.
    Events,
    /// The periodic rescan, or an event that asked for a rescan.
    Rescan,
    /// The source was resumed after a pause.
    Resumed,
}

/// One change the watcher made to the index.
#[derive(Debug, Clone, PartialEq)]
pub struct WatchUpdate {
    /// The source folder, as it was added.
    pub source: PathBuf,
    /// What caused the change.
    pub trigger: Trigger,
    /// What changed, or why the source folder could not be scanned. A source
    /// that cannot be scanned keeps its entries (a drive may be unplugged).
    pub result: FmResult<Rescan>,
}

/// Watches source folders and keeps a shared [`FileIndex`] current. Dropping
/// it stops the thread.
pub struct Watcher {
    tx: Sender<Msg>,
    worker: Option<JoinHandle<()>>,
}

impl fmt::Debug for Watcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Watcher").finish_non_exhaustive()
    }
}

type Reply = Sender<FmResult<()>>;

enum Msg {
    Fs(notify::Result<notify::Event>),
    Add(PathBuf, Reply),
    Remove(PathBuf, Reply),
    Pause(PathBuf, bool, Reply),
    Stop,
}

impl Watcher {
    /// Starts the watcher thread. `on_update` runs on that thread for every
    /// change to `index`.
    pub fn start(
        index: Arc<Mutex<FileIndex>>,
        settings: WatchSettings,
        on_update: impl FnMut(WatchUpdate) + Send + 'static,
    ) -> FmResult<Self> {
        Self::start_with::<RecommendedWatcher>(index, settings, on_update)
    }

    /// [`Watcher::start`] over any OS watcher; tests put in one that fails.
    fn start_with<W: notify::Watcher + Send + 'static>(
        index: Arc<Mutex<FileIndex>>,
        settings: WatchSettings,
        on_update: impl FnMut(WatchUpdate) + Send + 'static,
    ) -> FmResult<Self> {
        let (tx, rx) = mpsc::channel();
        let events = tx.clone();
        let os = W::new(
            move |event| {
                let _ = events.send(Msg::Fs(event));
            },
            notify::Config::default(),
        )
        .map_err(|e| watch_error("File changes cannot be watched.", Path::new(""), e))?;
        let worker = Worker {
            os,
            index,
            settings,
            on_update: Box::new(on_update),
            sources: BTreeMap::new(),
            debounce: Debounce::new(&settings),
        };
        let worker = thread::Builder::new()
            .name("fm-watch".into())
            .spawn(move || worker.run(&rx))
            .map_err(|e| FmError::internal(format!("cannot start the watcher thread: {e}")))?;
        Ok(Self {
            tx,
            worker: Some(worker),
        })
    }

    /// Watches `root` and scans it once ([`Trigger::Added`]).
    pub fn add_source(&self, root: &Path) -> FmResult<()> {
        self.ask(|reply| Msg::Add(root.to_path_buf(), reply))
    }

    /// Stops watching `root`. Its entries stay in the index.
    pub fn remove_source(&self, root: &Path) -> FmResult<()> {
        self.ask(|reply| Msg::Remove(root.to_path_buf(), reply))
    }

    /// Pauses or resumes `root`. A paused source is neither watched nor
    /// rescanned; resuming rescans it at once ([`Trigger::Resumed`]).
    pub fn set_paused(&self, root: &Path, paused: bool) -> FmResult<()> {
        self.ask(|reply| Msg::Pause(root.to_path_buf(), paused, reply))
    }

    fn ask(&self, msg: impl FnOnce(Reply) -> Msg) -> FmResult<()> {
        let stopped = || FmError::internal("the watcher thread has stopped");
        let (reply, answer) = mpsc::channel();
        self.tx.send(msg(reply)).map_err(|_| stopped())?;
        answer.recv().map_err(|_| stopped())?
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Source {
    /// The real location; macOS reports events under it (`/private/var`
    /// for `/var`).
    canonical: Option<PathBuf>,
    paused: bool,
    /// `None` when the interval reaches past what an [`Instant`] holds.
    next_rescan: Option<Instant>,
}

struct Worker<W> {
    os: W,
    index: Arc<Mutex<FileIndex>>,
    settings: WatchSettings,
    on_update: Box<dyn FnMut(WatchUpdate) + Send>,
    sources: BTreeMap<PathBuf, Source>,
    debounce: Debounce,
}

impl<W: notify::Watcher> Worker<W> {
    fn run(mut self, rx: &Receiver<Msg>) {
        loop {
            let msg = match self.deadline() {
                Some(at) => rx.recv_timeout(at.saturating_duration_since(Instant::now())),
                None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            match msg {
                Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                Ok(Msg::Fs(event)) => self.event(event),
                Ok(Msg::Add(root, reply)) => {
                    let result = self.add(&root);
                    let added = result.is_ok();
                    let _ = reply.send(result);
                    if added {
                        self.full_rescan(&root, Trigger::Added);
                    }
                }
                Ok(Msg::Remove(root, reply)) => {
                    let _ = reply.send(self.remove(&root));
                }
                Ok(Msg::Pause(root, paused, reply)) => {
                    let result = self.pause(&root, paused);
                    let resumed = result.is_ok() && !paused;
                    let _ = reply.send(result);
                    if resumed {
                        self.full_rescan(&root, Trigger::Resumed);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
            self.due_work();
        }
    }

    /// The next time there is work without a message: a debounced batch or a
    /// periodic rescan.
    fn deadline(&self) -> Option<Instant> {
        let rescans = self
            .sources
            .values()
            .filter(|s| !s.paused)
            .filter_map(|s| s.next_rescan);
        self.debounce.deadline().into_iter().chain(rescans).min()
    }

    fn due_work(&mut self) {
        let now = Instant::now();
        if let Some(paths) = self.debounce.take_due(now) {
            self.apply(paths);
        }
        let due: Vec<PathBuf> = self
            .sources
            .iter()
            .filter(|(_, s)| !s.paused && s.next_rescan.is_some_and(|at| at <= now))
            .map(|(root, _)| root.clone())
            .collect();
        for root in due {
            self.full_rescan(&root, Trigger::Rescan);
        }
    }

    fn add(&mut self, root: &Path) -> FmResult<()> {
        if self.sources.contains_key(root) {
            return Err(FmError::invalid("The folder is already a source.")
                .with_detail(root.display().to_string()));
        }
        if !root.is_dir() {
            return Err(FmError::invalid("The source is not a folder.")
                .with_detail(root.display().to_string()));
        }
        self.watch(root)?;
        self.sources.insert(
            root.to_path_buf(),
            Source {
                canonical: root.canonicalize().ok(),
                paused: false,
                next_rescan: Some(Instant::now()),
            },
        );
        Ok(())
    }

    fn remove(&mut self, root: &Path) -> FmResult<()> {
        let source = self
            .sources
            .remove(root)
            .ok_or_else(|| not_a_source(root))?;
        if !source.paused {
            self.unwatch(root);
        }
        self.debounce.forget(root);
        Ok(())
    }

    fn pause(&mut self, root: &Path, paused: bool) -> FmResult<()> {
        let source = self.sources.get(root).ok_or_else(|| not_a_source(root))?;
        if source.paused == paused {
            return Ok(());
        }
        if !paused {
            self.watch(root)?;
        }
        if let Some(source) = self.sources.get_mut(root) {
            source.paused = paused;
        }
        if paused {
            self.unwatch(root);
            self.debounce.forget(root);
        }
        Ok(())
    }

    /// Watches `root` for events. A source whose folder can be read counts as
    /// watched even when part of its tree is not: inotify (Linux) watches
    /// every subfolder on its own and fails on an unreadable one or at the
    /// watch limit, where the other OS watch the root alone. The first scan
    /// and the periodic rescan cover what the events miss.
    fn watch(&mut self, root: &Path) -> FmResult<()> {
        let Err(e) = self.os.watch(root, RecursiveMode::Recursive) else {
            return Ok(());
        };
        if fs::read_dir(root).is_err() {
            return Err(watch_error(
                "The source folder could not be watched.",
                root,
                e,
            ));
        }
        tracing::warn!(
            "{} is only partly watched; the periodic rescan covers the rest: {e}",
            root.display()
        );
        Ok(())
    }

    /// Stops watching `root`, then watches again the active sources that nest
    /// with it: inotify (Linux) keeps one watch per folder, shared by nested
    /// sources, and unwatching a folder drops every watch below it.
    fn unwatch(&mut self, root: &Path) {
        let _ = self.os.unwatch(root);
        let nested: Vec<PathBuf> = self
            .sources
            .iter()
            .filter(|(other, s)| {
                !s.paused
                    && other.as_path() != root
                    && (other.starts_with(root) || root.starts_with(other))
            })
            .map(|(other, _)| other.clone())
            .collect();
        // Outer sources first (a parent sorts before its children), so an
        // inner source is watched again after its parent's watch is renewed.
        for other in nested {
            let _ = self.os.unwatch(&other);
            if let Err(e) = self.watch(&other) {
                tracing::warn!("cannot watch {} again: {e}", other.display());
            }
        }
    }

    fn event(&mut self, event: notify::Result<notify::Event>) {
        let now = Instant::now();
        let paths = match event {
            Ok(event) if event.need_rescan() => return self.rescan_soon(),
            Ok(event) if matches!(event.kind, EventKind::Access(_)) => return,
            Ok(event) => event.paths,
            Err(e) if e.paths.is_empty() => {
                tracing::warn!("file watching failed, rescanning: {e}");
                return self.rescan_soon();
            }
            Err(e) => {
                tracing::warn!("file watching failed for some paths: {e}");
                e.paths
            }
        };
        for path in paths {
            if let Some(path) = self.in_source(&path) {
                self.debounce.push(path, now);
            }
        }
    }

    /// `path` as seen from the active source that holds it, the innermost
    /// one when sources nest.
    fn in_source(&self, path: &Path) -> Option<PathBuf> {
        self.sources
            .iter()
            .filter(|(_, s)| !s.paused)
            .filter_map(|(root, s)| {
                if path.starts_with(root) {
                    return Some((root, path.to_path_buf()));
                }
                let rest = path.strip_prefix(s.canonical.as_ref()?).ok()?;
                Some((root, root.join(rest)))
            })
            .max_by_key(|(root, _)| root.components().count())
            .map(|(_, path)| path)
    }

    fn rescan_soon(&mut self) {
        let now = Instant::now();
        for source in self.sources.values_mut() {
            source.next_rescan = Some(now);
        }
    }

    /// Applies a debounced batch, one refresh per source.
    fn apply(&mut self, paths: Vec<PathBuf>) {
        let mut by_source: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
        for path in paths {
            let root = self
                .sources
                .iter()
                .filter(|(root, s)| !s.paused && path.starts_with(root))
                .max_by_key(|(root, _)| root.components().count())
                .map(|(root, _)| root.clone());
            if let Some(root) = root {
                by_source.entry(root).or_default().push(path);
            }
        }
        for (root, paths) in by_source {
            let rescan = lock(&self.index).refresh(&root, &paths);
            if !rescan.is_quiet() {
                (self.on_update)(WatchUpdate {
                    source: root,
                    trigger: Trigger::Events,
                    result: Ok(rescan),
                });
            }
        }
    }

    fn full_rescan(&mut self, root: &Path, trigger: Trigger) {
        let Some(source) = self.sources.get_mut(root) else {
            return;
        };
        let now = Instant::now();
        source.next_rescan = now.checked_add(self.settings.rescan_interval);
        let result = discover(root).map(|found| lock(&self.index).rescan(root, &found));
        if let Err(e) = &result {
            tracing::warn!("rescan of {} failed: {e}", root.display());
        }
        (self.on_update)(WatchUpdate {
            source: root.to_path_buf(),
            trigger,
            result,
        });
    }
}

/// The index, even after a panic elsewhere left the lock poisoned: every
/// index operation leaves it consistent before it can fail.
fn lock(index: &Mutex<FileIndex>) -> MutexGuard<'_, FileIndex> {
    index.lock().unwrap_or_else(PoisonError::into_inner)
}

fn not_a_source(root: &Path) -> FmError {
    FmError::not_found("The source folder").with_detail(root.display().to_string())
}

fn watch_error(message: &str, path: &Path, e: notify::Error) -> FmError {
    match e.kind {
        notify::ErrorKind::Io(io) => FmError::io(message, path, &io),
        notify::ErrorKind::PathNotFound => FmError::new(ErrorCode::NotFound, message)
            .with_detail(format!("{}: not found", path.display())),
        kind => FmError::new(ErrorCode::Io, message)
            .with_detail(format!("{}: {kind:?}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::test_support::TempDir;

    const REGULAR: &str = "google-static/asap/Asap_Condensed-ExtraLightItalic.ttf";
    const OTHER: &str = "google-static/anybody/Anybody_Expanded-MediumItalic.ttf";

    /// The time the criterion allows from a file change to the index.
    const WITHIN: Duration = Duration::from_secs(1);

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

    /// A running watcher on a fresh, empty source folder.
    /// Fields drop in order: the watcher lets go of the folder before it is
    /// removed (Windows does not remove a watched folder).
    struct Fixture {
        watcher: Watcher,
        updates: Receiver<WatchUpdate>,
        index: Arc<Mutex<FileIndex>>,
        root: PathBuf,
        tmp: TempDir,
    }

    impl Fixture {
        fn new(label: &str, settings: WatchSettings) -> Self {
            Self::with::<RecommendedWatcher>(label, settings)
        }

        fn with<W: notify::Watcher + Send + 'static>(label: &str, settings: WatchSettings) -> Self {
            let tmp = TempDir::new(label);
            let root = tmp.path().join("Schriften ü");
            fs::create_dir_all(&root).expect("create source");
            let index = Arc::new(Mutex::new(FileIndex::new()));
            let (tx, updates) = mpsc::channel();
            let watcher = Watcher::start_with::<W>(index.clone(), settings, move |u| {
                let _ = tx.send(u);
            })
            .expect("start watcher");
            let fixture = Self {
                watcher,
                updates,
                index,
                root,
                tmp,
            };
            fixture.watcher.add_source(&fixture.root).expect("add");
            let first = fixture.next(Duration::from_secs(5)).expect("first scan");
            assert_eq!(first.trigger, Trigger::Added);
            fixture
        }

        fn next(&self, within: Duration) -> Option<WatchUpdate> {
            self.updates.recv_timeout(within).ok()
        }

        /// Waits until the index satisfies `done`, and returns how long it
        /// took. Fails after 5 s, so a slow runner reports a number.
        fn until(&self, done: impl Fn(&FileIndex) -> bool) -> Duration {
            let start = Instant::now();
            while !done(&lock(&self.index)) {
                let left = Duration::from_secs(5).saturating_sub(start.elapsed());
                assert!(!left.is_zero(), "the index did not update within 5 s");
                let _ = self.next(left.min(Duration::from_millis(50)));
            }
            start.elapsed()
        }

        fn indexed(&self, rel: &str) -> Option<crate::scanner::IndexedFile> {
            lock(&self.index).get(&self.root.join(rel)).cloned()
        }
    }

    /// The OS watcher, but every watch reports the inotify watch limit, as
    /// Linux does when it has watched only part of a large source.
    struct OverLimit(RecommendedWatcher);

    impl notify::Watcher for OverLimit {
        fn new<F: notify::EventHandler>(
            handler: F,
            config: notify::Config,
        ) -> notify::Result<Self> {
            RecommendedWatcher::new(handler, config).map(Self)
        }

        fn watch(&mut self, path: &Path, mode: RecursiveMode) -> notify::Result<()> {
            self.0.watch(path, mode)?;
            Err(notify::Error::new(notify::ErrorKind::MaxFilesWatch).add_path(path.to_path_buf()))
        }

        fn unwatch(&mut self, path: &Path) -> notify::Result<()> {
            self.0.unwatch(path)
        }

        fn kind() -> notify::WatcherKind {
            RecommendedWatcher::kind()
        }
    }

    fn present(index: &FileIndex, path: &Path) -> bool {
        index.get(path).is_some_and(|f| !f.gone)
    }

    fn gone(index: &FileIndex, path: &Path) -> bool {
        index.get(path).is_some_and(|f| f.gone)
    }

    #[test]
    fn a_created_file_is_indexed_within_a_second() {
        let f = Fixture::new("watch-create", WatchSettings::default());
        let path = f.root.join("sub/a.ttf");

        let expected = crate::scanner::FileHash::of(&fixture(REGULAR));
        put(&path, &fixture(REGULAR));
        let took = f.until(|i| i.get(&path).is_some_and(|e| !e.gone && e.hash == expected));

        assert!(took < WITHIN, "create took {took:?}");
    }

    #[test]
    fn a_modified_file_is_read_again_within_a_second() {
        let f = Fixture::new("watch-modify", WatchSettings::default());
        let path = f.root.join("a.ttf");
        put(&path, &fixture(REGULAR));
        f.until(|i| present(i, &path));

        let other = fixture(OTHER);
        let expected = crate::scanner::FileHash::of(&other);
        put(&path, &other);
        let took = f.until(|i| i.get(&path).is_some_and(|e| e.hash == expected));

        assert!(took < WITHIN, "modify took {took:?}");
    }

    #[test]
    fn a_removed_file_is_marked_gone_within_a_second() {
        let f = Fixture::new("watch-remove", WatchSettings::default());
        let path = f.root.join("a.ttf");
        put(&path, &fixture(REGULAR));
        f.until(|i| present(i, &path));

        fs::remove_file(&path).expect("remove");
        let took = f.until(|i| gone(i, &path));

        assert!(took < WITHIN, "remove took {took:?}");
    }

    #[test]
    fn a_renamed_file_moves_within_a_second() {
        let f = Fixture::new("watch-rename", WatchSettings::default());
        let from = f.root.join("a.ttf");
        let to = f.root.join("b.ttf");
        put(&from, &fixture(REGULAR));
        f.until(|i| present(i, &from));

        fs::rename(&from, &to).expect("rename");
        let took = f.until(|i| gone(i, &from) && present(i, &to));

        assert!(took < WITHIN, "rename took {took:?}");
    }

    #[test]
    fn a_folder_renamed_into_and_out_of_the_source_is_followed() {
        let f = Fixture::new("watch-folder", WatchSettings::default());
        let outside = f.tmp.path().join("incoming");
        put(&outside.join("a.ttf"), &fixture(REGULAR));
        put(&outside.join("deep/b.ttf"), &fixture(OTHER));
        let a = f.root.join("incoming/a.ttf");
        let b = f.root.join("incoming/deep/b.ttf");

        fs::rename(&outside, f.root.join("incoming")).expect("move in");
        f.until(|i| present(i, &a) && present(i, &b));

        fs::rename(f.root.join("incoming"), &outside).expect("move out");
        f.until(|i| gone(i, &a) && gone(i, &b));
    }

    #[test]
    fn a_burst_of_events_is_applied_in_few_batches() {
        let f = Fixture::new("watch-burst", WatchSettings::default());
        let regular = fixture(REGULAR);
        let paths: Vec<PathBuf> = (0..20).map(|n| f.root.join(format!("f{n}.ttf"))).collect();

        for path in &paths {
            put(path, &regular);
            put(path, &regular);
        }
        let start = Instant::now();
        let mut batches = 0;
        while !paths.iter().all(|p| present(&lock(&f.index), p)) {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "not indexed in 5 s"
            );
            if f.next(Duration::from_millis(50)).is_some() {
                batches += 1;
            }
        }
        while f.next(Duration::from_millis(700)).is_some() {
            batches += 1;
        }

        // 40 writes raise far more than 40 OS events; a few batches apply them.
        assert!(batches <= 5, "{batches} batches for 40 writes");
    }

    #[test]
    fn the_periodic_rescan_catches_a_missed_event() {
        let settings = WatchSettings {
            rescan_interval: Duration::from_millis(300),
            ..WatchSettings::default()
        };
        let f = Fixture::new("watch-periodic", settings);
        let ghost = f.root.join("ghost.ttf");
        // An entry for a file that is not there: its remove event was missed.
        {
            let mut index = lock(&f.index);
            index.insert(crate::scanner::IndexedFile {
                path: ghost.clone(),
                size: 1,
                modified: None,
                hash: crate::scanner::FileHash::of(b""),
                parsed: crate::parser::parse(b""),
                gone: false,
            });
        }

        // No file-system event names it; only a rescan can find it gone.
        let start = Instant::now();
        loop {
            let update = f.next(Duration::from_secs(5)).expect("a periodic rescan");
            assert!(start.elapsed() < Duration::from_secs(5), "no rescan in 5 s");
            if update.trigger == Trigger::Rescan
                && update.result.expect("rescanned").gone == vec![ghost.clone()]
            {
                break;
            }
        }
        assert!(gone(&lock(&f.index), &ghost));
    }

    #[test]
    fn a_paused_source_is_not_watched_until_resumed() {
        let f = Fixture::new("watch-pause", WatchSettings::default());
        let path = f.root.join("a.ttf");

        f.watcher.set_paused(&f.root, true).expect("pause");
        put(&path, &fixture(REGULAR));
        assert!(f.next(Duration::from_millis(1500)).is_none());
        assert!(f.indexed("a.ttf").is_none());

        f.watcher.set_paused(&f.root, false).expect("resume");
        let update = f.next(Duration::from_secs(5)).expect("resume rescan");
        assert_eq!(update.trigger, Trigger::Resumed);
        assert_eq!(update.result.expect("rescanned").added, vec![path.clone()]);

        // Watched again after the resume.
        fs::remove_file(&path).expect("remove");
        let took = f.until(|i| gone(i, &path));
        assert!(took < WITHIN, "remove after resume took {took:?}");
    }

    #[test]
    fn nested_sources_stay_watched_when_one_is_paused_or_removed() {
        let f = Fixture::new("watch-nested", WatchSettings::default());
        let inner = f.root.join("inner");
        fs::create_dir_all(&inner).expect("create inner");
        f.watcher.add_source(&inner).expect("add inner");
        loop {
            let update = f.next(Duration::from_secs(5)).expect("inner first scan");
            if update.source == inner && update.trigger == Trigger::Added {
                break;
            }
        }

        // The outer source paused: the inner one still sees its events.
        f.watcher.set_paused(&f.root, true).expect("pause outer");
        let a = inner.join("a.ttf");
        put(&a, &fixture(REGULAR));
        let took = f.until(|i| present(i, &a));
        assert!(took < WITHIN, "inner create took {took:?}");
        f.watcher.set_paused(&f.root, false).expect("resume outer");

        // The inner source removed: the outer one still sees events below it.
        f.watcher.remove_source(&inner).expect("remove inner");
        let b = inner.join("b.ttf");
        put(&b, &fixture(OTHER));
        let took = f.until(|i| present(i, &b));
        assert!(took < WITHIN, "outer create took {took:?}");
    }

    #[test]
    fn a_partly_watched_source_is_added_and_resumed() {
        // `with` asserts the first scan, so the add did not fail.
        let f = Fixture::with::<OverLimit>("watch-partly", WatchSettings::default());
        let path = f.root.join("a.ttf");
        put(&path, &fixture(REGULAR));
        f.until(|i| present(i, &path));

        f.watcher.set_paused(&f.root, true).expect("pause");
        f.watcher.set_paused(&f.root, false).expect("resume");
        let update = f.next(Duration::from_secs(5)).expect("resume rescan");
        assert_eq!(update.trigger, Trigger::Resumed);
    }

    #[test]
    fn unknown_missing_and_duplicate_sources_are_errors() {
        let f = Fixture::new("watch-errors", WatchSettings::default());
        let missing = f.tmp.path().join("missing");

        let err = f.watcher.set_paused(&missing, true).expect_err("unknown");
        assert_eq!(err.code(), ErrorCode::NotFound);
        let err = f.watcher.remove_source(&missing).expect_err("unknown");
        assert_eq!(err.code(), ErrorCode::NotFound);
        let err = f.watcher.add_source(&missing).expect_err("missing");
        assert_eq!(err.code(), ErrorCode::InvalidRequest);
        let err = f.watcher.add_source(&f.root).expect_err("duplicate");
        assert_eq!(err.code(), ErrorCode::InvalidRequest);
    }

    #[test]
    fn a_removed_source_keeps_its_entries_and_is_not_watched() {
        let f = Fixture::new("watch-removed", WatchSettings::default());
        let path = f.root.join("a.ttf");
        put(&path, &fixture(REGULAR));
        f.until(|i| present(i, &path));

        f.watcher.remove_source(&f.root).expect("remove source");
        fs::remove_file(&path).expect("remove");

        assert!(f.next(Duration::from_millis(1500)).is_none());
        assert!(present(&lock(&f.index), &path));
    }

    #[test]
    fn the_debounce_waits_for_quiet_and_coalesces_paths() {
        let settings = WatchSettings::default();
        let mut d = Debounce::new(&settings);
        let t0 = Instant::now();
        assert_eq!(d.deadline(), None);
        assert_eq!(d.take_due(t0), None);

        d.push("a".into(), t0);
        d.push("b".into(), t0 + Duration::from_millis(50));
        d.push("a".into(), t0 + Duration::from_millis(100));

        assert_eq!(d.deadline(), Some(t0 + Duration::from_millis(300)));
        assert_eq!(d.take_due(t0 + Duration::from_millis(299)), None);
        assert_eq!(
            d.take_due(t0 + Duration::from_millis(300)),
            Some(vec![PathBuf::from("a"), PathBuf::from("b")])
        );
        assert_eq!(d.deadline(), None);
    }

    #[test]
    fn the_debounce_applies_a_long_burst_after_max_wait() {
        let settings = WatchSettings::default();
        let mut d = Debounce::new(&settings);
        let t0 = Instant::now();

        // An event every 100 ms never leaves 200 ms of quiet.
        for n in 0..10u64 {
            d.push(format!("f{n}").into(), t0 + Duration::from_millis(100 * n));
        }

        assert_eq!(d.deadline(), Some(t0 + settings.max_wait));
        assert_eq!(
            d.take_due(t0 + settings.max_wait).map(|p| p.len()),
            Some(10)
        );
        assert!(settings.max_wait + settings.quiet < WITHIN);
    }

    #[test]
    fn the_rescan_interval_comes_from_the_config() {
        assert_eq!(
            WatchSettings::from_config(&Config::default()).rescan_interval,
            Duration::from_secs(60 * 60)
        );
        let config = Config {
            rescan_interval_minutes: 0,
            ..Config::default()
        };
        assert_eq!(
            WatchSettings::from_config(&config).rescan_interval,
            Duration::from_secs(60)
        );
    }
}
