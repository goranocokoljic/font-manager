//! Project sets (E7.4): the `project_set` and `set_face` tables.
//!
//! A set is a named group of faces the user activates and deactivates
//! together (`Target::Set`, resolved in `index/activation.rs`). Deleting a
//! set never touches a face.
//!
//! A set travels as JSON ([`SetExport`]): each font is named by the blake3
//! hash of its file, so a teammate's copy at another path still matches. A
//! file that holds several faces (a collection) is told apart by family and
//! style. What the import cannot match comes back in `missing`.

use fm_api::organization::{CreateSetRequest, ImportSetResponse, ProjectSet, SetEntry, SetExport};
use fm_api::{ErrorCode, FaceId, SetId};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::{Index, db_error};
use crate::error::{FmError, FmResult};
use crate::scanner::FileHash;

/// Every face of a set with what an export names it by, in face order.
const SET_FONTS: &str = "
SELECT face.id, face.family, face.subfamily, font_file.hash
FROM set_face
JOIN face ON face.id = set_face.face_id
JOIN font_file ON font_file.id = face.file_id
WHERE set_face.set_id = ?1
ORDER BY face.id";

/// The faces of the files with this hash that are still there, faces of
/// active sources first.
const FACES_BY_HASH: &str = "
SELECT face.id, face.family, face.subfamily, face.collection_index
FROM font_file
JOIN face ON face.file_id = font_file.id
JOIN source ON source.id = font_file.source_id
WHERE font_file.hash = ?1 AND font_file.gone = 0
ORDER BY source.paused, face.id";

impl Index {
    /// Every set, A to Z, each with its faces in id order.
    pub fn sets(&self) -> FmResult<Vec<ProjectSet>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name FROM project_set ORDER BY name COLLATE NOCASE, id")
            .map_err(|e| db_error(&e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| db_error(&e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| db_error(&e))?;
        rows.into_iter()
            .map(|(id, name)| {
                Ok(ProjectSet {
                    id: SetId(id),
                    name,
                    faces: members(&self.conn, id)?,
                })
            })
            .collect()
    }

    /// One set; an unknown one is `NotFound`.
    pub fn set(&self, id: SetId) -> FmResult<ProjectSet> {
        let name: String = self
            .conn
            .query_row("SELECT name FROM project_set WHERE id = ?1", [id.0], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|e| db_error(&e))?
            .ok_or_else(|| FmError::not_found(format!("Set {id}")))?;
        Ok(ProjectSet {
            id,
            name,
            faces: members(&self.conn, id.0)?,
        })
    }

    /// Creates a set of faces; a face named twice is in it once. The name is
    /// trimmed; an empty one is `InvalidRequest`, one another set has (in
    /// any case) is `Conflict`, an unknown face `NotFound`. No face is
    /// fine: the user fills the set later.
    pub fn create_set(&mut self, req: &CreateSetRequest) -> FmResult<ProjectSet> {
        let name = set_name(&req.name)?;
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        if name_taken(&tx, name)? {
            return Err(FmError::new(
                ErrorCode::Conflict,
                format!("There is already a set named “{name}”."),
            ));
        }
        ensure_faces(&tx, &req.faces)?;
        let id = insert_set(&tx, name, &req.faces)?;
        tx.commit().map_err(|e| db_error(&e))?;
        self.set(id)
    }

    /// Deletes a set; its faces stay in the library. Unknown is `NotFound`.
    pub fn delete_set(&mut self, id: SetId) -> FmResult<()> {
        let deleted = self
            .conn
            .execute("DELETE FROM project_set WHERE id = ?1", [id.0])
            .map_err(|e| db_error(&e))?;
        if deleted == 0 {
            return Err(FmError::not_found(format!("Set {id}")));
        }
        Ok(())
    }

    /// A set as portable JSON: name, and per face its family, style and the
    /// hash of its file. A face whose file is not read yet has no hash; the
    /// export is refused (`Conflict`) rather than silently shorter.
    pub fn export_set(&self, id: SetId) -> FmResult<SetExport> {
        let set = self.set(id)?;
        let mut stmt = self.conn.prepare(SET_FONTS).map_err(|e| db_error(&e))?;
        let rows = stmt
            .query_map([id.0], |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<Vec<u8>>>(3)?,
                ))
            })
            .map_err(|e| db_error(&e))?;
        let mut fonts = Vec::new();
        for row in rows {
            let (face, family, style, hash) = row.map_err(|e| db_error(&e))?;
            let hash = hash
                .and_then(|h| <[u8; 32]>::try_from(h).ok())
                .ok_or_else(|| {
                    FmError::new(
                        ErrorCode::Conflict,
                        format!(
                            "“{family} {style}” has not been read yet. Scan the library, then export the set again."
                        ),
                    )
                    .with_detail(format!("face {face} has no file hash"))
                })?;
            fonts.push(SetEntry {
                family,
                style,
                hash: FileHash::from(hash).to_hex(),
            });
        }
        Ok(SetExport {
            name: set.name,
            fonts,
        })
    }

    /// Creates a set from portable JSON. Each entry is matched by the hash
    /// of a file in this library that is still there; in a file with several
    /// faces, by family and style too. Entries that match nothing (or whose
    /// hash is not a hash) are returned in `missing`, in their order. A name
    /// another set has gets " (2)", " (3)"… so an import never fails on it.
    pub fn import_set(&mut self, export: &SetExport) -> FmResult<ImportSetResponse> {
        let base = set_name(&export.name)?;
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        let mut faces: Vec<FaceId> = Vec::new();
        let mut missing = Vec::new();
        for entry in &export.fonts {
            match match_entry(&tx, entry)? {
                Some(face) => {
                    if !faces.contains(&face) {
                        faces.push(face);
                    }
                }
                None => missing.push(entry.clone()),
            }
        }
        let name = free_name(&tx, base)?;
        let id = insert_set(&tx, &name, &faces)?;
        tx.commit().map_err(|e| db_error(&e))?;
        Ok(ImportSetResponse {
            set: self.set(id)?,
            missing,
        })
    }
}

