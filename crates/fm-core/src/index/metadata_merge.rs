//! The kept copy takes over the metadata (E56.5, ADR 0027): before a copy
//! leaves the library, the faces of the copy that stays take over what the
//! user put on the matching faces of that copy.
//!
//! - **Faces match by PostScript name**, so a `.ttc` and loose files match
//!   face by face. A face of the leaving copy without a match keeps its
//!   data on that copy; it comes back with undo.
//! - **Sets join**: tags, manual collections and project sets. Favorite is
//!   on when any copy has it.
//! - **Single values** (rating, the face's license with its seats, notes)
//!   keep the kept face's value when it has one and take a leaving face's
//!   when it has none. Where two copies hold different values nothing is
//!   guessed: [`MergePlan::conflicts`] lists both, the kept value (or, when
//!   the kept face has none, the first leaving copy's) is the default, and
//!   [`MergePlan::choose`] takes the other one.
//! - **The log.** Everything the merge wrote is a `resolution_merge` row
//!   with the value before and after, written in the transaction of the
//!   resolution's log ([`Index::insert_resolution`]). Undo takes back a
//!   row only while the face still holds what the merge wrote, so a change
//!   the user made after the cleanup stays.
//! - **Sidecars.** The kept faces' writes mark them dirty through the
//!   triggers (schema V7); the leaving faces are marked too, so the
//!   sidecars of both sources are written through the sidecar path (E7.3).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use fm_api::FaceId;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use super::{Index, db_error};
use crate::error::{FmError, FmResult};

/// What the user put on one face.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FaceMeta {
    /// Favorite.
    pub favorite: bool,
    /// 1–5 stars.
    pub rating: Option<u8>,
    /// Notes.
    pub notes: Option<String>,
    /// The face's license: its type as stored (the `fm-api` serde name) and
    /// its seats. `None` when neither is set.
    pub license: Option<(Option<String>, Option<u32>)>,
    /// Tags, by id, with their names.
    pub tags: BTreeMap<u32, String>,
    /// Manual collections.
    pub collections: BTreeSet<u32>,
    /// Project sets.
    pub sets: BTreeSet<u32>,
}

/// A field with one value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MergeField {
    /// The rating.
    Rating,
    /// The face's license and seats, one value.
    License,
    /// The notes.
    Notes,
}

/// A value of a [`MergeField`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeValue {
    /// A rating.
    Rating(u8),
    /// A license type (as stored) and seats.
    License(Option<String>, Option<u32>),
    /// Notes.
    Notes(String),
}

/// Two copies with different values in one field: nothing is guessed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeConflict {
    /// The kept face the value is for.
    pub kept_face: FaceId,
    /// The field.
    pub field: MergeField,
    /// The face whose value is the default: the kept face when it has one.
    pub default_from: FaceId,
    /// The default value.
    pub default: MergeValue,
    /// The face with the other value.
    pub other_face: FaceId,
    /// The other value.
    pub other: MergeValue,
}

/// The user takes the value of `take` for `field` of `kept_face`, instead
/// of the default of its conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergeChoice {
    /// The kept face.
    pub kept_face: FaceId,
    /// The field.
    pub field: MergeField,
    /// The face whose value is taken: the `other_face` of a conflict.
    pub take: FaceId,
}

/// What the merge adds to one kept face.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceMerge {
    /// The kept face.
    pub kept: FaceId,
    /// The leaving faces it takes over from, in request order.
    pub from: Vec<FaceId>,
    /// Tags it gets.
    pub tags: Vec<u32>,
    /// Manual collections it joins.
    pub collections: Vec<u32>,
    /// Sets it joins.
    pub sets: Vec<u32>,
    /// It becomes a favorite.
    pub favorite: bool,
    /// The rating to write.
    pub rating: Option<u8>,
    /// The license to write.
    pub license: Option<(Option<String>, Option<u32>)>,
    /// The notes to write.
    pub notes: Option<String>,
}

/// What a resolution does to the kept copy's metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergePlan {
    /// One entry per kept face that takes something over.
    pub faces: Vec<FaceMerge>,
    /// Every field where two copies differ.
    pub conflicts: Vec<MergeConflict>,
}

