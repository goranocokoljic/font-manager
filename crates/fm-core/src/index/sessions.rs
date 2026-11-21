//! Cleanup sessions (E56.7, ADR 0027): the resolution log extended (schema
//! V28). A session is a `resolution` row; each file it touched is a
//! `resolution_move` row with the set it left and the copy kept for it. A
//! "Keep this one" action is a session of one set, so there is one history.
//! The logic is in `crate::resolve::cleanup`; this module reads and writes
//! the rows.

use std::path::Path;

use fm_api::cleanup::{CleanupSession, SessionFile, SessionKind};
use fm_api::library::{TrashMove, TrashMoveStatus};
use fm_api::{FaceId, ResolutionId};
use rusqlite::{OptionalExtension, params};

use super::files::{path_from_sql, path_to_sql};
use super::metadata_merge::{self, MergePlan};
use super::resolutions::{
    HISTORY, StoredMove, set_gone, status_from_sql, status_to_sql, trashed_to_sql,
};
use super::{Index, db_error};
use crate::error::{FmError, FmResult};

/// One file of a session: the stored move plus the set it left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionMove {
    /// The move, as "Keep this one" stores it.
    pub stored: StoredMove,
    /// The set it left; `None` for an action from before E56.7.
    pub set: Option<String>,
    /// The copy kept for it; `None` for an action from before E56.7 (the
    /// action's kept copy).
    pub kept: Option<String>,
    /// Its size in bytes, when known.
    pub size: Option<u32>,
}

/// The head row of a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionHead {
    /// Id.
    pub id: ResolutionId,
    /// When.
    pub at: String,
    /// The kept copy of a "Keep this one" action; empty for a cleanup.
    pub kept: String,
    /// The name; `None` for "Keep this one".
    pub name: Option<String>,
    /// Where it came from.
    pub kind: SessionKind,
    /// `done`, `running` or `cancelled`.
    pub state: String,
    /// Every moved file is back.
    pub undone: bool,
}

const HEAD: &str = "SELECT id, at, kept, name, kind, state, undone FROM resolution";

