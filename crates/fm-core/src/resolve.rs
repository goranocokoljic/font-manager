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
//! - **Events.** Moved copies' faces are sent as removed, restored ones as
//!   added, in one [`Event::LibraryChanged`].

use std::fs;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use fm_api::event::Event;
use fm_api::library::{KeepCopyRequest, Resolution, TrashMove, TrashMoveStatus};
use fm_api::{ApiResult, ErrorCode, FaceId, ResolutionId};
use fm_platform::{Trash, TrashErrorKind, Trashed};

use crate::activation::Emit;
use crate::error::FmError;
use crate::index::{Index, StoredMove};

/// Why a copy the app activated does not move (rule 3).
const ACTIVE_COPY: &str = "This copy is active. Deactivate it first, then move it to the trash.";
/// Why a copy that is no longer on disk does not move.
const NOT_THERE: &str = "The file is not there any more.";

/// Moves duplicate copies to the trash and back, and keeps the log.
pub struct DuplicateResolver {
    index: Arc<Mutex<Index>>,
    trash: Arc<dyn Trash>,
    emit: Emit,
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
        }
    }

    fn lock(&self) -> MutexGuard<'_, Index> {
        // The log and the gone flags are only changed in statements that
        // stand alone, so a panic elsewhere leaves them usable.
        self.index.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// [`fm_api::LibraryService::keep_copy`].
    pub fn keep_copy(&self, req: KeepCopyRequest) -> ApiResult<Resolution> {
        let mut index = self.lock();
        req.check(&index.duplicates()?)?;
        let mut wanted: Vec<&str> = req.trash.iter().map(String::as_str).collect();
        wanted.push(&req.keep);
        let files = index.copy_files(&wanted)?;
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
            let (status, message) = if file.active {
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
            match self.trash.trash(&m.path) {
                Ok(item) => {
                    index.set_file_gone(&m.path, true)?;
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
            let (id, at) = index.insert_resolution(&req.keep, &moves)?;
            (Some(id), at)
        } else {
            (None, index.now()?)
        };
        drop(index);
        if !removed.is_empty() {
            (self.emit)(Event::LibraryChanged {
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
                    index.set_file_gone(&m.path, false)?;
                    let shown = m.row.path.as_str();
                    if let Some(file) = index.copy_files(&[shown])?.get(shown) {
                        added.extend(&file.faces);
                    }
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
        drop(index);
        if !added.is_empty() {
            (self.emit)(Event::LibraryChanged {
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

#[cfg(test)]
mod tests;