impl MergePlan {
    /// Takes the other value of each choice's conflict. A choice that names
    /// no conflict of this plan is `InvalidRequest`.
    pub fn choose(&mut self, choices: &[MergeChoice]) -> FmResult<()> {
        for choice in choices {
            let conflict = self
                .conflicts
                .iter()
                .find(|c| {
                    c.kept_face == choice.kept_face
                        && c.field == choice.field
                        && c.other_face == choice.take
                })
                .ok_or_else(|| {
                    FmError::invalid(format!(
                        "Face {} has no conflict on that field with face {}.",
                        choice.kept_face, choice.take
                    ))
                })?;
            let other = conflict.other.clone();
            let Some(face) = self.faces.iter_mut().find(|f| f.kept == choice.kept_face) else {
                continue;
            };
            match other {
                MergeValue::Rating(r) => face.rating = Some(r),
                MergeValue::License(t, s) => face.license = Some((t, s)),
                MergeValue::Notes(n) => face.notes = Some(n),
            }
        }
        Ok(())
    }

    /// The plan adds nothing to any face.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.faces.iter().all(|f| {
            f.tags.is_empty()
                && f.collections.is_empty()
                && f.sets.is_empty()
                && !f.favorite
                && f.rating.is_none()
                && f.license.is_none()
                && f.notes.is_none()
        })
    }
}

/// A `user_meta` row: favorite, rating, notes, license type, seats.
type MetaRow = (
    bool,
    Option<u8>,
    Option<String>,
    Option<String>,
    Option<u32>,
);

const META: &str = "
SELECT favorite, rating, notes, license_type, seats FROM user_meta WHERE face_id = ?1";

const TAGS: &str = "
SELECT tag.id, tag.name FROM face_tag JOIN tag ON tag.id = face_tag.tag_id
WHERE face_tag.face_id = ?1";

const COLLECTIONS: &str = "
SELECT collection_face.collection_id FROM collection_face
JOIN collection ON collection.id = collection_face.collection_id
WHERE collection_face.face_id = ?1 AND collection.is_smart = 0";

const SETS: &str = "SELECT set_id FROM set_face WHERE face_id = ?1";

