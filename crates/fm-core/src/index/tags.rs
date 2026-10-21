//! Tags and user metadata (E7.2): the `tag`, `face_tag` and `user_meta`
//! tables.
//!
//! A tag is a free-form name the user puts on faces. Names are trimmed and
//! unique without regard to case: tagging with "display" reuses "Display".
//! A tag no face carries any more is deleted, so the names the Info tab
//! offers to complete are the ones in use. Favorite, rating and notes are one
//! `user_meta` row per face, written on the first change.
//!
//! Search reads all of it (`index/search.rs`), so the filters and their
//! counts follow every write here. Every write is mirrored to the sidecars
//! of the faces' families (`index/sidecars.rs`).

use std::collections::{BTreeMap, BTreeSet};

use fm_api::organization::{
    SelectionTag, SelectionTags, Tag, TagFacesRequest, UntagFacesRequest, UserMetaUpdate,
};
use fm_api::{FaceId, FamilyId, TagId};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{Index, db_error, sidecars};
use crate::error::{FmError, FmResult};
use crate::sidecar::Field;

/// Every tag with the number of its faces the library shows (files not gone,
/// sources not paused), A to Z.
const TAGS: &str = "
SELECT t.id, t.name,
       (SELECT COUNT(*)
        FROM face_tag AS ft
        JOIN face ON face.id = ft.face_id
        JOIN font_file ON font_file.id = face.file_id
        JOIN source ON source.id = font_file.source_id
        WHERE ft.tag_id = t.id AND font_file.gone = 0 AND source.paused = 0)
FROM tag AS t
ORDER BY t.name COLLATE NOCASE, t.id";

/// Writes the fields of a [`UserMetaUpdate`] that are set: `NULL` keeps a
/// field, a rating of 0 and empty notes clear it.
const UPDATE_META: &str = "
INSERT INTO user_meta (face_id, favorite, rating, notes)
VALUES (?1, coalesce(?2, 0), nullif(?3, 0), nullif(?4, ''))
ON CONFLICT (face_id) DO UPDATE SET
    favorite = coalesce(?2, favorite),
    rating = CASE WHEN ?3 IS NULL THEN rating ELSE nullif(?3, 0) END,
    notes = CASE WHEN ?4 IS NULL THEN notes ELSE nullif(?4, '') END";

