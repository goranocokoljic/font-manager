//! The Quick Collection (E13.3): the `quick_face` table.
//!
//! One ordered list of faces the user gathers while browsing (one key adds
//! or drops the selected fonts), reviews, and then keeps as a manual
//! collection or a tag through the collection and tag API. It is stored in
//! the index, so it outlives a restart; a face that leaves the index leaves
//! the list (`ON DELETE CASCADE`). A face the library does not show (its
//! file is gone, its source paused) stays in the table but is not listed
//! while it is hidden.

use fm_api::FaceId;
use fm_api::organization::QuickCollection;
use rusqlite::params;

use super::{Index, db_error};
use crate::error::{FmError, FmResult};

impl Index {
    /// The Quick Collection, first added first, visible faces only.
    pub fn quick_collection(&self) -> FmResult<QuickCollection> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT face_id FROM quick_face ORDER BY position, face_id")
            .map_err(|e| db_error(&e))?;
        let ids = stmt
            .query_map([], |row| row.get::<_, u32>(0))
            .map_err(|e| db_error(&e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| db_error(&e))?;
        drop(stmt);
        let mut faces = Vec::with_capacity(ids.len());
        for id in ids.into_iter().map(FaceId) {
            if self.family_of(id)?.is_some() {
                faces.push(self.face_summary(id)?);
            }
        }
        Ok(QuickCollection { faces })
    }

    /// Adds faces to the end of the Quick Collection, in the order given;
    /// a face already in it keeps its place, a face named twice is added
    /// once. An unknown face is `NotFound` and nothing is added.
    pub fn add_to_quick_collection(&mut self, faces: &[FaceId]) -> FmResult<QuickCollection> {
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        {
            let mut known = tx
                .prepare_cached("SELECT 1 FROM face WHERE id = ?1")
                .map_err(|e| db_error(&e))?;
            for FaceId(face) in faces {
                if !known.exists([face]).map_err(|e| db_error(&e))? {
                    return Err(FmError::not_found(format!("Face {face}")));
                }
            }
            let mut add = tx
                .prepare_cached(
                    "INSERT OR IGNORE INTO quick_face (face_id, position)
                     SELECT ?1, coalesce(max(position), 0) + 1 FROM quick_face",
                )
                .map_err(|e| db_error(&e))?;
            for face in faces {
                add.execute(params![face.0]).map_err(|e| db_error(&e))?;
            }
        }
        tx.commit().map_err(|e| db_error(&e))?;
        self.quick_collection()
    }

    /// Removes faces from the Quick Collection; a face not in it is
    /// ignored. Returns what is left.
    pub fn remove_from_quick_collection(&mut self, faces: &[FaceId]) -> FmResult<QuickCollection> {
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        {
            let mut remove = tx
                .prepare_cached("DELETE FROM quick_face WHERE face_id = ?1")
                .map_err(|e| db_error(&e))?;
            for face in faces {
                remove.execute([face.0]).map_err(|e| db_error(&e))?;
            }
        }
        tx.commit().map_err(|e| db_error(&e))?;
        self.quick_collection()
    }

    /// Empties the Quick Collection; the faces stay in the library.
    pub fn clear_quick_collection(&mut self) -> FmResult<()> {
        self.conn
            .execute("DELETE FROM quick_face", [])
            .map_err(|e| db_error(&e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