fn head(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionHead> {
    let kind: String = row.get(4)?;
    Ok(SessionHead {
        id: ResolutionId(row.get(0)?),
        at: row.get(1)?,
        kept: row.get(2)?,
        name: row.get(3)?,
        kind: if kind == "cleanup" {
            SessionKind::Cleanup
        } else {
            SessionKind::KeepCopy
        },
        state: row.get(5)?,
        undone: row.get(6)?,
    })
}

impl Index {
    /// The library files with a face the app activated, by display path,
    /// with the mode of the first such face (a plan's activation moves).
    pub(crate) fn active_files(
        &self,
    ) -> FmResult<std::collections::HashMap<String, fm_api::activation::ActivationMode>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT font_file.path, activation.mode FROM activation
                 JOIN face ON face.id = activation.face_id
                 JOIN font_file ON font_file.id = face.file_id
                 WHERE font_file.gone = 0 ORDER BY face.id",
            )
            .map_err(|e| db_error(&e))?;
        let mut rows = stmt.query([]).map_err(|e| db_error(&e))?;
        let mut out = std::collections::HashMap::new();
        while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
            let path = path_from_sql(row.get_ref(0).map_err(|e| db_error(&e))?)?;
            let mode: String = row.get(1).map_err(|e| db_error(&e))?;
            let mode = if mode == "permanent" {
                fm_api::activation::ActivationMode::Permanent
            } else {
                fm_api::activation::ActivationMode::Session
            };
            out.entry(path.to_string_lossy().into_owned())
                .or_insert(mode);
        }
        Ok(out)
    }

    /// Whether the app activated one of `faces` now (the apply checks this
    /// again under the lock just before a file goes to the trash).
    pub(crate) fn any_face_active(&self, faces: &[FaceId]) -> FmResult<bool> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT EXISTS (SELECT 1 FROM activation WHERE face_id = ?1)")
            .map_err(|e| db_error(&e))?;
        for face in faces {
            let active: bool = stmt
                .query_row([face.0], |r| r.get(0))
                .map_err(|e| db_error(&e))?;
            if active {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Starts a cleanup session named `name`: its head row, `running`.
    /// Returns its id and time.
    pub(crate) fn start_session(&mut self, name: &str) -> FmResult<(ResolutionId, String)> {
        let at = self.now()?;
        self.conn
            .execute(
                "INSERT INTO resolution (at, kept, name, kind, state)
                 VALUES (?1, '', ?2, 'cleanup', 'running')",
                params![at, name],
            )
            .map_err(|e| db_error(&e))?;
        let id = u32::try_from(self.conn.last_insert_rowid())
            .map_err(|_| FmError::internal("a session id does not fit the API"))?;
        Ok((ResolutionId(id), at))
    }

    /// Writes the files of the sets of session `id` in `sets` (each with
    /// its moves and its merge), marks every `Trashed` file gone, and writes
    /// each merge onto the kept copy with its log tagged with the set, in
    /// one transaction: either all of it is written or none. Returns whether
    /// a merge was written: the caller then writes the sidecars with
    /// [`Index::mirror_session_sidecars`], once for many sets (the index is
    /// the truth; the sidecars mirror it).
    pub(crate) fn write_session_sets(
        &mut self,
        id: ResolutionId,
        sets: &[(&str, Vec<SessionMove>, MergePlan)],
    ) -> FmResult<bool> {
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        let mut merged = false;
        for (set, moves, merge) in sets {
            for m in moves {
                let s = &m.stored;
                let (location, info) = trashed_to_sql(s.trashed.as_ref());
                let status = status_to_sql(s.row.status)?;
                tx.prepare_cached(
                    "INSERT INTO resolution_move
                         (resolution_id, position, path, display, status, message, location, info,
                          set_id, kept, size)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                )
                .and_then(|mut st| {
                    st.execute(params![
                        id.0,
                        s.position,
                        path_to_sql(&s.path),
                        s.row.path,
                        status,
                        s.row.message,
                        location,
                        info,
                        m.set,
                        m.kept,
                        m.size,
                    ])
                })
                .map_err(|e| db_error(&e))?;
                if s.row.status == TrashMoveStatus::Trashed {
                    set_gone(&tx, &s.path, true)?;
                }
            }
            if !merge.faces.is_empty() {
                metadata_merge::apply(&tx, i64::from(id.0), merge)?;
                metadata_merge::tag_set(&tx, i64::from(id.0), set)?;
                merged = true;
            }
        }
        tx.commit().map_err(|e| db_error(&e))?;
        Ok(merged)
    }

    /// Writes the sidecars of the metadata the merges of a session changed.
    pub(crate) fn mirror_session_sidecars(&mut self) {
        self.mirror_sidecars();
    }

    /// Ends session `id`: `done`, or `cancelled`.
    pub(crate) fn finish_session(&mut self, id: ResolutionId, cancelled: bool) -> FmResult<()> {
        self.conn
            .execute(
                "UPDATE resolution SET state = ?2 WHERE id = ?1",
                params![id.0, if cancelled { "cancelled" } else { "done" }],
            )
            .map_err(|e| db_error(&e))?;
        Ok(())
    }

    /// The head of session `id`, or `None`.
    pub(crate) fn session_head(&self, id: ResolutionId) -> FmResult<Option<SessionHead>> {
        self.conn
            .query_row(&format!("{HEAD} WHERE id = ?1"), [id.0], head)
            .optional()
            .map_err(|e| db_error(&e))
    }

    /// The last [`HISTORY`] sessions, newest first: cleanups and "Keep this
    /// one" actions.
    pub(crate) fn session_heads(&self) -> FmResult<Vec<SessionHead>> {
        let mut stmt = self
            .conn
            .prepare(&format!("{HEAD} ORDER BY id DESC LIMIT ?1"))
            .map_err(|e| db_error(&e))?;
        let rows = stmt
            .query_map([HISTORY], head)
            .map_err(|e| db_error(&e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| db_error(&e))?;
        Ok(rows)
    }

    /// The files of session `id`, in the order they were done.
    pub(crate) fn session_moves(&self, id: ResolutionId) -> FmResult<Vec<SessionMove>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT position, path, display, status, message, location, info, set_id, kept, size
                 FROM resolution_move WHERE resolution_id = ?1 ORDER BY position",
            )
            .map_err(|e| db_error(&e))?;
        let mut rows = stmt.query([id.0]).map_err(|e| db_error(&e))?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
            let get = |i: usize| row.get_ref(i).map_err(|e| db_error(&e));
            let opt_path = |i: usize| -> FmResult<Option<std::path::PathBuf>> {
                match get(i)? {
                    rusqlite::types::ValueRef::Null => Ok(None),
                    v => path_from_sql(v).map(Some),
                }
            };
            let location = opt_path(5)?;
            let info = opt_path(6)?;
            let status: String = row.get(3).map_err(|e| db_error(&e))?;
            out.push(SessionMove {
                stored: StoredMove {
                    position: row.get(0).map_err(|e| db_error(&e))?,
                    path: path_from_sql(get(1)?)?,
                    row: TrashMove {
                        path: row.get(2).map_err(|e| db_error(&e))?,
                        status: status_from_sql(&status)?,
                        message: row.get(4).map_err(|e| db_error(&e))?,
                    },
                    trashed: location.map(|l| (l, info)),
                },
                set: row.get(7).map_err(|e| db_error(&e))?,
                kept: row.get(8).map_err(|e| db_error(&e))?,
                size: row.get(9).map_err(|e| db_error(&e))?,
            });
        }
        Ok(out)
    }

    /// Writes the status and message of the files of session `id` after an
    /// undo and clears the gone flag of every `Restored` one; takes back the
    /// merge of every set in `sets_back` (all its moved copies are back),
    /// or the whole merge when `all_back` and the session's files name no
    /// set; and whether every moved file is back. One transaction.
    pub(crate) fn update_session(
        &mut self,
        id: ResolutionId,
        moves: &[SessionMove],
        sets_back: &[String],
        undone: bool,
    ) -> FmResult<Vec<FaceId>> {
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        for m in moves {
            let s = &m.stored;
            tx.execute(
                "UPDATE resolution_move SET status = ?3, message = ?4
                 WHERE resolution_id = ?1 AND position = ?2",
                params![
                    id.0,
                    s.position,
                    status_to_sql(s.row.status)?,
                    s.row.message
                ],
            )
            .map_err(|e| db_error(&e))?;
            if s.row.status == TrashMoveStatus::Restored {
                set_gone(&tx, &s.path, false)?;
            }
        }
        tx.execute(
            "UPDATE resolution SET undone = ?2 WHERE id = ?1",
            params![id.0, undone],
        )
        .map_err(|e| db_error(&e))?;
        let mut touched = Vec::new();
        for set in sets_back {
            touched.extend(metadata_merge::take_back_set(&tx, i64::from(id.0), set)?);
        }
        // An action from before E56.7 logged its merge without a set: it is
        // taken back once every copy is back, as `update_resolution` does.
        if undone {
            touched.extend(metadata_merge::take_back(&tx, i64::from(id.0))?);
        }
        tx.commit().map_err(|e| db_error(&e))?;
        if !touched.is_empty() {
            self.mirror_sidecars();
        }
        Ok(touched)
    }
}