impl Index {
    /// Every tag, A to Z, with its visible face count.
    pub fn tags(&self) -> FmResult<Vec<Tag>> {
        let mut stmt = self.conn.prepare(TAGS).map_err(|e| db_error(&e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Tag {
                    id: TagId(row.get(0)?),
                    name: row.get(1)?,
                    face_count: row.get(2)?,
                })
            })
            .map_err(|e| db_error(&e))?;
        rows.collect::<Result<_, _>>().map_err(|e| db_error(&e))
    }

    /// The faces `faces` and every face of `families` (as the library lists
    /// them), each once, the faces first. An unknown family is `NotFound`.
    pub fn faces_of(&self, faces: &[FaceId], families: &[FamilyId]) -> FmResult<Vec<FaceId>> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::with_capacity(faces.len());
        out.extend(faces.iter().copied().filter(|f| seen.insert(*f)));
        for family in families {
            let members = self
                .family_faces(*family)?
                .ok_or_else(|| FmError::not_found(format!("Family {family}")))?;
            out.extend(members.into_iter().filter(|f| seen.insert(*f)));
        }
        Ok(out)
    }

    /// The tags the faces of a selection (faces and whole families) carry,
    /// A to Z, with how many of its faces carry each and whether all do.
    /// An empty selection carries none; an unknown face or family is
    /// `NotFound`.
    pub fn selection_tags(
        &mut self,
        faces: &[FaceId],
        families: &[FamilyId],
    ) -> FmResult<SelectionTags> {
        let faces = self.faces_of(faces, families)?;
        let mut counts: BTreeMap<u32, u32> = BTreeMap::new();
        {
            let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
            ensure_faces(&tx, &faces)?;
            {
                let mut carried = tx
                    .prepare("SELECT tag_id FROM face_tag WHERE face_id = ?1")
                    .map_err(|e| db_error(&e))?;
                for FaceId(face) in &faces {
                    let ids = carried
                        .query_map([face], |row| row.get::<_, u32>(0))
                        .map_err(|e| db_error(&e))?;
                    for id in ids {
                        *counts.entry(id.map_err(|e| db_error(&e))?).or_default() += 1;
                    }
                }
            }
            tx.commit().map_err(|e| db_error(&e))?;
        }
        let total = u32::try_from(faces.len()).unwrap_or(u32::MAX);
        let tags = self
            .tags()?
            .into_iter()
            .filter_map(|t| {
                counts.get(&t.id.0).map(|&n| SelectionTag {
                    tag: t.id,
                    face_count: n,
                    all: n == total,
                })
            })
            .collect();
        Ok(SelectionTags { faces, tags })
    }

    /// Puts tags on faces and on every face of whole families, creating the
    /// tags that do not exist yet; returns the tags named, in request order.
    /// A face that has a tag keeps it once. No fonts or no name is
    /// `InvalidRequest`; an unknown face or family `NotFound`.
    pub fn tag_faces(&mut self, req: &TagFacesRequest) -> FmResult<Vec<Tag>> {
        let mut names: Vec<&str> = Vec::new();
        for name in req.tags.iter().map(|n| n.trim()).filter(|n| !n.is_empty()) {
            if !names.iter().any(|n| same_name(n, name)) {
                names.push(name);
            }
        }
        if names.is_empty() {
            return Err(FmError::invalid("Give the tag a name."));
        }
        let faces = self.faces_of(&req.faces, &req.families)?;
        if faces.is_empty() {
            return Err(FmError::invalid("Choose the fonts to tag."));
        }
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        ensure_faces(&tx, &faces)?;
        let mut ids = Vec::with_capacity(names.len());
        {
            let mut find = tx
                .prepare("SELECT id FROM tag WHERE name = ?1")
                .map_err(|e| db_error(&e))?;
            let mut create = tx
                .prepare("INSERT INTO tag (name) VALUES (?1)")
                .map_err(|e| db_error(&e))?;
            let mut attach = tx
                .prepare("INSERT OR IGNORE INTO face_tag (face_id, tag_id) VALUES (?1, ?2)")
                .map_err(|e| db_error(&e))?;
            for name in names {
                let found: Option<u32> = find
                    .query_row([name], |row| row.get(0))
                    .optional()
                    .map_err(|e| db_error(&e))?;
                let id = match found {
                    Some(id) => id,
                    None => {
                        create.execute([name]).map_err(|e| db_error(&e))?;
                        u32::try_from(tx.last_insert_rowid())
                            .map_err(|_| FmError::internal("tag id out of range"))?
                    }
                };
                for FaceId(face) in &faces {
                    attach
                        .execute(params![face, id])
                        .map_err(|e| db_error(&e))?;
                }
                ids.push(TagId(id));
            }
        }
        tx.commit().map_err(|e| db_error(&e))?;
        self.mirror_sidecars();
        let all = self.tags()?;
        Ok(ids
            .iter()
            .filter_map(|id| all.iter().find(|t| t.id == *id).cloned())
            .collect())
    }

    /// Takes tags off faces; a tag left on no face is deleted. A face that
    /// does not have the tag is left alone; an unknown face or tag is
    /// `NotFound`.
    pub fn untag_faces(&mut self, req: &UntagFacesRequest) -> FmResult<()> {
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        ensure_faces(&tx, &req.faces)?;
        {
            let mut known = tx
                .prepare("SELECT 1 FROM tag WHERE id = ?1")
                .map_err(|e| db_error(&e))?;
            let mut detach = tx
                .prepare("DELETE FROM face_tag WHERE face_id = ?1 AND tag_id = ?2")
                .map_err(|e| db_error(&e))?;
            for TagId(tag) in &req.tags {
                if !known.exists([tag]).map_err(|e| db_error(&e))? {
                    return Err(FmError::not_found(format!("Tag {tag}")));
                }
                for FaceId(face) in &req.faces {
                    detach
                        .execute(params![face, tag])
                        .map_err(|e| db_error(&e))?;
                }
            }
            let mut unused = tx
                .prepare(
                    "DELETE FROM tag WHERE id = ?1
                     AND NOT EXISTS (SELECT 1 FROM face_tag WHERE tag_id = ?1)",
                )
                .map_err(|e| db_error(&e))?;
            for TagId(tag) in &req.tags {
                unused.execute([tag]).map_err(|e| db_error(&e))?;
            }
        }
        tx.commit().map_err(|e| db_error(&e))?;
        self.mirror_sidecars();
        Ok(())
    }

    /// Sets favorite, rating (1–5 stars; 0 clears it) and notes (empty
    /// clears them) on faces; fields left out stay as they are. A rating
    /// above 5 is `InvalidRequest`; an unknown face `NotFound`. Setting a
    /// field dismisses its sidecar conflicts on those faces.
    pub fn update_user_meta(&mut self, req: &UserMetaUpdate) -> FmResult<()> {
        if req.rating.is_some_and(|r| r > 5) {
            return Err(FmError::invalid(
                "A rating is 1 to 5 stars, or 0 to clear it.",
            ));
        }
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        ensure_faces(&tx, &req.faces)?;
        {
            let mut write = tx.prepare(UPDATE_META).map_err(|e| db_error(&e))?;
            for FaceId(face) in &req.faces {
                write
                    .execute(params![face, req.favorite, req.rating, req.notes])
                    .map_err(|e| db_error(&e))?;
                let set = [
                    (Field::Favorite, req.favorite.is_some()),
                    (Field::Rating, req.rating.is_some()),
                    (Field::Notes, req.notes.is_some()),
                ];
                for (field, _) in set.into_iter().filter(|(_, set)| *set) {
                    sidecars::resolve_conflicts(&tx, *face, field)?;
                }
            }
        }
        tx.commit().map_err(|e| db_error(&e))?;
        self.mirror_sidecars();
        Ok(())
    }
}

/// Tag names are the same when they differ only in ASCII case, as the
/// `tag.name` column (`COLLATE NOCASE`) compares them.
fn same_name(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// `NotFound` for the first face of `faces` the index does not have.
fn ensure_faces(tx: &Transaction<'_>, faces: &[FaceId]) -> FmResult<()> {
    let mut known = tx
        .prepare("SELECT 1 FROM face WHERE id = ?1")
        .map_err(|e| db_error(&e))?;
    for FaceId(face) in faces {
        if !known.exists([face]).map_err(|e| db_error(&e))? {
            return Err(FmError::not_found(format!("Face {face}")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
