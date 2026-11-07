//! Collections (E7.1): the `collection` and `collection_face` tables.
//!
//! A collection is a named folder of faces the user fills by hand; it can be
//! nested under another one and ordered among its siblings
//! (`collection.position`). A face can be in any number of collections, and
//! deleting a collection (with its children) never touches a face.
//!
//! A smart collection keeps its rule here too (a [`RuleGroup`] as JSON; a
//! phase-1 rule is plain filters); it fills itself, so faces cannot be
//! added to it. Its count is evaluated now, never stored (E14.2,
//! `index/search/smart.rs`).
//!
//! A collection can have a colour, a key of the API's fixed palette. It is
//! this library's own: sidecars carry what a face is in, not how it looks.
//!
//! Every change to what a face is in, or to the names on the way to it, is
//! mirrored to the sidecars of the faces' families (`index/sidecars.rs`).

use std::collections::HashMap;

use fm_api::CollectionId;
use fm_api::common::Classification;
use fm_api::organization::{
    COLLECTION_COLORS, Collection, CollectionFacesRequest, CreateCollectionRequest,
    MoveCollectionRequest, RenameCollectionRequest, RuleGroup, UpdateCollectionRulesRequest,
};
use fm_api::search::Filters;
use fm_api::{ErrorCode, FaceId};
use rusqlite::{OptionalExtension, Transaction, params};

use super::search::smart::{Member, check_rule, load_rules, stored_rule};
use super::{Index, db_error, sidecars};
use crate::error::{FmError, FmResult};

/// Every collection with the number of its faces the library shows (files
/// not gone, sources not paused).
const COLLECTIONS: &str = "
SELECT c.id, c.parent_id, c.name, c.rules,
       (SELECT COUNT(*)
        FROM collection_face AS cf
        JOIN face ON face.id = cf.face_id
        JOIN font_file ON font_file.id = face.file_id
        JOIN source ON source.id = font_file.source_id
        WHERE cf.collection_id = c.id AND font_file.gone = 0 AND source.paused = 0),
       c.color
FROM collection AS c
ORDER BY c.position, c.id";

