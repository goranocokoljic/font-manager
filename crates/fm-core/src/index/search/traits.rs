//! Glyph-trait search (E49.5): the faces a search's trait filters let
//! through, and the trait catalog with its counts in the library.
//!
//! The detections are the `glyph_traits` rows the trait pass wrote (E49.4).
//! A value filter matches a row of that value detected with confidence
//! ([`CONFIDENT`]): a guess between two values is not passed off as a fact.
//! A range filter reads the measured number itself. A face without a row
//! for the trait — it lacks the glyph, or the pass has not measured it yet —
//! passes no filter on that trait and counts under no value: it is never
//! a "none".
//!
//! One indexed query per filter on `(trait, value)`, intersected; the rows
//! are few next to the faces, so a search keeps inside its 50 ms.

use std::collections::{HashMap, HashSet};

use fm_api::search::{
    GlyphTraitInfo, GlyphTraitKind, GlyphTraitMeasure, GlyphTraitThreshold, GlyphTraitValue,
    TraitFilter,
};
use rusqlite::{Connection, params};

use super::{Index, db_error};
use crate::error::{FmError, FmResult};
use crate::glyph_traits::{self, GlyphTrait, TraitKind};
use crate::glyphs::traits::CONFIDENT;

/// The faces that pass every filter of `filters`; `None` when there is
/// none, so every face passes. An unknown trait, value or measure, a range
/// on an enum trait, or a range without a bound is `InvalidRequest`.
pub(super) fn matching(
    conn: &Connection,
    filters: &[TraitFilter],
) -> FmResult<Option<HashSet<u32>>> {
    let mut passing: Option<HashSet<u32>> = None;
    for filter in filters {
        let faces = faces_of(conn, filter)?;
        passing = Some(match passing {
            None => faces,
            Some(before) => before.intersection(&faces).copied().collect(),
        });
    }
    Ok(passing)
}

/// The catalog trait `id`, or `InvalidRequest`.
fn catalog_trait(id: &str) -> FmResult<&'static GlyphTrait> {
    glyph_traits::find(id)?.ok_or_else(|| {
        let known: Vec<&str> = glyph_traits::load()
            .map(|all| all.iter().map(|t| t.id.as_str()).collect())
            .unwrap_or_default();
        FmError::invalid(format!(
            "There is no glyph trait {id}. The traits are {}.",
            known.join(", ")
        ))
    })
}

/// The faces one filter lets through.
fn faces_of(conn: &Connection, filter: &TraitFilter) -> FmResult<HashSet<u32>> {
    match filter {
        TraitFilter::Value { trait_id, value } => {
            let t = catalog_trait(trait_id)?;
            if t.value(value).is_none() {
                let values: Vec<&str> = t.values.iter().map(|v| v.id.as_str()).collect();
                return Err(FmError::invalid(format!(
                    "{trait_id} has no value {value}. Its values are {}.",
                    values.join(", ")
                )));
            }
            collect(
                conn,
                "SELECT face_id FROM glyph_traits
                 WHERE trait = ?1 AND value = ?2 AND confidence >= ?3",
                params![trait_id, value, CONFIDENT],
            )
        }
        TraitFilter::Range {
            trait_id,
            measure,
            min,
            max,
        } => {
            let t = catalog_trait(trait_id)?;
            if t.kind != TraitKind::Scalar {
                return Err(FmError::invalid(format!(
                    "{trait_id} measures no number; ask for one of its values."
                )));
            }
            let measure = match measure {
                Some(id) => t.measures.iter().find(|m| m.id == *id).ok_or_else(|| {
                    let ids: Vec<&str> = t.measures.iter().map(|m| m.id.as_str()).collect();
                    FmError::invalid(format!(
                        "{trait_id} has no measure {id}. Its measures are {}.",
                        ids.join(", ")
                    ))
                })?,
                None => t
                    .measures
                    .first()
                    .ok_or_else(|| FmError::invalid(format!("{trait_id} has no measure.")))?,
            };
            if min.is_none() && max.is_none() {
                return Err(FmError::invalid(format!(
                    "A range on {trait_id} needs a lower or an upper bound."
                )));
            }
            if min.is_some_and(|v| !v.is_finite()) || max.is_some_and(|v| !v.is_finite()) {
                return Err(FmError::invalid(format!(
                    "A bound on {trait_id} is not a number."
                )));
            }
            if let (Some(lo), Some(hi)) = (min, max)
                && lo > hi
            {
                return Err(FmError::invalid(format!(
                    "The range on {trait_id} is empty: {lo} is above {hi}."
                )));
            }
            // The measure id is the catalog's (ASCII letters), never the
            // caller's text, so the path is safe to build.
            let path = format!("$.\"{}\"", measure.id);
            collect(
                conn,
                "SELECT face_id FROM (
                     SELECT face_id, CAST(json_extract(scalars, ?2) AS REAL) AS v
                     FROM glyph_traits WHERE trait = ?1 AND json_valid(scalars))
                 WHERE v IS NOT NULL AND (?3 IS NULL OR v >= ?3) AND (?4 IS NULL OR v <= ?4)",
                params![trait_id, path, min, max],
            )
        }
    }
}

