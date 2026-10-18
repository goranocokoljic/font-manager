//! Text search over the index (E3.3): [`Index::search`].
//!
//! The `face_fts` trigram table finds the candidate faces of a query; the
//! core's [`Query`] scores them; hits are grouped by family (the grouping
//! key of [`crate::grouping`]) and ranked. A query whose candidates hold no
//! hit is checked against every face once more: a typo in the middle of a
//! short word shares no trigram with the right word, and still deserves an
//! answer.
//!
//! Filters (E3.4, [`crate::filter`]) apply to the faces that match the
//! text: a family is a result when one of its faces matches and passes every
//! filter. The same pass counts the results of every filter value.
//!
//! Faces of files that are gone or of paused sources are not searched.
//!
//! The searchable rows are read once and kept in memory until the database
//! changes ([`SearchCache`]): this connection's writes move
//! `total_changes`, another process's commits move `PRAGMA data_version`.
//! A query then costs the scoring alone, well inside the 50 ms budget.

use std::collections::{HashMap, HashSet};

use fm_api::activation::ActivationMode;
use fm_api::common::{Axis, FontFormat, Slant};
use fm_api::library::{FamilyActivation, FamilySummary, LicenseType};
use fm_api::search::{FamilyHit, Filters, SearchRequest, SearchResponse};
use fm_api::{CollectionId, FaceId, FamilyId, SourceId, TagId};
use rusqlite::OptionalExtension;

use super::files::format_from_sql;
use super::{Index, db_error};
use crate::error::{FmError, FmResult};
use crate::filter::{FaceFacts, Slots, Tag, Tally, failed, tag, values};
use crate::grouping::{family_key, slant_rank};
use crate::search::{FAMILY, FIELD_COUNT, FOUNDRY, FaceText, Query};

/// Every searchable face with what its family summary needs.
const VISIBLE_FACES: &str = "
SELECT face.id, fts.family, fts.style, fts.postscript_name, fts.foundry, fts.designer,
       fts.tags, fts.file_name, face.weight, face.width, face.slant, face.is_variable,
       face.axes, font_file.format, activation.mode, face.added,
       CASE WHEN face.is_variable AND json_valid(face.parsed)
            THEN json_array_length(face.parsed, '$.namedInstances') END,
       font_file.source_id, face.features, user_meta.license_type,
       coalesce(user_meta.favorite, 0), user_meta.rating,
       face.is_color, face.healthy
FROM face
JOIN face_fts AS fts ON fts.rowid = face.id
JOIN font_file ON font_file.id = face.file_id
JOIN source ON source.id = font_file.source_id
LEFT JOIN activation ON activation.face_id = face.id
LEFT JOIN user_meta ON user_meta.face_id = face.id
WHERE font_file.gone = 0 AND source.paused = 0
ORDER BY face.id";

/// The searchable rows and the database state they were read at.
#[derive(Default)]
pub(super) struct SearchCache {
    stamp: Option<(i64, u64)>,
    rows: Vec<Row>,
    /// The filter values the rows can be counted under.
    slots: Slots,
}

/// One searchable face.
struct Row {
    id: u32,
    text: FaceText,
    /// The family name as written, and its grouping key.
    family: String,
    family_key: String,
    /// What the filters look at.
    facts: FaceFacts,
    /// The [`Slots`] of its filter values.
    slots: Vec<u32>,
    active: Option<ActivationMode>,
    added: Option<String>,
    /// Styles it lists under its family: its named instances, or itself.
    styles: u32,
}

