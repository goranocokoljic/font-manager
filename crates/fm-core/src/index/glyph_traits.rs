//! Glyph traits in the index (E49.4): what the trait pass
//! ([`crate::trait_pass`]) still has to measure, and what it measured.
//!
//! - **What to measure.** A face of a read file (its `font_file.hash` set)
//!   needs a trait when its `glyph_trait_stamp` is missing, of another
//!   detector version, or of another file hash (the file changed). So a
//!   version bump measures only that trait again, a changed file all of its
//!   traits, and a pass stopped halfway resumes where it stopped.
//! - **What was measured.** A detection is a `glyph_traits` row; a trait the
//!   face lacks the glyph for is a stamp without a row. A write whose face is
//!   gone, or whose file was read again since, writes nothing: the next pass
//!   measures the new file.
//! - **Descriptors.** The shape descriptors of the trait glyphs (E49.8) ride
//!   along: [`crate::glyphs::similarity::STAMP`] is one more key of the
//!   stamps, so a descriptor version bump describes every face again and
//!   measures no trait.

use fm_api::FaceId;
use fm_api::search::DetectedTrait;
use rusqlite::{OptionalExtension, params};

use super::glyph_descriptors::{DescribedFace, write_descriptors};
use super::{Index, db_error};
use crate::error::FmResult;
use crate::glyph_traits::{self, GlyphTrait};
use crate::glyphs::similarity::{self, Descriptor};
use crate::glyphs::traits::{CONFIDENT, Detection};

/// One face the trait pass has to measure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitTodo {
    /// The face.
    pub face: FaceId,
    /// Its font file.
    pub path: String,
    /// The face's index in a collection; 0 otherwise.
    pub collection_index: u32,
    /// The file hash the measures are of.
    pub file_hash: Vec<u8>,
    /// The traits to measure, in the order of the versions asked for.
    pub traits: Vec<String>,
}

/// What the pass measured on one face.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasuredFace {
    /// The face.
    pub face: FaceId,
    /// The file hash measured ([`TraitTodo::file_hash`]).
    pub file_hash: Vec<u8>,
    /// Whether the face is variable (measured at its default instance).
    pub variable: bool,
    /// Every trait measured, with its detector version and the detection;
    /// `None` when the face lacks the glyph or could not be read.
    pub traits: Vec<(String, u32, Option<Detection>)>,
    /// The shape descriptors of the trait glyphs (E49.8), when the pass
    /// described them: written, and the face stamped with
    /// [`similarity::STAMP`] at [`similarity::VERSION`].
    pub descriptors: Option<Vec<(u32, Option<Descriptor>)>>,
}

/// One stored detection.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredTrait {
    /// The trait id.
    pub trait_id: String,
    /// The value id.
    pub value: String,
    /// A scalar trait's first measure.
    pub scalar: Option<f64>,
    /// A scalar trait's measures, as a JSON object.
    pub scalars: Option<String>,
    /// How sure the detector was.
    pub confidence: f64,
    /// The detector's version.
    pub detector_version: u32,
    /// `default` for a variable face, measured at its default instance;
    /// `static` otherwise.
    pub instance: String,
}

/// The faces whose traits are not all measured at `versions` for their
/// file's current hash: every read face but those with a current stamp for
/// each trait. `?1` is the versions as a JSON object, `?2` their count.
///
/// The stamps are counted per face, through the `(face_id, trait)` key:
/// a batch query stops after its `LIMIT` faces and reads only their stamps.
/// (A subquery over every stamp, as before, was read again for each batch:
/// 70 of the 83 s of a 15k-face pass.)
const NEEDS: &str = "
font_file.hash IS NOT NULL
AND (SELECT COUNT(*) FROM glyph_trait_stamp s
     JOIN json_each(?1) v ON v.key = s.trait AND v.value = s.detector_version
     WHERE s.face_id = face.id AND s.file_hash = font_file.hash) < ?2";

fn versions_json(versions: &[(&str, u32)]) -> String {
    let map: serde_json::Map<String, serde_json::Value> = versions
        .iter()
        .map(|(id, v)| ((*id).to_owned(), serde_json::Value::from(*v)))
        .collect();
    serde_json::Value::Object(map).to_string()
}

