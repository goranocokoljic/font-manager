//! Families and faces as the library and preview services return them
//! (E5.11): [`Index::family`], [`Index::face_detail`], [`Index::face_file`]
//! and [`Index::faces_of_source`].
//!
//! A family is what search groups ([`Index::list_families`]): its id is the
//! id of its first visible face. A face's stored `face.parsed` holds what
//! the inspector shows, the vertical metrics with it (E16.2); a face stored
//! without them (unreadable, or written before E16.2 and not scanned since)
//! has [`Index::face_detail`] read them from the file (one file, read only).

use std::collections::HashMap;
use std::path::PathBuf;

use fm_api::activation::ActivationMode;
use fm_api::common::{Embedding, Health, Metrics, Slant};
use fm_api::library::{FaceDetail, FaceSummary, Family, LicenseInfo, LicenseType, OpenLicense};
use fm_api::preview::FontSource;
use fm_api::search::{CoverageReport, CoverageRequest, FaceCoverage};
use fm_api::{FaceId, FamilyId, SourceId};
use rusqlite::{OptionalExtension, Row};

use super::files::{format_from_sql, path_from_sql};
use super::{Index, db_error};
use crate::coverage;
use crate::error::{FmError, FmResult};
use crate::features;
use crate::license;
use crate::palettes;
use crate::parser::ParsedFace;
use crate::scanner::FileHash;
use crate::web_css::FaceStyle;
use crate::web_font::{self, ExportFace};

/// One face with what its summary and detail need.
const FACE: &str = "
SELECT face.id, face.family, face.subfamily, face.postscript_name, face.weight, face.width,
       face.slant, face.is_variable, font_file.format, activation.mode, face.parsed,
       face.healthy, font_file.path, face.collection_index, font_file.size, font_file.hash,
       face.fstype, face.foundry, face.designer, coalesce(user_meta.favorite, 0),
       user_meta.rating, user_meta.notes, user_meta.license_type, face.open_license
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

