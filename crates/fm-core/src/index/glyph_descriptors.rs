//! Glyph shape descriptors in the index (E49.8, ADR 0023): one row per face
//! and character ([`crate::glyphs::similarity`]).
//!
//! - **Current.** A row is current when it is of the descriptor's
//!   [`similarity::VERSION`] and of the face's file hash as it is now; any
//!   other row is described again. A row without a descriptor is a face that
//!   does not draw the character (or cannot be read as a font): it is not
//!   described again until the file or the version changes.
//! - **Who writes.** The trait pass ([`crate::trait_pass`]) writes the trait
//!   glyphs of every face; a similar-glyph query writes, first, any other
//!   character it is asked about ([`Index::glyph_descriptor_todo`]). A write
//!   whose face is gone, or whose file was read again since, writes nothing.

use fm_api::FaceId;
use rusqlite::params;

use super::{Index, db_error};
use crate::error::FmResult;
use crate::glyphs::similarity::{self, Descriptor};

/// One face whose descriptor of a character is missing or out of date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorTodo {
    /// The face.
    pub face: FaceId,
    /// Its font file.
    pub path: String,
    /// The face's index in a collection; 0 otherwise.
    pub collection_index: u32,
    /// The file hash the descriptor is of.
    pub file_hash: Vec<u8>,
}

/// The descriptors of one face, as measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribedFace {
    /// The face.
    pub face: FaceId,
    /// The file hash described.
    pub file_hash: Vec<u8>,
    /// Each character with its descriptor; `None` when the face does not
    /// draw it.
    pub glyphs: Vec<(u32, Option<Descriptor>)>,
}

impl Index {
    /// The faces of read files (their hash set) whose descriptor of
    /// `code_point` is missing, of another version or of another file hash,
    /// by face id.
    pub fn glyph_descriptor_todo(&self, code_point: u32) -> FmResult<Vec<DescriptorTodo>> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT face.id, font_file.path, face.collection_index, font_file.hash
                 FROM face JOIN font_file ON font_file.id = face.file_id
                 LEFT JOIN glyph_descriptor d
                   ON d.code_point = ?1 AND d.face_id = face.id
                 WHERE font_file.hash IS NOT NULL
                   AND (d.face_id IS NULL OR d.version != ?2 OR d.file_hash != font_file.hash)
                 ORDER BY face.id",
            )
            .map_err(|e| db_error(&e))?;
        stmt.query_map(params![code_point, similarity::VERSION], |r| {
            Ok(DescriptorTodo {
                face: FaceId(r.get(0)?),
                path: r.get(1)?,
                collection_index: r.get(2)?,
                file_hash: r.get(3)?,
            })
        })
        .map_err(|e| db_error(&e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_error(&e))
    }

    /// Writes `described` in one transaction, at the current version. A face
    /// that is gone or whose file hash changed since is left alone. Returns
    /// the faces written.
    pub fn write_glyph_descriptors(&mut self, described: &[DescribedFace]) -> FmResult<u32> {
        if described.is_empty() {
            return Ok(0);
        }
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        let written = write_descriptors(&tx, described)?;
        tx.commit().map_err(|e| db_error(&e))?;
        Ok(written)
    }

    /// The current descriptors of `code_point` with their faces, by face id;
    /// faces that do not draw it are left out.
    pub(super) fn current_glyph_descriptors(
        &self,
        code_point: u32,
    ) -> FmResult<Vec<(u32, Descriptor)>> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT d.face_id, d.descriptor FROM glyph_descriptor d
                 JOIN face ON face.id = d.face_id
                 JOIN font_file ON font_file.id = face.file_id
                 WHERE d.code_point = ?1 AND d.version = ?2
                   AND d.file_hash = font_file.hash AND d.descriptor IS NOT NULL
                 ORDER BY d.face_id",
            )
            .map_err(|e| db_error(&e))?;
        let rows = stmt
            .query_map(params![code_point, similarity::VERSION], |r| {
                Ok((r.get::<_, u32>(0)?, r.get::<_, Vec<u8>>(1)?))
            })
            .map_err(|e| db_error(&e))?;
        let mut out = Vec::new();
        for row in rows {
            let (face, bytes) = row.map_err(|e| db_error(&e))?;
            // A row of another length is not of this version: skipped.
            if let Some(d) = Descriptor::from_bytes(bytes) {
                out.push((face, d));
            }
        }
        Ok(out)
    }
}

/// The rows of `described` written in the open transaction `tx`; the faces
/// written.
pub(super) fn write_descriptors(
    tx: &rusqlite::Transaction<'_>,
    described: &[DescribedFace],
) -> FmResult<u32> {
    let mut current = tx
        .prepare_cached(
            "SELECT 1 FROM face JOIN font_file ON font_file.id = face.file_id
             WHERE face.id = ?1 AND font_file.hash = ?2",
        )
        .map_err(|e| db_error(&e))?;
    let mut upsert = tx
        .prepare_cached(
            "INSERT OR REPLACE INTO glyph_descriptor
             (code_point, face_id, version, file_hash, descriptor)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .map_err(|e| db_error(&e))?;
    let mut written = 0;
    for f in described {
        let face = i64::from(f.face.0);
        if !current
            .exists(params![face, f.file_hash])
            .map_err(|e| db_error(&e))?
        {
            continue;
        }
        for (code_point, descriptor) in &f.glyphs {
            upsert
                .execute(params![
                    code_point,
                    face,
                    similarity::VERSION,
                    f.file_hash,
                    descriptor.as_ref().map(Descriptor::as_bytes)
                ])
                .map_err(|e| db_error(&e))?;
        }
        written += 1;
    }
    Ok(written)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
impl Index {
    /// The stored descriptor of `code_point` of `face`, current or not;
    /// `None` without a row or for a row without a descriptor.
    pub(crate) fn glyph_debug_descriptor(
        &self,
        face: FaceId,
        code_point: u32,
    ) -> Option<Descriptor> {
        use rusqlite::OptionalExtension;
        self.conn
            .query_row(
                "SELECT descriptor FROM glyph_descriptor WHERE face_id = ?1 AND code_point = ?2",
                params![face.0, code_point],
                |r| r.get::<_, Option<Vec<u8>>>(0),
            )
            .optional()
            .ok()
            .flatten()
            .flatten()
            .and_then(Descriptor::from_bytes)
    }

    /// The rows the connection has changed since it opened.
    pub(crate) fn trait_debug_total_changes(&self) -> u64 {
        self.conn.total_changes()
    }
}