/// The name a "Keep this one" action shows as a session: "Keep <file>".
fn keep_copy_name(kept: &str) -> String {
    let file = Path::new(kept)
        .file_name()
        .map_or_else(|| kept.to_owned(), |n| n.to_string_lossy().into_owned());
    format!("Keep {file}")
}

/// The API session of `head` and its files. `running` is whether the apply
/// is still going in this process: a session stored as running without it
/// was stopped (the app quit or crashed) and reads as cancelled.
pub(crate) fn session(head: &SessionHead, moves: &[SessionMove], running: bool) -> CleanupSession {
    let files: Vec<SessionFile> = moves
        .iter()
        .map(|m| SessionFile {
            path: m.stored.row.path.clone(),
            set: m.set.clone(),
            kept: m.kept.clone().unwrap_or_else(|| head.kept.clone()),
            size: m.size,
            status: m.stored.row.status,
            message: m.stored.row.message.clone(),
        })
        .collect();
    let trashed: Vec<&SessionFile> = files
        .iter()
        .filter(|f| f.status == TrashMoveStatus::Trashed)
        .collect();
    let sets: std::collections::BTreeSet<&str> = files
        .iter()
        .map(|f| f.set.as_deref().unwrap_or(f.kept.as_str()))
        .collect();
    let frees = trashed
        .iter()
        .fold(0u32, |sum, f| sum.saturating_add(f.size.unwrap_or(0)));
    CleanupSession {
        id: head.id,
        name: head
            .name
            .clone()
            .unwrap_or_else(|| keep_copy_name(&head.kept)),
        at: head.at.clone(),
        kind: head.kind,
        cancelled: head.state == "cancelled" || (head.state == "running" && !running),
        running: head.state == "running" && running,
        undone: head.undone,
        sets: u32::try_from(sets.len()).unwrap_or(u32::MAX),
        trashed: u32::try_from(trashed.len()).unwrap_or(u32::MAX),
        frees,
        files,
    }
}

#[cfg(test)]
mod tests;
