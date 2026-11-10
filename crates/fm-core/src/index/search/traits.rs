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
//! The rows are read once per database change, with the search rows
//! ([`Detections`], kept in the search cache), each value as its place in
//! the catalog and each measure as a number: a filter is then one pass over
//! one trait's detections in memory, inside the 50 ms search budget. A
//! query per filter, with the measures parsed from JSON on every search,
//! was not.

use std::collections::{HashMap, HashSet};

use fm_api::search::{
    GlyphTraitInfo, GlyphTraitKind, GlyphTraitMeasure, GlyphTraitThreshold, GlyphTraitValue,
    TraitFilter,
};
use rusqlite::Connection;

use super::{Index, db_error};
use crate::error::{FmError, FmResult};
use crate::glyph_traits::{self, GlyphTrait, TraitKind};
use crate::glyphs::traits::CONFIDENT;

/// Every stored detection, by trait id, in catalog terms.
#[derive(Debug, Default)]
pub(super) struct Detections {
    by_trait: HashMap<String, Vec<Detected>>,
}

/// One face's detection of one trait.
#[derive(Debug)]
struct Detected {
    face: u32,
    /// The value's place in the catalog trait's values; `None` for a value
    /// the catalog does not list (a detector newer than the catalog).
    value: Option<u8>,
    confidence: f64,
    /// The measures in the catalog trait's order; NaN where not measured.
    measures: Box<[f64]>,
}

impl Detections {
    /// Reads every `glyph_traits` row once. A row of a trait the catalog
    /// does not list is left out; measures that are not a JSON object of
    /// numbers read as not measured.
    fn read(conn: &Connection) -> FmResult<Self> {
        let catalog = glyph_traits::load()?;
        let mut stmt = conn
            .prepare_cached("SELECT face_id, trait, value, confidence, scalars FROM glyph_traits")
            .map_err(|e| db_error(&e))?;
        let mut rows = stmt.query([]).map_err(|e| db_error(&e))?;
        let mut by_trait: HashMap<String, Vec<Detected>> = HashMap::new();
        while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
            let e = |e: rusqlite::Error| db_error(&e);
            let trait_id: String = row.get(1).map_err(e)?;
            let Some(t) = catalog.iter().find(|t| t.id == trait_id) else {
                continue;
            };
            let value: String = row.get(2).map_err(e)?;
            let scalars: Option<String> = row.get(4).map_err(e)?;
            let parsed: Option<serde_json::Map<String, serde_json::Value>> =
                scalars.and_then(|json| serde_json::from_str(&json).ok());
            let measures = t
                .measures
                .iter()
                .map(|m| {
                    parsed
                        .as_ref()
                        .and_then(|map| map.get(&m.id))
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(f64::NAN)
                })
                .collect();
            by_trait.entry(trait_id).or_default().push(Detected {
                face: row.get(0).map_err(e)?,
                value: t
                    .values
                    .iter()
                    .position(|v| v.id == value)
                    .and_then(|at| u8::try_from(at).ok()),
                confidence: row.get(3).map_err(e)?,
                measures,
            });
        }
        Ok(Self { by_trait })
    }

    fn of(&self, trait_id: &str) -> &[Detected] {
        self.by_trait.get(trait_id).map_or(&[], Vec::as_slice)
    }
}

/// What one filter asks, checked against the catalog.
enum Wanted<'a> {
    Value(&'a str, u8),
    Range(&'a str, usize, Option<f64>, Option<f64>),
}

/// Checks `filters` against the catalog, before anything is read: an
/// unknown trait, value or measure, a range on an enum trait, or a range
/// without a bound is `InvalidRequest`.
fn check(filters: &[TraitFilter]) -> FmResult<Vec<Wanted<'_>>> {
    filters.iter().map(wanted).collect()
}

