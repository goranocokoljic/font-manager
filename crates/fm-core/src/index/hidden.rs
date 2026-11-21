//! Two outcomes that touch no file (E56.8, ADR 0027; schema V29).
//!
//! - **Hidden copies.** A `hidden_copy` row names a file by its path and
//!   hash; the triggers of V29 keep that file `gone`, so it leaves every
//!   library, search and activation list, across scans, rescans, rebuilds
//!   and restarts, while its file stays where it is (rule 2). Another file
//!   at the path (another hash) drops the row and is shown.
//! - **Decisions.** A `duplicate_decision` row remembers that a set was
//!   answered "not duplicates" or "keep both" for exactly its files' hashes
//!   ([`fm_api::cleanup::decision_key`]); `crate::duplicate_sets` leaves
//!   such a set out while it has exactly those files.
//!
//! Every change marks the faces of the files it names for their family's
//! sidecar, so the sidecars carry both to another machine and to a rebuilt
//! index (`index/sidecars.rs`).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use fm_api::cleanup::{DuplicateAnswer, DuplicateDecision, HiddenCopy, decision_key};
use fm_api::{DecisionId, ResolutionId};
use rusqlite::{Connection, OptionalExtension, params};

use super::files::{path_from_sql, path_to_sql};
use super::{Index, db_error};
use crate::error::{FmError, FmResult};

/// Marks the faces of the file at `path` for their family's sidecar.
const DIRTY_PATH: &str = "
INSERT INTO sidecar_dirty (face_id)
SELECT face.id FROM face JOIN font_file ON font_file.id = face.file_id
WHERE font_file.path = ?1 AND face.id NOT IN (SELECT face_id FROM sidecar_dirty)";

/// Marks the faces of the files with the hash `?1` for their sidecar.
const DIRTY_HASH: &str = "
INSERT INTO sidecar_dirty (face_id)
SELECT face.id FROM face JOIN font_file ON font_file.id = face.file_id
WHERE font_file.hash = ?1 AND face.id NOT IN (SELECT face_id FROM sidecar_dirty)";

/// The answer as stored.
pub(crate) fn answer_to_sql(a: DuplicateAnswer) -> &'static str {
    match a {
        DuplicateAnswer::NotDuplicates => "notDuplicates",
        DuplicateAnswer::KeepBoth => "keepBoth",
    }
}

/// The answer from its stored name; `None` for a name this build does not
/// know (a newer version's sidecar).
pub(crate) fn answer_from_sql(s: &str) -> Option<DuplicateAnswer> {
    match s {
        "notDuplicates" => Some(DuplicateAnswer::NotDuplicates),
        "keepBoth" => Some(DuplicateAnswer::KeepBoth),
        _ => None,
    }
}

/// The 32 bytes of a hex hash; `None` when it is not one.
pub(crate) fn hash_bytes(hex: &str) -> Option<[u8; 32]> {
    blake3::Hash::from_hex(hex).ok().map(|h| *h.as_bytes())
}

/// Hides the file at `path` with the hash `hash` (hex) in `conn`; `None`
/// for its session, set and kept copy when they are not known (a sidecar).
/// The V29 trigger marks the file gone when it is indexed with that hash.
/// Its faces are marked for their sidecar. Returns `false` when the hash is
/// not one.
pub(crate) fn hide(
    conn: &Connection,
    path: &Path,
    hash: &str,
    at: &str,
    origin: Option<(ResolutionId, Option<&str>, Option<&str>)>,
) -> FmResult<bool> {
    let Some(bytes) = hash_bytes(hash) else {
        return Ok(false);
    };
    let (session, set, kept) =
        origin.map_or((None, None, None), |(s, set, kept)| (Some(s.0), set, kept));
    conn.execute(
        "INSERT OR REPLACE INTO hidden_copy (path, hash, at, resolution_id, set_id, kept)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![path_to_sql(path), bytes.as_slice(), at, session, set, kept],
    )
    .map_err(|e| db_error(&e))?;
    conn.execute(DIRTY_PATH, [path_to_sql(path)])
        .map_err(|e| db_error(&e))?;
    Ok(true)
}

/// Shows the file at `path` again in `conn`: drops its row, clears its gone
/// flag when the file is on disk (a file that is not stays gone, as a scan
/// would leave it), and marks its faces for their sidecar.
pub(crate) fn show(conn: &Connection, path: &Path) -> FmResult<()> {
    conn.execute(
        "DELETE FROM hidden_copy WHERE path = ?1",
        [path_to_sql(path)],
    )
    .map_err(|e| db_error(&e))?;
    if std::fs::symlink_metadata(path).is_ok() {
        conn.execute(
            "UPDATE font_file SET gone = 0 WHERE path = ?1",
            [path_to_sql(path)],
        )
        .map_err(|e| db_error(&e))?;
    }
    conn.execute(DIRTY_PATH, [path_to_sql(path)])
        .map_err(|e| db_error(&e))?;
    Ok(())
}

