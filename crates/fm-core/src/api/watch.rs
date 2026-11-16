//! Watched folders in the library (E51.1): [`CoreApi::with_watcher`] starts a
//! [`Watcher`] over every source and keeps it for the API's lifetime. The app
//! turns it on (it hosts the MCP server and the local endpoint too); the
//! CLI's one-shot direct link does not.
//!
//! - **What is watched** follows the index: after every change to the
//!   sources (added, removed, paused, resumed, rebuilt, a catalog download
//!   folder) [`CoreApi::sync_watch`] tells the watcher what changed. A source
//!   is registered without a scan; the user's scan reads a new one, and the
//!   periodic rescan catches what changed while the app was closed. A paused
//!   source is not watched; resuming it rescans it at once (`watch.rs`).
//!   The first sync, of every source at start, runs on a thread of its own
//!   (`fm-watch-start`): registering a large tree (inotify watches every
//!   subfolder) must not delay the first page (rule 8, cold start).
//! - **Every update** is applied to the index and sent as one
//!   [`Event::LibraryChanged`] naming the faces it added, removed and read
//!   again, as a scan does. Faces of a source whose sidecars changed count
//!   as read again: the merge ([`Index::apply_rescan`]) may have changed
//!   their metadata.
//! - **Memory** (ADR 0019): the file index is loaded from the database for
//!   one change and let go after, so an idle watcher holds none. A change
//!   that comes while a scan request holds the file index uses it and leaves
//!   it to the scan. Scans and changes take the file index one at a time.
//! - The watcher only reads font files (ADR 0004, rule 2).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use fm_api::library::Source;
use fm_api::{Event, FaceId, SettingsService};

use super::{Broadcast, CoreApi, TraitPass, changes, lock};
use crate::error::{FmError, FmResult};
use crate::index::Index;
use crate::scanner::{FileIndex, WatchSettings, WatchSink, WatchUpdate, Watcher};

/// The running watcher and the sources it was told about, by folder, with
/// whether each is paused.
pub(super) struct Watching {
    pub(super) watcher: Watcher,
    pub(super) known: Mutex<HashMap<PathBuf, bool>>,
}

impl Watching {
    /// Tells the watcher how `sources` differ from what it watches. A folder
    /// that cannot be watched (not there now) is logged and tried again at
    /// the next sync.
    fn sync(&self, sources: &[Source]) {
        let mut known = lock(&self.known);
        let wanted: HashMap<PathBuf, bool> = sources
            .iter()
            .map(|s| (PathBuf::from(&s.path), s.paused))
            .collect();
        known.retain(|root, _| {
            if wanted.contains_key(root) {
                return true;
            }
            if let Err(e) = self.watcher.remove_source(root) {
                tracing::warn!(source = %root.display(), error = %e, "source not unwatched");
            }
            false
        });
        for (root, &paused) in &wanted {
            let result = match known.get(root) {
                Some(&was) if was == paused => continue,
                Some(_) => self.watcher.set_paused(root, paused),
                None => self.watcher.watch_source(root, paused),
            };
            match result {
                Ok(()) => {
                    known.insert(root.clone(), paused);
                }
                Err(e) => {
                    tracing::warn!(source = %root.display(), error = %e, "source not watched");
                }
            }
        }
    }
}

impl CoreApi {
    /// Starts watching every source with the rescan interval of the
    /// settings. Call it after [`CoreApi::with_trait_pass`], so faces the
    /// watcher adds are measured too.
    #[must_use]
    pub fn with_watcher(self) -> Self {
        let settings = self
            .settings
            .get_settings()
            .map(|s| WatchSettings::from_settings(&s))
            .unwrap_or_default();
        self.with_watch_settings(settings)
    }

    /// [`CoreApi::with_watcher`] with these timings; tests debounce longer.
    #[must_use]
    pub fn with_watch_settings(mut self, settings: WatchSettings) -> Self {
        let sink = LibrarySink {
            index: Arc::clone(&self.index),
            files: Arc::clone(&self.files),
            events: Arc::clone(&self.events),
            trait_pass: self.trait_pass.clone(),
        };
        match Watcher::start_sink(sink, settings) {
            Ok(watcher) => {
                let watching = Arc::new(Watching {
                    watcher,
                    known: Mutex::new(HashMap::new()),
                });
                let (index, first) = (Arc::clone(&self.index), Arc::clone(&watching));
                let started = std::thread::Builder::new()
                    .name("fm-watch-start".into())
                    .spawn(move || sync_with(&index, &first));
                if let Err(e) = started {
                    tracing::warn!(error = %e, "watching on the calling thread");
                    sync_with(&self.index, &watching);
                }
                self.watching = Some(watching);
            }
            Err(e) => tracing::warn!(error = %e, "folders are not watched"),
        }
        self
    }