impl Index {
    /// The faces that pass every filter of `filters`; `None` when there is
    /// no filter, so every face passes. Checks the filters first, so a bad
    /// one is `InvalidRequest` before anything is read; reads the
    /// detections once per database change. Call it after
    /// `refresh_search_cache`, which forgets them on a change.
    pub(super) fn trait_faces(&self, filters: &[TraitFilter]) -> FmResult<Option<HashSet<u32>>> {
        let wanted = check(filters)?;
        if wanted.is_empty() {
            return Ok(None);
        }
        self.read_detections()?;
        let cache = self.search_cache.borrow();
        let detections = cache
            .detections
            .as_ref()
            .ok_or_else(|| FmError::internal("glyph-trait detections were not read"))?;
        Ok(Some(passing(detections, &wanted)))
    }

    /// Reads the detections into the search cache unless they are there.
    fn read_detections(&self) -> FmResult<()> {
        if self.search_cache.borrow().detections.is_none() {
            let detections = Detections::read(&self.conn)?;
            self.search_cache.borrow_mut().detections = Some(detections);
        }
        Ok(())
    }
}

/// The faces of `detections` that pass every one of `wanted`.
fn passing(detections: &Detections, wanted: &[Wanted<'_>]) -> HashSet<u32> {
    let mut passing: Option<HashSet<u32>> = None;
    for w in wanted {
        let faces: HashSet<u32> = match *w {
            Wanted::Value(trait_id, at) => detections
                .of(trait_id)
                .iter()
                .filter(|d| d.value == Some(at) && d.confidence >= CONFIDENT)
                .map(|d| d.face)
                .collect(),
            Wanted::Range(trait_id, measure, min, max) => detections
                .of(trait_id)
                .iter()
                .filter(|d| {
                    d.measures.get(measure).is_some_and(|&v| {
                        !v.is_nan() && min.is_none_or(|lo| v >= lo) && max.is_none_or(|hi| v <= hi)
                    })
                })
                .map(|d| d.face)
                .collect(),
        };
        passing = Some(match passing {
            None => faces,
            Some(before) => before.intersection(&faces).copied().collect(),
        });
    }
    passing.unwrap_or_default()
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

/// What one filter asks, or why it cannot be asked.
fn wanted(filter: &TraitFilter) -> FmResult<Wanted<'_>> {
    match filter {
        TraitFilter::Value { trait_id, value } => {
            let t = catalog_trait(trait_id)?;
            let at = t
                .values
                .iter()
                .position(|v| v.id == *value)
                .and_then(|at| u8::try_from(at).ok())
                .ok_or_else(|| {
                    let values: Vec<&str> = t.values.iter().map(|v| v.id.as_str()).collect();
                    FmError::invalid(format!(
                        "{trait_id} has no value {value}. Its values are {}.",
                        values.join(", ")
                    ))
                })?;
            Ok(Wanted::Value(trait_id, at))
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
            let at = match measure {
                Some(id) => t.measures.iter().position(|m| m.id == *id).ok_or_else(|| {
                    let ids: Vec<&str> = t.measures.iter().map(|m| m.id.as_str()).collect();
                    FmError::invalid(format!(
                        "{trait_id} has no measure {id}. Its measures are {}.",
                        ids.join(", ")
                    ))
                })?,
                None if t.measures.is_empty() => {
                    return Err(FmError::invalid(format!("{trait_id} has no measure.")));
                }
                None => 0,
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
            Ok(Wanted::Range(trait_id, at, *min, *max))
        }
    }
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
        self.read_detections()?;
        let cache = self.search_cache.borrow();
        let families: HashMap<u32, &str> = cache
            .rows
            .iter()
            .map(|row| (row.id, row.family_key.as_str()))
            .collect();
        let detections = cache
            .detections
            .as_ref()
            .ok_or_else(|| FmError::internal("glyph-trait detections were not read"))?;
        let mut counts: HashMap<String, TraitCount<'_>> = HashMap::new();
        for t in catalog {
            for d in detections.of(&t.id) {
                // Faces of gone files and paused sources are not searchable.
                let Some(family) = families.get(&d.face) else {
                    continue;
                };
                let count = counts.entry(t.id.clone()).or_default();
                count.measured += 1;
                if d.confidence < CONFIDENT {
                    count.unsure += 1;
                    continue;
                }
                let Some(value) = d.value.and_then(|at| t.values.get(usize::from(at))) else {
                    continue;
                };
                let (faces, keys) = count.values.entry(value.id.clone()).or_default();
                *faces += 1;
                keys.insert(family);
            }
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
