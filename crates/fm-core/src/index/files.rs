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
//!
//! Each face is written with its coverage (E3.7): the `cmap` as a
//! [`CodePointSet`] in `face.cmap`, and its coverage of every language
//! preset in `face.languages` (see [`crate::coverage`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fm_api::SourceId;
use fm_api::common::{FontFormat, Health};
use rusqlite::types::{Value, ValueRef};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::Serialize;

use super::faces::open_license_to_sql;
use super::{Index, db_error, languages};
use crate::coverage::{self, CodePointSet};
use crate::error::{FmError, FmResult};
use crate::grouping::{face_style, family_name};
use crate::license;
use crate::parallel::map_ordered;
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, FileIndex, IndexedFile, Rescan};

/// How many files one transaction writes.
pub const BATCH_SIZE: usize = 500;

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
    /// The file is no longer found; only set its gone flag. Its faces were
    /// not read again, so they stay as they are (ADR 0004). A file not yet
    /// in the index is written in full.
    Gone(&'a IndexedFile),
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
                  fstype, parsed, cmap, languages, foundry, designer, is_color, healthy,
                  open_license, metrics, added)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
        ?20, ?21, ?22, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
ON CONFLICT (file_id, collection_index) DO UPDATE SET
    family = excluded.family, subfamily = excluded.subfamily,
    typographic_family = excluded.typographic_family,
    postscript_name = excluded.postscript_name, weight = excluded.weight,
    width = excluded.width, slant = excluded.slant, is_variable = excluded.is_variable,
    axes = excluded.axes, features = excluded.features, fstype = excluded.fstype,
    parsed = excluded.parsed, cmap = excluded.cmap, languages = excluded.languages,
    foundry = excluded.foundry, designer = excluded.designer,
    is_color = excluded.is_color, healthy = excluded.healthy,
    open_license = excluded.open_license, metrics = excluded.metrics";

/// Faces the file no longer has. An active face stays (rule 3): the app
/// registered it, so it must stay deregistrable.
const DELETE_OLD_FACES: &str = "
DELETE FROM face
WHERE file_id = ?1 AND collection_index >= ?2
  AND id NOT IN (SELECT face_id FROM activation)";

const MARK_FOUND: &str = "UPDATE font_file SET gone = 0 WHERE path = ?1 AND gone = 1";

const MARK_GONE: &str = "UPDATE font_file SET gone = 1 WHERE path = ?1";

