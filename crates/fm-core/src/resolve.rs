//! "Keep this one" (E8.3): resolve a duplicate group by moving the other
//! copies to the OS trash, with undo.
//!
//! - **Rule 2.** A copy goes to the OS trash through [`Trash`], never to
//!   `remove_file`. Only a copy listed with the kept copy in a duplicate
//!   group can move ([`KeepCopyRequest::check`]), and only while the kept
//!   copy is still on disk, so the last copy of a font never moves.
//! - **Rule 3.** A copy the app activated stays where it is (`Skipped`):
//!   moving it would leave a registration that points nowhere.
//! - **Dry run.** The same checks, nothing moved: the plan the UI shows
//!   before the user confirms.
//! - **Undo.** Every move is logged with where the trash put the file;
//!   [`DuplicateResolver::undo_resolution`] moves it back, never over a
//!   file that is at the old path again. A trashed copy is `gone` in the
//!   index, so its ids, tags and ratings come back with it (ADR 0004).
//! - **Metadata (E56.5).** Before the copies leave, the kept copy takes over
//!   what the user put on them (`index/metadata_merge.rs`), in the
//!   transaction of the log; undo takes back only what the merge added.
//! - **Events.** Moved copies' faces are sent as removed, restored ones as
//!   added, in one [`Event::LibraryChanged`].

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use fm_api::event::Event;
use fm_api::library::{KeepCopyRequest, Resolution, TrashMove, TrashMoveStatus};
use fm_api::{ApiResult, ErrorCode, FaceId, ResolutionId};
use fm_platform::{Trash, TrashErrorKind, Trashed};

use crate::activation::Emit;
use crate::error::FmError;
use crate::index::{CopyFile, Index, MergeChoice, MergePlan, StoredMove};

/// Why a copy the app activated does not move (rule 3).
const ACTIVE_COPY: &str = "This copy is active. Deactivate it first, then move it to the trash.";
/// Why a copy in a system font folder does not move (rule 4).
const SYSTEM_COPY: &str = "This copy is in a system font folder. Font Fu never moves system fonts.";
/// Why a copy that is no longer on disk does not move.
const NOT_THERE: &str = "The file is not there any more.";

/// Moves duplicate copies to the trash and back, and keeps the log.
pub struct DuplicateResolver {
    index: Arc<Mutex<Index>>,
    trash: Arc<dyn Trash>,
    emit: Emit,
    /// The OS font folders: a copy in one never moves (rule 4).
    system: Vec<PathBuf>,
}

impl std::fmt::Debug for DuplicateResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DuplicateResolver").finish_non_exhaustive()
    }
}

impl DuplicateResolver {
    /// A resolver over `index` that moves files through `trash` and sends
    /// every change to `emit`.
    pub fn new(
        index: Arc<Mutex<Index>>,
        trash: Arc<dyn Trash>,
        emit: impl Fn(Event) + Send + Sync + 'static,
    ) -> Self {
        Self {
            index,
            trash,
            emit: Arc::new(emit),
            system: crate::duplicate_sets::system_folders(),
        }
    }

    /// The same resolver with `folders` as the OS font folders (tests).
    #[must_use]
    pub fn with_system_folders(mut self, folders: Vec<PathBuf>) -> Self {
        self.system = crate::duplicate_sets::with_links(folders);
        self
    }

    fn lock(&self) -> MutexGuard<'_, Index> {
        // The log and the gone flags are only changed in statements that
        // stand alone, so a panic elsewhere leaves them usable.
        self.index.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// [`fm_api::LibraryService::keep_copy`]: [`Self::keep_copy_with`]
    /// with the default of every conflict, the kept copy's value (E56.5).
    pub fn keep_copy(&self, req: KeepCopyRequest) -> ApiResult<Resolution> {
        self.keep_copy_with(req, &[])
    }

    /// What `req` would merge onto the kept copy (E56.5): what it takes
    /// over, and every field where two copies differ. Reads only; the
    /// request is checked as [`Self::keep_copy`] checks it.
    pub fn merge_plan(&self, req: &KeepCopyRequest) -> ApiResult<MergePlan> {
        let index = self.lock();
        req.check(&index.duplicates()?)?;
        let mut wanted: Vec<&str> = req.trash.iter().map(String::as_str).collect();
        wanted.push(&req.keep);
        let files = index.copy_files(&wanted, &self.system)?;
        let kept = files
            .get(&req.keep)
            .map(|f| f.faces.clone())
            .unwrap_or_default();
        let leaving: Vec<FaceId> = req
            .trash
            .iter()
            .filter_map(|p| files.get(p))
            .flat_map(|f| f.faces.iter().copied())
            .collect();
        Ok(index.merge_plan(&kept, &leaving)?)
    }