impl Index {
    /// Every collection, each parent before its children, siblings in the
    /// user's order.
    pub fn collections(&self) -> FmResult<Vec<Collection>> {
        let mut stmt = self.conn.prepare(COLLECTIONS).map_err(|e| db_error(&e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, Option<u32>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, u32>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })
            .map_err(|e| db_error(&e))?;
        let rows = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| db_error(&e))?;
        drop(stmt);
        let smart_counts = self.smart_face_counts()?;
        let mut children: HashMap<Option<CollectionId>, Vec<Collection>> = HashMap::new();
        for (id, parent, name, rules, count, color) in rows {
            let rule_group = rules
                .map(|json| {
                    stored_rule(&json).map_err(|e| {
                        FmError::internal(format!("collection {id} has unreadable rules: {e}"))
                    })
                })
                .transpose()?;
            let id = CollectionId(id);
            children
                .entry(parent.map(CollectionId))
                .or_default()
                .push(Collection {
                    id,
                    parent: parent.map(CollectionId),
                    name,
                    face_count: match rule_group {
                        Some(_) => smart_counts.get(&id).copied().unwrap_or(0),
                        None => count,
                    },
                    rules: rule_group
                        .as_ref()
                        .map(|g| g.plain_filters().cloned().unwrap_or_default()),
                    rule_group,
                    color,
                });
        }
        let mut out = Vec::new();
        walk(None, &mut children, &mut out);
        Ok(out)
    }

    /// One collection, as [`Index::collections`] lists it.
    pub fn collection(&self, id: CollectionId) -> FmResult<Collection> {
        self.collections()?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| FmError::not_found(format!("Collection {id}")))
    }

    /// Creates a collection, last among its siblings. The name is trimmed;
    /// an empty one is `InvalidRequest`, one a sibling already has is
    /// `Conflict`, an unknown parent is `NotFound`.
    ///
    /// With `rules` or `rule_group` (not both: `InvalidRequest`) it is smart.
    /// A rule naming an unknown collection is `NotFound`; one that leads
    /// back to the new collection, or names an unknown language preset, is
    /// `InvalidRequest`, as is a rule [`check_rule`] refuses (an empty group,
    /// a range whose minimum is above its maximum). A colour that is not a
    /// key of the palette is `InvalidRequest`.
    pub fn create_collection(&mut self, req: &CreateCollectionRequest) -> FmResult<Collection> {
        let name = collection_name(&req.name)?;
        let color = collection_color(req.color.as_deref())?;
        let group = match (&req.rules, &req.rule_group) {
            (Some(_), Some(_)) => {
                return Err(FmError::invalid(
                    "Give a smart collection rules or a rule group, not both.",
                ));
            }
            (rules, group) => group.clone().or_else(|| rules.clone().map(RuleGroup::from)),
        };
        if let Some(group) = &group {
            check_rule(group)?;
        }
        let rules = group
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| FmError::internal(format!("collection rules: {e}")))?;
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        if let Some(parent) = req.parent {
            ensure_exists(&tx, parent)?;
        }
        ensure_free_name(&tx, req.parent, name, None)?;
        let id = CollectionId(insert_collection(
            &tx,
            req.parent.map(|p| p.0),
            name,
            rules.as_deref(),
            color,
        )?);
        if let Some(group) = &group {
            // Checked with the new row in place, so a rule naming the new
            // collection itself is a loop, not an unknown collection. An
            // error drops the transaction: nothing is kept.
            for named in group.collections() {
                ensure_exists(&tx, named)?;
            }
            Member::compile(&tx, &load_rules(&tx)?, id)?;
        }
        tx.commit().map_err(|e| db_error(&e))?;
        self.collection(id)
    }

    /// Replaces the rule of a smart collection (E14.1), checked as
    /// [`Index::create_collection`] checks it. A manual collection is
    /// `InvalidRequest`, an unknown one `NotFound`; on any error the old
    /// rule stays.
    pub fn update_collection_rules(
        &mut self,
        req: &UpdateCollectionRulesRequest,
    ) -> FmResult<Collection> {
        check_rule(&req.rule_group)?;
        let rules = serde_json::to_string(&req.rule_group)
            .map_err(|e| FmError::internal(format!("collection rules: {e}")))?;
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        let smart: Option<bool> = tx
            .query_row(
                "SELECT rules IS NOT NULL FROM collection WHERE id = ?1",
                [req.collection.0],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| db_error(&e))?;
        match smart {
            None => return Err(FmError::not_found(format!("Collection {}", req.collection))),
            Some(false) => {
                return Err(FmError::invalid(
                    "A manual collection has no rules; make a smart collection instead.",
                ));
            }
            Some(true) => {}
        }
        tx.execute(
            "UPDATE collection SET rules = ?2 WHERE id = ?1",
            params![req.collection.0, rules],
        )
        .map_err(|e| db_error(&e))?;
        // As on create: checked with the new rule in place, so a loop back
        // to this collection is refused and the transaction dropped.
        for named in req.rule_group.collections() {
            ensure_exists(&tx, named)?;
        }
        Member::compile(&tx, &load_rules(&tx)?, req.collection)?;
        tx.commit().map_err(|e| db_error(&e))?;
        self.collection(req.collection)
    }

    /// Renames a collection and sets or removes its colour; the same rules as
    /// [`Index::create_collection`]. No colour in the request keeps the colour; an
    /// empty one removes it.
    /// The name is what the sidecars mirror: a colour alone writes none of them.
    pub fn rename_collection(&mut self, req: &RenameCollectionRequest) -> FmResult<Collection> {
        let name = collection_name(&req.name)?;
        let color = match req.color.as_deref() {
            None => None,
            Some("") => Some(None),
            key => Some(collection_color(key)?),
        };
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        let parent = parent_of(&tx, req.collection)?;
        ensure_free_name(&tx, parent, name, Some(req.collection))?;
        let renamed = tx
            .execute(
                "UPDATE collection SET name = ?2 WHERE id = ?1 AND name != ?2",
                params![req.collection.0, name],
            )
            .map_err(|e| db_error(&e))?;
        if renamed > 0 {
            sidecars::mark_collection_dirty(&tx, req.collection.0)?;
        }
        if let Some(color) = color {
            tx.execute(
                "UPDATE collection SET color = ?2 WHERE id = ?1",
                params![req.collection.0, color],
            )
            .map_err(|e| db_error(&e))?;
        }
        tx.commit().map_err(|e| db_error(&e))?;
        self.mirror_sidecars();
        self.collection(req.collection)
    }

    /// Moves a collection, with its children, under `parent` at `position`
    /// among the new siblings (past the end: last). Into itself or one of
    /// its children is `InvalidRequest`; a sibling there with the same name
    /// is `Conflict`.
    pub fn move_collection(&mut self, req: &MoveCollectionRequest) -> FmResult<Collection> {
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        ensure_exists(&tx, req.collection)?;
        let mut up = req.parent;
        while let Some(id) = up {
            if id == req.collection {
                return Err(FmError::invalid(
                    "A collection cannot go inside itself or one of its children.",
                ));
            }
            up = parent_of(&tx, id)?;
        }
        let name: String = tx
            .query_row(
                "SELECT name FROM collection WHERE id = ?1",
                [req.collection.0],
                |row| row.get(0),
            )
            .map_err(|e| db_error(&e))?;
        ensure_free_name(&tx, req.parent, &name, Some(req.collection))?;

        let mut siblings: Vec<u32> = {
            let mut stmt = tx
                .prepare(
                    "SELECT id FROM collection WHERE parent_id IS ?1 AND id != ?2
                     ORDER BY position, id",
                )
                .map_err(|e| db_error(&e))?;
            stmt.query_map(params![req.parent.map(|p| p.0), req.collection.0], |row| {
                row.get(0)
            })
            .map_err(|e| db_error(&e))?
            .collect::<Result<_, _>>()
            .map_err(|e| db_error(&e))?
        };
        let at = (req.position as usize).min(siblings.len());
        siblings.insert(at, req.collection.0);
        tx.execute(
            "UPDATE collection SET parent_id = ?2 WHERE id = ?1",
            params![req.collection.0, req.parent.map(|p| p.0)],
        )
        .map_err(|e| db_error(&e))?;
        {
            let mut stmt = tx
                .prepare("UPDATE collection SET position = ?2 WHERE id = ?1")
                .map_err(|e| db_error(&e))?;
            for (position, id) in siblings.iter().enumerate() {
                let position = i64::try_from(position).unwrap_or(i64::MAX);
                stmt.execute(params![id, position])
                    .map_err(|e| db_error(&e))?;
            }
        }
        sidecars::mark_collection_dirty(&tx, req.collection.0)?;
        tx.commit().map_err(|e| db_error(&e))?;
        self.mirror_sidecars();
        self.collection(req.collection)
    }

    /// Deletes a collection and its children. Faces are not touched.
    pub fn delete_collection(&mut self, id: CollectionId) -> FmResult<()> {
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        sidecars::mark_collection_dirty(&tx, id.0)?;
        // `ON DELETE CASCADE` removes the children and the memberships.
        let deleted = tx
            .execute("DELETE FROM collection WHERE id = ?1", [id.0])
            .map_err(|e| db_error(&e))?;
        if deleted == 0 {
            return Err(FmError::not_found(format!("Collection {id}")));
        }
        tx.commit().map_err(|e| db_error(&e))?;
        self.mirror_sidecars();
        Ok(())
    }

    /// Creates the default smart collections ([`default_collections`]) at
    /// the top level, after any there already are, unless this library has
    /// had them already: they are made once, so a default the user renamed
    /// or deleted is not made again, and [`Index::rebuild`] keeps the mark.
    /// A name a top-level collection already has is skipped. Returns whether
    /// they were made now.
    pub fn create_default_collections(&mut self) -> FmResult<bool> {
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        let retired = retire_replaced_defaults(&tx)?;
        let made = tx
            .prepare("SELECT 1 FROM index_meta WHERE key = ?1")
            .and_then(|mut stmt| stmt.exists([DEFAULTS_KEY]))
            .map_err(|e| db_error(&e))?;
        if made {
            tx.commit().map_err(|e| db_error(&e))?;
            if retired {
                self.mirror_sidecars();
            }
            return Ok(false);
        }
        for (name, filters) in default_collections() {
            if ensure_free_name(&tx, None, name, None).is_err() {
                continue;
            }
            let rules = serde_json::to_string(&RuleGroup::from(filters))
                .map_err(|e| FmError::internal(format!("collection rules: {e}")))?;
            insert_collection(&tx, None, name, Some(&rules), None)?;
        }
        tx.execute(
            "INSERT INTO index_meta (key, value)
             VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
            [DEFAULTS_KEY],
        )
        .map_err(|e| db_error(&e))?;
        tx.commit().map_err(|e| db_error(&e))?;
        if retired {
            self.mirror_sidecars();
        }
        Ok(true)
    }

    /// The collections, manual and smart, that hold at least one of `faces`
    /// now, in id order (E42.9). A face in no collection, or one the library
    /// does not show, adds none.
    pub fn collections_holding(&self, faces: &[FaceId]) -> FmResult<Vec<CollectionId>> {
        let ids: Vec<u32> = faces.iter().map(|f| f.0).collect();
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT DISTINCT cf.collection_id
                 FROM collection_face AS cf
                 JOIN face ON face.id = cf.face_id
                 JOIN font_file ON font_file.id = face.file_id
                 JOIN source ON source.id = font_file.source_id
                 WHERE cf.face_id = ?1 AND font_file.gone = 0 AND source.paused = 0",
            )
            .map_err(|e| db_error(&e))?;
        let mut out: Vec<CollectionId> = Vec::new();
        for id in &ids {
            let rows = stmt
                .query_map([id], |row| row.get::<_, u32>(0))
                .map_err(|e| db_error(&e))?;
            for row in rows {
                out.push(CollectionId(row.map_err(|e| db_error(&e))?));
            }
        }
        drop(stmt);
        out.extend(self.smart_collections_holding(&ids)?);
        out.sort_unstable();
        out.dedup();
        Ok(out)
    }

    /// Adds faces, and every face of whole families, to a manual collection.
    /// A face already in it stays once; it stays in its other collections.
    pub fn add_to_collection(&mut self, req: &CollectionFacesRequest) -> FmResult<Collection> {
        self.edit_members(
            req,
            "INSERT OR IGNORE INTO collection_face (collection_id, face_id) VALUES (?1, ?2)",
        )
    }

    /// Removes faces, or whole families, from a manual collection.
    pub fn remove_from_collection(&mut self, req: &CollectionFacesRequest) -> FmResult<Collection> {
        self.edit_members(
            req,
            "DELETE FROM collection_face WHERE collection_id = ?1 AND face_id = ?2",
        )
    }

    /// Runs `sql` (collection id, face id) for every face `req` names, in
    /// one transaction; an unknown face or family is `NotFound`.
    fn edit_members(&mut self, req: &CollectionFacesRequest, sql: &str) -> FmResult<Collection> {
        let faces = self.faces_of(&req.faces, &req.families)?;
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        let smart: bool = tx
            .query_row(
                "SELECT is_smart FROM collection WHERE id = ?1",
                [req.collection.0],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| db_error(&e))?
            .ok_or_else(|| FmError::not_found(format!("Collection {}", req.collection)))?;
        if smart {
            return Err(FmError::invalid(
                "A smart collection fills itself from its rules.",
            ));
        }
        {
            let mut known = tx
                .prepare("SELECT 1 FROM face WHERE id = ?1")
                .map_err(|e| db_error(&e))?;
            let mut edit = tx.prepare(sql).map_err(|e| db_error(&e))?;
            for FaceId(face) in faces {
                if !known.exists([face]).map_err(|e| db_error(&e))? {
                    return Err(FmError::not_found(format!("Face {face}")));
                }
                edit.execute(params![req.collection.0, face])
                    .map_err(|e| db_error(&e))?;
            }
        }
        tx.commit().map_err(|e| db_error(&e))?;
        self.mirror_sidecars();
        self.collection(req.collection)
    }
}