/// Remembers `answer` for the set `set` with the files `hashes` in `conn`,
/// or changes the answer of the one remembered; its files' faces are marked
/// for their sidecar. Returns its id.
pub(crate) fn decide(
    conn: &Connection,
    set: &str,
    hashes: &[String],
    answer: DuplicateAnswer,
    at: &str,
) -> FmResult<DecisionId> {
    let joined = joined(hashes);
    conn.execute(
        "INSERT INTO duplicate_decision (set_id, hashes, answer, at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (set_id, hashes) DO UPDATE SET answer = excluded.answer",
        params![set, joined, answer_to_sql(answer), at],
    )
    .map_err(|e| db_error(&e))?;
    let id: u32 = conn
        .query_row(
            "SELECT id FROM duplicate_decision WHERE set_id = ?1 AND hashes = ?2",
            params![set, joined],
            |r| r.get(0),
        )
        .map_err(|e| db_error(&e))?;
    mark_hashes(conn, hashes)?;
    Ok(DecisionId(id))
}

/// Forgets the decision for `set` and `hashes` in `conn`, and marks its
/// files' faces for their sidecar.
pub(crate) fn forget(conn: &Connection, set: &str, hashes: &[String]) -> FmResult<()> {
    conn.execute(
        "DELETE FROM duplicate_decision WHERE set_id = ?1 AND hashes = ?2",
        params![set, joined(hashes)],
    )
    .map_err(|e| db_error(&e))?;
    mark_hashes(conn, hashes)
}

fn mark_hashes(conn: &Connection, hashes: &[String]) -> FmResult<()> {
    for h in hashes {
        if let Some(bytes) = hash_bytes(h) {
            conn.execute(DIRTY_HASH, [bytes.as_slice()])
                .map_err(|e| db_error(&e))?;
        }
    }
    Ok(())
}

/// The hashes sorted (one per copy, repeats kept), joined by commas: as
/// stored.
fn joined(hashes: &[String]) -> String {
    let mut h: Vec<&str> = hashes.iter().map(String::as_str).collect();
    h.sort_unstable();
    h.join(",")
}

/// One stored decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredDecision {
    /// Id.
    pub id: DecisionId,
    /// The set.
    pub set: String,
    /// The hashes, sorted.
    pub hashes: Vec<String>,
    /// The answer.
    pub answer: DuplicateAnswer,
    /// When.
    pub at: String,
}

/// Every stored decision, newest first.
pub(crate) fn decisions(conn: &Connection) -> FmResult<Vec<StoredDecision>> {
    let mut stmt = conn
        .prepare("SELECT id, set_id, hashes, answer, at FROM duplicate_decision ORDER BY id DESC")
        .map_err(|e| db_error(&e))?;
    let mut rows = stmt.query([]).map_err(|e| db_error(&e))?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
        let answer: String = row.get(3).map_err(|e| db_error(&e))?;
        let Some(answer) = answer_from_sql(&answer) else {
            continue;
        };
        let hashes: String = row.get(2).map_err(|e| db_error(&e))?;
        out.push(StoredDecision {
            id: DecisionId(row.get(0).map_err(|e| db_error(&e))?),
            set: row.get(1).map_err(|e| db_error(&e))?,
            hashes: hashes
                .split(',')
                .filter(|h| !h.is_empty())
                .map(str::to_owned)
                .collect(),
            answer,
            at: row.get(4).map_err(|e| db_error(&e))?,
        });
    }
    Ok(out)
}

/// One stored hidden copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredHidden {
    /// The path, exactly as indexed.
    pub path: PathBuf,
    /// Its hash, hex.
    pub hash: String,
    /// The session that hid it.
    pub session: Option<ResolutionId>,
}

/// Every hidden copy, by path.
pub(crate) fn hidden_rows(conn: &Connection) -> FmResult<Vec<StoredHidden>> {
    let mut stmt = conn
        .prepare("SELECT path, lower(hex(hash)), resolution_id FROM hidden_copy ORDER BY path")
        .map_err(|e| db_error(&e))?;
    let mut rows = stmt.query([]).map_err(|e| db_error(&e))?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
        out.push(StoredHidden {
            path: path_from_sql(row.get_ref(0).map_err(|e| db_error(&e))?)?,
            hash: row.get(1).map_err(|e| db_error(&e))?,
            session: row
                .get::<_, Option<u32>>(2)
                .map_err(|e| db_error(&e))?
                .map(ResolutionId),
        });
    }
    Ok(out)
}

