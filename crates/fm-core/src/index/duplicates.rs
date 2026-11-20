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

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use fm_api::library::{CopyFacts, DuplicateCopy, DuplicateGroup, DuplicateKind};
use fm_api::{FaceId, SourceId};
use rusqlite::Row;
use rusqlite::types::{Value, ValueRef};

use super::files::{format_from_sql, path_from_sql};
use super::{Index, db_error};
use crate::duplicate_sets::{SetFace, SetFile, in_system_folder};
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

/// The files the library shows that have been read, each with its faces in
/// collection order (a file without faces once, with NULL face columns), for
/// [`Index::duplicate_set_files`].
const SET_FILES: &str = "
SELECT font_file.id, font_file.path, font_file.size, font_file.hash, font_file.format,
       font_file.source_id, source.kind, face.id, face.postscript_name,
       nullif(trim(CASE WHEN json_valid(face.parsed)
                        THEN json_extract(face.parsed, '$.version') END), '')
FROM font_file
JOIN source ON source.id = font_file.source_id
LEFT JOIN face ON face.file_id = font_file.id
WHERE font_file.gone = 0 AND source.paused = 0 AND font_file.hash IS NOT NULL
ORDER BY font_file.id, face.collection_index";

/// What [`Index::copy_facts`] reads of one face, its file and its user data.
const COPY_FACTS: &str = "
SELECT font_file.mtime,
       CASE WHEN json_valid(face.parsed) THEN json_extract(face.parsed, '$.glyphCount') END,
       face.is_variable, face.axes, face.features, activation.mode,
       coalesce(user_meta.favorite, 0), user_meta.rating,
       coalesce(trim(user_meta.notes), '') <> '', face.languages
FROM face
JOIN font_file ON font_file.id = face.file_id
LEFT JOIN activation ON activation.face_id = face.id
LEFT JOIN user_meta ON user_meta.face_id = face.id
WHERE face.id = ?1";

/// The names a face carries, one query per kind, each sorted.
const FACE_TAGS: &str = "SELECT tag.name FROM face_tag JOIN tag ON tag.id = face_tag.tag_id
WHERE face_tag.face_id = ?1 ORDER BY tag.name COLLATE NOCASE, tag.name";
const FACE_COLLECTIONS: &str = "SELECT collection.name FROM collection_face
JOIN collection ON collection.id = collection_face.collection_id
WHERE collection_face.face_id = ?1 ORDER BY collection.name COLLATE NOCASE, collection.id";
const FACE_SETS: &str = "SELECT project_set.name FROM set_face
JOIN project_set ON project_set.id = set_face.set_id
WHERE set_face.face_id = ?1 ORDER BY project_set.name COLLATE NOCASE, project_set.id";
const FACE_DOCUMENTS: &str =
    "SELECT document FROM activation_document WHERE face_id = ?1 ORDER BY seq DESC, document";

