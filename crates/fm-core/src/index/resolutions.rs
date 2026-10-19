//! The resolution log (E8.3): the "Keep this one" actions of the Problems
//! view, with where the OS trash put each copy, so undo can move it back.
//! The logic is in `crate::resolve`; this module reads and writes the rows.

use std::collections::HashMap;
use std::path::PathBuf;

use fm_api::library::{Resolution, TrashMove, TrashMoveStatus};
use fm_api::{FaceId, ResolutionId};
use rusqlite::types::Value;
use rusqlite::{OptionalExtension, params};

use super::files::{path_from_sql, path_to_sql};
use super::{Index, db_error};
use crate::error::{FmError, FmResult};

/// How many actions the Problems history lists.
const HISTORY: u32 = 100;

/// A file of the library as "Keep this one" needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CopyFile {
    /// The file, exactly as indexed.
    pub path: PathBuf,
    /// Its faces.
    pub faces: Vec<FaceId>,
    /// The app activated one of its faces.
    pub active: bool,
}

/// One copy of a stored action: the API row plus what undo needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredMove {
    /// Its place in the request.
    pub position: u32,
    /// The file, exactly as indexed.
    pub path: PathBuf,
    /// The API row.
    pub row: TrashMove,
    /// Where the trash put it, and the OS record of the move.
    pub trashed: Option<(PathBuf, Option<PathBuf>)>,
}

const FILE_FACES: &str = "SELECT id FROM face WHERE file_id = ?1 ORDER BY id";

const FILE_ACTIVE: &str = "
SELECT EXISTS (SELECT 1 FROM activation JOIN face ON face.id = activation.face_id
               WHERE face.file_id = ?1)";

