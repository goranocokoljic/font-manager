//! The rows of the license inventory (E15.4, [`crate::inventory`]):
//! [`Index::license_inventory`].
//!
//! The families are the ones search lists for an empty query: every visible
//! family, or with a collection (manual or smart) the families with a face
//! in it. A row describes the whole family, as the list and the declared
//! license do: its styles, formats and embedding are those of all its
//! visible faces, not only the ones in the collection.

use std::collections::HashMap;

use fm_api::common::{Embedding, Page};
use fm_api::search::{Filters, SearchRequest};
use fm_api::{CollectionId, FaceId};
use rusqlite::params_from_iter;

use super::{Index, db_error};
use crate::error::FmResult;
use crate::inventory::InventoryRow;
use crate::license;

/// What the inventory reads from one face that the family summary lacks.
struct FaceLicense {
    fs_type: u16,
    url: Option<String>,
}

impl Index {
    /// One row per family, A to Z: every visible family, or those with a
    /// face in `collection`. An unknown collection is `NotFound`; a smart
    /// collection whose rule leads back to itself is `InvalidRequest`.
    pub fn license_inventory(
        &self,
        collection: Option<CollectionId>,
    ) -> FmResult<Vec<InventoryRow>> {
        let found = self.search(&SearchRequest {
            query: String::new(),
            filters: Filters {
                collection,
                ..Filters::default()
            },
            page: Page {
                offset: 0,
                limit: u32::MAX,
            },
            rule: None,
        })?;
        let mut rows = Vec::with_capacity(found.families.len());
        for hit in found.families {
            let family = hit.family;
            let faces = self.family_faces(family.id)?.unwrap_or(hit.faces);
            let facts = self.face_licenses(&faces)?;
            let mut embedding: Vec<Embedding> = Vec::new();
            for e in faces
                .iter()
                .filter_map(|f| facts.get(f))
                .map(|f| license::embedding(f.fs_type))
            {
                if !embedding.contains(&e) {
                    embedding.push(e);
                }
            }
            embedding.sort_by_key(|e| embedding_rank(*e));
            let license_url = faces
                .iter()
                .filter_map(|f| facts.get(f)?.url.clone())
                .next();
            let declared = self.declared_license(&family.name)?.unwrap_or_default();
            rows.push(InventoryRow {
                foundry: family.foundry,
                styles: family.style_count,
                formats: family.formats,
                // The family's declaration (E15.2) wins over a face's own.
                declared: declared.license_type.or(family.declared_license),
                seats: declared.seats,
                purchase_date: declared.purchase_date,
                receipt: declared.receipt.map(|r| r.display().to_string()),
                open_license: family.open_license,
                embedding,
                license_url,
                source_folder: family.location,
                family: family.name,
            });
        }
        rows.sort_by_cached_key(|r| r.family.to_lowercase());
        Ok(rows)
    }

    /// The `OS/2.fsType` and license URL of each of `faces`.
    fn face_licenses(&self, faces: &[FaceId]) -> FmResult<HashMap<FaceId, FaceLicense>> {
        if faces.is_empty() {
            return Ok(HashMap::new());
        }
        let marks = vec!["?"; faces.len()].join(",");
        let sql = format!(
            "SELECT id, fstype,
                    CASE WHEN json_valid(parsed) THEN json_extract(parsed, '$.licenseUrl') END
             FROM face WHERE id IN ({marks})"
        );
        let mut stmt = self.conn.prepare(&sql).map_err(|e| db_error(&e))?;
        let rows = stmt
            .query_map(params_from_iter(faces.iter().map(|f| f.0)), |row| {
                Ok((
                    FaceId(row.get(0)?),
                    FaceLicense {
                        fs_type: row.get::<_, Option<u16>>(1)?.unwrap_or(0),
                        url: row
                            .get::<_, Option<String>>(2)?
                            .map(|u| u.trim().to_owned())
                            .filter(|u| !u.is_empty()),
                    },
                ))
            })
            .map_err(|e| db_error(&e))?;
        rows.collect::<Result<_, _>>().map_err(|e| db_error(&e))
    }
}

/// Most permissive first, as `OS/2.fsType` ranks them.
fn embedding_rank(e: Embedding) -> u8 {
    match e {
        Embedding::Installable => 0,
        Embedding::Editable => 1,
        Embedding::PreviewAndPrint => 2,
        Embedding::Restricted => 3,
    }
}
