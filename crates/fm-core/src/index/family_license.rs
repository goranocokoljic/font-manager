//! Declared licenses (E15.2): the license columns of the `family_meta` table.
//!
//! The license the user declares (type, seats, purchase date, receipt) belongs
//! to the family, like its notes (`index/family_notes.rs`): it is stored by
//! the family's grouping key ([`family_key`]), so it applies to every face of
//! the family, and a rescan or a rebuilt index finds it by the family's name.
//!
//! Every write is mirrored to the family's sidecars (`index/sidecars.rs`). In
//! a sidecar the receipt is written relative to that source's `.fontlibrary`
//! folder when the copy is in it, so a synced folder carries it to teammates
//! ([`DeclaredLicense::to_sidecar`]).
//!
//! The receipt file itself is copied and trashed by the API
//! (`crate::receipt`); the index only stores where the copy is.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use fm_api::library::LicenseType;
use fm_api::{FaceId, FamilyId, SourceId};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::family_notes::{delete_empty_row, faces_of_family};
use super::files::{path_from_sql, path_to_sql};
use super::{Index, db_error};
use crate::error::{FmError, FmResult};
use crate::grouping::family_key;
use crate::sidecar::{self, Field, SIDECAR_DIR};

/// The license the user declared for a family.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeclaredLicense {
    /// The license type.
    pub license_type: Option<LicenseType>,
    /// Seats.
    pub seats: Option<u32>,
    /// Purchase date, `YYYY-MM-DD`.
    pub purchase_date: Option<String>,
    /// The receipt copy the app made.
    pub receipt: Option<PathBuf>,
}

impl DeclaredLicense {
    /// Nothing declared.
    pub fn is_empty(&self) -> bool {
        self.license_type.is_none()
            && self.seats.is_none()
            && self.purchase_date.is_none()
            && self.receipt.is_none()
    }

    /// The license as the sidecar of the source at `root` writes it.
    pub(super) fn to_sidecar(&self, root: &Path) -> sidecar::License {
        sidecar::License {
            license_type: self.license_type,
            seats: self.seats,
            purchase_date: self.purchase_date.clone(),
            receipt: self
                .receipt
                .as_deref()
                .and_then(|r| receipt_to_sidecar(r, root)),
        }
    }

    /// The license the sidecar of the source at `root` holds.
    pub(super) fn from_sidecar(license: &sidecar::License, root: &Path) -> Self {
        Self {
            license_type: license.license_type,
            seats: license.seats,
            purchase_date: license.purchase_date.clone(),
            receipt: license
                .receipt
                .as_deref()
                .map(|r| receipt_from_sidecar(r, root)),
        }
    }
}

/// What declaring a family's license needs to know first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LicenseTarget {
    /// What is declared now.
    pub current: Option<DeclaredLicense>,
    /// The `.fontlibrary` folder of the source of the family's first face;
    /// `None` when that source is a system folder, paused or offline.
    pub sidecar_dir: Option<PathBuf>,
}

impl Index {
    /// What is declared for `family` now, and where a receipt "next to the
    /// font" goes. A family the library does not show is `NotFound`.
    pub fn license_target(&self, family: FamilyId) -> FmResult<LicenseTarget> {
        let not_found = || FmError::not_found(format!("Family {family}"));
        let summary = self.family_summary(family)?.ok_or_else(not_found)?;
        let faces = self.family_faces(family)?.ok_or_else(not_found)?;
        let current = license_by_key(&self.conn, &family_key(&summary.name))?;
        let sidecar_dir = match faces.first() {
            Some(face) => self.face_source(*face)?.map(|src| src.join(SIDECAR_DIR)),
            None => None,
        };
        Ok(LicenseTarget {
            current,
            sidecar_dir,
        })
    }

    /// Declares the license of a family, replacing what was declared; an
    /// empty license clears it. Returns the family's visible faces, the ones
    /// the list shows again. A family the library does not show is
    /// `NotFound`. Declaring dismisses the family's sidecar conflicts on it.
    pub fn declare_license(
        &mut self,
        family: FamilyId,
        license: &DeclaredLicense,
    ) -> FmResult<Vec<FaceId>> {
        let not_found = || FmError::not_found(format!("Family {family}"));
        let summary = self.family_summary(family)?.ok_or_else(not_found)?;
        let faces = self.family_faces(family)?.ok_or_else(not_found)?;
        let key = family_key(&summary.name);
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        set_license(&tx, &key, Some(license))?;
        for face in faces_of_family(&tx, &key)? {
            tx.prepare_cached("INSERT OR IGNORE INTO sidecar_dirty (face_id) VALUES (?1)")
                .and_then(|mut s| s.execute([face]))
                .map_err(|e| db_error(&e))?;
            tx.prepare_cached("DELETE FROM sidecar_conflict WHERE face_id = ?1 AND field = ?2")
                .and_then(|mut s| s.execute(params![face, Field::FamilyLicense.as_str()]))
                .map_err(|e| db_error(&e))?;
        }
        tx.commit().map_err(|e| db_error(&e))?;
        self.mirror_sidecars();
        Ok(faces)
    }

    /// The license declared for the family named `family`, if any.
    pub(super) fn declared_license(&self, family: &str) -> FmResult<Option<DeclaredLicense>> {
        license_by_key(&self.conn, &family_key(family))
    }

