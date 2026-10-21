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
//! The `supports` filter (E3.8) checks each face's `cmap` against the
//! characters asked for ([`Need`]). A face that lacks at most
//! [`NEAR_MISS`](crate::coverage::NEAR_MISS) of them is still a result: it lists what it lacks, and its
//! family ranks after the families with a complete face.
//!
//! Faces of files that are gone or of paused sources are not searched.
//!
//! A `collection` filter naming a smart collection (E14.2) applies its rule
//! ([`smart`]), compiled once per search.
//!
//! The searchable rows are read once and kept in memory until the database
//! changes ([`SearchCache`]): this connection's writes move
//! `total_changes`, another process's commits move `PRAGMA data_version`.
//! A query then costs the scoring alone, well inside the 50 ms budget.

use std::collections::{HashMap, HashSet};

use fm_api::activation::ActivationMode;
use fm_api::common::Paged;
use fm_api::common::{Axis, FontFormat, Slant};
use fm_api::library::{
    FamilyActivation, FamilySort, FamilySummary, LicenseType, ListFamiliesRequest, OpenLicense,
    StyleMark,
};
use fm_api::search::{FamilyHit, Missing, SearchRequest, SearchResponse, WeightRange};
use fm_api::{CollectionId, FaceId, FamilyId, SourceId, TagId};

use super::files::format_from_sql;
use super::{Index, db_error};
use crate::coverage::{CodePointSet, Fit};
use crate::error::{FmError, FmResult};
use crate::filter::{
    FaceFacts, Slots, Tag, Tally, failed, failed_added, failed_supports, tag, values,
    with_collection,
};
use crate::grouping::{family_key, slant_rank};
use crate::license;
use crate::search::{FAMILY, FIELD_COUNT, FOUNDRY, FaceText, Query};
use smart::Member;

/// Every searchable face with what its family summary needs.
const VISIBLE_FACES: &str = "
SELECT face.id, fts.family, fts.style, fts.postscript_name, fts.foundry, fts.designer,
       fts.tags, fts.file_name, face.weight, face.width, face.slant, face.is_variable,
       face.axes, font_file.format, activation.mode, face.added,
       CASE WHEN face.is_variable AND json_valid(face.parsed)
            THEN json_array_length(face.parsed, '$.namedInstances') END,
       font_file.source_id, face.features, user_meta.license_type,
       coalesce(user_meta.favorite, 0), user_meta.rating,
       face.is_color, face.healthy, face.cmap,
       CASE WHEN json_valid(face.parsed)
            THEN json_extract(face.parsed, '$.loclLanguages') END,
       CASE WHEN typeof(font_file.path) = 'text' THEN font_file.path END,
       CASE WHEN json_valid(face.parsed) THEN json_extract(face.parsed, '$.version') END,
       face.open_license, face.fstype
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
    /// Tag names by id, for the family summaries.
    tag_names: HashMap<TagId, String>,
    /// Family notes by grouping key, for the family summaries.
    family_notes: HashMap<String, String>,
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
    /// The code points it maps; `None` when not stored (indexed before E3.7)
    /// or damaged.
    cmap: Option<CodePointSet>,
    /// Its `locl` language systems (`cyrl/SRB`).
    locl: Vec<String>,
    /// The folder of its file; `None` for a path that is not UTF-8.
    location: Option<String>,
    /// Its version string (name ID 5).
    version: Option<String>,
    /// The open license its name table states.
    open_license: Option<OpenLicense>,
}