    /// Brings what the watcher watches in line with the sources of the
    /// index. Never called with the index locked: the watcher thread may be
    /// waiting for it.
    /// While the first sync still runs, this waits for it.
    pub(super) fn sync_watch(&self) {
        if let Some(watching) = &self.watching {
            sync_with(&self.index, watching);
        }
    }
}

/// [`Watching::sync`] with the sources of `index`.
fn sync_with(index: &Mutex<Index>, watching: &Watching) {
    // The guard must go before `sync`: a temporary in a `match` scrutinee
    // would live to the end of the match.
    let sources = lock(index).sources();
    match sources {
        Ok(sources) => watching.sync(&sources),
        Err(e) => tracing::warn!(error = %e, "watched sources not updated"),
    }
}

/// Applies the watcher's changes to the library.
struct LibrarySink {
    index: Arc<Mutex<Index>>,
    files: Arc<Mutex<Option<FileIndex>>>,
    events: Arc<Broadcast>,
    trait_pass: Option<TraitPass>,
}

impl WatchSink for LibrarySink {
    fn change(
        &mut self,
        change: &mut dyn FnMut(&mut FileIndex) -> Option<WatchUpdate>,
    ) -> FmResult<()> {
        let mut files = lock(&self.files);
        let borrowed = files.is_some();
        if !borrowed {
            *files = Some(lock(&self.index).load_files()?);
        }
        let Some(loaded) = files.as_mut() else {
            return Err(FmError::internal("the file index is not loaded"));
        };
        let applied = match change(loaded) {
            Some(update) => self.apply(loaded, &update),
            None => Ok(None),
        };
        // A failed write leaves the file index ahead of the database: the
        // next scan loads it back rather than skip what was not written.
        if !borrowed || applied.is_err() {
            *files = None;
        }
        drop(files);
        if !borrowed {
            fm_platform::memory::release_free_memory();
        }
        if let Some(mut event) = applied? {
            // The UI tells the user of fonts the watcher added (E51.2).
            if let Event::LibraryChanged { from_watcher, .. } = &mut event {
                *from_watcher = true;
            }
            let read = matches!(&event, Event::LibraryChanged { added, updated, .. }
                if !added.is_empty() || !updated.is_empty());
            self.events.send(&event);
            if read && let Some(pass) = &self.trait_pass {
                pass.start();
            }
        }
        Ok(())
    }

    fn failed(&mut self, update: WatchUpdate) {
        if let Err(e) = update.result {
            tracing::warn!(source = %update.source.display(), error = %e, "watched source not scanned");
        }
    }
}

impl LibrarySink {
    /// Writes `update` to the index and returns what the library gained and
    /// lost; `None` when nothing changed or the source is gone.
    fn apply(&self, files: &FileIndex, update: &WatchUpdate) -> FmResult<Option<Event>> {
        let rescan = match &update.result {
            Ok(rescan) => rescan,
            Err(e) => {
                tracing::warn!(source = %update.source.display(), error = %e, "watched source not scanned");
                return Ok(None);
            }
        };
        let mut index = lock(&self.index);
        let Some(source) = index
            .sources()?
            .into_iter()
            .find(|s| Path::new(&s.path) == update.source)
        else {
            // Removed while the change was on its way.
            return Ok(None);
        };
        let before = index.faces_of_source(source.id)?;
        index.apply_rescan(source.id, files, rescan)?;
        let after = index.faces_of_source(source.id)?;
        drop(index);
        let event = changes(source.id, &before, &after, rescan);
        if update.sidecars.is_empty() {
            return Ok(event);
        }
        Ok(with_sidecars(event, source.id, &after))
    }
}

/// `event` with every face of the source that it does not add or remove
/// read again: its sidecar was merged.
fn with_sidecars(
    event: Option<Event>,
    source: fm_api::SourceId,
    after: &HashMap<FaceId, PathBuf>,
) -> Option<Event> {
    let (mut added, mut removed, mut updated) = match event {
        Some(Event::LibraryChanged {
            added,
            removed,
            updated,
            ..
        }) => (added, removed, updated),
        _ => (Vec::new(), Vec::new(), Vec::new()),
    };
    let named: HashSet<FaceId> = added
        .iter()
        .chain(&removed)
        .chain(&updated)
        .copied()
        .collect();
    updated.extend(after.keys().filter(|f| !named.contains(f)).copied());
    if added.is_empty() && removed.is_empty() && updated.is_empty() {
        return None;
    }
    added.sort();
    removed.sort();
    updated.sort();
    Some(Event::LibraryChanged {
        from_watcher: false,
        source: Some(source),
        added,
        removed,
        updated,
    })
}
