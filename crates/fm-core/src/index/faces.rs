//! Families and faces as the library and preview services return them
//! (E5.11): [`Index::family`], [`Index::face_detail`], [`Index::face_file`]
//! and [`Index::faces_of_source`].
//!
//! A family is what search groups ([`Index::list_families`]): its id is the
//! id of its first visible face. A face's stored `face.parsed` holds what
//! the inspector shows; the vertical metrics are not stored, so
//! [`Index::face_detail`] reads them from the file (one file, read only).

use std::collections::HashMap;
use std::path::PathBuf;

use fm_api::activation::ActivationMode;
use fm_api::common::{Embedding, FeatureInfo, Health, Metrics, Slant};
use fm_api::library::{FaceDetail, FaceSummary, Family, LicenseInfo, LicenseType, OpenLicense};
use fm_api::preview::FontSource;
use fm_api::search::{CoverageReport, CoverageRequest};
use fm_api::{FaceId, FamilyId, SourceId};
use rusqlite::{OptionalExtension, Row};

use super::files::{format_from_sql, path_from_sql};
use super::{Index, db_error};
use crate::coverage;
use crate::error::{FmError, FmResult};
use crate::parser::ParsedFace;
use crate::scanner::FileHash;

/// One face with what its summary and detail need.
const FACE: &str = "
SELECT face.id, face.family, face.subfamily, face.postscript_name, face.weight, face.width,
       face.slant, face.is_variable, font_file.format, activation.mode, face.parsed,
       face.healthy, font_file.path, face.collection_index, font_file.size, font_file.hash,
       face.fstype, face.foundry, face.designer, coalesce(user_meta.favorite, 0),
       user_meta.rating, user_meta.notes, user_meta.license_type
FROM face
JOIN font_file ON font_file.id = face.file_id
LEFT JOIN activation ON activation.face_id = face.id
LEFT JOIN user_meta ON user_meta.face_id = face.id
WHERE face.id = ?1";

/// A face's file: what the webview loads and the rasterizer reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceLocation {
    /// The file, where the user keeps it.
    pub path: PathBuf,
    /// The face's index in a collection file; 0 otherwise.
    pub collection_index: u32,
    /// The hash of the bytes last read; `None` until a scan read the file.
    pub hash: Option<FileHash>,
}

/// One face row read by [`FACE`].
struct FaceRow {
    summary: FaceSummary,
    parsed: Option<ParsedFace>,
    file: FaceLocation,
    size: u64,
    fs_type: u16,
    foundry: Option<String>,
    designer: Option<String>,
    favorite: bool,
    rating: Option<u8>,
    notes: Option<String>,
    license_type: Option<LicenseType>,
}

impl Index {
    /// The family `family` with every visible face, light to bold, narrow to
    /// wide, upright before italic. An id that is not a family's is `NotFound`.
    pub fn family(&self, family: FamilyId) -> FmResult<Family> {
        let not_found = || FmError::not_found(format!("Family {family}"));
        let summary = self.family_summary(family)?.ok_or_else(not_found)?;
        let ids = self.family_faces(family)?.ok_or_else(not_found)?;
        let mut faces = Vec::with_capacity(ids.len());
        let mut designer = None;
        for id in ids {
            let row = self.face_row(id)?;
            designer = designer.or(row.designer);
            faces.push(row.summary);
        }
        faces.sort_by_key(|f| (f.weight, f.width, f.slant != Slant::Upright, f.id));
        Ok(Family {
            summary,
            designer,
            faces,
        })
    }

