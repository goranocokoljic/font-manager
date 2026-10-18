//! Collections (E7.1): the `collection` and `collection_face` tables.
//!
//! A collection is a named folder of faces the user fills by hand; it can be
//! nested under another one and ordered among its siblings
//! (`collection.position`). A face can be in any number of collections, and
//! deleting a collection (with its children) never touches a face.
//!
//! A smart collection keeps its rules here too; it fills itself, so faces
//! cannot be added to it. Its rules are not evaluated yet (E14.2), so its
//! count is 0.

use std::collections::HashMap;

use fm_api::CollectionId;
use fm_api::organization::{
    Collection, CollectionFacesRequest, CreateCollectionRequest, MoveCollectionRequest,
    RenameCollectionRequest,
};
use fm_api::search::Filters;
use fm_api::{ErrorCode, FaceId};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{Index, db_error};
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
        WHERE cf.collection_id = c.id AND font_file.gone = 0 AND source.paused = 0)
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
                ))
            })
            .map_err(|e| db_error(&e))?;
        let mut children: HashMap<Option<CollectionId>, Vec<Collection>> = HashMap::new();
        for row in rows {
            let (id, parent, name, rules, count) = row.map_err(|e| db_error(&e))?;
            let rules = rules
                .map(|json| {
                    serde_json::from_str::<Filters>(&json).map_err(|e| {
                        FmError::internal(format!("collection {id} has unreadable rules: {e}"))
                    })
                })
                .transpose()?;
            children
                .entry(parent.map(CollectionId))
                .or_default()
                .push(Collection {
                    id: CollectionId(id),
                    parent: parent.map(CollectionId),
                    name,
                    face_count: if rules.is_some() { 0 } else { count },
                    rules,
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
    pub fn create_collection(&mut self, req: &CreateCollectionRequest) -> FmResult<Collection> {
        let name = collection_name(&req.name)?;
        let rules = req
            .rules
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| FmError::internal(format!("collection rules: {e}")))?;
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        if let Some(parent) = req.parent {
            ensure_exists(&tx, parent)?;
        }
        ensure_free_name(&tx, req.parent, name, None)?;
        tx.execute(
            "INSERT INTO collection (parent_id, name, is_smart, rules, position)
             VALUES (?1, ?2, ?3, ?4,
                     (SELECT coalesce(max(position) + 1, 0) FROM collection
                      WHERE parent_id IS ?1))",
            params![req.parent.map(|p| p.0), name, rules.is_some(), rules],
        )
        .map_err(|e| db_error(&e))?;
        let id = CollectionId(
            u32::try_from(tx.last_insert_rowid())
                .map_err(|_| FmError::internal("collection id out of range"))?,
        );
        tx.commit().map_err(|e| db_error(&e))?;
        self.collection(id)
    }

    /// Renames a collection; the same rules as [`Index::create_collection`].
    pub fn rename_collection(&mut self, req: &RenameCollectionRequest) -> FmResult<Collection> {
        let name = collection_name(&req.name)?;
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        let parent = parent_of(&tx, req.collection)?;
        ensure_free_name(&tx, parent, name, Some(req.collection))?;
        tx.execute(
            "UPDATE collection SET name = ?2 WHERE id = ?1",
            params![req.collection.0, name],
        )
        .map_err(|e| db_error(&e))?;
        tx.commit().map_err(|e| db_error(&e))?;
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
        tx.commit().map_err(|e| db_error(&e))?;
        self.collection(req.collection)
    }

    /// Deletes a collection and its children. Faces are not touched.
    pub fn delete_collection(&mut self, id: CollectionId) -> FmResult<()> {
        // `ON DELETE CASCADE` removes the children and the memberships.
        let deleted = self
            .conn
            .execute("DELETE FROM collection WHERE id = ?1", [id.0])
            .map_err(|e| db_error(&e))?;
        if deleted == 0 {
            return Err(FmError::not_found(format!("Collection {id}")));
        }
        Ok(())
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
        let mut faces = req.faces.clone();
        for family in &req.families {
            let members = self
                .family_faces(*family)?
                .ok_or_else(|| FmError::not_found(format!("Family {family}")))?;
            faces.extend(members);
        }
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
        self.collection(req.collection)
    }
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
