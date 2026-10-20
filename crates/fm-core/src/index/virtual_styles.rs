//! Virtual styles (E10.2): the `virtual_style` table.
//!
//! A virtual style is an instance of a variable face, at coordinates the
//! user chose, pinned in one manual collection or one project set under a
//! name of its own ("Condensed Semibold"). It is a row in the index and an
//! entry in the family's sidecar (`index/sidecars.rs`); the font file is
//! never written or copied (rule 2). The OS registers files, not instances,
//! so activating the holder activates the face once
//! (`index/activation.rs`).

use fm_api::common::{Axis, AxisValue};
use fm_api::organization::{PinInstanceRequest, VirtualStyle, VirtualStylesRequest};
use fm_api::{CollectionId, ErrorCode, FaceId, SetId, VirtualStyleId};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use super::{Index, db_error};
use crate::error::{FmError, FmResult};
use crate::sidecar::Pin;

const STYLE: &str =
    "SELECT id, face_id, name, coordinates, collection_id, set_id FROM virtual_style";

/// What holds a virtual style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Holder {
    /// A manual collection.
    Collection(u32),
    /// A project set.
    Set(u32),
}

impl Holder {
    fn columns(self) -> (Option<u32>, Option<u32>) {
        match self {
            Self::Collection(id) => (Some(id), None),
            Self::Set(id) => (None, Some(id)),
        }
    }
}

impl Index {
    /// Pins an instance of a variable face in a manual collection or a set.
    /// The name is trimmed; an empty one, neither or both holders, a smart
    /// collection, a static face, an unknown axis or a coordinate outside
    /// its axis is `InvalidRequest`; a name the face already has in that
    /// holder is `Conflict`; an unknown face or holder is `NotFound`. An
    /// axis left out is at its default.
    pub fn pin_instance(&mut self, req: &PinInstanceRequest) -> FmResult<VirtualStyle> {
        let name = req.name.trim();
        if name.is_empty() {
            return Err(FmError::invalid("Give the style a name."));
        }
        let holder = holder(&self.conn, req.collection, req.set)?;
        let (label, axes) = face_axes(&self.conn, req.face)?;
        let coordinates = resolve_coordinates(&label, &axes, &req.coordinates)?;
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        let id = insert_style(&tx, req.face.0, name, &coordinates, holder)?.ok_or_else(|| {
            FmError::new(
                ErrorCode::Conflict,
                format!("“{label}” already has a style named “{name}” here."),
            )
        })?;
        tx.commit().map_err(|e| db_error(&e))?;
        self.mirror_sidecars();
        self.virtual_style(VirtualStyleId(id))
    }