impl Index {
    /// Ranked families for a text query and filters, a page at a time,
    /// with the result count of every filter value.
    ///
    /// The `supports` filter (E3.8) and a smart collection are not applied
    /// yet: they are an [`fm_api::ErrorCode::Unsupported`] error rather than
    /// results that ignore them. An unknown collection is `NotFound`.
    pub fn search(&self, req: &SearchRequest) -> FmResult<SearchResponse> {
        self.check_filters(&req.filters)?;
        let query = Query::parse(&req.query);
        self.refresh_search_cache()?;
        let cache = self.search_cache.borrow();
        let rows = cache.rows.as_slice();
        let candidates = query
            .fts_match()
            .map(|expr| self.fts_candidates(&expr))
            .transpose()?;
        let mut hits = score(&query, rows, candidates.as_ref());
        if hits.is_empty() && candidates.is_some() {
            hits = score(&query, rows, None);
        }

        let mut members: HashMap<&str, Vec<&Row>> = HashMap::new();
        for row in rows {
            if hits.contains_key(row.family_key.as_str()) {
                members.entry(&row.family_key).or_default().push(row);
            }
        }
        let mut tally = Tally::new(&cache.slots);
        let mut families: Vec<FamilyHit> = Vec::with_capacity(hits.len());
        for (number, (key, faces)) in hits.into_iter().enumerate() {
            let number = u32::try_from(number).unwrap_or(u32::MAX);
            let mut faces: Vec<(&Row, f32)> = faces
                .into_iter()
                .filter(|(row, _)| {
                    let failed = failed(&req.filters, &row.text, &row.facts);
                    tally.add(number, failed, &row.slots);
                    failed == 0
                })
                .collect();
            if faces.is_empty() {
                continue;
            }
            let Some(family) = members.get(key).map(|m| summary(m)) else {
                continue;
            };
            faces.sort_by(|(a, sa), (b, sb)| {
                sb.total_cmp(sa)
                    .then(a.text.weight.cmp(&b.text.weight))
                    .then(a.text.width.cmp(&b.text.width))
                    .then(slant_rank(a.text.slant).cmp(&slant_rank(b.text.slant)))
                    .then(a.id.cmp(&b.id))
            });
            families.push(FamilyHit {
                family,
                score: faces.first().map_or(0.0, |(_, s)| *s),
                faces: faces.iter().map(|(row, _)| FaceId(row.id)).collect(),
                missing: None,
            });
        }
        families.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| family_key(&a.family.name).cmp(&family_key(&b.family.name)))
                .then(a.family.id.cmp(&b.family.id))
        });
        let total = u32::try_from(families.len()).unwrap_or(u32::MAX);
        let families = families
            .into_iter()
            .skip(req.page.offset as usize)
            .take(req.page.limit as usize)
            .collect();
        Ok(SearchResponse {
            families,
            total,
            counts: Some(tally.finish()),
        })
    }

    /// Refuses the filters search cannot apply yet, and unknown collections.
    fn check_filters(&self, filters: &Filters) -> FmResult<()> {
        if filters.supports.is_some() {
            return Err(FmError::unsupported(
                "The language and character filter is not available yet.",
            ));
        }
        if let Some(id) = filters.collection {
            let smart: Option<bool> = self
                .conn
                .query_row(
                    "SELECT is_smart FROM collection WHERE id = ?1",
                    [id.0],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| db_error(&e))?;
            match smart {
                None => return Err(FmError::not_found(format!("Collection {id}"))),
                Some(true) => {
                    return Err(FmError::unsupported(
                        "Filtering by a smart collection is not available yet.",
                    ));
                }
                Some(false) => {}
            }
        }
        Ok(())
    }

    /// The ids of the faces that share a trigram with the query.
    fn fts_candidates(&self, expr: &str) -> FmResult<HashSet<u32>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT rowid FROM face_fts WHERE face_fts MATCH ?1")
            .map_err(|e| db_error(&e))?;
        let ids = stmt
            .query_map([expr], |row| row.get::<_, u32>(0))
            .map_err(|e| db_error(&e))?
            .collect::<Result<_, _>>()
            .map_err(|e| db_error(&e))?;
        Ok(ids)
    }

    /// Reads the searchable rows again when the database changed since.
    fn refresh_search_cache(&self) -> FmResult<()> {
        let data_version: i64 = self
            .conn
            .pragma_query_value(None, "data_version", |row| row.get(0))
            .map_err(|e| db_error(&e))?;
        let stamp = Some((data_version, self.conn.total_changes()));
        if self.search_cache.borrow().stamp == stamp {
            return Ok(());
        }
        let mut slots = Slots::default();
        let rows = self.visible_faces(&mut slots)?;
        *self.search_cache.borrow_mut() = SearchCache { stamp, rows, slots };
        Ok(())
    }

    fn visible_faces(&self, slots: &mut Slots) -> FmResult<Vec<Row>> {
        let mut tags = self.pairs("SELECT face_id, tag_id FROM face_tag", TagId)?;
        let mut collections = self.pairs(
            "SELECT face_id, collection_id FROM collection_face",
            CollectionId,
        )?;
        let mut stmt = self
            .conn
            .prepare_cached(VISIBLE_FACES)
            .map_err(|e| db_error(&e))?;
        let mut rows = stmt.query([]).map_err(|e| db_error(&e))?;
        let mut out = Vec::new();
        let mut slants = Names::new(name_of::<Slant>);
        let mut modes = Names::new(name_of::<ActivationMode>);
        let mut formats = Names::new(|f| format_from_sql(f).ok().flatten());
        let mut licenses = Names::new(name_of::<LicenseType>);
        while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
            let text = |i: usize| {
                row.get_ref(i)
                    .and_then(|v| Ok(v.as_str_or_null()?))
                    .map_err(|e| db_error(&e))
            };
            let mut fields: [Option<&str>; FIELD_COUNT] = Default::default();
            for (i, field) in fields.iter_mut().enumerate() {
                *field = text(i + 1)?.filter(|s| !s.is_empty());
            }
            let is_variable: bool = row.get(11).map_err(|e| db_error(&e))?;
            let axes = if is_variable {
                text(12)?
                    .and_then(|json| serde_json::from_str::<Vec<Axis>>(json).ok())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let instances: Option<i64> = row.get(16).map_err(|e| db_error(&e))?;
            let id: u32 = row.get(0).map_err(|e| db_error(&e))?;
            let family = fields[FAMILY].unwrap_or_default().to_owned();
            let slant = text(10)?
                .and_then(|s| slants.get(s))
                .unwrap_or(Slant::Upright);
            let active = text(14)?.and_then(|m| modes.get(m));
            let mut features: Vec<Tag> = text(18)?.map(feature_tags).unwrap_or_default();
            features.sort_unstable();
            let facts = FaceFacts {
                classification: None,
                is_variable,
                color: row.get(22).map_err(|e| db_error(&e))?,
                format: text(13)?.and_then(|f| formats.get(f)),
                features,
                license: text(19)?.and_then(|l| licenses.get(l)),
                foundry: fields[FOUNDRY].map(str::to_owned),
                source: SourceId(row.get(17).map_err(|e| db_error(&e))?),
                tags: tags.remove(&id).unwrap_or_default(),
                collections: collections.remove(&id).unwrap_or_default(),
                active: active.is_some(),
                favorite: row.get(20).map_err(|e| db_error(&e))?,
                rating: row.get(21).map_err(|e| db_error(&e))?,
                problems: !row.get::<_, bool>(23).map_err(|e| db_error(&e))?,
            };
            let face_text = FaceText::new(
                fields,
                row.get(8).map_err(|e| db_error(&e))?,
                row.get(9).map_err(|e| db_error(&e))?,
                slant,
                axes,
            );
            out.push(Row {
                id,
                family_key: family_key(&family),
                family,
                slots: slots.intern(&values(&face_text, &facts)),
                text: face_text,
                facts,
                active,
                added: text(15)?.map(str::to_owned),
                styles: instances
                    .and_then(|n| u32::try_from(n).ok())
                    .unwrap_or(0)
                    .max(1),
            });
        }
        Ok(out)
    }

    /// `(face, id)` pairs from `sql`, by face.
    fn pairs<T>(&self, sql: &str, id: fn(u32) -> T) -> FmResult<HashMap<u32, Vec<T>>> {
        let mut stmt = self.conn.prepare_cached(sql).map_err(|e| db_error(&e))?;
        let mut rows = stmt.query([]).map_err(|e| db_error(&e))?;
        let mut out: HashMap<u32, Vec<T>> = HashMap::new();
        while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
            let face: u32 = row.get(0).map_err(|e| db_error(&e))?;
            out.entry(face)
                .or_default()
                .push(id(row.get(1).map_err(|e| db_error(&e))?));
        }
        Ok(out)
    }
}