/// A face to export as a web font ([`Index::web_export_face`]).
#[derive(Debug, Clone, PartialEq)]
pub struct WebExportFace {
    /// The face's file, output name and label.
    pub face: ExportFace,
    /// Its license keeps it off the web.
    pub restricted: bool,
    /// What its `@font-face` block says about it (E20.3).
    pub style: FaceStyle,
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
    /// The open license recognized when the face was scanned.
    open_license: Option<OpenLicense>,
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
    /// the ones the scan stored; without them they are read from the file,
    /// and a file that cannot be read gives zero metrics and a warning in the
    /// log, not an error, so the rest still shows. An unknown face is
    /// `NotFound`.
    pub fn face_detail(&self, face: FaceId) -> FmResult<FaceDetail> {
        let row = self.face_row(face)?;
        let is_color = row.parsed.as_ref().is_some_and(|p| p.color);
        let stored = row
            .parsed
            .as_ref()
            .map(|p| p.metrics)
            .filter(|m| m.units_per_em > 0);
        // The file is read only when the stored metrics are missing or the
        // face is a color face whose palettes the inspector shows.
        let data = (stored.is_none() || is_color).then(|| read_file(&row.file));
        let metrics = stored
            .map_or_else(
                || {
                    let index = row.file.collection_index;
                    match &data {
                        Some(d) => d
                            .as_ref()
                            .map_err(Clone::clone)
                            .and_then(|d| crate::preview::face_metrics(d, index)),
                        None => read_file(&row.file)
                            .and_then(|d| crate::preview::face_metrics(&d, index)),
                    }
                },
                Ok,
            )
            .unwrap_or_else(|e| {
                tracing::warn!(%face, error = %e, detail = ?e.detail(), "face metrics not read");
                Metrics {
                    units_per_em: 0,
                    ascender: 0,
                    descender: 0,
                    x_height: None,
                    cap_height: None,
                }
            });
        // Only a color face can have palettes; a plain one skips the read.
        let palettes = match (&data, is_color) {
            (Some(Ok(d)), true) => palettes::face_palettes(d, row.file.collection_index)
                .unwrap_or_else(|e| {
                    tracing::warn!(%face, error = %e, detail = ?e.detail(), "face palettes not read");
                    Vec::new()
                }),
            _ => Vec::new(),
        };
        let tags = self.face_tag_names(face)?;
        let family_notes = self.family_notes(&row.summary.family_name)?;
        let declared = self
            .declared_license(&row.summary.family_name)?
            .unwrap_or_default();
        let code_point_count = u32::try_from(self.face_cmap(face)?.len()).ok();
        let parsed = row.parsed.as_ref();
        let text = |f: fn(&ParsedFace) -> &Option<String>| parsed.and_then(|p| f(p).clone());
        let license_description = text(|p| &p.license);
        let license_url = text(|p| &p.license_url);
        let typographic = parsed.is_some_and(|p| p.typographic_family.is_some());
        let embedding = license::embedding(row.fs_type);
        // The family's declaration (E15.2) wins over a face's own.
        let declared_type = declared.license_type.or(row.license_type);
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
                open_license: row.open_license,
                description: license_description,
                url: license_url,
                embedding,
                declared: declared_type,
                seats: declared.seats,
                purchase_date: declared.purchase_date,
                receipt: declared.receipt.map(|r| r.display().to_string()),
                web_use: Some(
                    license::web_use(embedding, row.open_license, declared_type).to_api(),
                ),
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
                        .map(|tag| {
                            features::feature_info_named(
                                tag,
                                p.feature_names.get(tag).map(String::as_str),
                            )
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
            family_notes,
            palettes,
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

    /// The coverage list of `face` (`face_coverage`, E11.3) from its stored
    /// `cmap` and `locl` languages. An unknown face is `NotFound`.
    pub fn face_coverage(&self, face: FaceId) -> FmResult<FaceCoverage> {
        let row = self.face_row(face)?;
        let set = self.face_cmap(face)?;
        let locl = row.parsed.map(|p| p.locl_languages).unwrap_or_default();
        coverage::face_coverage(face, &set, &locl)
    }

    /// The file of the face `face`. An unknown face is `NotFound`.
    pub fn face_file(&self, face: FaceId) -> FmResult<FaceLocation> {
        Ok(self.face_row(face)?.file)
    }

    /// What a web export needs about the face `face` (E20.1): its file, its
    /// output name and label, whether its license keeps it off the web
    /// (a restricted embedding and no web license declared, the family's
    /// declaration first), and its `@font-face` style (E20.3): the
    /// typographic family (name ID 16, else 1, else the library's family
    /// name), slant, weight class and, for a variable face, its axes. An
    /// unknown face is `NotFound`.
    pub fn web_export_face(&self, face: FaceId) -> FmResult<WebExportFace> {
        let row = self.face_row(face)?;
        let declared = self
            .declared_license(&row.summary.family_name)?
            .and_then(|d| d.license_type)
            .or(row.license_type);
        let restricted = license::embedding(row.fs_type) == Embedding::Restricted
            && declared != Some(LicenseType::Web);
        let s = &row.summary;
        let parsed = row.parsed.as_ref();
        let style = FaceStyle {
            family: parsed
                .and_then(|p| p.typographic_family.clone().or_else(|| p.family.clone()))
                .filter(|f| !f.trim().is_empty())
                .unwrap_or_else(|| s.family_name.clone()),
            slant: s.slant,
            weight: s.weight,
            axes: if s.is_variable {
                parsed.map(|p| p.axes.clone()).unwrap_or_default()
            } else {
                Vec::new()
            },
        };
        Ok(WebExportFace {
            face: ExportFace {
                path: row.file.path,
                collection_index: row.file.collection_index,
                file_stem: web_font::file_stem(&s.postscript_name, u64::from(face.0)),
                label: format!("{} {}", s.family_name, s.style),
            },
            restricted,
            style,
        })
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

    /// The list row of the face `face`, grouped into its family as search
    /// groups it. An unknown face is `NotFound`.
    pub(super) fn face_summary(&self, face: FaceId) -> FmResult<FaceSummary> {
        Ok(self.face_row(face)?.summary)
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
    let open_license: Option<OpenLicense> =
        name(row.get(23).map_err(e)?).and_then(|v| serde_json::from_value(v).ok());
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
        open_license,
    })
}

fn read_file(file: &FaceLocation) -> FmResult<Vec<u8>> {
    std::fs::read(&file.path)
        .map_err(|e| FmError::io("The font file could not be read.", &file.path, &e))
}

/// An open license as `face.open_license` stores it: its `fm-api` serde name.
pub(super) fn open_license_to_sql(license: Option<OpenLicense>) -> Option<String> {
    license
        .and_then(|l| serde_json::to_value(l).ok())
        .and_then(|v| v.as_str().map(str::to_owned))
}

/// Fills `face.open_license` of the faces indexed before it existed, from
/// the license text their `face.parsed` holds (the V12 migration).
pub(super) fn fill_open_licenses(tx: &rusqlite::Transaction<'_>) -> rusqlite::Result<()> {
    let mut read = tx.prepare(
        "SELECT id, json_extract(parsed, '$.license'), json_extract(parsed, '$.licenseUrl')
         FROM face WHERE json_valid(parsed)",
    )?;
    let mut write = tx.prepare("UPDATE face SET open_license = ?2 WHERE id = ?1")?;
    let mut rows = read.query([])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let description: Option<String> = row.get(1)?;
        let url: Option<String> = row.get(2)?;
        let found = license::open_license(description.as_deref(), url.as_deref());
        if let Some(name) = open_license_to_sql(found) {
            write.execute(rusqlite::params![id, name])?;
        }
    }
    Ok(())
}