/// The `index_meta` key set once the default smart collections were made;
/// its value is when.
const DEFAULTS_KEY: &str = "default_collections";

/// The `index_meta` key set once [`replaced_defaults`] were retired.
const RETIRED_KEY: &str = "default_collections_retired";

/// What "Recently" means for the default smart collections, in days.
pub(crate) const RECENT_DAYS: u32 = 30;

/// The default smart collections (E14.3), in sidebar order: each a name and
/// one plain rule.
pub(crate) fn default_collections() -> Vec<(&'static str, Filters)> {
    let rule = |set: fn(&mut Filters)| {
        let mut f = Filters::default();
        set(&mut f);
        f
    };
    vec![
        (
            "Recently activated",
            rule(|f| f.activated_within_days = Some(RECENT_DAYS)),
        ),
        ("Variable", rule(|f| f.variable = Some(true))),
        (
            "Monospace",
            rule(|f| f.classification = vec![Classification::Mono]),
        ),
        ("Web-licensed", rule(|f| f.web_ok = Some(true))),
    ]
}

/// The defaults the sidebar's own rows replaced (E41.3): "Recently added" is
/// a row at its top, and Problems appears once, as the section at its foot.
fn replaced_defaults() -> Vec<(&'static str, Filters)> {
    vec![
        (
            "Recently added",
            Filters {
                added_within_days: Some(RECENT_DAYS),
                ..Filters::default()
            },
        ),
        (
            "Problems",
            Filters {
                problems: Some(true),
                ..Filters::default()
            },
        ),
    ]
}

