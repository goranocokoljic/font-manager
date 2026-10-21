//! Named comparisons (E13.2): the `comparison` and `comparison_face` tables.
//!
//! A comparison is the Compare sheet's 2–4 faces in order, its mode and its
//! sample text, kept under a name. A face row keeps the face's id, PostScript
//! name and "Family Style" name from when it was saved, with no foreign key:
//! a face that leaves the index (its file removed) or that the library hides
//! (its folder paused) is reported as missing by that name, and the
//! comparison opens with the faces left. A face id that now holds another
//! PostScript name (SQLite can reuse the highest freed row id) is missing too,
//! so a comparison never opens a different font.

use fm_api::FaceId;
use fm_api::comparison::{
    CompareMode, Comparison, ComparisonEntry, MAX_COMPARISON_FACES, MIN_COMPARISON_FACES,
    MissingFace, SaveComparisonRequest,
};
use fm_api::ids::ComparisonId;
use rusqlite::{OptionalExtension, params};

use super::{Index, db_error};
use crate::error::{FmError, FmResult};

/// The `comparison.mode` value of a mode (its `fm-api` serde name).
fn mode_to_sql(mode: CompareMode) -> &'static str {
    match mode {
        CompareMode::SideBySide => "sideBySide",
        CompareMode::Stacked => "stacked",
        CompareMode::Overlay => "overlay",
    }
}

/// The mode of a `comparison.mode` value; the schema allows no other.
fn mode_from_sql(value: &str) -> CompareMode {
    match value {
        "stacked" => CompareMode::Stacked,
        "overlay" => CompareMode::Overlay,
        _ => CompareMode::SideBySide,
    }
}

/// The name a comparison is saved under: trimmed, not empty.
fn checked_name(name: &str) -> FmResult<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err(FmError::invalid("A comparison needs a name."));
    }
    Ok(name)
}

/// 2–4 faces, none twice.
fn check_faces(faces: &[FaceId]) -> FmResult<()> {
    if !(MIN_COMPARISON_FACES..=MAX_COMPARISON_FACES).contains(&faces.len()) {
        return Err(FmError::invalid(format!(
            "A comparison holds {MIN_COMPARISON_FACES} to {MAX_COMPARISON_FACES} fonts, not {}.",
            faces.len()
        )));
    }
    if faces
        .iter()
        .enumerate()
        .any(|(i, f)| faces[..i].contains(f))
    {
        return Err(FmError::invalid("A comparison names each font once."));
    }
    Ok(())
}