    /// Every declared license type, by grouping key.
    pub(super) fn all_declared_types(&self) -> FmResult<HashMap<String, LicenseType>> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT family_key, license_type FROM family_meta WHERE license_type IS NOT NULL",
            )
            .map_err(|e| db_error(&e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| db_error(&e))?;
        let mut out = HashMap::new();
        for row in rows {
            let (key, name) = row.map_err(|e| db_error(&e))?;
            if let Some(license) = type_from_sql(&name) {
                out.insert(key, license);
            }
        }
        Ok(out)
    }

    /// The folder of the source of `face`, when it may hold a `.fontlibrary`
    /// folder: not a system source, not paused, and there.
    fn face_source(&self, face: FaceId) -> FmResult<Option<PathBuf>> {
        let source: Option<u32> = self
            .conn
            .query_row(
                "SELECT font_file.source_id FROM face
                 JOIN font_file ON font_file.id = face.file_id WHERE face.id = ?1",
                [face.0],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| db_error(&e))?;
        match source {
            Some(source) => self.sidecar_root(SourceId(source)),
            None => Ok(None),
        }
    }
}

/// The license stored for the family with grouping key `key`.
pub(super) fn license_by_key(conn: &Connection, key: &str) -> FmResult<Option<DeclaredLicense>> {
    let row = conn
        .prepare_cached(
            "SELECT license_type, seats, purchase_date, receipt FROM family_meta
             WHERE family_key = ?1",
        )
        .and_then(|mut s| {
            s.query_row([key], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Value>(3)?,
                ))
            })
            .optional()
        })
        .map_err(|e| db_error(&e))?;
    let Some((license_type, seats, purchase_date, receipt)) = row else {
        return Ok(None);
    };
    let receipt = match &receipt {
        Value::Null => None,
        Value::Text(t) => Some(PathBuf::from(t)),
        Value::Blob(b) => Some(path_from_sql(rusqlite::types::ValueRef::Blob(b))?),
        _ => None,
    };
    let license = DeclaredLicense {
        license_type: license_type.as_deref().and_then(type_from_sql),
        seats: seats.and_then(|s| u32::try_from(s).ok()),
        purchase_date,
        receipt,
    };
    Ok(Some(license).filter(|l| !l.is_empty()))
}

/// Stores the license of the family with grouping key `key`; none or an
/// empty license clears it, and a row left with nothing in it is deleted.
pub(super) fn set_license(
    tx: &Transaction<'_>,
    key: &str,
    license: Option<&DeclaredLicense>,
) -> FmResult<()> {
    let empty = DeclaredLicense::default();
    let license = license.unwrap_or(&empty);
    let receipt = license.receipt.as_deref().map_or(Value::Null, path_to_sql);
    tx.prepare_cached(
        "INSERT INTO family_meta (family_key, license_type, seats, purchase_date, receipt)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (family_key) DO UPDATE SET license_type = excluded.license_type,
             seats = excluded.seats, purchase_date = excluded.purchase_date,
             receipt = excluded.receipt",
    )
    .and_then(|mut s| {
        s.execute(params![
            key,
            license.license_type.map(type_to_sql),
            license.seats,
            license.purchase_date,
            receipt
        ])
    })
    .map_err(|e| db_error(&e))?;
    delete_empty_row(tx, key)
}

/// The `fm-api` serde name of a license type (`desktop`, `personalOnly`).
fn type_to_sql(license: LicenseType) -> String {
    serde_json::to_value(license)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn type_from_sql(name: &str) -> Option<LicenseType> {
    serde_json::from_value(serde_json::Value::String(name.to_owned())).ok()
}

/// `receipt` as a sidecar names it: relative to the `.fontlibrary` folder of
/// the source at `root`, with `/` separators, when it is in it; else as it
/// is. A path that is not UTF-8 is left out.
fn receipt_to_sidecar(receipt: &Path, root: &Path) -> Option<String> {
    match receipt.strip_prefix(root.join(SIDECAR_DIR)) {
        Ok(rel) => {
            let parts: Option<Vec<&str>> = rel
                .components()
                .map(|c| match c {
                    Component::Normal(part) => part.to_str(),
                    _ => None,
                })
                .collect();
            parts.map(|p| p.join("/"))
        }
        Err(_) => receipt.to_str().map(str::to_owned),
    }
}

/// The receipt a sidecar of the source at `root` names. A plain relative
/// path is in that source's `.fontlibrary` folder. Anything else (an
/// absolute path, or one written on another OS, or one that climbs out with
/// `..`) is kept as written: it round-trips to the sidecar unchanged and is
/// never resolved against this machine's folders.
fn receipt_from_sidecar(receipt: &str, root: &Path) -> PathBuf {
    let plain = !receipt.is_empty()
        && receipt
            .split('/')
            .all(|part| !matches!(part, "" | "." | "..") && !part.contains(['\\', ':']));
    if plain && !Path::new(receipt).is_absolute() {
        receipt
            .split('/')
            .fold(root.join(SIDECAR_DIR), |dir, part| dir.join(part))
    } else {
        PathBuf::from(receipt)
    }
}

#[cfg(test)]
mod tests;