/// Deletes, once per library, the [`replaced_defaults`] an earlier version
/// made and the user left as they were: same name and rule, top level, no
/// colour, no children. One the user changed is theirs and stays. Returns
/// whether it deleted any.
fn retire_replaced_defaults(tx: &Transaction<'_>) -> FmResult<bool> {
    let done = tx
        .prepare("SELECT 1 FROM index_meta WHERE key = ?1")
        .and_then(|mut stmt| stmt.exists([RETIRED_KEY]))
        .map_err(|e| db_error(&e))?;
    if done {
        return Ok(false);
    }
    let mut retired = false;
    for (name, filters) in replaced_defaults() {
        let found = tx
            .query_row(
                "SELECT c.id, c.rules FROM collection c
                 WHERE c.parent_id IS NULL AND c.is_smart = 1 AND c.name = ?1
                   AND c.color IS NULL
                   AND NOT EXISTS (SELECT 1 FROM collection k WHERE k.parent_id = c.id)",
                [name],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()
            .map_err(|e| db_error(&e))?;
        let Some((id, Some(rules))) = found else {
            continue;
        };
        let same = serde_json::from_str::<RuleGroup>(&rules)
            .is_ok_and(|group| group.plain_filters() == Some(&filters));
        if same {
            sidecars::mark_collection_dirty(tx, id)?;
            tx.execute("DELETE FROM collection WHERE id = ?1", [id])
                .map_err(|e| db_error(&e))?;
            retired = true;
        }
    }
    tx.execute(
        "INSERT INTO index_meta (key, value)
         VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
        [RETIRED_KEY],
    )
    .map_err(|e| db_error(&e))?;
    Ok(retired)
}

/// Inserts a collection last among the children of `parent`; returns its id.
pub(super) fn insert_collection(
    tx: &Transaction<'_>,
    parent: Option<u32>,
    name: &str,
    rules: Option<&str>,
    color: Option<&str>,
) -> FmResult<u32> {
    tx.execute(
        "INSERT INTO collection (parent_id, name, is_smart, rules, color, position)
         VALUES (?1, ?2, ?3, ?4, ?5,
                 (SELECT coalesce(max(position) + 1, 0) FROM collection
                  WHERE parent_id IS ?1))",
        params![parent, name, rules.is_some(), rules, color],
    )
    .map_err(|e| db_error(&e))?;
    u32::try_from(tx.last_insert_rowid())
        .map_err(|_| FmError::internal("collection id out of range"))
}

/// Appends the children of `parent`, each followed by its own children.
fn walk(
    parent: Option<CollectionId>,
    children: &mut HashMap<Option<CollectionId>, Vec<Collection>>,
    out: &mut Vec<Collection>,
) {
    for c in children.remove(&parent).unwrap_or_default() {
        let id = c.id;
        out.push(c);
        walk(Some(id), children, out);
    }
}

/// The trimmed name; empty is `InvalidRequest`.
fn collection_name(name: &str) -> FmResult<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err(FmError::invalid("Give the collection a name."));
    }
    Ok(name)
}