impl Index {
    /// How many faces still need a trait measured at `versions`.
    pub fn trait_pass_remaining(&self, versions: &[(&str, u32)]) -> FmResult<u32> {
        let sql = format!(
            "SELECT COUNT(*) FROM face JOIN font_file ON font_file.id = face.file_id WHERE {NEEDS}"
        );
        let n: i64 = self
            .conn
            .query_row(
                &sql,
                params![versions_json(versions), versions.len() as i64],
                |r| r.get(0),
            )
            .map_err(|e| db_error(&e))?;
        Ok(u32::try_from(n).unwrap_or(u32::MAX))
    }

    /// Up to `limit` faces after face id `after` that still need a trait
    /// measured at `versions`, by face id, each with the traits it needs.
    pub fn trait_pass_todo(
        &self,
        versions: &[(&str, u32)],
        after: i64,
        limit: u32,
    ) -> FmResult<Vec<TraitTodo>> {
        let json = versions_json(versions);
        let sql = format!(
            "SELECT face.id, font_file.path, face.collection_index, font_file.hash
             FROM face JOIN font_file ON font_file.id = face.file_id
             WHERE face.id > ?3 AND {NEEDS}
             ORDER BY face.id LIMIT ?4"
        );
        let mut stmt = self.conn.prepare(&sql).map_err(|e| db_error(&e))?;
        let rows = stmt
            .query_map(params![json, versions.len() as i64, after, limit], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, u32>(2)?,
                    r.get::<_, Vec<u8>>(3)?,
                ))
            })
            .map_err(|e| db_error(&e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| db_error(&e))?;
        let mut done = self
            .conn
            .prepare_cached(
                "SELECT detector_version FROM glyph_trait_stamp
                 WHERE face_id = ?1 AND trait = ?2 AND file_hash = ?3",
            )
            .map_err(|e| db_error(&e))?;
        let mut out = Vec::with_capacity(rows.len());
        for (id, path, collection_index, file_hash) in rows {
            let mut traits = Vec::new();
            for (trait_id, version) in versions {
                let stamped: Option<u32> = done
                    .query_row(params![id, trait_id, file_hash], |r| r.get(0))
                    .optional()
                    .map_err(|e| db_error(&e))?;
                if stamped != Some(*version) {
                    traits.push((*trait_id).to_owned());
                }
            }
            out.push(TraitTodo {
                face: FaceId(u32::try_from(id).unwrap_or(u32::MAX)),
                path,
                collection_index,
                file_hash,
                traits,
            });
        }
        Ok(out)
    }

    /// Writes what the pass measured, in one transaction. A face that is gone
    /// or whose file hash changed since it was listed is left alone. Returns
    /// the faces written.
    pub fn write_glyph_traits(&mut self, measured: &[MeasuredFace]) -> FmResult<u32> {
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        let mut written = 0;
        {
            let mut current = tx
                .prepare_cached(
                    "SELECT 1 FROM face JOIN font_file ON font_file.id = face.file_id
                     WHERE face.id = ?1 AND font_file.hash = ?2",
                )
                .map_err(|e| db_error(&e))?;
            let mut clear = tx
                .prepare_cached("DELETE FROM glyph_traits WHERE face_id = ?1 AND trait = ?2")
                .map_err(|e| db_error(&e))?;
            let mut insert = tx
                .prepare_cached(
                    "INSERT INTO glyph_traits
                     (face_id, trait, value, scalar, scalars, confidence, detector_version, instance)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                )
                .map_err(|e| db_error(&e))?;
            let mut stamp = tx
                .prepare_cached(
                    "INSERT OR REPLACE INTO glyph_trait_stamp
                     (face_id, trait, detector_version, file_hash) VALUES (?1, ?2, ?3, ?4)",
                )
                .map_err(|e| db_error(&e))?;
            for m in measured {
                let face = i64::from(m.face.0);
                if !current
                    .exists(params![face, m.file_hash])
                    .map_err(|e| db_error(&e))?
                {
                    continue;
                }
                let instance = if m.variable { "default" } else { "static" };
                for (trait_id, version, detection) in &m.traits {
                    clear
                        .execute(params![face, trait_id])
                        .map_err(|e| db_error(&e))?;
                    if let Some(d) = detection {
                        let scalars = (!d.scalars.is_empty()).then(|| {
                            let map: serde_json::Map<String, serde_json::Value> = d
                                .scalars
                                .iter()
                                .map(|(k, v)| (k.clone(), serde_json::Value::from(*v)))
                                .collect();
                            serde_json::Value::Object(map).to_string()
                        });
                        insert
                            .execute(params![
                                face,
                                trait_id,
                                d.value,
                                d.scalars.first().map(|(_, v)| *v),
                                scalars,
                                d.confidence,
                                version,
                                instance
                            ])
                            .map_err(|e| db_error(&e))?;
                    }
                    stamp
                        .execute(params![face, trait_id, version, m.file_hash])
                        .map_err(|e| db_error(&e))?;
                }
                if let Some(glyphs) = &m.descriptors {
                    let described = DescribedFace {
                        face: m.face,
                        file_hash: m.file_hash.clone(),
                        glyphs: glyphs.clone(),
                    };
                    write_descriptors(&tx, std::slice::from_ref(&described))?;
                    stamp
                        .execute(params![
                            face,
                            similarity::STAMP,
                            similarity::VERSION,
                            m.file_hash
                        ])
                        .map_err(|e| db_error(&e))?;
                }
                written += 1;
            }
        }
        tx.commit().map_err(|e| db_error(&e))?;
        Ok(written)
    }

    /// The stored detections of `face`, by trait id.
    pub fn face_glyph_traits(&self, face: FaceId) -> FmResult<Vec<StoredTrait>> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT trait, value, scalar, scalars, confidence, detector_version, instance
                 FROM glyph_traits WHERE face_id = ?1 ORDER BY trait",
            )
            .map_err(|e| db_error(&e))?;
        stmt.query_map([face.0], |r| {
            Ok(StoredTrait {
                trait_id: r.get(0)?,
                value: r.get(1)?,
                scalar: r.get(2)?,
                scalars: r.get(3)?,
                confidence: r.get(4)?,
                detector_version: r.get(5)?,
                instance: r.get(6)?,
            })
        })
        .map_err(|e| db_error(&e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_error(&e))
    }

    /// The traits `face` is detected with confidence, as the family page
    /// shows them (E49.10): see [`confident_traits`].
    pub fn face_traits(&self, face: FaceId) -> FmResult<Vec<DetectedTrait>> {
        Ok(confident_traits(
            glyph_traits::load()?,
            &self.face_glyph_traits(face)?,
        ))
    }
}

