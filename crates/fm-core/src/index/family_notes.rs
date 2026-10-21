//! Family notes (E13.5): the `family_meta` table.
//!
//! A family's notes belong to the family, not to one of its faces: they are
//! stored by the family's grouping key ([`family_key`]), the key the library
//! groups faces by and the sidecar file is named by. A rescan that indexes
//! the faces again, or a rebuilt index, finds them by the family's name.
//!
//! Every write is mirrored to the sidecars of the family (`index/sidecars.rs`):
//! the family's faces are marked dirty, so each source that holds the family
//! writes its sidecar again.

use std::collections::HashMap;

use fm_api::FaceId;
use fm_api::organization::FamilyNotesUpdate;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::{Index, db_error};
use crate::error::{FmError, FmResult};
use crate::grouping::family_key;
use crate::sidecar::Field;

impl Index {
    /// Sets the notes of a family; an empty string clears them. Returns the
    /// family's visible faces, the ones the list shows again. A family the
    /// library does not show is `NotFound`. Setting the notes dismisses the
    /// family's sidecar conflicts on them.
    pub fn update_family_notes(&mut self, req: &FamilyNotesUpdate) -> FmResult<Vec<FaceId>> {
        let not_found = || FmError::not_found(format!("Family {}", req.family));
        let summary = self.family_summary(req.family)?.ok_or_else(not_found)?;
        let faces = self.family_faces(req.family)?.ok_or_else(not_found)?;
        let key = family_key(&summary.name);
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        set_family_notes(&tx, &key, Some(req.notes.as_str()))?;
        for face in faces_of_family(&tx, &key)? {
            tx.prepare_cached("INSERT OR IGNORE INTO sidecar_dirty (face_id) VALUES (?1)")
                .and_then(|mut s| s.execute([face]))
                .map_err(|e| db_error(&e))?;
            tx.prepare_cached("DELETE FROM sidecar_conflict WHERE face_id = ?1 AND field = ?2")
                .and_then(|mut s| s.execute(params![face, Field::FamilyNotes.as_str()]))
                .map_err(|e| db_error(&e))?;
        }
        tx.commit().map_err(|e| db_error(&e))?;
        self.mirror_sidecars();
        Ok(faces)
    }

    /// The notes of the family named `family`, if any.
    pub(super) fn family_notes(&self, family: &str) -> FmResult<Option<String>> {
        family_notes_by_key(&self.conn, &family_key(family))
    }

    /// Every family's notes, by grouping key.
    pub(super) fn all_family_notes(&self) -> FmResult<HashMap<String, String>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT family_key, notes FROM family_meta WHERE notes IS NOT NULL")
            .map_err(|e| db_error(&e))?;
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| db_error(&e))?
            .collect::<Result<_, _>>()
            .map_err(|e| db_error(&e))
    }
}

/// The notes stored for the family with grouping key `key`.
pub(super) fn family_notes_by_key(conn: &Connection, key: &str) -> FmResult<Option<String>> {
    conn.prepare_cached("SELECT notes FROM family_meta WHERE family_key = ?1")
        .and_then(|mut s| s.query_row([key], |row| row.get(0)).optional())
        .map(Option::flatten)
        .map_err(|e| db_error(&e))
}

/// Stores the notes of the family with grouping key `key`; none or empty
/// notes clear them, and a row left with nothing in it is deleted.
pub(super) fn set_family_notes(
    tx: &Transaction<'_>,
    key: &str,
    notes: Option<&str>,
) -> FmResult<()> {
    tx.prepare_cached(
        "INSERT INTO family_meta (family_key, notes) VALUES (?1, ?2)
         ON CONFLICT (family_key) DO UPDATE SET notes = excluded.notes",
    )
    .and_then(|mut s| s.execute(params![key, notes.filter(|n| !n.is_empty())]))
    .map_err(|e| db_error(&e))?;
    delete_empty_row(tx, key)
}

/// Deletes the `family_meta` row of `key` when it holds neither notes nor a
/// declared license.
pub(super) fn delete_empty_row(tx: &Transaction<'_>, key: &str) -> FmResult<()> {
    tx.prepare_cached(
        "DELETE FROM family_meta WHERE family_key = ?1 AND notes IS NULL
         AND license_type IS NULL AND seats IS NULL AND purchase_date IS NULL
         AND receipt IS NULL",
    )
    .and_then(|mut s| s.execute([key]))
    .map_err(|e| db_error(&e))?;
    Ok(())
}

/// Every indexed face of the family with grouping key `key`, in every
/// source, paused ones included: each source's sidecar carries the notes.
pub(super) fn faces_of_family(tx: &Transaction<'_>, key: &str) -> FmResult<Vec<u32>> {
    let mut stmt = tx
        .prepare_cached("SELECT id, family FROM face")
        .map_err(|e| db_error(&e))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| db_error(&e))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, family) = row.map_err(|e| db_error(&e))?;
        if family_key(&family) == key {
            out.push(id);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