    /// The virtual styles of a collection or a set, A to Z. Neither or both
    /// is `InvalidRequest`, an unknown one `NotFound`.
    pub fn virtual_styles(&self, req: &VirtualStylesRequest) -> FmResult<Vec<VirtualStyle>> {
        let (collection, set) = holder(&self.conn, req.collection, req.set)?.columns();
        let mut stmt = self
            .conn
            .prepare(&format!(
                "{STYLE} WHERE collection_id IS ?1 AND set_id IS ?2
                 ORDER BY name COLLATE NOCASE, id"
            ))
            .map_err(|e| db_error(&e))?;
        let rows = stmt
            .query_map(params![collection, set], style_row)
            .map_err(|e| db_error(&e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| db_error(&e))?;
        rows.into_iter().collect()
    }

    /// Deletes a virtual style; its face stays. Unknown is `NotFound`.
    pub fn delete_virtual_style(&mut self, id: VirtualStyleId) -> FmResult<()> {
        let deleted = self
            .conn
            .execute("DELETE FROM virtual_style WHERE id = ?1", [id.0])
            .map_err(|e| db_error(&e))?;
        if deleted == 0 {
            return Err(FmError::not_found(format!("Virtual style {id}")));
        }
        self.mirror_sidecars();
        Ok(())
    }

    fn virtual_style(&self, id: VirtualStyleId) -> FmResult<VirtualStyle> {
        self.conn
            .query_row(&format!("{STYLE} WHERE id = ?1"), [id.0], style_row)
            .optional()
            .map_err(|e| db_error(&e))?
            .ok_or_else(|| FmError::not_found(format!("Virtual style {id}")))?
    }
}

/// The coordinates of an instance: one for every axis of the face, in the
/// font's axis order, the default where `requested` leaves an axis out. A
/// face without axes, a tag the face does not have or names twice, and a
/// value outside its axis (or not a number) are `InvalidRequest`. `label`
/// names the face in the message.
pub(crate) fn resolve_coordinates(
    label: &str,
    axes: &[Axis],
    requested: &[AxisValue],
) -> FmResult<Vec<AxisValue>> {
    if axes.is_empty() {
        return Err(FmError::invalid(format!(
            "“{label}” is not a variable font, so it has no instances to pin."
        )));
    }
    for (i, c) in requested.iter().enumerate() {
        let Some(axis) = axes.iter().find(|a| a.tag == c.tag) else {
            let tags: Vec<&str> = axes.iter().map(|a| a.tag.as_str()).collect();
            return Err(FmError::invalid(format!(
                "“{label}” has no axis “{}”. Its axes are {}.",
                c.tag,
                tags.join(", ")
            )));
        };
        if requested[..i].iter().any(|o| o.tag == c.tag) {
            return Err(FmError::invalid(format!(
                "The axis “{}” is given twice.",
                c.tag
            )));
        }
        if !c.value.is_finite() || c.value < axis.min || c.value > axis.max {
            return Err(FmError::invalid(format!(
                "{} ({}) goes from {} to {}; {} is outside it.",
                axis.name, axis.tag, axis.min, axis.max, c.value
            )));
        }
    }
    Ok(axes
        .iter()
        .map(|a| AxisValue {
            tag: a.tag.clone(),
            value: requested
                .iter()
                .find(|c| c.tag == a.tag)
                .map_or(a.default, |c| c.value),
        })
        .collect())
}

/// Inserts a style; `None` when the face has a style of that name in that
/// holder already.
pub(super) fn insert_style(
    tx: &Transaction<'_>,
    face: u32,
    name: &str,
    coordinates: &[AxisValue],
    holder: Holder,
) -> FmResult<Option<u32>> {
    let (collection, set) = holder.columns();
    let json = serde_json::to_string(coordinates)
        .map_err(|e| FmError::internal(format!("virtual style coordinates: {e}")))?;
    let inserted = tx
        .prepare_cached(
            "INSERT OR IGNORE INTO virtual_style (face_id, name, coordinates, collection_id, set_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .and_then(|mut s| s.execute(params![face, name, json, collection, set]))
        .map_err(|e| db_error(&e))?;
    if inserted == 0 {
        return Ok(None);
    }
    u32::try_from(tx.last_insert_rowid())
        .map(Some)
        .map_err(|_| FmError::internal("virtual style id out of range"))
}

/// The virtual styles of `face` as sidecar pins; `path` names a collection
/// by its names from the top level (a style in a collection it cannot name
/// is left out).
pub(super) fn face_pins(
    tx: &Transaction<'_>,
    face: u32,
    path: impl Fn(u32) -> Option<Vec<String>>,
) -> FmResult<Vec<Pin>> {
    let mut stmt = tx
        .prepare_cached(
            "SELECT virtual_style.name, virtual_style.coordinates, virtual_style.collection_id,
                    project_set.name
             FROM virtual_style LEFT JOIN project_set ON project_set.id = virtual_style.set_id
             WHERE virtual_style.face_id = ?1",
        )
        .map_err(|e| db_error(&e))?;
    let rows = stmt
        .query_map([face], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<u32>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|e| db_error(&e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_error(&e))?;
    let mut pins = Vec::with_capacity(rows.len());
    for (name, json, collection, set) in rows {
        let coordinates: Vec<AxisValue> = serde_json::from_str(&json).unwrap_or_default();
        let collection = match collection {
            Some(id) => match path(id) {
                Some(p) => Some(p),
                None => continue,
            },
            None => None,
        };
        pins.push(Pin {
            name,
            collection,
            set,
            coordinates: coordinates.into_iter().map(|c| (c.tag, c.value)).collect(),
        });
    }
    Ok(pins)
}

/// The set named `name` (in any case), made empty when there is none: a pin
/// from another machine names its set like a collection path names its
/// collections, which a merge makes too.
pub(super) fn set_by_name(tx: &Transaction<'_>, name: &str, create: bool) -> FmResult<Option<u32>> {
    let id: Option<u32> = tx
        .prepare_cached("SELECT id FROM project_set WHERE name = ?1 COLLATE NOCASE ORDER BY id")
        .and_then(|mut s| s.query_row([name], |r| r.get(0)).optional())
        .map_err(|e| db_error(&e))?;
    match id {
        Some(id) => Ok(Some(id)),
        None if create => super::sets::insert_set(tx, name, &[]).map(|s| Some(s.0)),
        None => Ok(None),
    }
}

/// Makes the index hold `pin` on `face` in `holder`: inserts it, or sets the
/// coordinates of the style of that name. `true` when the index changed.
pub(super) fn put_pin(
    tx: &Transaction<'_>,
    face: u32,
    pin: &Pin,
    holder: Holder,
) -> FmResult<bool> {
    let (_, axes) = face_axes(tx, FaceId(face))?;
    // The font's axis order first, any other tag after it.
    let mut coordinates: Vec<AxisValue> = axes
        .iter()
        .map(|a| AxisValue {
            tag: a.tag.clone(),
            value: pin.coordinates.get(&a.tag).copied().unwrap_or(a.default),
        })
        .collect();
    coordinates.extend(
        pin.coordinates
            .iter()
            .filter(|(tag, _)| !axes.iter().any(|a| a.tag == **tag))
            .map(|(tag, value)| AxisValue {
                tag: tag.clone(),
                value: *value,
            }),
    );
    let json = serde_json::to_string(&coordinates)
        .map_err(|e| FmError::internal(format!("virtual style coordinates: {e}")))?;
    let (collection, set) = holder.columns();
    let updated = tx
        .prepare_cached(
            "UPDATE virtual_style SET coordinates = ?1
             WHERE face_id = ?2 AND collection_id IS ?3 AND set_id IS ?4
               AND name = ?5 COLLATE NOCASE AND coordinates <> ?1",
        )
        .and_then(|mut s| s.execute(params![json, face, collection, set, pin.name]))
        .map_err(|e| db_error(&e))?;
    if updated > 0 {
        return Ok(true);
    }
    Ok(insert_style(tx, face, &pin.name, &coordinates, holder)?.is_some())
}

/// Deletes the style `name` of `face` in `holder`; `true` when there was one.
pub(super) fn remove_pin(
    tx: &Transaction<'_>,
    face: u32,
    name: &str,
    holder: Holder,
) -> FmResult<bool> {
    let (collection, set) = holder.columns();
    tx.prepare_cached(
        "DELETE FROM virtual_style
         WHERE face_id = ?1 AND collection_id IS ?2 AND set_id IS ?3 AND name = ?4 COLLATE NOCASE",
    )
    .and_then(|mut s| s.execute(params![face, collection, set, name]))
    .map(|n| n > 0)
    .map_err(|e| db_error(&e))
}

/// The one holder named, checked: a manual collection or a set that exists.
fn holder(
    conn: &Connection,
    collection: Option<CollectionId>,
    set: Option<SetId>,
) -> FmResult<Holder> {
    match (collection, set) {
        (Some(c), None) => {
            let smart: bool = conn
                .query_row(
                    "SELECT is_smart FROM collection WHERE id = ?1",
                    [c.0],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| db_error(&e))?
                .ok_or_else(|| FmError::not_found(format!("Collection {c}")))?;
            if smart {
                return Err(FmError::invalid(
                    "A virtual style goes in a manual collection or a set, not a smart collection.",
                ));
            }
            Ok(Holder::Collection(c.0))
        }
        (None, Some(s)) => {
            let exists = conn
                .prepare_cached("SELECT 1 FROM project_set WHERE id = ?1")
                .and_then(|mut st| st.exists([s.0]))
                .map_err(|e| db_error(&e))?;
            if !exists {
                return Err(FmError::not_found(format!("Set {s}")));
            }
            Ok(Holder::Set(s.0))
        }
        _ => Err(FmError::invalid(
            "Choose one collection or one set for the virtual style.",
        )),
    }
}

/// The face's name for messages and its variation axes; `NotFound` when
/// unknown. Unreadable axes JSON reads as none.
fn face_axes(conn: &Connection, face: FaceId) -> FmResult<(String, Vec<Axis>)> {
    let (family, style, variable, axes): (String, String, bool, String) = conn
        .query_row(
            "SELECT family, subfamily, is_variable, axes FROM face WHERE id = ?1",
            [face.0],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(|e| db_error(&e))?
        .ok_or_else(|| FmError::not_found(format!("Face {face}")))?;
    let axes = if variable {
        serde_json::from_str(&axes).unwrap_or_default()
    } else {
        Vec::new()
    };
    Ok((format!("{family} {style}"), axes))
}

fn style_row(row: &Row<'_>) -> rusqlite::Result<FmResult<VirtualStyle>> {
    let id: u32 = row.get(0)?;
    let face: u32 = row.get(1)?;
    let name: String = row.get(2)?;
    let json: String = row.get(3)?;
    let collection: Option<u32> = row.get(4)?;
    let set: Option<u32> = row.get(5)?;
    let coordinates = serde_json::from_str(&json)
        .map_err(|e| FmError::internal(format!("virtual style {id} coordinates: {e}")));
    Ok(coordinates.map(|coordinates| VirtualStyle {
        id: VirtualStyleId(id),
        face: FaceId(face),
        name,
        coordinates,
        collection: collection.map(CollectionId),
        set: set.map(SetId),
    }))
}

#[cfg(test)]
mod tests;
