//! Activation state (E6.6): the `activation`, `activation_registration` and
//! `last_session` tables, and what an activation target stands for.
//!
//! The activation table is the list of what the app registered with the OS
//! (rule 3): `crate::activation` writes a row only after the OS call worked
//! and removes it only after the OS call undid it.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use fm_api::activation::{ActivationMode, Target};
use fm_api::common::{FontFormat, Health, Timestamp};
use fm_api::{FaceId, SetId};
use fm_platform::{Mode, Registration};
use rusqlite::types::ValueRef;
use rusqlite::{OptionalExtension, params};

use super::files::{format_from_sql, path_from_sql, path_to_sql};
use super::{Index, db_error};
use crate::error::{FmError, FmResult};

/// What the activation service needs to know about one face.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FaceRow {
    /// The face.
    pub id: FaceId,
    /// Its font file, in place.
    pub path: PathBuf,
    /// Its PostScript name.
    pub postscript_name: String,
    /// The file's format; `None` for data that is no font.
    pub format: Option<FontFormat>,
    /// Why the face cannot be used, when it cannot.
    pub unhealthy: Option<String>,
    /// The last scan did not find the file.
    pub gone: bool,
    /// Its source is paused.
    pub paused: bool,
}

/// A face the app has active, as the activation table holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveRow {
    /// The face.
    pub face: FaceId,
    /// Session or permanent.
    pub mode: ActivationMode,
    /// Since when.
    pub since: Timestamp,
    /// The document that asked for it, if any.
    pub document: Option<String>,
    /// What the OS holds for it; `None` when nothing was recorded.
    pub registration: Option<Registration>,
}

const FACE_ROW: &str = "
SELECT face.postscript_name, font_file.path, font_file.format, face.healthy,
       CASE WHEN json_valid(face.parsed) THEN json_extract(face.parsed, '$.health') END,
       font_file.gone, source.paused
FROM face
JOIN font_file ON font_file.id = face.file_id
JOIN source ON source.id = font_file.source_id
WHERE face.id = ?1";

const ACTIVE_ROWS: &str = "
SELECT a.face_id, a.mode, a.since, a.document, r.path, r.handle
FROM activation AS a
LEFT JOIN activation_registration AS r ON r.face_id = a.face_id
ORDER BY a.face_id";

