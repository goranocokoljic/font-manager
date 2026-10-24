//! Catalog downloads (E22.3): the `catalog_download` and
//! `catalog_download_file` tables.
//!
//! A row records where a family downloaded from an online catalog came from:
//! the catalog, its id of the family, its version, the kind of files, the
//! date, and the name and blake3 hash of every file written. Nothing links
//! it to `font_file`, so a rescan or a rebuild never drops it; a file is
//! found again when the list is read, by its hash among the files the
//! library has (so a file the user moved is followed), else by the path it
//! was written to (after a rebuild, before the rescan has read it again).

use std::path::Path;

use fm_api::SourceId;
use fm_api::catalog::{CatalogDownload, CatalogDownloadFile, CatalogFileKind, CatalogId};
use rusqlite::{OptionalExtension, params};

use super::files::{path_from_sql, path_to_sql};
use super::{Index, db_error};
use crate::catalog::Downloaded;
use crate::error::{FmError, FmResult};

/// The `catalog_download.catalog` value of a catalog (its serde name).
fn catalog_to_sql(id: CatalogId) -> FmResult<String> {
    match serde_json::to_value(id) {
        Ok(serde_json::Value::String(s)) => Ok(s),
        _ => Err(FmError::internal(format!("catalog id {id:?}"))),
    }
}

/// The catalog of a `catalog_download.catalog` value; `None` for a catalog
/// this build does not know (a newer build wrote it).
fn catalog_from_sql(value: &str) -> Option<CatalogId> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).ok()
}

fn kind_to_sql(kind: CatalogFileKind) -> &'static str {
    match kind {
        CatalogFileKind::Static => "static",
        CatalogFileKind::Variable => "variable",
    }
}

fn kind_from_sql(value: &str) -> CatalogFileKind {
    if value == "variable" {
        CatalogFileKind::Variable
    } else {
        CatalogFileKind::Static
    }
}

fn hex(hash: &[u8]) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

impl Index {
    /// Records what a download of `family` from `catalog` into `source`
    /// wrote, at `downloaded_at` (RFC 3339, UTC).
    pub fn record_catalog_download(
        &mut self,
        catalog: CatalogId,
        family: &str,
        source: SourceId,
        downloaded_at: &str,
        got: &Downloaded,
    ) -> FmResult<()> {
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        tx.execute(
            "INSERT INTO catalog_download
                 (catalog, family, catalog_family_id, version, kind, downloaded_at, source_id, folder)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                catalog_to_sql(catalog)?,
                family,
                got.family_id,
                got.version,
                kind_to_sql(got.kind),
                downloaded_at,
                source.0,
                path_to_sql(&got.folder),
            ],
        )
        .map_err(|e| db_error(&e))?;
        let id = tx.last_insert_rowid();
        {
            let mut insert = tx
                .prepare_cached(
                    "INSERT OR IGNORE INTO catalog_download_file (download_id, name, hash, path)
                     VALUES (?1, ?2, ?3, ?4)",
                )
                .map_err(|e| db_error(&e))?;
            for file in &got.written {
                insert
                    .execute(params![
                        id,
                        file.name,
                        file.hash.as_slice(),
                        path_to_sql(&file.path)
                    ])
                    .map_err(|e| db_error(&e))?;
            }
        }
        tx.commit().map_err(|e| db_error(&e))
    }

    /// Every recorded download, newest first, each file with where the
    /// library has it now: the indexed file with its hash (the one at the
    /// path it was written to first), else an indexed file at that path,
    /// else `None`. A download of a catalog this build does not know is
    /// left out.
    pub fn catalog_downloads(&self) -> FmResult<Vec<CatalogDownload>> {
        let heads = {
            let mut stmt = self
                .conn
                .prepare_cached(
                    "SELECT id, catalog, family, catalog_family_id, version, kind,
                            downloaded_at, source_id, folder
                     FROM catalog_download ORDER BY downloaded_at DESC, id DESC",
                )
                .map_err(|e| db_error(&e))?;
            stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, u32>(7)?,
                    path_from_sql(row.get_ref(8)?).map(|p| p.to_string_lossy().into_owned()),
                ))
            })
            .map_err(|e| db_error(&e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| db_error(&e))?
        };
        let mut out = Vec::with_capacity(heads.len());
        for (id, catalog, family, family_id, version, kind, at, source, folder) in heads {
            let Some(catalog) = catalog_from_sql(&catalog) else {
                continue;
            };
            out.push(CatalogDownload {
                catalog,
                family,
                catalog_family_id: family_id,
                version,
                kind: kind_from_sql(&kind),
                downloaded_at: at,
                source: SourceId(source),
                folder: folder?,
                files: self.catalog_download_files(id)?,
            });
        }
        Ok(out)
    }

    fn catalog_download_files(&self, id: i64) -> FmResult<Vec<CatalogDownloadFile>> {
        let rows = {
            let mut stmt = self
                .conn
                .prepare_cached(
                    "SELECT name, hash, path FROM catalog_download_file
                     WHERE download_id = ?1 ORDER BY name",
                )
                .map_err(|e| db_error(&e))?;
            stmt.query_map([id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    path_from_sql(row.get_ref(2)?),
                ))
            })
            .map_err(|e| db_error(&e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| db_error(&e))?
        };
        let mut files = Vec::with_capacity(rows.len());
        for (name, hash, written) in rows {
            let path = self.downloaded_file_now(&hash, &written?)?;
            files.push(CatalogDownloadFile {
                name,
                hash: hex(&hash),
                path: path.map(|p| p.to_string_lossy().into_owned()),
            });
        }
        Ok(files)
    }

    /// Where the library has the file with `hash` now, the one at `written`
    /// first; else a file at `written` whose hash is not known yet. A file
    /// the last scan did not find (`gone`) does not count.
    fn downloaded_file_now(
        &self,
        hash: &[u8],
        written: &Path,
    ) -> FmResult<Option<std::path::PathBuf>> {
        let written = path_to_sql(written);
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT path FROM font_file
                 WHERE gone = 0 AND (hash = ?1 OR (hash IS NULL AND path = ?2))
                 ORDER BY hash IS NULL, path <> ?2, path LIMIT 1",
            )
            .map_err(|e| db_error(&e))?;
        let found = stmt
            .query_row(params![hash, written], |row| {
                Ok(path_from_sql(row.get_ref(0)?))
            })
            .optional()
            .map_err(|e| db_error(&e))?;
        found.transpose()
    }
}
