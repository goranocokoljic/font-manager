//! Scan results in the index: the `font_file` and `face` tables.
//!
//! [`Index::persist_files`] writes what the scanner read ([`IndexedFile`]s)
//! in batches of [`BATCH_SIZE`] files, one transaction per batch: a batch
//! lands whole or not at all, and a 10k-font scan commits 20 times instead of
//! 10,000. A file is matched by its path, a face by its file and collection
//! index, so writing a file again keeps its face ids and with them the tags,
//! ratings, collections and activation state attached to them.
//!
//! [`Index::load_files`] reads the tables back into a [`FileIndex`], so the
//! next scan skips every file whose size and modification time are unchanged.
//! A row the scan cannot trust (no read stamp after [`Index::rebuild`], JSON
//! from another version) loads without a modification time: the scan reads
//! that file again.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fm_api::SourceId;
use fm_api::common::{FontFormat, Health};
use rusqlite::types::{Value, ValueRef};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::Serialize;

use super::{Index, db_error};
use crate::error::{FmError, FmResult};
use crate::grouping::{face_style, family_name};
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, FileIndex, IndexedFile, Rescan};

/// How many files one transaction writes.
pub const BATCH_SIZE: usize = 500;

/// The highest Unicode code point; a coverage range past it is damage.
const MAX_CODE_POINT: u32 = 0x10_FFFF;

/// What a write of scan results did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Persisted {
    /// Files written (or, for an unchanged file, marked found again).
    pub files: usize,
    /// Faces written.
    pub faces: usize,
    /// Transactions committed.
    pub batches: usize,
}