/// The detections of `stored` a value filter finds the face by, in
/// `catalog` order: of a trait the catalog offers (`enabled`; a disabled
/// detector misses the accuracy bar, so its answer is not shown as a fact),
/// of a value the catalog lists, and at least [`CONFIDENT`] (a guess between
/// two values is not a trait of the face).
#[allow(clippy::cast_possible_truncation)] // A confidence in 0..=1 fits an f32.
pub(crate) fn confident_traits(
    catalog: &[GlyphTrait],
    stored: &[StoredTrait],
) -> Vec<DetectedTrait> {
    catalog
        .iter()
        .filter(|t| t.enabled)
        .filter_map(|t| {
            let d = stored.iter().find(|d| d.trait_id == t.id)?;
            (d.confidence >= CONFIDENT && t.value(&d.value).is_some()).then(|| DetectedTrait {
                trait_id: t.id.clone(),
                value: d.value.clone(),
                confidence: d.confidence as f32,
            })
        })
        .collect()
}

#[cfg(test)]
impl Index {
    /// The face of the file whose path ends in `name`.
    pub(crate) fn trait_debug_face(&self, name: &str) -> Option<FaceId> {
        self.conn
            .query_row(
                "SELECT face.id FROM face JOIN font_file ON font_file.id = face.file_id
                 WHERE font_file.path LIKE '%' || ?1 ORDER BY face.id LIMIT 1",
                [name],
                |r| r.get(0),
            )
            .optional()
            .ok()
            .flatten()
            .map(FaceId)
    }

    /// Runs `sql` on the database.
    pub(crate) fn trait_debug_sql(&self, sql: &str) {
        self.conn.execute_batch(sql).expect("test SQL");
    }

    /// The columns of the database index `name`.
    pub(crate) fn trait_debug_index_columns(&self, name: &str) -> Vec<String> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM pragma_index_info(?1) ORDER BY seqno")
            .expect("prepare");
        stmt.query_map([name], |r| r.get(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows")
    }
}