fn collect(conn: &Connection, sql: &str, params: impl rusqlite::Params) -> FmResult<HashSet<u32>> {
    let mut stmt = conn.prepare_cached(sql).map_err(|e| db_error(&e))?;
    stmt.query_map(params, |r| r.get::<_, u32>(0))
        .map_err(|e| db_error(&e))?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|e| db_error(&e))
}

/// What one trait counts in the library.
#[derive(Default)]
struct TraitCount<'a> {
    measured: u32,
    unsure: u32,
    /// Faces and family keys per value id.
    values: HashMap<String, (u32, HashSet<&'a str>)>,
}

impl Index {
    /// The glyph trait catalog in catalog order, each value with how many
    /// searchable faces (and family groups) are detected with it, with
    /// confidence; per trait the faces measured and the unsure ones.
    pub fn list_glyph_traits(&self) -> FmResult<Vec<GlyphTraitInfo>> {
        let catalog = glyph_traits::load()?;
        self.refresh_search_cache()?;
        let cache = self.search_cache.borrow();
        let families: HashMap<u32, &str> = cache
            .rows
            .iter()
            .map(|row| (row.id, row.family_key.as_str()))
            .collect();
        let mut counts: HashMap<String, TraitCount<'_>> = HashMap::new();
        let mut stmt = self
            .conn
            .prepare_cached("SELECT face_id, trait, value, confidence FROM glyph_traits")
            .map_err(|e| db_error(&e))?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, u32>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, f64>(3)?,
                ))
            })
            .map_err(|e| db_error(&e))?;
        for row in rows {
            let (face, trait_id, value, confidence) = row.map_err(|e| db_error(&e))?;
            // Faces of gone files and paused sources are not searchable.
            let Some(family) = families.get(&face) else {
                continue;
            };
            let count = counts.entry(trait_id).or_default();
            count.measured += 1;
            if confidence < CONFIDENT {
                count.unsure += 1;
                continue;
            }
            let (faces, keys) = count.values.entry(value).or_default();
            *faces += 1;
            keys.insert(family);
        }
        Ok(catalog.iter().map(|t| info(t, counts.get(&t.id))).collect())
    }
}

fn info(t: &GlyphTrait, count: Option<&TraitCount<'_>>) -> GlyphTraitInfo {
    let english = |labels: &glyph_traits::Labels| labels.get("en").cloned().unwrap_or_default();
    GlyphTraitInfo {
        id: t.id.clone(),
        script: t.script.clone(),
        glyph: t.glyph.to_string(),
        kind: match t.kind {
            TraitKind::Enum => GlyphTraitKind::Enum,
            TraitKind::Scalar => GlyphTraitKind::Scalar,
        },
        name: english(&t.name),
        description: t.description.clone(),
        enabled: t.enabled,
        measures: t
            .measures
            .iter()
            .map(|m| GlyphTraitMeasure {
                id: m.id.clone(),
                description: m.description.clone(),
                min: m.min,
                max: m.max,
            })
            .collect(),
        values: t
            .values
            .iter()
            .map(|v| {
                let (faces, families) = count
                    .and_then(|c| c.values.get(&v.id))
                    .map_or((0, 0), |(faces, keys)| {
                        (*faces, u32::try_from(keys.len()).unwrap_or(u32::MAX))
                    });
                GlyphTraitValue {
                    id: v.id.clone(),
                    label: english(&v.label),
                    description: v.description.clone(),
                    when: v
                        .when
                        .iter()
                        .map(|w| GlyphTraitThreshold {
                            measure: w.measure.clone(),
                            min: w.min,
                            max: w.max,
                        })
                        .collect(),
                    example: v.example.clone(),
                    faces,
                    families,
                }
            })
            .collect(),
        measured: count.map_or(0, |c| c.measured),
        unsure: count.map_or(0, |c| c.unsure),
    }
}

#[cfg(test)]
mod tests;