impl Index {
    /// [`fm_api::LibraryService::list_hidden_copies`]: every hidden copy, by
    /// path.
    pub fn hidden_copies(&self) -> FmResult<Vec<HiddenCopy>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT hidden_copy.path, lower(hex(hidden_copy.hash)), hidden_copy.at,
                        hidden_copy.resolution_id, hidden_copy.set_id, hidden_copy.kept,
                        (SELECT face.family FROM face WHERE face.file_id = font_file.id
                         ORDER BY face.collection_index LIMIT 1),
                        font_file.hash = hidden_copy.hash
                 FROM hidden_copy LEFT JOIN font_file ON font_file.path = hidden_copy.path
                 ORDER BY hidden_copy.path",
            )
            .map_err(|e| db_error(&e))?;
        let mut rows = stmt.query([]).map_err(|e| db_error(&e))?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
            let path = path_from_sql(row.get_ref(0).map_err(|e| db_error(&e))?)?;
            out.push(HiddenCopy {
                path: path.to_string_lossy().into_owned(),
                hash: row.get(1).map_err(|e| db_error(&e))?,
                at: row.get(2).map_err(|e| db_error(&e))?,
                session: row
                    .get::<_, Option<u32>>(3)
                    .map_err(|e| db_error(&e))?
                    .map(ResolutionId),
                set: row.get(4).map_err(|e| db_error(&e))?,
                kept: row.get(5).map_err(|e| db_error(&e))?,
                family: row.get(6).map_err(|e| db_error(&e))?,
                in_effect: row
                    .get::<_, Option<bool>>(7)
                    .map_err(|e| db_error(&e))?
                    .unwrap_or(false),
            });
        }
        Ok(out)
    }

    /// The hidden copy at the display path `path`, as stored.
    pub(crate) fn hidden_at(&self, path: &str) -> FmResult<Option<StoredHidden>> {
        Ok(hidden_rows(&self.conn)?
            .into_iter()
            .find(|h| h.path.to_string_lossy() == path))
    }

    /// Hides the file at `path`, whose hash is `hash` (hex), for the session
    /// `session`, out of the set `set` with the kept copy `kept`. The caller
    /// writes the sidecars.
    pub(crate) fn hide_copy(
        &self,
        path: &Path,
        hash: &str,
        session: ResolutionId,
        set: &str,
        kept: Option<&str>,
    ) -> FmResult<bool> {
        let at = self.now()?;
        hide(
            &self.conn,
            path,
            hash,
            &at,
            Some((session, Some(set), kept)),
        )
    }

    /// Shows the file at `path` again without writing the sidecars (a
    /// cleanup whose log could not be written puts it back).
    pub(crate) fn unhide_copy(&self, path: &Path) -> FmResult<()> {
        show(&self.conn, path)
    }

    /// Shows the hidden copy at `path` again outside a session (one read
    /// from a sidecar), and writes the sidecars.
    pub(crate) fn show_hidden(&mut self, path: &Path) -> FmResult<()> {
        show(&self.conn, path)?;
        self.mirror_sidecars();
        Ok(())
    }

    /// The keys ([`decision_key`]) of every remembered decision.
    pub fn decided_keys(&self) -> FmResult<HashSet<String>> {
        Ok(self.decided_answers()?.into_keys().collect())
    }

    /// Every remembered answer by its key ([`decision_key`]).
    pub fn decided_answers(&self) -> FmResult<HashMap<String, DuplicateAnswer>> {
        Ok(decisions(&self.conn)?
            .iter()
            .map(|d| (decision_key(&d.set, &d.hashes), d.answer))
            .collect())
    }

    /// Remembers `answer` for `set` with the files `hashes` and writes the
    /// sidecars.
    pub fn decide_set(
        &mut self,
        set: &str,
        hashes: &[String],
        answer: DuplicateAnswer,
    ) -> FmResult<DecisionId> {
        let at = self.now()?;
        let id = decide(&self.conn, set, hashes, answer, &at)?;
        self.mirror_sidecars();
        Ok(id)
    }

    /// The remembered decisions, newest first, each with the library files
    /// that have one of its hashes now; `outdated` is left false for the
    /// caller, which knows the sets.
    pub fn duplicate_decisions(&self) -> FmResult<Vec<DuplicateDecision>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT path FROM font_file WHERE hash = ?1 AND gone = 0")
            .map_err(|e| db_error(&e))?;
        let mut out = Vec::new();
        for d in decisions(&self.conn)? {
            let mut files = Vec::new();
            let unique: BTreeSet<&String> = d.hashes.iter().collect();
            for h in unique {
                let Some(bytes) = hash_bytes(h) else {
                    continue;
                };
                let mut rows = stmt.query([bytes.as_slice()]).map_err(|e| db_error(&e))?;
                while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
                    let path = path_from_sql(row.get_ref(0).map_err(|e| db_error(&e))?)?;
                    files.push(path.to_string_lossy().into_owned());
                }
            }
            files.sort();
            out.push(DuplicateDecision {
                id: d.id,
                set: d.set,
                answer: d.answer,
                hashes: d.hashes,
                files,
                at: d.at,
                outdated: false,
            });
        }
        Ok(out)
    }

    /// Takes the decision `id` back and writes the sidecars. `NotFound` when
    /// there is none.
    pub fn take_back_decision(&mut self, id: DecisionId) -> FmResult<()> {
        let found: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT set_id, hashes FROM duplicate_decision WHERE id = ?1",
                [id.0],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| db_error(&e))?;
        let (set, hashes) = found.ok_or_else(|| FmError::not_found(format!("Decision {id}")))?;
        let hashes: Vec<String> = hashes.split(',').map(str::to_owned).collect();
        forget(&self.conn, &set, &hashes)?;
        self.mirror_sidecars();
        Ok(())
    }
}

#[cfg(test)]
mod tests;