/// The colour, when it is a key of the palette; else `InvalidRequest`.
fn collection_color(color: Option<&str>) -> FmResult<Option<&str>> {
    match color {
        Some(key) if !COLLECTION_COLORS.contains(&key) => Err(FmError::invalid(
            "Pick one of the eight collection colours.",
        )),
        _ => Ok(color),
    }
}

fn ensure_exists(tx: &Transaction<'_>, id: CollectionId) -> FmResult<()> {
    parent_of(tx, id).map(|_| ())
}

/// The parent of `id`; an unknown collection is `NotFound`.
fn parent_of(tx: &Transaction<'_>, id: CollectionId) -> FmResult<Option<CollectionId>> {
    tx.query_row(
        "SELECT parent_id FROM collection WHERE id = ?1",
        [id.0],
        |row| row.get::<_, Option<u32>>(0),
    )
    .optional()
    .map_err(|e| db_error(&e))?
    .map(|parent| parent.map(CollectionId))
    .ok_or_else(|| FmError::not_found(format!("Collection {id}")))
}

/// `Conflict` when a child of `parent` other than `except` is called `name`.
fn ensure_free_name(
    tx: &Transaction<'_>,
    parent: Option<CollectionId>,
    name: &str,
    except: Option<CollectionId>,
) -> FmResult<()> {
    let taken = tx
        .prepare(
            "SELECT 1 FROM collection
             WHERE parent_id IS ?1 AND name = ?2 AND id IS NOT ?3",
        )
        .and_then(|mut stmt| stmt.exists(params![parent.map(|p| p.0), name, except.map(|e| e.0)]))
        .map_err(|e| db_error(&e))?;
    if taken {
        return Err(FmError::new(
            ErrorCode::Conflict,
            format!("A collection named \"{name}\" already exists here."),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