    /// Everything the inspector shows about the face `face`. The metrics are
    /// read from the file; a file that cannot be read gives zero metrics and
    /// a warning in the log, not an error, so the rest still shows. An
    /// unknown face is `NotFound`.
    pub fn face_detail(&self, face: FaceId) -> FmResult<FaceDetail> {
        let row = self.face_row(face)?;
        let metrics = read_metrics(&row.file).unwrap_or_else(|e| {
            tracing::warn!(%face, error = %e, detail = ?e.detail(), "face metrics not read");
            Metrics {
                units_per_em: 0,
                ascender: 0,
                descender: 0,
                x_height: None,
                cap_height: None,
            }
        });
        let tags = self.face_tag_names(face)?;
        let code_point_count = u32::try_from(self.face_cmap(face)?.len()).ok();
        let parsed = row.parsed.as_ref();
        let text = |f: fn(&ParsedFace) -> &Option<String>| parsed.and_then(|p| f(p).clone());
        let license_description = text(|p| &p.license);
        let license_url = text(|p| &p.license_url);
        let typographic = parsed.is_some_and(|p| p.typographic_family.is_some());
        Ok(FaceDetail {
            path: row.file.path.display().to_string(),
            collection_index: row.file.collection_index,
            file_size: u32::try_from(row.size).unwrap_or(u32::MAX),
            hash: row.file.hash.map(|h| h.to_hex()).unwrap_or_default(),
            version: text(|p| &p.version),
            copyright: text(|p| &p.copyright),
            designer: row.designer,
            vendor_id: text(|p| &p.vendor_id),
            license: LicenseInfo {
                open_license: open_license(license_description.as_deref(), license_url.as_deref()),
                description: license_description,
                url: license_url,
                embedding: embedding(row.fs_type),
                declared: row.license_type,
            },
            metrics,
            axes: parsed.map(|p| p.axes.clone()).unwrap_or_default(),
            instances: parsed
                .map(|p| p.named_instances.clone())
                .unwrap_or_default(),
            features: parsed
                .map(|p| {
                    p.features
                        .iter()
                        .map(|tag| FeatureInfo {
                            tag: tag.clone(),
                            name: None,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            // Scripts are not stored per face yet (parked).
            scripts: Vec::new(),
            tags,
            favorite: row.favorite,
            rating: row.rating,
            notes: row.notes,
            // Name IDs 1 and 2 are "legacy" only beside a typographic name.
            legacy_family: typographic.then(|| text(|p| &p.family)).flatten(),
            legacy_style: typographic.then(|| text(|p| &p.subfamily)).flatten(),
            foundry: row.foundry,
            italic_angle: parsed.and_then(|p| p.italic_angle),
            code_point_count,
            face: row.summary,
        })
    }

    /// Which characters of `req.target` the face lacks, and per chosen
    /// language how much of it the face covers ([`coverage::report`]). An
    /// unknown face is `NotFound`, an unknown preset `InvalidRequest`.
    pub fn check_coverage(&self, req: &CoverageRequest) -> FmResult<CoverageReport> {
        let row = self.face_row(req.face)?;
        let set = self.face_cmap(req.face)?;
        let locl = row.parsed.map(|p| p.locl_languages).unwrap_or_default();
        coverage::report(req.face, &set, &locl, &req.target)
    }

    /// The file of the face `face`. An unknown face is `NotFound`.
    pub fn face_file(&self, face: FaceId) -> FmResult<FaceLocation> {
        Ok(self.face_row(face)?.file)
    }

    /// The webview's view of [`Index::face_file`].
    pub fn font_source(&self, face: FaceId) -> FmResult<FontSource> {
        let file = self.face_file(face)?;
        Ok(FontSource {
            face,
            path: file.path.display().to_string(),
            collection_index: file.collection_index,
            hash: file.hash.map(|h| h.to_hex()).unwrap_or_default(),
        })
    }

    /// Every face of the source `source` with its file, gone ones included:
    /// the before and after of a scan, to tell what it added and removed.
    pub fn faces_of_source(&self, source: SourceId) -> FmResult<HashMap<FaceId, PathBuf>> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT face.id, font_file.path FROM face
                 JOIN font_file ON font_file.id = face.file_id
                 WHERE font_file.source_id = ?1",
            )
            .map_err(|e| db_error(&e))?;
        let mut rows = stmt.query([source.0]).map_err(|e| db_error(&e))?;
        let mut out = HashMap::new();
        while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
            let id = FaceId(row.get(0).map_err(|e| db_error(&e))?);
            let path = path_from_sql(row.get_ref(1).map_err(|e| db_error(&e))?)?;
            out.insert(id, path);
        }
        Ok(out)
    }

    fn face_row(&self, face: FaceId) -> FmResult<FaceRow> {
        let mut stmt = self.conn.prepare_cached(FACE).map_err(|e| db_error(&e))?;
        let row = stmt
            .query_row([face.0], |row| Ok(read_row(row)))
            .optional()
            .map_err(|e| db_error(&e))?
            .ok_or_else(|| FmError::not_found(format!("Face {face}")))??;
        let mut row = row;
        if let Some((family, name)) = self.family_of(face)? {
            row.summary.family = family;
            row.summary.family_name = name;
        }
        Ok(row)
    }

    /// The names of the tags on `face`, A to Z.
    fn face_tag_names(&self, face: FaceId) -> FmResult<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT tag.name FROM face_tag JOIN tag ON tag.id = face_tag.tag_id
                 WHERE face_tag.face_id = ?1 ORDER BY tag.name COLLATE NOCASE",
            )
            .map_err(|e| db_error(&e))?;
        let names = stmt
            .query_map([face.0], |row| row.get(0))
            .map_err(|e| db_error(&e))?
            .collect::<Result<_, _>>()
            .map_err(|e| db_error(&e))?;
        Ok(names)
    }
}