    /// Keeps one copy and moves the others to the trash; before they leave,
    /// the kept copy takes over their metadata (E56.5), with `choices`
    /// taking the other value of a conflict of [`Self::merge_plan`]. Only
    /// the copies that moved give their metadata; the merge, the gone flags
    /// and the undo log are one transaction, so when nothing moves or the
    /// log cannot be written the kept copy's metadata is as it was.
    pub fn keep_copy_with(
        &self,
        req: KeepCopyRequest,
        choices: &[MergeChoice],
    ) -> ApiResult<Resolution> {
        if !choices.is_empty() {
            self.merge_plan(&req)?.choose(choices)?;
        }
        let mut index = self.lock();
        req.check(&index.duplicates()?)?;
        let mut wanted: Vec<&str> = req.trash.iter().map(String::as_str).collect();
        wanted.push(&req.keep);
        let files = index.copy_files(&wanted, &self.system)?;
        let kept_there = files
            .get(&req.keep)
            .is_some_and(|f| fs::symlink_metadata(&f.path).is_ok());
        if !kept_there {
            return Err(FmError::new(
                ErrorCode::NotFound,
                format!(
                    "The copy to keep is not there any more, so nothing was moved: {}",
                    req.keep
                ),
            )
            .into());
        }

        let mut moves: Vec<StoredMove> = Vec::new();
        for (position, shown) in req.trash.iter().enumerate() {
            let position = u32::try_from(position).unwrap_or(u32::MAX);
            // `check` passed, so every copy is a library file.
            let Some(file) = files.get(shown) else {
                continue;
            };
            let (status, message) = if file.system {
                (TrashMoveStatus::Skipped, Some(SYSTEM_COPY))
            } else if file.active {
                (TrashMoveStatus::Skipped, Some(ACTIVE_COPY))
            } else if fs::symlink_metadata(&file.path).is_err() {
                (TrashMoveStatus::Skipped, Some(NOT_THERE))
            } else {
                (TrashMoveStatus::Planned, None)
            };
            moves.push(StoredMove {
                position,
                path: file.path.clone(),
                row: TrashMove {
                    path: shown.clone(),
                    status,
                    message: message.map(str::to_owned),
                },
                trashed: None,
            });
        }
        if req.dry_run {
            return Ok(Resolution {
                id: None,
                kept: req.keep,
                at: index.now()?,
                dry_run: true,
                undone: false,
                moves: moves.into_iter().map(|m| m.row).collect(),
            });
        }

        let mut removed: Vec<FaceId> = Vec::new();
        for m in &mut moves {
            if m.row.status != TrashMoveStatus::Planned {
                continue;
            }
            // Nothing in this loop returns early: a copy that moved must
            // reach the log below, or be put back.
            match self.trash.trash(&m.path) {
                Ok(item) => {
                    if let Some(file) = files.get(&m.row.path) {
                        removed.extend(&file.faces);
                    }
                    m.row.status = TrashMoveStatus::Trashed;
                    m.trashed = Some((item.location, item.info));
                }
                Err(e) => {
                    tracing::warn!(path = %m.path.display(), error = %e, "moving a copy to the trash failed");
                    // A drive without a trash leaves the copy where it is.
                    m.row.status = if e.kind == TrashErrorKind::NoTrash {
                        TrashMoveStatus::Skipped
                    } else {
                        TrashMoveStatus::Failed
                    };
                    m.row.message = Some(e.message().to_owned());
                }
            }
        }

        let moved = moves
            .iter()
            .any(|m| m.row.status == TrashMoveStatus::Trashed);
        let (id, at) = if moved {
            let merge = match moved_merge(&index, &files, &req.keep, &moves, choices) {
                Ok(merge) => merge,
                Err(e) => {
                    self.put_back(&moves);
                    return Err(e.into());
                }
            };
            match index.insert_resolution(&req.keep, &moves, &merge) {
                Ok((id, at)) => (Some(id), at),
                Err(e) => {
                    // Without the log there is no undo: put every moved
                    // copy back before the error is reported.
                    self.put_back(&moves);
                    return Err(e.into());
                }
            }
        } else {
            (None, index.now()?)
        };
        drop(index);
        if !removed.is_empty() {
            (self.emit)(Event::LibraryChanged {
                from_watcher: false,
                source: None,
                added: Vec::new(),
                removed,
                updated: Vec::new(),
            });
        }
        Ok(Resolution {
            id,
            kept: req.keep,
            at,
            dry_run: false,
            undone: false,
            moves: moves.into_iter().map(|m| m.row).collect(),
        })
    }