/// One row change of a batch.
enum Write<'a> {
    /// Write the file and its faces.
    File(&'a IndexedFile),
    /// The file was not read again; only clear its gone flag.
    Found(&'a Path),
}

const UPSERT_FILE: &str = "
INSERT INTO font_file (source_id, path, hash, format, size, mtime, health, gone)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
ON CONFLICT (path) DO UPDATE SET
    source_id = excluded.source_id, hash = excluded.hash, format = excluded.format,
    size = excluded.size, mtime = excluded.mtime, health = excluded.health,
    gone = excluded.gone
RETURNING id";

const UPSERT_FACE: &str = "
INSERT INTO face (file_id, collection_index, family, subfamily, typographic_family,
                  postscript_name, weight, width, slant, is_variable, axes, features,
                  fstype, parsed, coverage)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
ON CONFLICT (file_id, collection_index) DO UPDATE SET
    family = excluded.family, subfamily = excluded.subfamily,
    typographic_family = excluded.typographic_family,
    postscript_name = excluded.postscript_name, weight = excluded.weight,
    width = excluded.width, slant = excluded.slant, is_variable = excluded.is_variable,
    axes = excluded.axes, features = excluded.features, fstype = excluded.fstype,
    parsed = excluded.parsed, coverage = excluded.coverage";

/// Faces the file no longer has. An active face stays (rule 3): the app
/// registered it, so it must stay deregistrable.
const DELETE_OLD_FACES: &str = "
DELETE FROM face
WHERE file_id = ?1 AND collection_index >= ?2
  AND id NOT IN (SELECT face_id FROM activation)";

const MARK_FOUND: &str = "UPDATE font_file SET gone = 0 WHERE path = ?1 AND gone = 1";

impl Index {
    /// Writes `files`, found under the source `source`, in transactions of
    /// [`BATCH_SIZE`] files.
    ///
    /// On an error the failing batch is rolled back and the batches before
    /// it stay written; writing the same files again completes the job.
    pub fn persist_files<'a>(
        &mut self,
        source: SourceId,
        files: impl IntoIterator<Item = &'a IndexedFile>,
    ) -> FmResult<Persisted> {
        self.persist_with(source, files, &mut |_| {})
    }

    /// [`Index::persist_files`], calling `progress` with the number of files
    /// written after each committed batch.
    pub fn persist_with<'a>(
        &mut self,
        source: SourceId,
        files: impl IntoIterator<Item = &'a IndexedFile>,
        progress: &mut dyn FnMut(usize),
    ) -> FmResult<Persisted> {
        let writes: Vec<_> = files.into_iter().map(Write::File).collect();
        self.write_batches(source, &writes, progress)
    }

    /// Writes what one rescan of the source `source` changed: added, changed
    /// and gone files in full, and the gone flag of unchanged files that came
    /// back. `index` is the file index the rescan updated.
    pub fn apply_rescan(
        &mut self,
        source: SourceId,
        index: &FileIndex,
        rescan: &Rescan,
    ) -> FmResult<Persisted> {
        let full = rescan
            .added
            .iter()
            .chain(&rescan.changed)
            .chain(&rescan.gone);
        let writes: Vec<_> = full
            .filter_map(|path| index.get(path).map(Write::File))
            .chain(rescan.unchanged.iter().map(|p| Write::Found(p)))
            .collect();
        self.write_batches(source, &writes, &mut |_| {})
    }

    /// Every indexed file with its faces, gone ones included.
    pub fn load_files(&self) -> FmResult<FileIndex> {
        let mut faces = self.load_faces()?;
        let mut stmt = self
            .conn
            .prepare("SELECT id, path, hash, format, size, mtime, health, gone FROM font_file")
            .map_err(|e| db_error(&e))?;
        let mut rows = stmt.query([]).map_err(|e| db_error(&e))?;
        let mut index = FileIndex::new();
        while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
            let id: i64 = row.get(0).map_err(|e| db_error(&e))?;
            let path = path_from_sql(row.get_ref(1).map_err(|e| db_error(&e))?)?;
            let hash: Option<Vec<u8>> = row.get(2).map_err(|e| db_error(&e))?;
            let format: String = row.get(3).map_err(|e| db_error(&e))?;
            let size: i64 = row.get(4).map_err(|e| db_error(&e))?;
            let mtime: Option<i64> = row.get(5).map_err(|e| db_error(&e))?;
            let health: String = row.get(6).map_err(|e| db_error(&e))?;
            let gone: bool = row.get(7).map_err(|e| db_error(&e))?;

            let (faces, faces_ok) = faces.remove(&id).unwrap_or((Vec::new(), true));
            let hash = hash.and_then(|h| <[u8; 32]>::try_from(h).ok());
            let health = serde_json::from_str::<Health>(&health).ok();
            let format = format_from_sql(&format);
            let trusted = faces_ok && hash.is_some() && health.is_some() && format.is_ok();
            let modified = mtime.and_then(time_from_nanos).filter(|_| trusted);
            index.insert(IndexedFile {
                path,
                size: u64::try_from(size).unwrap_or(0),
                modified,
                hash: FileHash::from(hash.unwrap_or([0; 32])),
                parsed: ParsedFile {
                    format: format.unwrap_or(None),
                    health: health.unwrap_or_else(|| Health::Broken("not read yet".into())),
                    faces,
                },
                gone,
            });
        }
        Ok(index)
    }

    /// The faces by file id, in collection order, and whether every one of a
    /// file's rows could be read back.
    fn load_faces(&self) -> FmResult<HashMap<i64, (Vec<ParsedFace>, bool)>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT file_id, parsed, coverage FROM face ORDER BY file_id, collection_index",
            )
            .map_err(|e| db_error(&e))?;
        let mut rows = stmt.query([]).map_err(|e| db_error(&e))?;
        let mut out: HashMap<i64, (Vec<ParsedFace>, bool)> = HashMap::new();
        while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
            let file_id: i64 = row.get(0).map_err(|e| db_error(&e))?;
            let parsed: Option<String> = row.get(1).map_err(|e| db_error(&e))?;
            let coverage: Option<Vec<u8>> = row.get(2).map_err(|e| db_error(&e))?;
            let entry = out.entry(file_id).or_insert_with(|| (Vec::new(), true));
            let face = parsed
                .and_then(|json| serde_json::from_str::<ParsedFace>(&json).ok())
                .zip(
                    coverage
                        .as_deref()
                        .map_or(Some(Vec::new()), unpack_coverage),
                );
            match face {
                Some((mut face, code_points)) => {
                    face.code_points = code_points;
                    entry.0.push(face);
                }
                None => entry.1 = false,
            }
        }
        Ok(out)
    }

    fn write_batches(
        &mut self,
        source: SourceId,
        writes: &[Write<'_>],
        progress: &mut dyn FnMut(usize),
    ) -> FmResult<Persisted> {
        let known = self
            .conn
            .query_row("SELECT 1 FROM source WHERE id = ?1", [source.0], |_| Ok(()))
            .optional()
            .map_err(|e| db_error(&e))?;
        if known.is_none() {
            return Err(FmError::not_found(format!("Source {source}")));
        }
        let mut out = Persisted::default();
        for batch in writes.chunks(BATCH_SIZE) {
            let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
            for write in batch {
                match write {
                    Write::File(file) => out.faces += write_file(&tx, source, file)?,
                    Write::Found(path) => {
                        tx.prepare_cached(MARK_FOUND)
                            .and_then(|mut s| s.execute([path_to_sql(path)]))
                            .map_err(|e| db_error(&e))?;
                    }
                }
            }
            tx.commit().map_err(|e| db_error(&e))?;
            out.files += batch.len();
            out.batches += 1;
            progress(out.files);
        }
        Ok(out)
    }
}