impl Index {
    /// Ranked families for a text query and filters, a page at a time,
    /// with the result count of every filter value.
    ///
    /// With a `supports` filter every hit says what its best face misses
    /// ([`FamilyHit::missing`]); an unknown language preset is
    /// `InvalidRequest`. A smart collection applies its rule; one whose
    /// rule leads back to itself is `InvalidRequest`. An unknown collection
    /// is `NotFound`.
    pub fn search(&self, req: &SearchRequest) -> FmResult<SearchResponse> {
        let member = self.collection_member(req.filters.collection)?;
        let need = smart::need(&req.filters)?;
        let cutoff = smart::cutoff(&self.conn, req.filters.added_within_days)?;
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
        // Families are ranked on what the hits carry; the summary, the
        // costly part, is built for the requested page only.
        let mut ranked: Vec<Ranked<'_>> = Vec::with_capacity(hits.len());
        for (number, (key, faces)) in hits.into_iter().enumerate() {
            let number = u32::try_from(number).unwrap_or(u32::MAX);
            let mut faces: Vec<(&Row, f32, Option<Fit>)> = faces
                .into_iter()
                .filter_map(|(row, score)| {
                    let mut failed = failed(&req.filters, &row.text, &row.facts)
                        | failed_added(cutoff.as_deref(), row.added.as_deref());
                    if let Some(member) = &member {
                        failed = with_collection(failed, member.holds(row, &[]));
                    }
                    let fit = need.as_ref().and_then(|need| {
                        let fit = row.cmap.as_ref().map(|set| need.check(set, &row.locl));
                        failed |= failed_supports(fit.as_ref());
                        fit
                    });
                    tally.add(number, failed, &row.slots);
                    (failed == 0).then_some((row, score, fit))
                })
                .collect();
            if faces.is_empty() {
                continue;
            }
            let Some(members) = members.get(key) else {
                continue;
            };
            let Some(first) = members.first() else {
                continue;
            };
            faces.sort_by(|(a, sa, fa), (b, sb, fb)| {
                lacking(fa.as_ref())
                    .cmp(&lacking(fb.as_ref()))
                    .then(sb.total_cmp(sa))
                    .then(a.text.weight.cmp(&b.text.weight))
                    .then(a.text.width.cmp(&b.text.width))
                    .then(slant_rank(a.text.slant).cmp(&slant_rank(b.text.slant)))
                    .then(a.id.cmp(&b.id))
            });
            ranked.push(Ranked {
                key,
                id: first.id,
                members,
                score: faces.first().map_or(0.0, |(_, s, _)| *s),
                missing: faces
                    .first()
                    .and_then(|(_, _, fit)| fit.as_ref())
                    .map(|fit| Missing {
                        chars: fit.chars.iter().map(char::to_string).collect(),
                        has_locl: fit.has_locl,
                    }),
                faces: faces.iter().map(|(row, _, _)| FaceId(row.id)).collect(),
            });
        }
        // `key` is the family key of the family's name: the first member's.
        ranked.sort_by(|a, b| {
            let lacks = |hit: &Ranked<'_>| hit.missing.as_ref().map_or(0, |m| m.chars.len());
            lacks(a)
                .cmp(&lacks(b))
                .then(b.score.total_cmp(&a.score))
                .then_with(|| a.key.cmp(b.key))
                .then(a.id.cmp(&b.id))
        });
        let total = u32::try_from(ranked.len()).unwrap_or(u32::MAX);
        let families = ranked
            .into_iter()
            .skip(req.page.offset as usize)
            .take(req.page.limit as usize)
            .map(|hit| FamilyHit {
                family: summary(hit.members, &cache),
                faces: hit.faces,
                score: hit.score,
                missing: hit.missing,
            })
            .collect();
        Ok(SearchResponse {
            families,
            total,
            counts: Some(tally.finish()),
        })
    }

