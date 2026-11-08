//! What a font drop on a collection or a tag would do (E44.2).
//!
//! While fonts are dragged, the sidebar shows each target's face count now
//! and after the drop ("31 → 34"). The numbers count what the sidebar
//! counts: faces the library shows (files not gone, sources not paused),
//! each once. A face the target already holds adds nothing.

use std::collections::BTreeSet;

use fm_api::organization::{CollectionDropCount, DropCounts, TagDropCount};
use fm_api::{FaceId, FamilyId};
use rusqlite::OptionalExtension;

use super::{Index, db_error};
use crate::error::{FmError, FmResult};

/// Whether a face exists and whether the library shows it.
const SHOWN: &str = "
SELECT font_file.gone = 0 AND source.paused = 0
FROM face
JOIN font_file ON font_file.id = face.file_id
JOIN source ON source.id = font_file.source_id
WHERE face.id = ?1";

impl Index {
    /// The face counts of every manual collection and every tag now and
    /// after a drop of `faces` and the faces of `families`. A smart
    /// collection is not a drop target and is left out. An unknown face or
    /// family is `NotFound`.
    pub fn drop_counts(&self, faces: &[FaceId], families: &[FamilyId]) -> FmResult<DropCounts> {
        let faces = self.faces_of(faces, families)?;
        let shown = self.shown_faces(&faces)?;
        let collections = self
            .collections()?
            .into_iter()
            .filter(|c| c.rule_group.is_none())
            .map(|c| {
                let held = self.held(
                    "SELECT 1 FROM collection_face WHERE collection_id = ?1 AND face_id = ?2",
                    c.id.0,
                    &shown,
                )?;
                Ok(CollectionDropCount {
                    collection: c.id,
                    before: c.face_count,
                    after: c.face_count + added(&shown, held),
                })
            })
            .collect::<FmResult<Vec<_>>>()?;
        let tags = self
            .tags()?
            .into_iter()
            .map(|t| {
                let held = self.held(
                    "SELECT 1 FROM face_tag WHERE tag_id = ?1 AND face_id = ?2",
                    t.id.0,
                    &shown,
                )?;
                Ok(TagDropCount {
                    tag: t.id,
                    before: t.face_count,
                    after: t.face_count + added(&shown, held),
                })
            })
            .collect::<FmResult<Vec<_>>>()?;
        Ok(DropCounts {
            faces,
            collections,
            tags,
        })
    }

    /// The faces of `faces` the library shows; an unknown face is `NotFound`.
    fn shown_faces(&self, faces: &[FaceId]) -> FmResult<BTreeSet<FaceId>> {
        let mut stmt = self.conn.prepare_cached(SHOWN).map_err(|e| db_error(&e))?;
        let mut out = BTreeSet::new();
        for face in faces {
            let shown: Option<bool> = stmt
                .query_row([face.0], |row| row.get(0))
                .optional()
                .map_err(|e| db_error(&e))?;
            match shown {
                None => return Err(FmError::not_found(format!("Face {face}"))),
                Some(true) => {
                    out.insert(*face);
                }
                Some(false) => {}
            }
        }
        Ok(out)
    }

    /// How many of `faces` the target `id` holds, by `sql` (target id, face id).
    fn held(&self, sql: &str, id: u32, faces: &BTreeSet<FaceId>) -> FmResult<u32> {
        let mut stmt = self.conn.prepare_cached(sql).map_err(|e| db_error(&e))?;
        let mut n = 0u32;
        for face in faces {
            if stmt.exists([id, face.0]).map_err(|e| db_error(&e))? {
                n += 1;
            }
        }
        Ok(n)
    }
}

/// The faces a drop adds: the shown ones the target does not hold yet.
fn added(shown: &BTreeSet<FaceId>, held: u32) -> u32 {
    u32::try_from(shown.len())
        .unwrap_or(u32::MAX)
        .saturating_sub(held)
}

#[cfg(test)]
mod tests;