const UPSERT_ACTIVATION: &str = "
INSERT INTO activation (face_id, mode, document, since)
VALUES (?1, ?2, NULL, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
ON CONFLICT (face_id) DO UPDATE SET mode = excluded.mode, since = excluded.since";

const UPSERT_REGISTRATION: &str = "
INSERT INTO activation_registration (face_id, path, handle) VALUES (?1, ?2, ?3)
ON CONFLICT (face_id) DO UPDATE SET path = excluded.path, handle = excluded.handle";

impl Index {
    /// The faces `target` stands for, in id order without repeats (a face
    /// list keeps its own order). An unknown face, family, collection or set
    /// is `NotFound`; a smart collection is `Unsupported` (its rules are not
    /// evaluated yet, E14.2).
    pub(crate) fn target_faces(&self, target: &Target) -> FmResult<Vec<FaceId>> {
        match target {
            Target::Faces { faces } => {
                let mut seen = HashSet::new();
                let mut out = Vec::with_capacity(faces.len());
                for &face in faces {
                    if !self.exists("SELECT 1 FROM face WHERE id = ?1", face.0)? {
                        return Err(FmError::not_found(format!("Face {face}")));
                    }
                    if seen.insert(face) {
                        out.push(face);
                    }
                }
                Ok(out)
            }
            Target::Family { family } => self
                .family_faces(*family)?
                .ok_or_else(|| FmError::not_found(format!("Family {family}"))),
            Target::Collection { collection } => {
                let smart: Option<bool> = self
                    .conn
                    .query_row(
                        "SELECT is_smart FROM collection WHERE id = ?1",
                        [collection.0],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(|e| db_error(&e))?;
                match smart {
                    None => Err(FmError::not_found(format!("Collection {collection}"))),
                    Some(true) => Err(FmError::unsupported(
                        "Activating a smart collection is not available yet.",
                    )),
                    Some(false) => self.face_ids(
                        "SELECT face_id FROM collection_face WHERE collection_id = ?1
                         ORDER BY face_id",
                        collection.0,
                    ),
                }
            }
            Target::Set { set } => self.set_faces(*set),
        }
    }

    fn set_faces(&self, set: SetId) -> FmResult<Vec<FaceId>> {
        if !self.exists("SELECT 1 FROM project_set WHERE id = ?1", set.0)? {
            return Err(FmError::not_found(format!("Set {set}")));
        }
        self.face_ids(
            "SELECT face_id FROM set_face WHERE set_id = ?1 ORDER BY face_id",
            set.0,
        )
    }

    fn exists(&self, sql: &str, id: u32) -> FmResult<bool> {
        self.conn
            .prepare_cached(sql)
            .and_then(|mut s| s.exists([id]))
            .map_err(|e| db_error(&e))
    }

    fn face_ids(&self, sql: &str, id: u32) -> FmResult<Vec<FaceId>> {
        let mut stmt = self.conn.prepare_cached(sql).map_err(|e| db_error(&e))?;
        let ids = stmt
            .query_map([id], |row| row.get::<_, u32>(0).map(FaceId))
            .map_err(|e| db_error(&e))?
            .collect::<Result<_, _>>()
            .map_err(|e| db_error(&e))?;
        Ok(ids)
    }

    /// Up to `limit` faces activation can register (a healthy TrueType or
    /// OpenType face whose file is there and whose source is not paused), in
    /// id order. The bench times activating them.
    pub fn activatable_faces(&self, limit: u32) -> FmResult<Vec<FaceId>> {
        self.face_ids(
            "SELECT face.id FROM face
             JOIN font_file ON font_file.id = face.file_id
             JOIN source ON source.id = font_file.source_id
             WHERE face.healthy = 1 AND font_file.gone = 0 AND source.paused = 0
               AND font_file.format IN ('ttf', 'otf', 'ttc', 'otc')
             ORDER BY face.id LIMIT ?1",
            limit,
        )
    }

    /// The file, name and state of each face, in order. An unknown face is
    /// `NotFound`.
    pub(crate) fn activation_faces(&self, faces: &[FaceId]) -> FmResult<Vec<FaceRow>> {
        let mut stmt = self
            .conn
            .prepare_cached(FACE_ROW)
            .map_err(|e| db_error(&e))?;
        let mut out = Vec::with_capacity(faces.len());
        for &id in faces {
            let row = stmt
                .query_row([id.0], |row| {
                    let path = path_from_sql(row.get_ref(1)?);
                    let health: Option<String> = row.get(4)?;
                    Ok((
                        row.get::<_, String>(0)?,
                        path,
                        row.get::<_, String>(2)?,
                        row.get::<_, bool>(3)?,
                        health,
                        row.get::<_, bool>(5)?,
                        row.get::<_, bool>(6)?,
                    ))
                })
                .optional()
                .map_err(|e| db_error(&e))?
                .ok_or_else(|| FmError::not_found(format!("Face {id}")))?;
            let (postscript_name, path, format, healthy, health, gone, paused) = row;
            let unhealthy = (!healthy).then(|| unhealthy_reason(health.as_deref()));
            out.push(FaceRow {
                id,
                path: path?,
                postscript_name,
                // An unreadable format reads as "no font": it is not activated.
                format: format_from_sql(&format).unwrap_or(None),
                unhealthy,
                gone,
                paused,
            });
        }
        Ok(out)
    }

    /// Every face the app has active, in id order.
    pub(crate) fn active_rows(&self) -> FmResult<Vec<ActiveRow>> {
        let mut stmt = self
            .conn
            .prepare_cached(ACTIVE_ROWS)
            .map_err(|e| db_error(&e))?;
        let rows = stmt
            .query_map([], |row| {
                let mode: String = row.get(1)?;
                let registration = match (row.get_ref(4)?, row.get_ref(5)?) {
                    (ValueRef::Null, _) | (_, ValueRef::Null) => None,
                    (path, handle) => Some((path_from_sql(path), path_from_sql(handle))),
                };
                Ok((
                    FaceId(row.get(0)?),
                    mode,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    registration,
                ))
            })
            .map_err(|e| db_error(&e))?;
        let mut out = Vec::new();
        for row in rows {
            let (face, mode, since, document, registration) = row.map_err(|e| db_error(&e))?;
            let mode = mode_from_sql(&mode)?;
            let registration = match registration {
                None => None,
                Some((path, handle)) => Some(Registration {
                    path: path?,
                    mode: platform_mode(mode),
                    handle: OsString::from(handle?),
                }),
            };
            out.push(ActiveRow {
                face,
                mode,
                since,
                document,
                registration,
            });
        }
        Ok(out)
    }

    /// Records that each face is active with its OS registration, in one
    /// transaction. A face already listed takes the new mode and registration.
    pub(crate) fn record_activations(
        &mut self,
        rows: &[(FaceId, ActivationMode, Registration)],
    ) -> FmResult<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        {
            let mut activation = tx
                .prepare_cached(UPSERT_ACTIVATION)
                .map_err(|e| db_error(&e))?;
            let mut registration = tx
                .prepare_cached(UPSERT_REGISTRATION)
                .map_err(|e| db_error(&e))?;
            for (face, mode, reg) in rows {
                activation
                    .execute(params![face.0, mode_to_sql(*mode)])
                    .map_err(|e| db_error(&e))?;
                registration
                    .execute(params![
                        face.0,
                        path_to_sql(&reg.path),
                        path_to_sql(Path::new(&reg.handle))
                    ])
                    .map_err(|e| db_error(&e))?;
            }
        }
        tx.commit().map_err(|e| db_error(&e))
    }

    /// Forgets that these faces are active, in one transaction.
    pub(crate) fn remove_activations(&mut self, faces: &[FaceId]) -> FmResult<()> {
        if faces.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        {
            let mut stmt = tx
                .prepare_cached("DELETE FROM activation WHERE face_id = ?1")
                .map_err(|e| db_error(&e))?;
            for face in faces {
                stmt.execute([face.0]).map_err(|e| db_error(&e))?;
            }
        }
        tx.commit().map_err(|e| db_error(&e))
    }

    /// Replaces the faces "reactivate on launch" restores.
    pub(crate) fn set_last_session(&mut self, faces: &[FaceId]) -> FmResult<()> {
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        tx.execute("DELETE FROM last_session", [])
            .map_err(|e| db_error(&e))?;
        {
            let mut stmt = tx
                .prepare_cached("INSERT OR IGNORE INTO last_session (face_id) VALUES (?1)")
                .map_err(|e| db_error(&e))?;
            for face in faces {
                stmt.execute([face.0]).map_err(|e| db_error(&e))?;
            }
        }
        tx.commit().map_err(|e| db_error(&e))
    }

    /// The faces that were session-active when the app last ran, in id order.
    pub(crate) fn last_session(&self) -> FmResult<Vec<FaceId>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT face_id FROM last_session ORDER BY face_id")
            .map_err(|e| db_error(&e))?;
        let ids = stmt
            .query_map([], |row| row.get::<_, u32>(0).map(FaceId))
            .map_err(|e| db_error(&e))?
            .collect::<Result<_, _>>()
            .map_err(|e| db_error(&e))?;
        Ok(ids)
    }
}

/// The OS mode of an API mode.
pub(crate) fn platform_mode(mode: ActivationMode) -> Mode {
    match mode {
        ActivationMode::Session => Mode::Session,
        ActivationMode::Permanent => Mode::Permanent,
    }
}

fn mode_to_sql(mode: ActivationMode) -> &'static str {
    match mode {
        ActivationMode::Session => "session",
        ActivationMode::Permanent => "permanent",
    }
}

fn mode_from_sql(text: &str) -> FmResult<ActivationMode> {
    match text {
        "session" => Ok(ActivationMode::Session),
        "permanent" => Ok(ActivationMode::Permanent),
        other => Err(FmError::internal(format!(
            "unknown activation mode {other:?}"
        ))),
    }
}

/// The plain-words reason of a face whose health is not `Ok`.
fn unhealthy_reason(health: Option<&str>) -> String {
    match health.and_then(|json| serde_json::from_str::<Health>(json).ok()) {
        Some(Health::Broken(reason) | Health::Unsupported(reason)) => reason,
        _ => "This font is damaged and cannot be activated.".to_owned(),
    }
}

#[cfg(test)]
mod tests;