/// Writes one file and its faces; returns the number of faces.
fn write_file(tx: &Transaction<'_>, source: SourceId, file: &IndexedFile) -> FmResult<usize> {
    let size = i64::try_from(file.size)
        .map_err(|_| FmError::internal(format!("file size {} is out of range", file.size)))?;
    let format = format_to_sql(file.parsed.format)?;
    let health = json(&file.parsed.health)?;
    let file_id: i64 = tx
        .prepare_cached(UPSERT_FILE)
        .and_then(|mut s| {
            s.query_row(
                params![
                    source.0,
                    path_to_sql(&file.path),
                    &file.hash.as_bytes()[..],
                    format,
                    size,
                    file.modified.and_then(time_to_nanos),
                    health,
                    file.gone,
                ],
                |row| row.get(0),
            )
        })
        .map_err(|e| db_error(&e))?;

    let mut face_stmt = tx.prepare_cached(UPSERT_FACE).map_err(|e| db_error(&e))?;
    for face in &file.parsed.faces {
        let style = face_style(&file.path, face);
        face_stmt
            .execute(params![
                file_id,
                face.index,
                family_name(&file.path, face),
                style.name,
                face.typographic_family,
                face.postscript_name.as_deref().unwrap_or_default(),
                style.weight,
                style.width,
                text(&style.slant)?,
                face.is_variable(),
                json(&face.axes)?,
                json(&face.features)?,
                face.fs_type.unwrap_or(0),
                json(face)?,
                pack_coverage(&face.code_points),
            ])
            .map_err(|e| db_error(&e))?;
    }
    let face_count = file
        .parsed
        .faces
        .iter()
        .map(|f| f.index + 1)
        .max()
        .unwrap_or(0);
    tx.prepare_cached(DELETE_OLD_FACES)
        .and_then(|mut s| s.execute(params![file_id, face_count]))
        .map_err(|e| db_error(&e))?;
    Ok(file.parsed.faces.len())
}

/// A path as SQL: TEXT when it is UTF-8, else its native bytes as a BLOB, so
/// every path the scanner finds is stored exactly.
fn path_to_sql(path: &Path) -> Value {
    match path.to_str() {
        Some(s) => Value::Text(s.to_owned()),
        None => Value::Blob(fm_platform::fs::path_to_bytes(path)),
    }
}

fn path_from_sql(value: ValueRef<'_>) -> FmResult<PathBuf> {
    let bad = || FmError::internal("a font file path in the index is damaged");
    match value {
        ValueRef::Text(bytes) => std::str::from_utf8(bytes)
            .map(PathBuf::from)
            .map_err(|_| bad()),
        ValueRef::Blob(bytes) => fm_platform::fs::path_from_bytes(bytes).ok_or_else(bad),
        _ => Err(bad()),
    }
}

/// The format's serde name (`ttf`), or `unknown` for data that is no font.
fn format_to_sql(format: Option<FontFormat>) -> FmResult<String> {
    format.map_or(Ok("unknown".to_owned()), |f| text(&f))
}

fn format_from_sql(text: &str) -> Result<Option<FontFormat>, serde_json::Error> {
    if text == "unknown" {
        return Ok(None);
    }
    serde_json::from_value(serde_json::Value::String(text.to_owned())).map(Some)
}

/// A unit enum as its bare serde name.
fn text<T: Serialize>(value: &T) -> FmResult<String> {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => Ok(s),
        Ok(other) => Err(FmError::internal(format!("{other} is not a name"))),
        Err(e) => Err(FmError::internal(format!("index value: {e}"))),
    }
}

fn json<T: Serialize>(value: &T) -> FmResult<String> {
    serde_json::to_string(value).map_err(|e| FmError::internal(format!("index value: {e}")))
}

/// Nanoseconds since the Unix epoch; negative before it. `None` past 2262.
fn time_to_nanos(time: SystemTime) -> Option<i64> {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_nanos()).ok(),
        Err(before) => i64::try_from(before.duration().as_nanos()).ok().map(|n| -n),
    }
}

fn time_from_nanos(nanos: i64) -> Option<SystemTime> {
    let offset = Duration::from_nanos(nanos.unsigned_abs());
    if nanos >= 0 {
        UNIX_EPOCH.checked_add(offset)
    } else {
        UNIX_EPOCH.checked_sub(offset)
    }
}

/// Sorted code points as runs: little-endian `u32` pairs (first, last).
fn pack_coverage(code_points: &[u32]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut points = code_points.iter().copied();
    let Some(mut first) = points.next() else {
        return out;
    };
    let mut last = first;
    for cp in points {
        if cp == last.wrapping_add(1) {
            last = cp;
            continue;
        }
        out.extend_from_slice(&first.to_le_bytes());
        out.extend_from_slice(&last.to_le_bytes());
        first = cp;
        last = cp;
    }
    out.extend_from_slice(&first.to_le_bytes());
    out.extend_from_slice(&last.to_le_bytes());
    out
}

/// The code points of [`pack_coverage`]; `None` for bytes it cannot have made.
fn unpack_coverage(bytes: &[u8]) -> Option<Vec<u32>> {
    let (pairs, rest) = bytes.as_chunks::<8>();
    if !rest.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for pair in pairs {
        let [a, b, c, d, e, f, g, h] = *pair;
        let first = u32::from_le_bytes([a, b, c, d]);
        let last = u32::from_le_bytes([e, f, g, h]);
        if first > last || last > MAX_CODE_POINT {
            return None;
        }
        out.extend(first..=last);
    }
    Some(out)
}

#[cfg(test)]
mod tests;
