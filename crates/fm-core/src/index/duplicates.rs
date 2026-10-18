//! Duplicate detection (E8.1): files that hold the same font, in groups.
//!
//! - An **exact** group is files with the same hash at different paths.
//! - A **near** group is faces with the same PostScript name and the same
//!   version (trimmed; no version counts as one value) in files with at least
//!   two different hashes. Files with the same hash alone are an exact group.
//! - An exact group whose files all sit in one near group is not listed
//!   again: the near group already lists them, and their equal hashes show
//!   they are identical. So a file is listed once per font it duplicates.
//!
//! Only the files the library shows count (not gone, source not paused), and
//! only once they are read (`hash` set). Each group's keeper is chosen by
//! [`DuplicateGroup::choose_keeper`], the one rule of the contract.

use std::collections::HashSet;

use fm_api::library::{DuplicateCopy, DuplicateGroup, DuplicateKind};
use fm_api::{FaceId, SourceId};
use rusqlite::Row;
use rusqlite::types::{Value, ValueRef};

use super::files::path_from_sql;
use super::{Index, db_error};
use crate::error::{FmError, FmResult};
use crate::scanner::FileHash;

/// The files the library shows that have been read, as `visible`, and their
/// faces with a PostScript name, as `named`. Every query below selects the
/// same columns, read by [`Copy::from_row`].
const VISIBLE: &str = "
WITH visible AS (
    SELECT font_file.id, font_file.path, font_file.size, font_file.hash,
           font_file.source_id, source.kind = 'libraryFolder' AS library
    FROM font_file JOIN source ON source.id = font_file.source_id
    WHERE font_file.gone = 0 AND source.paused = 0 AND font_file.hash IS NOT NULL
),
named AS (
    SELECT visible.*, face.id AS face_id, face.collection_index, face.postscript_name AS name,
           nullif(trim(CASE WHEN json_valid(face.parsed)
                            THEN json_extract(face.parsed, '$.version') END), '') AS version
    FROM visible JOIN face ON face.file_id = visible.id
    WHERE face.postscript_name <> ''
)";

/// Files whose hash another visible file has, with their first face.
const EXACT: &str = "
SELECT visible.id, visible.path, visible.size, visible.hash, visible.source_id, visible.library,
       first.face_id, first.name, first.version
FROM visible
LEFT JOIN named AS first ON first.id = visible.id AND first.collection_index =
    (SELECT min(collection_index) FROM named WHERE named.id = visible.id)
WHERE visible.hash IN (SELECT hash FROM visible GROUP BY hash HAVING COUNT(*) > 1)
ORDER BY visible.hash, visible.id";

/// Faces whose PostScript name and version appear in files with different
/// hashes, one group after another.
const NEAR: &str = "
SELECT id, path, size, hash, source_id, library, face_id, name, version
FROM named
WHERE (name, coalesce(version, '')) IN (
    SELECT name, coalesce(version, '') FROM named
    GROUP BY 1, 2 HAVING COUNT(DISTINCT hash) > 1)
ORDER BY name, coalesce(version, ''), id, collection_index";

impl Index {
    /// Exact and near duplicates among the files the library shows: near
    /// groups by PostScript name, then exact groups by hash; each group's
    /// recommended keeper first.
    pub fn duplicates(&self) -> FmResult<Vec<DuplicateGroup>> {
        let mut groups = Vec::new();
        let mut near_files: Vec<HashSet<i64>> = Vec::new();
        for rows in group_by(self.copies(NEAR)?, |c| {
            (c.name.clone(), c.copy.version.clone())
        }) {
            near_files.push(rows.iter().map(|c| c.file).collect());
            groups.push(group(DuplicateKind::Near, rows));
        }
        for rows in group_by(self.copies(EXACT)?, |c| c.copy.hash.clone()) {
            if near_files
                .iter()
                .any(|files| rows.iter().all(|c| files.contains(&c.file)))
            {
                continue;
            }
            groups.push(group(DuplicateKind::Exact, rows));
        }
        Ok(groups)
    }

    fn copies(&self, query: &str) -> FmResult<Vec<Copy>> {
        let mut stmt = self
            .conn
            .prepare(&format!("{VISIBLE}{query}"))
            .map_err(|e| db_error(&e))?;
        let rows = stmt.query_map([], Copy::raw).map_err(|e| db_error(&e))?;
        rows.map(|row| row.map_err(|e| db_error(&e)).and_then(Copy::from_raw))
            .collect()
    }
}

/// One row of [`EXACT`] or [`NEAR`]: a copy and what groups it.
struct Copy {
    file: i64,
    name: Option<String>,
    copy: DuplicateCopy,
}

/// The columns of a row before the path and hash are decoded.
type Raw = (
    i64,
    Value,
    i64,
    Vec<u8>,
    u32,
    bool,
    Option<u32>,
    Option<String>,
    Option<String>,
);

impl Copy {
    fn raw(row: &Row<'_>) -> rusqlite::Result<Raw> {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
            row.get(7)?,
            row.get(8)?,
        ))
    }

    fn from_raw(raw: Raw) -> FmResult<Self> {
        let (file, path, size, hash, source, library, face, name, version) = raw;
        let hash = <[u8; 32]>::try_from(hash.as_slice())
            .map_err(|_| FmError::internal("a font file hash in the index is damaged"))?;
        Ok(Self {
            file,
            name: name.clone(),
            copy: DuplicateCopy {
                path: path_from_sql(ValueRef::from(&path))?
                    .to_string_lossy()
                    .into_owned(),
                size: u32::try_from(size).unwrap_or(u32::MAX),
                hash: FileHash::from(hash).to_hex(),
                face: face.map(FaceId),
                version,
                source: SourceId(source),
                in_library_folder: library,
                keep: false,
            },
        })
    }
}

/// Runs of rows with the same key, in row order.
fn group_by<K: PartialEq>(rows: Vec<Copy>, key: impl Fn(&Copy) -> K) -> Vec<Vec<Copy>> {
    let mut runs: Vec<(K, Vec<Copy>)> = Vec::new();
    for row in rows {
        let k = key(&row);
        match runs.last_mut() {
            Some((last, run)) if *last == k => run.push(row),
            _ => runs.push((k, vec![row])),
        }
    }
    runs.into_iter().map(|(_, run)| run).collect()
}

/// A group of one file per copy (a collection file with the name twice is
/// listed once), keeper first.
fn group(kind: DuplicateKind, rows: Vec<Copy>) -> DuplicateGroup {
    let postscript_name = rows.iter().find_map(|c| c.name.clone());
    let mut files = HashSet::new();
    let mut group = DuplicateGroup {
        kind,
        postscript_name,
        copies: rows
            .into_iter()
            .filter(|c| files.insert(c.file))
            .map(|c| c.copy)
            .collect(),
    };
    group.choose_keeper();
    group
}

#[cfg(test)]
mod tests;