impl Index {
    /// The library files (not gone) shown as `paths` (the lossy display form
    /// [`Index::duplicates`] uses), by display path. A path no file has is
    /// missing from the map.
    pub(crate) fn copy_files(&self, paths: &[&str]) -> FmResult<HashMap<String, CopyFile>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path FROM font_file WHERE gone = 0")
            .map_err(|e| db_error(&e))?;
        let mut rows = stmt.query([]).map_err(|e| db_error(&e))?;
        let mut found = Vec::new();
        while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
            let path = path_from_sql(row.get_ref(1).map_err(|e| db_error(&e))?)?;
            let shown = path.to_string_lossy().into_owned();
            if paths.contains(&shown.as_str()) {
                found.push((row.get::<_, i64>(0).map_err(|e| db_error(&e))?, path, shown));
            }
        }
        let mut out = HashMap::new();
        for (id, path, shown) in found {
            let faces = self
                .conn
                .prepare_cached(FILE_FACES)
                .and_then(|mut s| {
                    s.query_map([id], |r| r.get::<_, u32>(0).map(FaceId))?
                        .collect::<Result<Vec<_>, _>>()
                })
                .map_err(|e| db_error(&e))?;
            let active: bool = self
                .conn
                .query_row(FILE_ACTIVE, [id], |r| r.get(0))
                .map_err(|e| db_error(&e))?;
            out.insert(
                shown,
                CopyFile {
                    path,
                    faces,
                    active,
                },
            );
        }
        Ok(out)
    }

    /// Now, as an RFC 3339 UTC timestamp.
    pub(crate) fn now(&self) -> FmResult<String> {
        self.conn
            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |r| {
                r.get(0)
            })
            .map_err(|e| db_error(&e))
    }

    /// Records an action and its copies, and marks every `Trashed` copy's
    /// file gone (its faces, tags and ids kept, ADR 0004), in one
    /// transaction: either all of it is written or none. Returns its id and
    /// time.
    pub(crate) fn insert_resolution(
        &mut self,
        kept: &str,
        moves: &[StoredMove],
    ) -> FmResult<(ResolutionId, String)> {
        let at = self.now()?;
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        tx.execute(
            "INSERT INTO resolution (at, kept) VALUES (?1, ?2)",
            params![at, kept],
        )
        .map_err(|e| db_error(&e))?;
        let id = tx.last_insert_rowid();
        for m in moves {
            let (location, info) = trashed_to_sql(m.trashed.as_ref());
            tx.execute(
                "INSERT INTO resolution_move
                     (resolution_id, position, path, display, status, message, location, info)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    id,
                    m.position,
                    path_to_sql(&m.path),
                    m.row.path,
                    status_to_sql(m.row.status)?,
                    m.row.message,
                    location,
                    info,
                ],
            )
            .map_err(|e| db_error(&e))?;
            if m.row.status == TrashMoveStatus::Trashed {
                set_gone(&tx, &m.path, true)?;
            }
        }
        tx.commit().map_err(|e| db_error(&e))?;
        let id = u32::try_from(id)
            .map_err(|_| FmError::internal("a resolution id does not fit the API"))?;
        Ok((ResolutionId(id), at))
    }

    /// Writes the status and message of the copies of `id` after an undo,
    /// whether every moved copy is back, and clears the gone flag of every
    /// `Restored` copy, in one transaction.
    pub(crate) fn update_resolution(
        &mut self,
        id: ResolutionId,
        moves: &[StoredMove],
        undone: bool,
    ) -> FmResult<()> {
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        for m in moves {
            tx.execute(
                "UPDATE resolution_move SET status = ?3, message = ?4
                 WHERE resolution_id = ?1 AND position = ?2",
                params![
                    id.0,
                    m.position,
                    status_to_sql(m.row.status)?,
                    m.row.message
                ],
            )
            .map_err(|e| db_error(&e))?;
            if m.row.status == TrashMoveStatus::Restored {
                set_gone(&tx, &m.path, false)?;
            }
        }
        tx.execute(
            "UPDATE resolution SET undone = ?2 WHERE id = ?1",
            params![id.0, undone],
        )
        .map_err(|e| db_error(&e))?;
        tx.commit().map_err(|e| db_error(&e))
    }

    /// The action `id` and its copies, or `None` when there is none.
    pub(crate) fn resolution(
        &self,
        id: ResolutionId,
    ) -> FmResult<Option<(Resolution, Vec<StoredMove>)>> {
        let head: Option<(String, String, bool)> = self
            .conn
            .query_row(
                "SELECT at, kept, undone FROM resolution WHERE id = ?1",
                [id.0],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(|e| db_error(&e))?;
        let Some((at, kept, undone)) = head else {
            return Ok(None);
        };
        let moves = self.stored_moves(id)?;
        let resolution = Resolution {
            id: Some(id),
            kept,
            at,
            dry_run: false,
            undone,
            moves: moves.iter().map(|m| m.row.clone()).collect(),
        };
        Ok(Some((resolution, moves)))
    }

    /// The last [`HISTORY`] actions, newest first.
    pub(crate) fn resolutions(&self) -> FmResult<Vec<Resolution>> {
        let ids: Vec<u32> = self
            .conn
            .prepare("SELECT id FROM resolution ORDER BY id DESC LIMIT ?1")
            .and_then(|mut s| {
                s.query_map([HISTORY], |r| r.get(0))?
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|e| db_error(&e))?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some((r, _)) = self.resolution(ResolutionId(id))? {
                out.push(r);
            }
        }
        Ok(out)
    }

    fn stored_moves(&self, id: ResolutionId) -> FmResult<Vec<StoredMove>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT position, path, display, status, message, location, info
                 FROM resolution_move WHERE resolution_id = ?1 ORDER BY position",
            )
            .map_err(|e| db_error(&e))?;
        let mut rows = stmt.query([id.0]).map_err(|e| db_error(&e))?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
            let get = |i: usize| row.get_ref(i).map_err(|e| db_error(&e));
            let location = match get(5)? {
                rusqlite::types::ValueRef::Null => None,
                v => Some(path_from_sql(v)?),
            };
            let info = match get(6)? {
                rusqlite::types::ValueRef::Null => None,
                v => Some(path_from_sql(v)?),
            };
            let status: String = row.get(3).map_err(|e| db_error(&e))?;
            out.push(StoredMove {
                position: row.get(0).map_err(|e| db_error(&e))?,
                path: path_from_sql(get(1)?)?,
                row: TrashMove {
                    path: row.get(2).map_err(|e| db_error(&e))?,
                    status: status_from_sql(&status)?,
                    message: row.get(4).map_err(|e| db_error(&e))?,
                },
                trashed: location.map(|l| (l, info)),
            });
        }
        Ok(out)
    }
}

/// Sets or clears the gone flag of the file at `path`.
fn set_gone(conn: &rusqlite::Connection, path: &std::path::Path, gone: bool) -> FmResult<()> {
    conn.execute(
        "UPDATE font_file SET gone = ?2 WHERE path = ?1",
        params![path_to_sql(path), gone],
    )
    .map_err(|e| db_error(&e))?;
    Ok(())
}

fn trashed_to_sql(trashed: Option<&(PathBuf, Option<PathBuf>)>) -> (Value, Value) {
    match trashed {
        Some((location, info)) => (
            path_to_sql(location),
            info.as_deref().map_or(Value::Null, path_to_sql),
        ),
        None => (Value::Null, Value::Null),
    }
}

fn status_to_sql(status: TrashMoveStatus) -> FmResult<String> {
    match serde_json::to_value(status) {
        Ok(serde_json::Value::String(s)) => Ok(s),
        _ => Err(FmError::internal("a trash move status has no name")),
    }
}

fn status_from_sql(text: &str) -> FmResult<TrashMoveStatus> {
    serde_json::from_value(serde_json::Value::String(text.to_owned()))
        .map_err(|_| FmError::internal(format!("unknown trash move status {text:?} in the index")))
}