    /// Moves every `Trashed` copy of `moves` back from the trash, after the
    /// log of their move could not be written. A copy that cannot come back
    /// is logged with where it is, so it can be found by hand.
    fn put_back(&self, moves: &[StoredMove]) {
        for m in moves {
            let Some((location, info)) = m.trashed.clone() else {
                continue;
            };
            let item = Trashed {
                original: m.path.clone(),
                location,
                info,
            };
            if let Err(e) = self.trash.restore(&item) {
                tracing::error!(
                    path = %m.path.display(),
                    location = %item.location.display(),
                    error = %e,
                    "a copy moved to the trash could not be logged or put back"
                );
            }
        }
    }

    /// [`fm_api::LibraryService::list_resolutions`].
    pub fn list_resolutions(&self) -> ApiResult<Vec<Resolution>> {
        Ok(self.lock().resolutions()?)
    }

    /// [`fm_api::LibraryService::undo_resolution`].
    pub fn undo_resolution(&self, id: ResolutionId) -> ApiResult<Resolution> {
        let mut index = self.lock();
        let (mut resolution, mut moves) = index
            .resolution(id)?
            .ok_or_else(|| FmError::not_found(format!("Action {id}")))?;
        let mut added: Vec<FaceId> = Vec::new();
        for m in &mut moves {
            // Moved copies, and copies an earlier undo could not put back.
            let waiting = matches!(
                m.row.status,
                TrashMoveStatus::Trashed | TrashMoveStatus::Failed
            );
            let Some((location, info)) = m.trashed.clone().filter(|_| waiting) else {
                continue;
            };
            let item = Trashed {
                original: m.path.clone(),
                location,
                info,
            };
            match self.trash.restore(&item) {
                Ok(()) => {
                    m.row.status = TrashMoveStatus::Restored;
                    m.row.message = None;
                }
                Err(e) => {
                    tracing::warn!(path = %m.path.display(), error = %e, "putting a copy back failed");
                    m.row.status = TrashMoveStatus::Failed;
                    m.row.message = Some(e.message().to_owned());
                }
            }
        }
        let undone = !moves
            .iter()
            .any(|m| m.trashed.is_some() && m.row.status != TrashMoveStatus::Restored);
        index.update_resolution(id, &moves, undone)?;
        // The restored files are in the library again: their faces are
        // sent as added. Only the event depends on this read.
        let back: Vec<&str> = moves
            .iter()
            .filter(|m| m.row.status == TrashMoveStatus::Restored && m.trashed.is_some())
            .map(|m| m.row.path.as_str())
            .collect();
        match index.copy_files(&back, &self.system) {
            Ok(files) => added.extend(files.values().flat_map(|f| f.faces.iter().copied())),
            Err(e) => tracing::warn!(error = %e, "reading the restored faces failed"),
        }
        drop(index);
        if !added.is_empty() {
            (self.emit)(Event::LibraryChanged {
                from_watcher: false,
                source: None,
                added,
                removed: Vec::new(),
                updated: Vec::new(),
            });
        }
        resolution.undone = undone;
        resolution.moves = moves.into_iter().map(|m| m.row).collect();
        Ok(resolution)
    }
}

/// The merge of the copies of `moves` that moved, with the `choices`
/// that still name one of its conflicts (a copy that did not move gives
/// nothing, so its conflicts are gone).
fn moved_merge(
    index: &Index,
    files: &std::collections::HashMap<String, CopyFile>,
    keep: &str,
    moves: &[StoredMove],
    choices: &[MergeChoice],
) -> crate::error::FmResult<MergePlan> {
    let kept = files.get(keep).map(|f| f.faces.clone()).unwrap_or_default();
    let leaving: Vec<FaceId> = moves
        .iter()
        .filter(|m| m.row.status == TrashMoveStatus::Trashed)
        .filter_map(|m| files.get(&m.row.path))
        .flat_map(|f| f.faces.iter().copied())
        .collect();
    let mut plan = index.merge_plan(&kept, &leaving)?;
    let still: Vec<MergeChoice> = choices
        .iter()
        .filter(|c| {
            plan.conflicts
                .iter()
                .any(|k| k.kept_face == c.kept_face && k.field == c.field && k.other_face == c.take)
        })
        .copied()
        .collect();
    plan.choose(&still)?;
    Ok(plan)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod merge_tests;