impl Index {
    /// Every saved comparison, by name (ignoring case), then by id.
    pub fn list_comparisons(&self) -> FmResult<Vec<ComparisonEntry>> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT c.id, c.name, c.mode,
                        (SELECT COUNT(*) FROM comparison_face f WHERE f.comparison_id = c.id)
                 FROM comparison c ORDER BY c.name COLLATE NOCASE, c.id",
            )
            .map_err(|e| db_error(&e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ComparisonEntry {
                    id: ComparisonId(row.get(0)?),
                    name: row.get(1)?,
                    mode: mode_from_sql(&row.get::<_, String>(2)?),
                    face_count: row.get(3)?,
                })
            })
            .map_err(|e| db_error(&e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| db_error(&e))?;
        Ok(rows)
    }

    /// One saved comparison: the faces the library shows, in the saved
    /// order, and the others as missing. An unknown id is `NotFound`.
    pub fn comparison(&self, id: ComparisonId) -> FmResult<Comparison> {
        let head = self
            .conn
            .query_row(
                "SELECT name, mode, sample_text FROM comparison WHERE id = ?1",
                [id.0],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| db_error(&e))?
            .ok_or_else(|| FmError::not_found(format!("Comparison {id}")))?;
        let saved = {
            let mut stmt = self
                .conn
                .prepare_cached(
                    "SELECT f.face_id, f.name, face.id IS NOT NULL
                     FROM comparison_face f
                     LEFT JOIN face ON face.id = f.face_id
                         AND face.postscript_name = f.postscript_name
                     WHERE f.comparison_id = ?1 ORDER BY f.position",
                )
                .map_err(|e| db_error(&e))?;
            stmt.query_map([id.0], |row| {
                Ok((
                    FaceId(row.get(0)?),
                    row.get::<_, String>(1)?,
                    row.get::<_, bool>(2)?,
                ))
            })
            .map_err(|e| db_error(&e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| db_error(&e))?
        };
        let mut faces = Vec::with_capacity(saved.len());
        let mut missing = Vec::new();
        for (face, name, same) in saved {
            if same && self.family_of(face)?.is_some() {
                faces.push(self.face_summary(face)?);
            } else {
                missing.push(MissingFace { id: face, name });
            }
        }
        Ok(Comparison {
            id,
            name: head.0,
            mode: mode_from_sql(&head.1),
            sample_text: head.2,
            faces,
            missing,
        })
    }

    /// Saves a comparison, new or over `req.id`. Fewer than 2 or more than 4
    /// faces, a face named twice, or an empty name is `InvalidRequest`; an
    /// unknown face or comparison is `NotFound`. Nothing is saved on an error.
    pub fn save_comparison(&mut self, req: &SaveComparisonRequest) -> FmResult<Comparison> {
        let name = checked_name(&req.name)?;
        check_faces(&req.faces)?;
        let mut faces = Vec::with_capacity(req.faces.len());
        for &face in &req.faces {
            let s = self.face_summary(face)?;
            faces.push((
                face,
                s.postscript_name,
                format!("{} {}", s.family_name, s.style),
            ));
        }
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        let id = match req.id {
            Some(id) => {
                let changed = tx
                    .execute(
                        "UPDATE comparison SET name = ?2, mode = ?3, sample_text = ?4
                         WHERE id = ?1",
                        params![id.0, name, mode_to_sql(req.mode), req.sample_text],
                    )
                    .map_err(|e| db_error(&e))?;
                if changed == 0 {
                    return Err(FmError::not_found(format!("Comparison {id}")));
                }
                tx.execute(
                    "DELETE FROM comparison_face WHERE comparison_id = ?1",
                    [id.0],
                )
                .map_err(|e| db_error(&e))?;
                id
            }
            None => {
                tx.execute(
                    "INSERT INTO comparison (name, mode, sample_text) VALUES (?1, ?2, ?3)",
                    params![name, mode_to_sql(req.mode), req.sample_text],
                )
                .map_err(|e| db_error(&e))?;
                let id = u32::try_from(tx.last_insert_rowid())
                    .map_err(|_| FmError::internal("comparison id out of range"))?;
                ComparisonId(id)
            }
        };
        {
            let mut add = tx
                .prepare_cached(
                    "INSERT INTO comparison_face
                         (comparison_id, position, face_id, postscript_name, name)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .map_err(|e| db_error(&e))?;
            for (position, (face, postscript, label)) in (0_u32..).zip(&faces) {
                add.execute(params![id.0, position, face.0, postscript, label])
                    .map_err(|e| db_error(&e))?;
            }
        }
        tx.commit().map_err(|e| db_error(&e))?;
        self.comparison(id)
    }

    /// Gives a saved comparison a new name; an empty name is
    /// `InvalidRequest`, an unknown id `NotFound`.
    pub fn rename_comparison(&mut self, id: ComparisonId, name: &str) -> FmResult<ComparisonEntry> {
        let name = checked_name(name)?;
        let changed = self
            .conn
            .execute(
                "UPDATE comparison SET name = ?2 WHERE id = ?1",
                params![id.0, name],
            )
            .map_err(|e| db_error(&e))?;
        if changed == 0 {
            return Err(FmError::not_found(format!("Comparison {id}")));
        }
        self.list_comparisons()?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| FmError::not_found(format!("Comparison {id}")))
    }

    /// Deletes a saved comparison; the faces are not touched. An unknown id
    /// is `NotFound`.
    pub fn delete_comparison(&mut self, id: ComparisonId) -> FmResult<()> {
        let changed = self
            .conn
            .execute("DELETE FROM comparison WHERE id = ?1", [id.0])
            .map_err(|e| db_error(&e))?;
        if changed == 0 {
            return Err(FmError::not_found(format!("Comparison {id}")));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
