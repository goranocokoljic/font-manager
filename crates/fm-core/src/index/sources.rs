//! The `source` table and its backup.
//!
//! The folders the user chose are the one thing a fresh index cannot find out
//! by itself, so every change to them is mirrored to `sources.json` next to
//! the database, in the same transaction. A damaged index is rebuilt from it.

use std::fs;
use std::path::Path;

use fm_api::library::{Source, SourceKind};
use fm_api::{ErrorCode, SourceId};
use rusqlite::{Transaction, params};
use serde::{Deserialize, Serialize};

use super::{Index, db_error};
use crate::error::{FmError, FmResult};

/// The file name of the sources backup, next to the database.
pub(crate) const BACKUP_FILE: &str = "sources.json";

/// One source as the backup stores it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceRecord {
    id: u32,
    path: String,
    kind: SourceKind,
    paused: bool,
}

impl Index {
    /// Every source, by id, with its face count.
    pub fn sources(&self) -> FmResult<Vec<Source>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT s.id, s.path, s.kind, s.paused, COUNT(face.id)
                 FROM source s
                 LEFT JOIN font_file f ON f.source_id = s.id
                 LEFT JOIN face ON face.file_id = f.id
                 GROUP BY s.id ORDER BY s.id",
            )
            .map_err(|e| db_error(&e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, bool>(3)?,
                    row.get::<_, u32>(4)?,
                ))
            })
            .map_err(|e| db_error(&e))?;
        let mut out = Vec::new();
        for row in rows {
            let (id, path, kind, paused, face_count) = row.map_err(|e| db_error(&e))?;
            out.push(Source {
                id: SourceId(id),
                path,
                kind: kind_from_sql(&kind)?,
                paused,
                face_count,
            });
        }
        Ok(out)
    }

    /// Adds a folder as a source. `Conflict` when the folder is already one.
    pub fn add_source(&mut self, path: &str, kind: SourceKind) -> FmResult<Source> {
        if path.trim().is_empty() {
            return Err(FmError::invalid("Choose a folder to add."));
        }
        let backup = self.backup_path();
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        let inserted = tx.execute(
            "INSERT INTO source (path, kind) VALUES (?1, ?2)",
            params![path, kind_to_sql(kind)],
        );
        if let Err(e) = inserted {
            return Err(
                if e.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) {
                    FmError::new(
                        ErrorCode::Conflict,
                        "This folder is already in the library.",
                    )
                } else {
                    db_error(&e)
                },
            );
        }
        let id = SourceId(tx.last_insert_rowid() as u32);
        commit_with_backup(tx, backup.as_deref())?;
        self.source(id)
    }

    /// Forgets a source and every file and face indexed from it. Never touches
    /// the folder. `Conflict` while one of its faces is active.
    pub fn remove_source(&mut self, id: SourceId) -> FmResult<()> {
        let backup = self.backup_path();
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        let removed = tx.execute("DELETE FROM source WHERE id = ?1", [id.0]);
        match removed {
            Ok(0) => return Err(FmError::not_found(format!("Source {id}"))),
            Ok(_) => {}
            Err(e) if e.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) => {
                return Err(FmError::new(
                    ErrorCode::Conflict,
                    "Deactivate the fonts of this folder before removing it.",
                )
                .with_detail(e.to_string()));
            }
            Err(e) => return Err(db_error(&e)),
        }
        commit_with_backup(tx, backup.as_deref())
    }

    /// Pauses (hides) or resumes a source.
    pub fn set_source_paused(&mut self, id: SourceId, paused: bool) -> FmResult<Source> {
        let backup = self.backup_path();
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        let changed = tx
            .execute(
                "UPDATE source SET paused = ?2 WHERE id = ?1",
                params![id.0, paused],
            )
            .map_err(|e| db_error(&e))?;
        if changed == 0 {
            return Err(FmError::not_found(format!("Source {id}")));
        }
        commit_with_backup(tx, backup.as_deref())?;
        self.source(id)
    }

    fn source(&self, id: SourceId) -> FmResult<Source> {
        self.sources()?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| FmError::not_found(format!("Source {id}")))
    }

    /// Fills an empty `source` table from the backup, keeping the ids. A
    /// missing backup means no sources; an unreadable one is logged and
    /// skipped, because the index must open either way.
    pub(super) fn restore_sources(&mut self) -> FmResult<Vec<Source>> {
        let Some(path) = self.backup_path() else {
            return Ok(Vec::new());
        };
        let records: Vec<SourceRecord> = match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(records) => records,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "sources backup is unreadable");
                    Vec::new()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "sources backup could not be read");
                Vec::new()
            }
        };
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        for r in &records {
            tx.execute(
                "INSERT OR IGNORE INTO source (id, path, kind, paused) VALUES (?1, ?2, ?3, ?4)",
                params![r.id, r.path, kind_to_sql(r.kind), r.paused],
            )
            .map_err(|e| db_error(&e))?;
        }
        tx.commit().map_err(|e| db_error(&e))?;
        self.sources()
    }
}

/// Writes the backup from inside `tx`, then commits. A failed write rolls the
/// change back, so the backup never lags behind the database.
fn commit_with_backup(tx: Transaction<'_>, backup: Option<&Path>) -> FmResult<()> {
    if let Some(path) = backup {
        let records = read_records(&tx)?;
        write_backup(path, &records)?;
    }
    tx.commit().map_err(|e| db_error(&e))
}

fn read_records(tx: &Transaction<'_>) -> FmResult<Vec<SourceRecord>> {
    let mut stmt = tx
        .prepare("SELECT id, path, kind, paused FROM source ORDER BY id")
        .map_err(|e| db_error(&e))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, bool>(3)?,
            ))
        })
        .map_err(|e| db_error(&e))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, path, kind, paused) = row.map_err(|e| db_error(&e))?;
        out.push(SourceRecord {
            id,
            path,
            kind: kind_from_sql(&kind)?,
            paused,
        });
    }
    Ok(out)
}

/// Writes to a temporary file first, so a crash never leaves half a backup.
fn write_backup(path: &Path, records: &[SourceRecord]) -> FmResult<()> {
    let failed = "The list of library folders could not be saved.";
    let json = serde_json::to_string_pretty(records)
        .map_err(|e| FmError::internal(format!("sources backup: {e}")))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json + "\n").map_err(|e| FmError::io(failed, &tmp, &e))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        FmError::io(failed, path, &e)
    })
}

fn kind_to_sql(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Watched => "watched",
        SourceKind::System => "system",
        SourceKind::LibraryFolder => "libraryFolder",
    }
}

fn kind_from_sql(kind: &str) -> FmResult<SourceKind> {
    match kind {
        "watched" => Ok(SourceKind::Watched),
        "system" => Ok(SourceKind::System),
        "libraryFolder" => Ok(SourceKind::LibraryFolder),
        other => Err(FmError::internal(format!("unknown source kind {other:?}"))),
    }
}