/// A set's faces, in id order.
fn members(conn: &Connection, set: u32) -> FmResult<Vec<FaceId>> {
    let mut stmt = conn
        .prepare_cached("SELECT face_id FROM set_face WHERE set_id = ?1 ORDER BY face_id")
        .map_err(|e| db_error(&e))?;
    let ids = stmt
        .query_map([set], |row| row.get::<_, u32>(0).map(FaceId))
        .map_err(|e| db_error(&e))?
        .collect::<Result<_, _>>()
        .map_err(|e| db_error(&e))?;
    Ok(ids)
}

/// The trimmed name; empty is `InvalidRequest`.
fn set_name(name: &str) -> FmResult<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err(FmError::invalid("Give the set a name."));
    }
    Ok(name)
}

fn name_taken(tx: &Transaction<'_>, name: &str) -> FmResult<bool> {
    tx.prepare_cached("SELECT 1 FROM project_set WHERE name = ?1 COLLATE NOCASE")
        .and_then(|mut s| s.exists([name]))
        .map_err(|e| db_error(&e))
}

/// `base`, or the first of "base (2)", "base (3)"… no set has.
fn free_name(tx: &Transaction<'_>, base: &str) -> FmResult<String> {
    if !name_taken(tx, base)? {
        return Ok(base.to_owned());
    }
    let mut n = 2u32;
    loop {
        let name = format!("{base} ({n})");
        if !name_taken(tx, &name)? {
            return Ok(name);
        }
        n += 1;
    }
}

fn ensure_faces(tx: &Transaction<'_>, faces: &[FaceId]) -> FmResult<()> {
    let mut known = tx
        .prepare_cached("SELECT 1 FROM face WHERE id = ?1")
        .map_err(|e| db_error(&e))?;
    for FaceId(face) in faces {
        if !known.exists([face]).map_err(|e| db_error(&e))? {
            return Err(FmError::not_found(format!("Face {face}")));
        }
    }
    Ok(())
}

fn insert_set(tx: &Transaction<'_>, name: &str, faces: &[FaceId]) -> FmResult<SetId> {
    tx.execute("INSERT INTO project_set (name) VALUES (?1)", [name])
        .map_err(|e| db_error(&e))?;
    let id = u32::try_from(tx.last_insert_rowid())
        .map_err(|_| FmError::internal("project set id out of range"))?;
    let mut add = tx
        .prepare_cached("INSERT OR IGNORE INTO set_face (set_id, face_id) VALUES (?1, ?2)")
        .map_err(|e| db_error(&e))?;
    for face in faces {
        add.execute(params![id, face.0]).map_err(|e| db_error(&e))?;
    }
    Ok(SetId(id))
}

/// The face an exported entry names in this library, if any. Among copies
/// of the file, one whose family and style match wins, then the first; a
/// collection file with no matching face is no match (its faces differ).
fn match_entry(tx: &Transaction<'_>, entry: &SetEntry) -> FmResult<Option<FaceId>> {
    let Ok(hash) = blake3::Hash::from_hex(entry.hash.trim()) else {
        return Ok(None);
    };
    let mut stmt = tx.prepare_cached(FACES_BY_HASH).map_err(|e| db_error(&e))?;
    let candidates = stmt
        .query_map([&hash.as_bytes()[..]], |row| {
            Ok((
                FaceId(row.get(0)?),
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, u32>(3)?,
            ))
        })
        .map_err(|e| db_error(&e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_error(&e))?;
    let named = candidates.iter().find(|(_, family, style, _)| {
        family.eq_ignore_ascii_case(entry.family.trim())
            && style.eq_ignore_ascii_case(entry.style.trim())
    });
    if let Some((face, ..)) = named {
        return Ok(Some(*face));
    }
    Ok(candidates
        .first()
        .filter(|first| candidates.iter().all(|c| c.3 == first.3))
        .map(|first| first.0))
}

#[cfg(test)]
mod tests;