    /// The test for the collection a search is filtered by; `NotFound` for
    /// an unknown one, `InvalidRequest` for a rule that leads back to itself.
    fn collection_member(&self, id: Option<CollectionId>) -> FmResult<Option<Member>> {
        let Some(id) = id else {
            return Ok(None);
        };
        let rules = smart::load_rules(&self.conn)?;
        if !rules.contains_key(&id) {
            return Err(FmError::not_found(format!("Collection {id}")));
        }
        Member::compile(&self.conn, &rules, id).map(Some)
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

    /// The visible faces of the family `family`, in id order, as search
    /// groups them; `None` when no family has that id.
    pub(super) fn family_faces(&self, family: FamilyId) -> FmResult<Option<Vec<FaceId>>> {
        self.refresh_search_cache()?;
        let cache = self.search_cache.borrow();
        let Some(first) = cache.rows.iter().find(|r| r.id == family.0) else {
            return Ok(None);
        };
        let faces: Vec<FaceId> = cache
            .rows
            .iter()
            .filter(|r| r.family_key == first.family_key)
            .map(|r| FaceId(r.id))
            .collect();
        // A family's id is the id of its first face; another face's id is not one.
        Ok((faces.first() == Some(&FaceId(family.0))).then_some(faces))
    }

    /// One page of families: every visible family, or those with a face in
    /// `req.source`, sorted by `req.order` when set, else by `req.sort`
    /// (name A to Z, newest first, most styles first); ties go by id.
    pub fn list_families(&self, req: &ListFamiliesRequest) -> FmResult<Paged<FamilySummary>> {
        self.refresh_search_cache()?;
        let cache = self.search_cache.borrow();
        let mut order: Vec<&str> = Vec::new();
        let mut members: HashMap<&str, Vec<&Row>> = HashMap::new();
        for row in &cache.rows {
            members
                .entry(&row.family_key)
                .or_insert_with(|| {
                    order.push(&row.family_key);
                    Vec::new()
                })
                .push(row);
        }
        let mut rows: Vec<FamilySummary> = order
            .iter()
            .filter_map(|key| members.get(key))
            .filter(|faces| {
                req.source
                    .is_none_or(|s| faces.iter().any(|f| f.facts.source == s))
            })
            .map(|faces| summary(faces, &cache))
            .collect();
        match (req.order, req.sort) {
            (Some(order), _) => order.sort(&mut rows),
            (None, FamilySort::Name) => {
                rows.sort_by_cached_key(|r| (r.name.to_lowercase(), r.id));
            }
            (None, FamilySort::DateAdded) => {
                rows.sort_by(|a, b| b.added.cmp(&a.added).then(a.id.cmp(&b.id)));
            }
            (None, FamilySort::StyleCount) => {
                rows.sort_by(|a, b| b.style_count.cmp(&a.style_count).then(a.id.cmp(&b.id)));
            }
        }
        let total = u32::try_from(rows.len()).unwrap_or(u32::MAX);
        let items = rows
            .into_iter()
            .skip(req.page.offset as usize)
            .take(req.page.limit as usize)
            .collect();
        Ok(Paged { items, total })
    }

    /// The summary of the family `family`; `None` when no visible family has
    /// that id.
    pub(super) fn family_summary(&self, family: FamilyId) -> FmResult<Option<FamilySummary>> {
        self.refresh_search_cache()?;
        let cache = self.search_cache.borrow();
        let Some(first) = cache.rows.iter().find(|r| r.id == family.0) else {
            return Ok(None);
        };
        let faces: Vec<&Row> = cache
            .rows
            .iter()
            .filter(|r| r.family_key == first.family_key)
            .collect();
        // A family's id is the id of its first face; another face's id is not one.
        if faces.first().map(|r| r.id) != Some(family.0) {
            return Ok(None);
        }
        Ok(Some(summary(&faces, &cache)))
    }

    /// The family a visible face belongs to, by id and name, as search
    /// groups it; `None` for a face that is not visible (its file is gone,
    /// its source paused).
    pub(super) fn family_of(&self, face: FaceId) -> FmResult<Option<(FamilyId, String)>> {
        self.refresh_search_cache()?;
        let cache = self.search_cache.borrow();
        let Some(row) = cache.rows.iter().find(|r| r.id == face.0) else {
            return Ok(None);
        };
        Ok(cache
            .rows
            .iter()
            .find(|r| r.family_key == row.family_key)
            .map(|first| (FamilyId(first.id), first.family.clone())))
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
        let tag_names = self.tag_names()?;
        let family_notes = self.all_family_notes()?;
        *self.search_cache.borrow_mut() = SearchCache {
            stamp,
            rows,
            slots,
            tag_names,
            family_notes,
        };
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
        let mut open_licenses = Names::new(name_of::<OpenLicense>);
        // A family's declared license (E15.2) is every one of its faces'.
        let declared = self.all_declared_types()?;
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
            let key = family_key(&family);
            let slant = text(10)?
                .and_then(|s| slants.get(s))
                .unwrap_or(Slant::Upright);
            let active = text(14)?.and_then(|m| modes.get(m));
            let mut features: Vec<Tag> = text(18)?.map(feature_tags).unwrap_or_default();
            features.sort_unstable();
            let open_license = text(28)?.and_then(|l| open_licenses.get(l));
            let facts = FaceFacts {
                classification: None,
                is_variable,
                color: row.get(22).map_err(|e| db_error(&e))?,
                format: text(13)?.and_then(|f| formats.get(f)),
                features,
                license: declared
                    .get(&key)
                    .copied()
                    .or(text(19)?.and_then(|l| licenses.get(l))),
                foundry: fields[FOUNDRY].map(str::to_owned),
                source: SourceId(row.get(17).map_err(|e| db_error(&e))?),
                tags: tags.remove(&id).unwrap_or_default(),
                collections: collections.remove(&id).unwrap_or_default(),
                active: active.is_some(),
                favorite: row.get(20).map_err(|e| db_error(&e))?,
                rating: row.get(21).map_err(|e| db_error(&e))?,
                problems: !row.get::<_, bool>(23).map_err(|e| db_error(&e))?,
                embedding: license::embedding(row.get(29).map_err(|e| db_error(&e))?),
                open_license,
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
                family_key: key,
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
                cmap: row
                    .get_ref(24)
                    .and_then(|v| Ok(v.as_blob_or_null()?))
                    .map_err(|e| db_error(&e))?
                    .and_then(CodePointSet::from_bytes),
                locl: text(25)?
                    .and_then(|json| serde_json::from_str(json).ok())
                    .unwrap_or_default(),
                location: text(26)?
                    .and_then(|path| std::path::Path::new(path).parent())
                    .map(|dir| dir.display().to_string()),
                version: text(27)?.map(str::to_owned),
                open_license,
            });
        }
        Ok(out)
    }

    /// Every tag's name by id.
    fn tag_names(&self) -> FmResult<HashMap<TagId, String>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT id, name FROM tag")
            .map_err(|e| db_error(&e))?;
        let names = stmt
            .query_map([], |row| Ok((TagId(row.get(0)?), row.get(1)?)))
            .map_err(|e| db_error(&e))?
            .collect::<Result<_, _>>()
            .map_err(|e| db_error(&e))?;
        Ok(names)
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