/// A [`FACE`] row. A face that is not visible keeps its own id as its
/// family id until [`Index::face_row`] finds its family.
fn read_row(row: &Row<'_>) -> FmResult<FaceRow> {
    let e = |e: rusqlite::Error| db_error(&e);
    let id = FaceId(row.get(0).map_err(e)?);
    let family: String = row.get(1).map_err(e)?;
    let name =
        |v: Option<String>| -> Option<serde_json::Value> { v.map(serde_json::Value::String) };
    let slant: Option<Slant> =
        name(row.get(6).map_err(e)?).and_then(|v| serde_json::from_value(v).ok());
    let mode: Option<ActivationMode> =
        name(row.get(9).map_err(e)?).and_then(|v| serde_json::from_value(v).ok());
    let parsed: Option<ParsedFace> = row
        .get::<_, Option<String>>(10)
        .map_err(e)?
        .and_then(|json| serde_json::from_str(&json).ok());
    let healthy: bool = row.get(11).map_err(e)?;
    let health = match parsed.as_ref().map(|p| p.health.clone()) {
        Some(health) => health,
        None if healthy => Health::Ok,
        None => Health::Broken("The face could not be read.".into()),
    };
    let format: String = row.get(8).map_err(e)?;
    let format = format_from_sql(&format)
        .ok()
        .flatten()
        .ok_or_else(|| FmError::internal(format!("face {id}: unknown format {format}")))?;
    let hash: Option<Vec<u8>> = row.get(15).map_err(e)?;
    let license_type: Option<LicenseType> =
        name(row.get(22).map_err(e)?).and_then(|v| serde_json::from_value(v).ok());
    Ok(FaceRow {
        summary: FaceSummary {
            id,
            family: FamilyId(id.0),
            family_name: family,
            style: row.get(2).map_err(e)?,
            postscript_name: row.get(3).map_err(e)?,
            weight: row.get(4).map_err(e)?,
            width: row.get(5).map_err(e)?,
            slant: slant.unwrap_or(Slant::Upright),
            format,
            is_variable: row.get(7).map_err(e)?,
            active: mode,
            health,
        },
        parsed,
        file: FaceLocation {
            path: path_from_sql(row.get_ref(12).map_err(e)?)?,
            collection_index: row.get(13).map_err(e)?,
            hash: hash
                .and_then(|h| <[u8; 32]>::try_from(h).ok())
                .map(FileHash::from),
        },
        size: row.get::<_, i64>(14).map_err(e)?.try_into().unwrap_or(0),
        fs_type: row.get(16).map_err(e)?,
        foundry: row.get(17).map_err(e)?,
        designer: row.get(18).map_err(e)?,
        favorite: row.get(19).map_err(e)?,
        rating: row.get(20).map_err(e)?,
        notes: row.get(21).map_err(e)?,
        license_type,
    })
}

fn read_metrics(file: &FaceLocation) -> FmResult<Metrics> {
    let data = std::fs::read(&file.path)
        .map_err(|e| FmError::io("The font file could not be read.", &file.path, &e))?;
    crate::preview::face_metrics(&data, file.collection_index)
}

/// The embedding permission `OS/2.fsType` grants: the least restrictive bit
/// set wins, as the OpenType spec says; no bit is installable.
fn embedding(fs_type: u16) -> Embedding {
    if fs_type & 0x0008 != 0 {
        Embedding::Editable
    } else if fs_type & 0x0004 != 0 {
        Embedding::PreviewAndPrint
    } else if fs_type & 0x0002 != 0 {
        Embedding::Restricted
    } else {
        Embedding::Installable
    }
}

/// The open license a font's license description (name ID 13) or URL
/// (name ID 14) names, when it names one.
pub(super) fn open_license(description: Option<&str>, url: Option<&str>) -> Option<OpenLicense> {
    let text = format!(
        "{} {}",
        description.unwrap_or_default(),
        url.unwrap_or_default()
    )
    .to_lowercase();
    if text.contains("open font license") || text.contains("/ofl") || text.contains("ofl.") {
        Some(OpenLicense::Ofl)
    } else if text.contains("apache license") || text.contains("apache.org/licenses") {
        Some(OpenLicense::Apache)
    } else if text.contains("ubuntu font licen") {
        Some(OpenLicense::Ufl)
    } else if text.contains("mit license") || text.contains("opensource.org/licenses/mit") {
        Some(OpenLicense::Mit)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedding_takes_the_least_restrictive_bit() {
        assert_eq!(embedding(0), Embedding::Installable);
        assert_eq!(embedding(0x0002), Embedding::Restricted);
        assert_eq!(embedding(0x0004), Embedding::PreviewAndPrint);
        assert_eq!(embedding(0x0008 | 0x0002), Embedding::Editable);
    }

    #[test]
    fn open_licenses_are_read_from_the_name_table_text() {
        assert_eq!(
            open_license(
                Some(
                    "This Font Software is licensed under the SIL Open Font License, Version 1.1."
                ),
                None
            ),
            Some(OpenLicense::Ofl)
        );
        assert_eq!(
            open_license(None, Some("https://openfontlicense.org")),
            None,
            "a bare site name is not a license statement"
        );
        assert_eq!(
            open_license(None, Some("http://www.apache.org/licenses/LICENSE-2.0")),
            Some(OpenLicense::Apache)
        );
        assert_eq!(open_license(Some("All rights reserved."), None), None);
    }
}