impl Index {
    /// Writes `files`, found under the source `source`, in transactions of
    /// [`BATCH_SIZE`] files.
    ///
    /// On an error the failing batch is rolled back and the batches before
    /// it stay written; writing the same files again completes the job.
    /// Then the source's sidecar metadata is merged ([`Index::sync_sidecars`]).
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
        let out = self.write_batches(source, &writes, progress)?;
        self.sync_after_scan(source);
        Ok(out)
    }

    /// Writes what one rescan of the source `source` changed: added and
    /// changed files in full, and only the gone flag of files that went away
    /// or came back unchanged. `index` is the file index the rescan updated.
    ///
    /// A gone file was not read, so its faces in `index` may be fewer than
    /// the database holds (a row that did not load back); its face rows, and
    /// the user data on them, are never touched. Then the source's sidecar
    /// metadata is merged ([`Index::sync_sidecars`]).
    pub fn apply_rescan(
        &mut self,
        source: SourceId,
        index: &FileIndex,
        rescan: &Rescan,
    ) -> FmResult<Persisted> {
        let read = rescan.added.iter().chain(&rescan.changed);
        let writes: Vec<_> = read
            .filter_map(|path| index.get(path).map(Write::File))
            .chain(
                rescan
                    .gone
                    .iter()
                    .filter_map(|path| index.get(path).map(Write::Gone)),
            )
            .chain(rescan.unchanged.iter().map(|p| Write::Found(p)))
            .collect();
        let out = self.write_batches(source, &writes, &mut |_| {})?;
        self.sync_after_scan(source);
        Ok(out)
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
            .prepare("SELECT file_id, parsed, cmap FROM face ORDER BY file_id, collection_index")
            .map_err(|e| db_error(&e))?;
        let mut rows = stmt.query([]).map_err(|e| db_error(&e))?;
        let mut out: HashMap<i64, (Vec<ParsedFace>, bool)> = HashMap::new();
        while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
            let file_id: i64 = row.get(0).map_err(|e| db_error(&e))?;
            let parsed: Option<String> = row.get(1).map_err(|e| db_error(&e))?;
            let cmap: Option<Vec<u8>> = row.get(2).map_err(|e| db_error(&e))?;
            let entry = out.entry(file_id).or_insert_with(|| (Vec::new(), true));
            let face = parsed
                .and_then(|json| serde_json::from_str::<ParsedFace>(&json).ok())
                .zip(cmap.as_deref().map_or(Some(Vec::new()), |bytes| {
                    CodePointSet::from_bytes(bytes).map(|set| set.iter().collect())
                }));
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
            // The rows are built on all cores; only the SQL runs in order.
            let mut rows = map_ordered(
                batch,
                |write| match write {
                    Write::File(file) => Some(FileRow::of(file)),
                    _ => None,
                },
                &mut |_| {},
            )
            .into_iter();
            let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
            for write in batch {
                let row = rows.next().flatten();
                match write {
                    Write::File(file) => {
                        let row = row.unwrap_or_else(|| FileRow::of(file))?;
                        out.faces += write_file(&tx, source, &row)?;
                    }
                    Write::Found(path) => {
                        tx.prepare_cached(MARK_FOUND)
                            .and_then(|mut s| s.execute([path_to_sql(path)]))
                            .map_err(|e| db_error(&e))?;
                    }
                    Write::Gone(file) => {
                        let marked = tx
                            .prepare_cached(MARK_GONE)
                            .and_then(|mut s| s.execute([path_to_sql(&file.path)]))
                            .map_err(|e| db_error(&e))?;
                        if marked == 0 {
                            out.faces += write_file(&tx, source, &FileRow::of(file)?)?;
                        }
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

/// One file's `font_file` row and its `face` rows, built before the
/// transaction so the CPU work (names, coverage, JSON) runs in parallel.
struct FileRow<'a> {
    file: &'a IndexedFile,
    size: i64,
    format: String,
    health: String,
    faces: Vec<FaceRow<'a>>,
    /// One past the highest collection index: older face rows from here on
    /// are deleted.
    face_count: u32,
}

/// The values of one `face` row, in [`UPSERT_FACE`] order.
struct FaceRow<'a> {
    index: u32,
    family: String,
    subfamily: String,
    typographic_family: Option<&'a str>,
    postscript_name: &'a str,
    weight: u16,
    width: u16,
    slant: String,
    is_variable: bool,
    axes: String,
    features: String,
    fs_type: u16,
    parsed: String,
    cmap: Vec<u8>,
    languages: String,
    foundry: Option<&'a str>,
    designer: Option<&'a str>,
    color: bool,
    healthy: bool,
    /// The open license the name table states, as stored.
    open_license: Option<String>,
    /// The vertical metrics as JSON (E16.2), read by auto size.
    metrics: String,
}

impl<'a> FileRow<'a> {
    fn of(file: &'a IndexedFile) -> FmResult<Self> {
        let size = i64::try_from(file.size)
            .map_err(|_| FmError::internal(format!("file size {} is out of range", file.size)))?;
        let faces = file
            .parsed
            .faces
            .iter()
            .map(|face| FaceRow::of(&file.path, face))
            .collect::<FmResult<_>>()?;
        Ok(Self {
            file,
            size,
            format: format_to_sql(file.parsed.format)?,
            health: json(&file.parsed.health)?,
            faces,
            face_count: file
                .parsed
                .faces
                .iter()
                .map(|f| f.index + 1)
                .max()
                .unwrap_or(0),
        })
    }
}

impl<'a> FaceRow<'a> {
    fn of(path: &Path, face: &'a ParsedFace) -> FmResult<Self> {
        let style = face_style(path, face);
        let cmap = CodePointSet::from_code_points(face.code_points.iter().copied());
        Ok(Self {
            index: face.index,
            family: family_name(path, face),
            subfamily: style.name,
            typographic_family: face.typographic_family.as_deref(),
            postscript_name: face.postscript_name.as_deref().unwrap_or_default(),
            weight: style.weight,
            width: style.width,
            slant: text(&style.slant)?,
            is_variable: face.is_variable(),
            axes: json(&face.axes)?,
            features: json(&face.features)?,
            fs_type: face.fs_type.unwrap_or(0),
            parsed: json(face)?,
            cmap: cmap.to_bytes(),
            languages: languages::to_sql(&coverage::languages(&cmap, &face.locl_languages)),
            foundry: face.foundry.as_deref(),
            designer: face.designer.as_deref(),
            color: face.color,
            healthy: face.health == Health::Ok,
            open_license: open_license_to_sql(license::open_license(
                face.license.as_deref(),
                face.license_url.as_deref(),
            )),
            metrics: json(&face.metrics)?,
        })
    }
}

/// Writes one file and its faces; returns the number of faces.
fn write_file(tx: &Transaction<'_>, source: SourceId, row: &FileRow<'_>) -> FmResult<usize> {
    let file = row.file;
    let file_id: i64 = tx
        .prepare_cached(UPSERT_FILE)
        .and_then(|mut s| {
            s.query_row(
                params![
                    source.0,
                    path_to_sql(&file.path),
                    &file.hash.as_bytes()[..],
                    row.format,
                    row.size,
                    file.modified.and_then(time_to_nanos),
                    row.health,
                    file.gone,
                ],
                |r| r.get(0),
            )
        })
        .map_err(|e| db_error(&e))?;

    let mut face_stmt = tx.prepare_cached(UPSERT_FACE).map_err(|e| db_error(&e))?;
    for face in &row.faces {
        face_stmt
            .execute(params![
                file_id,
                face.index,
                face.family,
                face.subfamily,
                face.typographic_family,
                face.postscript_name,
                face.weight,
                face.width,
                face.slant,
                face.is_variable,
                face.axes,
                face.features,
                face.fs_type,
                face.parsed,
                face.cmap,
                face.languages,
                face.foundry,
                face.designer,
                face.color,
                face.healthy,
                face.open_license,
                face.metrics,
            ])
            .map_err(|e| db_error(&e))?;
    }
    tx.prepare_cached(DELETE_OLD_FACES)
        .and_then(|mut s| s.execute(params![file_id, row.face_count]))
        .map_err(|e| db_error(&e))?;
    Ok(row.faces.len())
}

/// A path as SQL: TEXT when it is UTF-8, else its native bytes as a BLOB, so
/// every path the scanner finds is stored exactly.
pub(super) fn path_to_sql(path: &Path) -> Value {
    match path.to_str() {
        Some(s) => Value::Text(s.to_owned()),
        None => Value::Blob(fm_platform::fs::path_to_bytes(path)),
    }
}

pub(super) fn path_from_sql(value: ValueRef<'_>) -> FmResult<PathBuf> {
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

pub(super) fn format_from_sql(text: &str) -> Result<Option<FontFormat>, serde_json::Error> {
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

#[cfg(test)]
mod tests;