/// A family that passed the search, before its summary is built.
struct Ranked<'a> {
    /// Its grouping key.
    key: &'a str,
    /// Its id: the id of its first visible face.
    id: u32,
    /// Every visible face of the family, in id order.
    members: &'a [&'a Row],
    score: f32,
    missing: Option<Missing>,
    /// The faces that matched, best first.
    faces: Vec<FaceId>,
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

/// Characters a face lacks of the search's need, for ordering: 0 without a
/// `supports` filter.
fn lacking(fit: Option<&Fit>) -> usize {
    fit.map_or(0, |f| f.missing)
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
/// `cache` names the tags of [`FamilySummary::tags`] and holds the notes.
fn summary(faces: &[&Row], cache: &SearchCache) -> FamilySummary {
    let first = faces.first();
    let mut formats: Vec<FontFormat> = Vec::new();
    for format in faces.iter().filter_map(|f| f.facts.format) {
        if !formats.contains(&format) {
            formats.push(format);
        }
    }
    let active = faces.iter().filter(|f| f.active.is_some()).count();
    let mut styles: Vec<StyleMark> = faces
        .iter()
        .map(|f| StyleMark {
            weight: f.text.weight,
            italic: f.text.slant != Slant::Upright,
        })
        .collect();
    styles.sort_by_key(|s| (s.weight, s.italic));
    styles.dedup();
    let weight_range = faces.iter().map(|f| f.text.weight).fold(None, |range, w| {
        Some(
            range.map_or(WeightRange { min: w, max: w }, |r: WeightRange| {
                WeightRange {
                    min: r.min.min(w),
                    max: r.max.max(w),
                }
            }),
        )
    });
    let mut tags: Vec<String> = Vec::new();
    for name in faces
        .iter()
        .flat_map(|f| &f.facts.tags)
        .filter_map(|id| cache.tag_names.get(id))
    {
        if !tags.contains(name) {
            tags.push(name.clone());
        }
    }
    tags.sort_by_key(|t| t.to_lowercase());
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
        styles,
        weight_range,
        is_color: faces.iter().any(|f| f.facts.color),
        version: faces.iter().find_map(|f| f.version.clone()),
        location: first.and_then(|f| f.location.clone()),
        open_license: faces.iter().find_map(|f| f.open_license),
        declared_license: faces.iter().find_map(|f| f.facts.license),
        tags,
        notes: first.and_then(|f| cache.family_notes.get(&f.family_key).cloned()),
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

pub(super) mod smart;

#[cfg(test)]
mod tests;