/// The four-letter tags of a `face.features` JSON array.
fn feature_tags(json: &str) -> Vec<Tag> {
    // Tags never need JSON escapes; read them in place, owned only if they do.
    match serde_json::from_str::<Vec<&str>>(json) {
        Ok(tags) => tags.into_iter().filter_map(tag).collect(),
        Err(_) => serde_json::from_str::<Vec<String>>(json)
            .unwrap_or_default()
            .iter()
            .filter_map(|t| tag(t))
            .collect(),
    }
}

/// The scored faces by family key. `candidates` limits the faces looked at.
fn score<'a>(
    query: &Query,
    rows: &'a [Row],
    candidates: Option<&HashSet<u32>>,
) -> HashMap<&'a str, Vec<(&'a Row, f32)>> {
    let mut hits: HashMap<&str, Vec<(&Row, f32)>> = HashMap::new();
    for row in rows {
        if candidates.is_some_and(|c| !c.contains(&row.id)) {
            continue;
        }
        let score = if query.is_empty() {
            Some(0.0)
        } else {
            query.score(&row.text)
        };
        if let Some(score) = score {
            hits.entry(&row.family_key).or_default().push((row, score));
        }
    }
    hits
}

/// The family row of `faces`, every visible face of one family in id order.
fn summary(faces: &[&Row]) -> FamilySummary {
    let first = faces.first();
    let mut formats: Vec<FontFormat> = Vec::new();
    for format in faces.iter().filter_map(|f| f.facts.format) {
        if !formats.contains(&format) {
            formats.push(format);
        }
    }
    let active = faces.iter().filter(|f| f.active.is_some()).count();
    FamilySummary {
        id: FamilyId(first.map_or(0, |f| f.id)),
        name: first.map(|f| f.family.clone()).unwrap_or_default(),
        style_count: faces.iter().map(|f| f.styles).sum(),
        formats,
        foundry: faces.iter().find_map(|f| f.facts.foundry.clone()),
        classification: None,
        is_variable: faces.iter().any(|f| f.facts.is_variable),
        activation: match active {
            0 => FamilyActivation::Inactive,
            n if n == faces.len() => FamilyActivation::Active,
            _ => FamilyActivation::Partial,
        },
        added: faces
            .iter()
            .filter_map(|f| f.added.clone())
            .min()
            .unwrap_or_default(),
        // The search rows do not carry these yet; the detail list reads
        // `list_families`, which fills them.
        styles: Vec::new(),
        weight_range: None,
        is_color: false,
        version: None,
        location: None,
        open_license: None,
        declared_license: None,
        tags: Vec::new(),
    }
}

/// Names read from the rows, each parsed once per read: a library has a
/// handful of distinct slants, formats and modes over 10,000 rows.
struct Names<T> {
    parse: fn(&str) -> Option<T>,
    seen: Vec<(String, Option<T>)>,
}

impl<T: Copy> Names<T> {
    fn new(parse: fn(&str) -> Option<T>) -> Self {
        Self {
            parse,
            seen: Vec::new(),
        }
    }

    fn get(&mut self, name: &str) -> Option<T> {
        if let Some((_, value)) = self.seen.iter().find(|(n, _)| n == name) {
            return *value;
        }
        let value = (self.parse)(name);
        self.seen.push((name.to_owned(), value));
        value
    }
}

/// A unit enum from its serde name (`italic`, `session`).
fn name_of<T: serde::de::DeserializeOwned>(name: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(name.to_owned())).ok()
}

#[cfg(test)]
mod tests;