/// What the user put on `face`.
fn read_meta(conn: &Connection, face: FaceId) -> FmResult<FaceMeta> {
    let e = |e: rusqlite::Error| db_error(&e);
    let mut meta = FaceMeta::default();
    let row: Option<MetaRow> = conn
        .prepare_cached(META)
        .map_err(e)?
        .query_row([face.0], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .optional()
        .map_err(e)?;
    if let Some((favorite, rating, notes, license, seats)) = row {
        meta.favorite = favorite;
        meta.rating = rating;
        meta.notes = notes.filter(|n| !n.is_empty());
        meta.license = (license.is_some() || seats.is_some()).then_some((license, seats));
    }
    meta.tags = conn
        .prepare_cached(TAGS)
        .map_err(e)?
        .query_map([face.0], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(e)?
        .collect::<Result<_, _>>()
        .map_err(e)?;
    let ids = |sql: &str| -> FmResult<BTreeSet<u32>> {
        conn.prepare_cached(sql)
            .map_err(e)?
            .query_map([face.0], |r| r.get(0))
            .map_err(e)?
            .collect::<Result<_, _>>()
            .map_err(e)
    };
    meta.collections = ids(COLLECTIONS)?;
    meta.sets = ids(SETS)?;
    Ok(meta)
}

/// Each face's PostScript name and index in its file.
fn names(conn: &Connection, faces: &[FaceId]) -> FmResult<Vec<(FaceId, String, u32)>> {
    let mut stmt = conn
        .prepare_cached("SELECT postscript_name, collection_index FROM face WHERE id = ?1")
        .map_err(|e| db_error(&e))?;
    let mut out = Vec::with_capacity(faces.len());
    for face in faces {
        let row: Option<(String, u32)> = stmt
            .query_row([face.0], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()
            .map_err(|e| db_error(&e))?;
        let (name, index) = row.ok_or_else(|| FmError::not_found(format!("Face {face}")))?;
        out.push((*face, name, index));
    }
    Ok(out)
}

/// [`value`], for the tests of the resolver.
#[cfg(test)]
pub(crate) fn metadata_value_for_tests(meta: &FaceMeta, field: MergeField) -> Option<MergeValue> {
    value(meta, field)
}

/// A single value of a field, for comparing and listing.
fn value(meta: &FaceMeta, field: MergeField) -> Option<MergeValue> {
    match field {
        MergeField::Rating => meta.rating.map(MergeValue::Rating),
        MergeField::License => meta.license.clone().map(|(t, s)| MergeValue::License(t, s)),
        MergeField::Notes => meta.notes.clone().map(MergeValue::Notes),
    }
}

impl Index {
    /// Sets a face's own license type (as stored) and seats; nothing in the
    /// app writes them yet, so the tests do.
    #[cfg(test)]
    pub(crate) fn set_face_license_for_tests(
        &mut self,
        face: FaceId,
        license: &str,
        seats: Option<u32>,
    ) -> FmResult<()> {
        self.conn
            .execute(
                "INSERT INTO user_meta (face_id, license_type, seats) VALUES (?1, ?2, ?3)
                 ON CONFLICT (face_id) DO UPDATE SET license_type = ?2, seats = ?3",
                params![face.0, license, seats],
            )
            .map_err(|e| db_error(&e))?;
        Ok(())
    }

    /// What the user put on `face` (E56.5).
    pub fn face_meta(&self, face: FaceId) -> FmResult<FaceMeta> {
        read_meta(&self.conn, face)
    }

    /// What the faces `kept` (the kept copy's) take over from the faces
    /// `leaving` (of the copies that leave, in request order). Reads only.
    pub fn merge_plan(&self, kept: &[FaceId], leaving: &[FaceId]) -> FmResult<MergePlan> {
        let mut kept_names = names(&self.conn, kept)?;
        kept_names.sort_by_key(|(id, _, index)| (*index, id.0));
        let mut by_name: HashMap<&str, FaceId> = HashMap::new();
        for (id, name, _) in &kept_names {
            by_name.entry(name.as_str()).or_insert(*id);
        }
        // The leaving faces of each kept face, in request order.
        let mut from: Vec<(FaceId, Vec<FaceId>)> = Vec::new();
        for (id, name, _) in names(&self.conn, leaving)? {
            let Some(target) = by_name.get(name.as_str()).copied() else {
                continue;
            };
            if target == id {
                continue;
            }
            match from.iter_mut().find(|(k, _)| *k == target) {
                Some((_, list)) => list.push(id),
                None => from.push((target, vec![id])),
            }
        }
        let mut plan = MergePlan::default();
        for (target, sources) in from {
            let base = read_meta(&self.conn, target)?;
            let metas: Vec<(FaceId, FaceMeta)> = sources
                .iter()
                .map(|s| Ok((*s, read_meta(&self.conn, *s)?)))
                .collect::<FmResult<_>>()?;
            let mut merge = FaceMerge {
                kept: target,
                from: sources.clone(),
                tags: Vec::new(),
                collections: Vec::new(),
                sets: Vec::new(),
                favorite: false,
                rating: None,
                license: None,
                notes: None,
            };
            let add = |have: &BTreeSet<u32>, out: &mut Vec<u32>, theirs: &BTreeSet<u32>| {
                for id in theirs {
                    if !have.contains(id) && !out.contains(id) {
                        out.push(*id);
                    }
                }
            };
            let base_tags: BTreeSet<u32> = base.tags.keys().copied().collect();
            for (_, m) in &metas {
                let theirs: BTreeSet<u32> = m.tags.keys().copied().collect();
                add(&base_tags, &mut merge.tags, &theirs);
                add(&base.collections, &mut merge.collections, &m.collections);
                add(&base.sets, &mut merge.sets, &m.sets);
            }
            merge.favorite = !base.favorite && metas.iter().any(|(_, m)| m.favorite);
            for field in [MergeField::Rating, MergeField::License, MergeField::Notes] {
                let mut default = value(&base, field).map(|v| (target, v));
                let mut write = None;
                for (face, m) in &metas {
                    let Some(theirs) = value(m, field) else {
                        continue;
                    };
                    match &default {
                        None => {
                            write = Some(theirs.clone());
                            default = Some((*face, theirs));
                        }
                        Some((from, v)) if *v != theirs => plan.conflicts.push(MergeConflict {
                            kept_face: target,
                            field,
                            default_from: *from,
                            default: v.clone(),
                            other_face: *face,
                            other: theirs,
                        }),
                        Some(_) => {}
                    }
                }
                match write {
                    Some(MergeValue::Rating(r)) => merge.rating = Some(r),
                    Some(MergeValue::License(t, s)) => merge.license = Some((t, s)),
                    Some(MergeValue::Notes(n)) => merge.notes = Some(n),
                    None => {}
                }
            }
            plan.faces.push(merge);
        }
        Ok(plan)
    }
}

/// One row of the merge log.
struct LogRow<'a> {
    face: FaceId,
    field: &'a str,
    item: u32,
    before: Value,
    after: Value,
}

fn log(tx: &Transaction<'_>, resolution: i64, row: &LogRow<'_>) -> FmResult<()> {
    tx.prepare_cached(
        "INSERT INTO resolution_merge (resolution_id, face_id, field, item, before, after)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .and_then(|mut s| {
        s.execute(params![
            resolution,
            row.face.0,
            row.field,
            row.item,
            row.before.to_string(),
            row.after.to_string()
        ])
    })
    .map_err(|e| db_error(&e))?;
    Ok(())
}

/// Marks `face`'s family sidecar to be written.
fn mark_dirty(tx: &Transaction<'_>, face: FaceId) -> FmResult<()> {
    tx.execute(
        "INSERT INTO sidecar_dirty (face_id) SELECT ?1
         WHERE NOT EXISTS (SELECT 1 FROM sidecar_dirty WHERE face_id = ?1)",
        [face.0],
    )
    .map_err(|e| db_error(&e))?;
    Ok(())
}

fn license_json(l: Option<&(Option<String>, Option<u32>)>) -> Value {
    l.map_or(Value::Null, |(t, s)| json!([t, s]))
}

/// Writes `plan` onto the kept faces and logs every change under
/// `resolution`, in the caller's transaction.
pub(super) fn apply(tx: &Transaction<'_>, resolution: i64, plan: &MergePlan) -> FmResult<()> {
    let e = |e: rusqlite::Error| db_error(&e);
    for f in &plan.faces {
        let base = read_meta(tx, f.kept)?;
        let members = [
            (
                "tag",
                &f.tags,
                "INSERT OR IGNORE INTO face_tag (face_id, tag_id) VALUES (?1, ?2)",
            ),
            (
                "collection",
                &f.collections,
                "INSERT OR IGNORE INTO collection_face (face_id, collection_id) VALUES (?1, ?2)",
            ),
            (
                "set",
                &f.sets,
                "INSERT OR IGNORE INTO set_face (face_id, set_id) VALUES (?1, ?2)",
            ),
        ];
        for (field, ids, sql) in members {
            for id in ids {
                let n = tx.execute(sql, params![f.kept.0, id]).map_err(e)?;
                if n > 0 {
                    log(
                        tx,
                        resolution,
                        &LogRow {
                            face: f.kept,
                            field,
                            item: *id,
                            before: Value::Null,
                            after: Value::Null,
                        },
                    )?;
                }
            }
        }
        tx.execute(
            "INSERT INTO user_meta (face_id) SELECT ?1
             WHERE NOT EXISTS (SELECT 1 FROM user_meta WHERE face_id = ?1)",
            [f.kept.0],
        )
        .map_err(e)?;
        let single = |field: &str, sql: &str, before: Value, after: Value| -> FmResult<()> {
            if before == after {
                return Ok(());
            }
            tx.execute(sql, params![f.kept.0, after.to_string()])
                .map_err(e)?;
            log(
                tx,
                resolution,
                &LogRow {
                    face: f.kept,
                    field,
                    item: 0,
                    before,
                    after,
                },
            )
        };
        if f.favorite && !base.favorite {
            single("favorite", SET_FAVORITE, json!(false), json!(true))?;
        }
        if let Some(r) = f.rating {
            single("rating", SET_RATING, json!(base.rating), json!(r))?;
        }
        if let Some(l) = &f.license {
            single(
                "license",
                SET_LICENSE,
                license_json(base.license.as_ref()),
                license_json(Some(l)),
            )?;
        }
        if let Some(n) = &f.notes {
            single("notes", SET_NOTES, json!(base.notes), json!(n))?;
        }
        mark_dirty(tx, f.kept)?;
        for face in &f.from {
            mark_dirty(tx, *face)?;
        }
    }
    Ok(())
}

// Each takes the face and the value as JSON text (`?2`).
const SET_FAVORITE: &str =
    "UPDATE user_meta SET favorite = json_extract(?2, '$') WHERE face_id = ?1";
const SET_RATING: &str = "UPDATE user_meta SET rating = json_extract(?2, '$') WHERE face_id = ?1";
const SET_NOTES: &str = "UPDATE user_meta SET notes = json_extract(?2, '$') WHERE face_id = ?1";
const SET_LICENSE: &str = "UPDATE user_meta SET license_type = json_extract(?2, '$[0]'),
    seats = json_extract(?2, '$[1]') WHERE face_id = ?1";

/// Takes back what the merge of `resolution` wrote, where the face still
/// holds it, and forgets the log. Returns the faces touched, for the
/// sidecars.
pub(super) fn take_back(tx: &Transaction<'_>, resolution: i64) -> FmResult<Vec<FaceId>> {
    let e = |e: rusqlite::Error| db_error(&e);
    let rows: Vec<(u32, String, u32, String, String)> = tx
        .prepare(
            "SELECT face_id, field, item, before, after FROM resolution_merge
                  WHERE resolution_id = ?1 ORDER BY rowid DESC",
        )
        .map_err(e)?
        .query_map([resolution], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .map_err(e)?
        .collect::<Result<_, _>>()
        .map_err(e)?;
    let mut faces: Vec<FaceId> = Vec::new();
    for (face, field, item, before, after) in rows {
        let face = FaceId(face);
        if !faces.contains(&face) {
            faces.push(face);
        }
        let parse = |s: &str| serde_json::from_str::<Value>(s).unwrap_or(Value::Null);
        let (before, after) = (parse(&before), parse(&after));
        let member = match field.as_str() {
            "tag" => Some("DELETE FROM face_tag WHERE face_id = ?1 AND tag_id = ?2"),
            "collection" => {
                Some("DELETE FROM collection_face WHERE face_id = ?1 AND collection_id = ?2")
            }
            "set" => Some("DELETE FROM set_face WHERE face_id = ?1 AND set_id = ?2"),
            _ => None,
        };
        if let Some(sql) = member {
            tx.execute(sql, params![face.0, item]).map_err(e)?;
            if field == "tag" {
                tx.execute(
                    "DELETE FROM tag WHERE id = ?1
                     AND NOT EXISTS (SELECT 1 FROM face_tag WHERE tag_id = ?1)",
                    [item],
                )
                .map_err(e)?;
            }
            continue;
        }
        let now = read_meta(tx, face)?;
        let (current, sql) = match field.as_str() {
            "favorite" => (json!(now.favorite), SET_FAVORITE),
            "rating" => (json!(now.rating), SET_RATING),
            "license" => (license_json(now.license.as_ref()), SET_LICENSE),
            "notes" => (json!(now.notes), SET_NOTES),
            other => {
                tracing::warn!(field = other, "unknown field in the merge log");
                continue;
            }
        };
        // The user changed it after the cleanup: their change stays.
        if current != after {
            continue;
        }
        tx.execute(sql, params![face.0, before.to_string()])
            .map_err(e)?;
    }
    tx.execute(
        "DELETE FROM resolution_merge WHERE resolution_id = ?1",
        [resolution],
    )
    .map_err(e)?;
    for face in &faces {
        mark_dirty(tx, *face)?;
    }
    Ok(faces)
}