/// The tags of a stored JSON list: strings as they are, objects by `tag`
/// (`face.axes` holds axes, `face.features` tags).
fn tags_from_json(text: &str) -> Vec<String> {
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str(text) else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|v| match v {
            serde_json::Value::String(s) => Some(s),
            serde_json::Value::Object(mut o) => match o.remove("tag") {
                Some(serde_json::Value::String(s)) => Some(s),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

impl Index {
    /// The facts a decision between the copies of a duplicate set needs
    /// (E56.2), for each of `faces` the index holds: its file's time, its
    /// glyphs, axes and features, its activation, the user's data on it, and
    /// its language coverage (complete presets, and presets with `locl`).
    /// A face not in the index is left out.
    pub fn copy_facts(&self, faces: &[FaceId]) -> FmResult<HashMap<FaceId, CopyFacts>> {
        let mut out = HashMap::with_capacity(faces.len());
        {
            let mut main = self
                .conn
                .prepare_cached(COPY_FACTS)
                .map_err(|e| db_error(&e))?;
            for &face in faces {
                if out.contains_key(&face) {
                    continue;
                }
                let mut rows = main.query([face.0]).map_err(|e| db_error(&e))?;
                let Some(row) = rows.next().map_err(|e| db_error(&e))? else {
                    continue;
                };
                out.insert(face, facts_from_row(row)?);
            }
        }
        for (query, field) in [
            (FACE_TAGS, Names::Tags),
            (FACE_COLLECTIONS, Names::Collections),
            (FACE_SETS, Names::Sets),
            (FACE_DOCUMENTS, Names::Documents),
        ] {
            let mut stmt = self.conn.prepare_cached(query).map_err(|e| db_error(&e))?;
            for (face, facts) in &mut out {
                let names = stmt
                    .query_map([face.0], |row| row.get::<_, String>(0))
                    .and_then(Iterator::collect::<Result<Vec<_>, _>>)
                    .map_err(|e| db_error(&e))?;
                *match field {
                    Names::Tags => &mut facts.tags,
                    Names::Collections => &mut facts.collections,
                    Names::Sets => &mut facts.sets,
                    Names::Documents => &mut facts.activation_documents,
                } = names;
            }
        }
        Ok(out)
    }

    /// The files the library shows (not gone, source not paused, read), with
    /// their faces, as [`crate::duplicate_sets::sets`] takes them (E56.1). A
    /// file is a system copy when its source is the system fonts or its path
    /// is in one of `system` ([`crate::duplicate_sets::system_folders`]).
    pub fn duplicate_set_files(&self, system: &[PathBuf]) -> FmResult<Vec<SetFile>> {
        let mut stmt = self.conn.prepare(SET_FILES).map_err(|e| db_error(&e))?;
        let mut rows = stmt.query([]).map_err(|e| db_error(&e))?;
        let mut out: Vec<SetFile> = Vec::new();
        let mut last: Option<i64> = None;
        while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
            let get_err = |e: rusqlite::Error| db_error(&e);
            let id: i64 = row.get(0).map_err(get_err)?;
            if last != Some(id) {
                last = Some(id);
                let hash: Vec<u8> = row.get(3).map_err(get_err)?;
                let hash = <[u8; 32]>::try_from(hash.as_slice())
                    .map_err(|_| FmError::internal("a font file hash in the index is damaged"))?;
                let size: i64 = row.get(2).map_err(get_err)?;
                let format: String = row.get(4).map_err(get_err)?;
                let kind: String = row.get(6).map_err(get_err)?;
                let path = path_from_sql(row.get_ref(1).map_err(get_err)?)?;
                out.push(SetFile {
                    system: kind == "system" || in_system_folder(&path, system),
                    path: path.to_string_lossy().into_owned(),
                    size: u32::try_from(size).unwrap_or(u32::MAX),
                    hash: FileHash::from(hash).to_hex(),
                    format: format_from_sql(&format).unwrap_or(None),
                    source: SourceId(row.get(5).map_err(get_err)?),
                    in_library_folder: kind == "libraryFolder",
                    faces: Vec::new(),
                });
            }
            let face: Option<u32> = row.get(7).map_err(get_err)?;
            if let (Some(face), Some(file)) = (face, out.last_mut()) {
                file.faces.push(SetFace {
                    face: FaceId(face),
                    name: row
                        .get::<_, Option<String>>(8)
                        .map_err(get_err)?
                        .unwrap_or_default(),
                    version: row.get(9).map_err(get_err)?,
                });
            }
        }
        Ok(out)
    }

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

/// Which names [`Index::copy_facts`] reads in one pass.
#[derive(Clone, Copy)]
enum Names {
    Tags,
    Collections,
    Sets,
    Documents,
}

/// The facts of one [`COPY_FACTS`] row; the name lists are filled after.
fn facts_from_row(row: &Row<'_>) -> FmResult<CopyFacts> {
    let get_err = |e: rusqlite::Error| db_error(&e);
    let mtime: Option<i64> = row.get(0).map_err(get_err)?;
    let glyphs: Option<i64> = row.get(1).map_err(get_err)?;
    let axes: String = row.get(3).map_err(get_err)?;
    let features: String = row.get(4).map_err(get_err)?;
    let mode: Option<String> = row.get(5).map_err(get_err)?;
    let rating: Option<i64> = row.get(7).map_err(get_err)?;
    let languages: Option<String> = row.get(9).map_err(get_err)?;
    let languages = languages
        .as_deref()
        .and_then(super::languages::from_sql)
        .unwrap_or_default();
    let mut features = tags_from_json(&features);
    features.sort();
    features.dedup();
    Ok(CopyFacts {
        modified: mtime
            .filter(|n| *n >= 0)
            .map(|n| crate::catalog::rfc3339(n.unsigned_abs() / 1_000_000_000)),
        glyph_count: glyphs.and_then(|n| u32::try_from(n).ok()),
        variable: row.get(2).map_err(get_err)?,
        axes: tags_from_json(&axes),
        features,
        activation: mode
            .as_deref()
            .map(super::activation::mode_from_sql)
            .transpose()?,
        activation_documents: Vec::new(),
        favorite: row.get(6).map_err(get_err)?,
        rating: rating.and_then(|r| u8::try_from(r).ok()),
        tags: Vec::new(),
        has_notes: row.get(8).map_err(get_err)?,
        collections: Vec::new(),
        sets: Vec::new(),
        languages_full: languages
            .iter()
            .filter(|l| l.is_complete())
            .map(|l| l.code.clone())
            .collect(),
        languages_locl: languages
            .iter()
            .filter(|l| l.has_locl)
            .map(|l| l.code.clone())
            .collect(),
    })
}

#[cfg(test)]
mod tests;
