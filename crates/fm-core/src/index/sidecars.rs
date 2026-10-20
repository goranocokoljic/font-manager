//! Sidecar metadata in the index (E7.3): mirrors the user's metadata to
//! `<source>/.fontlibrary/<family-hash>.json` and merges what another machine
//! wrote there. The file format and the merge rule are in [`crate::sidecar`].
//!
//! - **Mirror.** Triggers mark every face whose tags, favorite, rating,
//!   notes, collections or virtual styles change (`sidecar_dirty`); a change
//!   of a family's notes marks its faces (`index/family_notes.rs`). After
//!   each such write [`Index::write_sidecars`] rewrites the sidecar of every
//!   dirty family. A sidecar that cannot be written (folder offline,
//!   read-only) leaves its faces dirty; the next write or scan tries again. The user's edit never
//!   fails because of it: the index is the source of truth (ADR 0003).
//! - **Merge.** A scan ([`Index::persist_files`], [`Index::apply_rescan`])
//!   ends with [`Index::sync_sidecars`]: every sidecar whose size or
//!   modification time changed since this machine last saw it is merged
//!   into the index, three-way against `sidecar_base`, then rewritten with
//!   the result. A sidecar is always merged before it is written, so a
//!   change another machine made is never overwritten unread.
//! - **Nothing is lost.** A value that loses a conflict is kept in
//!   `sidecar_conflict` and listed by [`Index::sidecar_problems`] until the
//!   user sets the field again. A sidecar that does not parse, or that a
//!   newer Font Manager wrote, is reported and never rewritten. Entries for
//!   faces this machine has not indexed are written back unchanged, and a
//!   deleted sidecar file is written again from the index, not taken as the
//!   other machine clearing everything.
//!
//! Only the `.fontlibrary` folder inside an existing source folder is ever
//! created; font files are never touched (rule 2). System sources get no
//! sidecars (rule 4), and paused sources are left alone until resumed.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use fm_api::library::{Problem, ProblemKind};
use fm_api::{FaceId, SourceId};
use rusqlite::{OptionalExtension, Transaction, params};

use super::family_notes;
use super::files::path_from_sql;
use super::virtual_styles::{self, Holder};
use super::{Index, db_error};
use crate::error::{FmError, FmResult};
use crate::grouping::family_key;
use crate::sidecar::{
    self, FaceEntry, FaceKey, Field, Meta, Parsed, Pin, PinKey, SIDECAR_DIR, Sidecar,
};

/// The `sidecar_conflict.field` of a sidecar that cannot be read.
const FILE_FIELD: &str = "file";
/// The `sidecar_conflict.field` of a collection that could not be joined.
const COLLECTION_FIELD: &str = "collection";
/// The `sidecar_conflict.field` of a virtual style that could not be joined.
const PIN_FIELD: &str = "pin";
/// Deeper than any collection tree a person makes; stops a damaged parent
/// loop.
const MAX_DEPTH: usize = 64;

/// What syncing sidecars did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SidecarSync {
    /// Sidecar files read because they changed on disk.
    pub read: usize,
    /// Faces whose metadata a merge changed in the index.
    pub faces_changed: usize,
    /// Families whose notes a merge changed in the index.
    pub families_changed: usize,
    /// Sidecar files written.
    pub written: usize,
    /// Conflicts found that were not known yet.
    pub conflicts: usize,
    /// Sidecar files that could not be read or written.
    pub failed: usize,
}

impl SidecarSync {
    fn add(&mut self, other: Self) {
        self.read += other.read;
        self.faces_changed += other.faces_changed;
        self.families_changed += other.families_changed;
        self.written += other.written;
        self.conflicts += other.conflicts;
        self.failed += other.failed;
    }
}

/// A source the sidecars of which may be read and written.
struct SourceDir {
    id: SourceId,
    root: PathBuf,
}

impl SourceDir {
    fn dir(&self) -> PathBuf {
        self.root.join(SIDECAR_DIR)
    }
}

/// One indexed face of a source.
struct SourceFace {
    id: u32,
    key: FaceKey,
    family: String,
    /// The family's sidecar file name.
    file_name: String,
    postscript: String,
}

/// The faces of one source, by portable key and by sidecar file name.
struct SourceFaces {
    faces: Vec<SourceFace>,
    by_key: HashMap<FaceKey, usize>,
}

impl SourceFaces {
    fn get(&self, key: &FaceKey) -> Option<&SourceFace> {
        self.by_key.get(key).map(|&i| &self.faces[i])
    }

    /// The faces whose family's sidecar is `name`.
    fn of_file<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a SourceFace> + 'a {
        self.faces.iter().filter(move |f| f.file_name == name)
    }
}

impl Index {
    /// Merges every changed sidecar of `source` into the index and writes the
    /// sidecars of its families whose metadata changed. A system, paused or
    /// missing source is left alone. Problems with single files are reported
    /// by [`Index::sidecar_problems`]; only a database error fails.
    pub fn sync_sidecars(&mut self, source: SourceId) -> FmResult<SidecarSync> {
        self.sync_source(source, false)
    }

    /// Writes the sidecar of every family with metadata changed since its
    /// sidecar was last written, merging the file on disk first.
    pub fn write_sidecars(&mut self) -> FmResult<SidecarSync> {
        self.conn
            .execute(
                "DELETE FROM sidecar_dirty WHERE face_id NOT IN (SELECT id FROM face)
                    OR face_id IN (SELECT face.id FROM face
                                   JOIN font_file ON font_file.id = face.file_id
                                   JOIN source ON source.id = font_file.source_id
                                   WHERE source.kind = 'system')",
                [],
            )
            .map_err(|e| db_error(&e))?;
        let sources: Vec<u32> = {
            let mut stmt = self
                .conn
                .prepare(
                    "SELECT DISTINCT font_file.source_id FROM sidecar_dirty
                     JOIN face ON face.id = sidecar_dirty.face_id
                     JOIN font_file ON font_file.id = face.file_id
                     ORDER BY 1",
                )
                .map_err(|e| db_error(&e))?;
            stmt.query_map([], |row| row.get(0))
                .map_err(|e| db_error(&e))?
                .collect::<Result<_, _>>()
                .map_err(|e| db_error(&e))?
        };
        let mut out = SidecarSync::default();
        for source in sources {
            out.add(self.sync_source(SourceId(source), true)?);
        }
        Ok(out)
    }

    /// [`Index::write_sidecars`] after a metadata write: a failure is logged,
    /// not returned, because the write itself succeeded.
    pub(super) fn mirror_sidecars(&mut self) {
        if let Err(e) = self.write_sidecars() {
            tracing::warn!(error = %e, "sidecar metadata could not be written");
        }
    }

    /// [`Index::sync_sidecars`] at the end of a scan, logged, not returned:
    /// the scan's own writes are committed.
    pub(super) fn sync_after_scan(&mut self, source: SourceId) {
        match self.sync_sidecars(source) {
            Ok(sync) if sync != SidecarSync::default() => {
                tracing::info!(source = source.0, ?sync, "sidecar metadata synced");
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(source = source.0, error = %e, "sidecar metadata could not be synced")
            }
        }
    }

    /// Sidecar values that lost a conflict, and sidecars that cannot be read,
    /// as Problems entries.
    pub fn sidecar_problems(&self) -> FmResult<Vec<Problem>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT c.face_id, c.field, c.local, c.disk, c.file, s.path, f.path
                 FROM sidecar_conflict AS c
                 JOIN source AS s ON s.id = c.source_id
                 LEFT JOIN face ON face.id = c.face_id
                 LEFT JOIN font_file AS f ON f.id = face.file_id
                 ORDER BY c.id",
            )
            .map_err(|e| db_error(&e))?;
        let mut rows = stmt.query([]).map_err(|e| db_error(&e))?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
            let face: Option<u32> = row.get(0).map_err(|e| db_error(&e))?;
            let field: String = row.get(1).map_err(|e| db_error(&e))?;
            let local: Option<String> = row.get(2).map_err(|e| db_error(&e))?;
            let disk: String = row.get(3).map_err(|e| db_error(&e))?;
            let file: String = row.get(4).map_err(|e| db_error(&e))?;
            let root: String = row.get(5).map_err(|e| db_error(&e))?;
            let font = match row.get_ref(6).map_err(|e| db_error(&e))? {
                rusqlite::types::ValueRef::Null => None,
                value => Some(path_from_sql(value)?),
            };
            let sidecar_path = Path::new(&root).join(SIDECAR_DIR).join(&file);
            let sidecar_text = sidecar_path.display().to_string();
            out.push(match (face, font) {
                (Some(face), Some(font)) => Problem {
                    kind: ProblemKind::SidecarConflict,
                    face: Some(FaceId(face)),
                    path: font.display().to_string(),
                    other: Some(sidecar_text),
                    message: conflict_message(&field, local.as_deref(), &disk),
                },
                _ => Problem {
                    kind: ProblemKind::SidecarConflict,
                    face: None,
                    path: sidecar_text,
                    other: None,
                    message: disk,
                },
            });
        }
        Ok(out)
    }

    /// Syncs the sidecars of one source: those changed on disk (unless
    /// `dirty_only`) and those of families with dirty faces.
    fn sync_source(&mut self, source: SourceId, dirty_only: bool) -> FmResult<SidecarSync> {
        let Some(src) = self.sidecar_source(source)? else {
            return Ok(SidecarSync::default());
        };
        let dirty = self.dirty_faces(source)?;
        let on_disk = if dirty_only {
            Vec::new()
        } else {
            list_sidecars(&src.dir())
        };
        if dirty.is_empty() && on_disk.is_empty() {
            return Ok(SidecarSync::default());
        }
        let faces = self.source_faces(&src)?;
        let mut dirty_files: BTreeSet<String> = BTreeSet::new();
        for face in &faces.faces {
            if dirty.contains(&face.id) {
                dirty_files.insert(face.file_name.clone());
            }
        }
        let names: BTreeSet<String> = on_disk
            .into_iter()
            .chain(dirty_files.iter().cloned())
            .collect();
        let mut out = SidecarSync::default();
        for name in names {
            let is_dirty = dirty_files.contains(&name);
            out.add(self.sync_file(&src, &faces, &name, is_dirty)?);
        }
        // Faces of this source without a sidecar of their own (a path that
        // is not UTF-8) cannot be mirrored; they are not dirty any more.
        let unmirrored: Vec<u32> = dirty
            .iter()
            .copied()
            .filter(|id| !faces.faces.iter().any(|f| f.id == *id))
            .collect();
        self.clear_dirty(&unmirrored)?;
        // A merge can change faces of another family (an entry in a sync
        // client's conflicted copy): write their sidecars too.
        if !dirty_only && out.faces_changed > 0 {
            out.add(self.sync_source(source, true)?);
        }
        Ok(out)
    }

    /// The source, when its sidecars may be touched now: not a system
    /// source, not paused, and its folder exists.
    fn sidecar_source(&self, source: SourceId) -> FmResult<Option<SourceDir>> {
        let row: Option<(String, String, bool)> = self
            .conn
            .query_row(
                "SELECT path, kind, paused FROM source WHERE id = ?1",
                [source.0],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|e| db_error(&e))?;
        let Some((path, kind, paused)) = row else {
            return Err(FmError::not_found(format!("Source {source}")));
        };
        let root = PathBuf::from(path);
        if kind == "system" || paused || !root.is_dir() {
            return Ok(None);
        }
        Ok(Some(SourceDir { id: source, root }))
    }

    fn dirty_faces(&self, source: SourceId) -> FmResult<BTreeSet<u32>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT sidecar_dirty.face_id FROM sidecar_dirty
                 JOIN face ON face.id = sidecar_dirty.face_id
                 JOIN font_file ON font_file.id = face.file_id
                 WHERE font_file.source_id = ?1",
            )
            .map_err(|e| db_error(&e))?;
        stmt.query_map([source.0], |row| row.get(0))
            .map_err(|e| db_error(&e))?
            .collect::<Result<_, _>>()
            .map_err(|e| db_error(&e))
    }

    fn clear_dirty(&self, faces: &[u32]) -> FmResult<()> {
        let mut stmt = self
            .conn
            .prepare_cached("DELETE FROM sidecar_dirty WHERE face_id = ?1")
            .map_err(|e| db_error(&e))?;
        for face in faces {
            stmt.execute([face]).map_err(|e| db_error(&e))?;
        }
        Ok(())
    }

    /// Every face of the source whose path, relative to the source, is text.
    fn source_faces(&self, src: &SourceDir) -> FmResult<SourceFaces> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT face.id, font_file.path, face.collection_index, face.family,
                        face.postscript_name
                 FROM face JOIN font_file ON font_file.id = face.file_id
                 WHERE font_file.source_id = ?1
                 ORDER BY face.id",
            )
            .map_err(|e| db_error(&e))?;
        let mut rows = stmt.query([src.id.0]).map_err(|e| db_error(&e))?;
        let mut faces = Vec::new();
        while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
            let path = path_from_sql(row.get_ref(1).map_err(|e| db_error(&e))?)?;
            let Some(file) = relative(&src.root, &path) else {
                continue;
            };
            let family: String = row.get(3).map_err(|e| db_error(&e))?;
            faces.push(SourceFace {
                id: row.get(0).map_err(|e| db_error(&e))?,
                key: FaceKey {
                    file,
                    index: row.get(2).map_err(|e| db_error(&e))?,
                },
                file_name: sidecar::file_name(&family),
                family,
                postscript: row.get(4).map_err(|e| db_error(&e))?,
            });
        }
        let by_key = faces
            .iter()
            .enumerate()
            .map(|(i, f)| (f.key.clone(), i))
            .collect();
        Ok(SourceFaces { faces, by_key })
    }

    /// Merges one sidecar file into the index and, when it is the sidecar of
    /// a family this source has, writes it back with the result.
    fn sync_file(
        &mut self,
        src: &SourceDir,
        faces: &SourceFaces,
        name: &str,
        dirty: bool,
    ) -> FmResult<SidecarSync> {
        let mut out = SidecarSync::default();
        let path = src.dir().join(name);
        let (base, stamp) = self.base(src.id, name)?;
        let on_disk = match read_file(&path) {
            Ok(on_disk) => on_disk,
            Err(e) => {
                tracing::warn!(error = %e, "sidecar not read");
                out.failed += 1;
                return Ok(out);
            }
        };
        let disk_stamp = on_disk.as_ref().map(|(_, s)| s.clone());
        if !dirty && disk_stamp.is_some() && disk_stamp == stamp {
            return Ok(out);
        }
        let disk = match on_disk {
            None => None,
            Some((text, _)) => match sidecar::parse(&text) {
                Parsed::Sidecar(s) => {
                    self.clear_file_problem(src.id, name)?;
                    Some(s)
                }
                Parsed::Newer(version) => {
                    out.conflicts += self.file_problem(
                        src.id,
                        name,
                        &format!(
                            "This sidecar was written by a newer Font Manager (format {version}); update the app to merge it. It is left unchanged."
                        ),
                    )?;
                    return Ok(out);
                }
                Parsed::Unreadable(reason) => {
                    tracing::warn!(file = %path.display(), %reason, "sidecar cannot be read");
                    out.conflicts += self.file_problem(
                        src.id,
                        name,
                        "This sidecar cannot be read, so its metadata was not merged. It is left unchanged; fix or remove it.",
                    )?;
                    return Ok(out);
                }
            },
        };
        if disk.is_some() {
            out.read += 1;
        }
        let family: Vec<&SourceFace> = faces.of_file(name).collect();
        let base_entries = base.entries();
        // A missing file is not another machine clearing everything: the
        // index writes it again.
        let disk_entries = disk
            .as_ref()
            .map_or_else(|| base_entries.clone(), Sidecar::entries);
        // An entry missing on disk was removed there only when the file
        // descends from this machine's version of it.
        let file_descends = disk.as_ref().is_none_or(|d| sidecar::descends(&base, d));

        // Merge every indexed face the file or the base mentions.
        let keys: BTreeSet<&FaceKey> = disk_entries.keys().chain(base_entries.keys()).collect();
        let empty = Meta::default();
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        let mut collections = Collections::load(&tx)?;
        // Collections a face is in on another machine that cannot be joined
        // here (a smart collection on the way) are kept as `collection`
        // conflict rows and carried in the sidecar, so this machine never
        // writes a removal it did not make. Once the way is clear (the smart
        // collection was deleted or renamed), the merge joins them.
        for key in keys {
            let Some(face) = faces.get(key) else {
                continue;
            };
            let held = local_meta(&tx, face.id, &collections)?;
            let carried = carried_paths(&tx, face.id)?;
            let carried_pins = carried_pins(&tx, face.id)?;
            let mut local = held.clone();
            local.collections.extend(carried.iter().cloned());
            local.pins.extend(carried_pins.iter().cloned());
            local.normalize();
            let base_entry = base_entries.get(key).copied();
            let disk_entry = disk_entries.get(key).copied();
            let merged = sidecar::merge(
                &sidecar::ancestor(base_entry, disk_entry, file_descends),
                &local,
                disk_entry.map_or(&empty, |e| &e.meta),
            );
            // A carried path the other machines no longer have is let go.
            for path in carried
                .iter()
                .filter(|p| !merged.meta.collections.contains(p))
            {
                drop_carried(&tx, face.id, path)?;
            }
            for pin in carried_pins
                .iter()
                .filter(|p| !merged.meta.pins.iter().any(|m| m.key() == p.key()))
            {
                drop_carried_pin(&tx, face.id, pin)?;
            }
            if merged.meta != held {
                let applied = apply(
                    &tx,
                    src.id,
                    name,
                    face.id,
                    &held,
                    &merged.meta,
                    &mut collections,
                )?;
                out.conflicts += applied.conflicts;
                if applied.changed {
                    out.faces_changed += 1;
                }
            }
            for c in &merged.conflicts {
                out.conflicts += insert_conflict(
                    &tx,
                    src.id,
                    name,
                    Some(face.id),
                    c.field.as_str(),
                    Some(&c.local),
                    &c.disk,
                )?;
            }
        }

        // The family's own notes merge by the same rule, when the family is
        // here; a conflict is kept on its first face, so it has a font to
        // show in Problems.
        let disk_family = disk
            .as_ref()
            .map_or(base.family_meta.as_ref(), |d| d.family_meta.as_ref());
        if let Some(first) = family.first() {
            let key = family_key(&first.family);
            let local = family_notes::family_notes_by_key(&tx, &key)?;
            let merged = sidecar::merge_family(
                sidecar::family_ancestor(base.family_meta.as_ref(), disk_family, file_descends)
                    .as_deref(),
                local.as_deref(),
                disk_family.and_then(|f| f.notes.as_deref()),
            );
            if merged.notes != local {
                family_notes::set_family_notes(&tx, &key, merged.notes.as_deref())?;
                out.families_changed += 1;
            }
            if let Some(c) = &merged.conflict {
                out.conflicts += insert_conflict(
                    &tx,
                    src.id,
                    name,
                    Some(first.id),
                    c.field.as_str(),
                    Some(&c.local),
                    &c.disk,
                )?;
            }
        }

        // The sidecar of a family here is written with what the index holds
        // now, plus the entries of faces this machine has not indexed.
        let new_base = if let Some(first) = family.first() {
            let mut next = Sidecar::new(first.family.clone());
            let notes = family_notes::family_notes_by_key(&tx, &family_key(&first.family))?;
            next.family_meta = notes.map(|notes| sidecar::FamilyEntry {
                history: sidecar::next_family_history(Some(&notes), disk_family),
                notes: Some(notes),
            });
            for face in &family {
                let mut meta = local_meta(&tx, face.id, &collections)?;
                meta.collections.extend(carried_paths(&tx, face.id)?);
                meta.pins.extend(carried_pins(&tx, face.id)?);
                meta.normalize();
                if !meta.is_empty() {
                    // The entry replaces the one on disk, which the merge
                    // has taken in: it descends from it.
                    let history =
                        sidecar::next_history(&meta, disk_entries.get(&face.key).copied());
                    next.faces.push(FaceEntry {
                        file: face.key.file.clone(),
                        index: face.key.index,
                        postscript_name: Some(face.postscript.clone()).filter(|p| !p.is_empty()),
                        meta,
                        history,
                    });
                }
            }
            if let Some(disk) = &disk {
                next.faces.extend(
                    disk.faces
                        .iter()
                        .filter(|e| faces.get(&e.key()).is_none())
                        .cloned(),
                );
            }
            next.normalize();
            next.history = sidecar::next_file_history(&next, Some(disk.as_ref().unwrap_or(&base)));
            tx.commit().map_err(|e| db_error(&e))?;
            let mut stamp = disk_stamp;
            let nothing = next.faces.is_empty() && next.family_meta.is_none();
            if disk.as_ref() != Some(&next) && !(disk.is_none() && nothing) {
                match write_file(src, name, &next) {
                    Ok(written) => stamp = Some(written),
                    Err(e) => {
                        // The faces stay dirty and the base stays as it
                        // was: the next write or scan merges and tries again.
                        tracing::warn!(error = %e, "sidecar not written");
                        out.failed += 1;
                        return Ok(out);
                    }
                }
                out.written += 1;
            }
            let ids: Vec<u32> = family.iter().map(|f| f.id).collect();
            self.clear_dirty(&ids)?;
            (next, stamp)
        } else {
            tx.commit().map_err(|e| db_error(&e))?;
            let Some(disk) = disk else {
                return Ok(out);
            };
            (disk, disk_stamp)
        };
        let (content, mut stamp) = new_base;
        // A file with entries of faces not indexed here, or with the notes
        // of a family not indexed here, is read again on every scan, so they
        // are merged as soon as their fonts arrive.
        let family_here = !family.is_empty();
        if content.faces.iter().any(|e| faces.get(&e.key()).is_none())
            || (!family_here && (content.family_meta.is_some() || base.family_meta.is_some()))
        {
            stamp = None;
        }
        let mut content = indexed_only(content, faces);
        // The family's notes in the base stay what this machine last took in
        // while no face of the family is indexed: notes it never took in are
        // not taken for a removal, and notes it kept are not taken for new.
        if !family_here {
            content.family_meta = base.family_meta.clone();
        }
        self.store_base(src.id, name, &content, stamp.as_deref())?;
        Ok(out)
    }

    /// The sidecar as this machine last saw it (empty when never), and the
    /// file's stamp then.
    fn base(&self, source: SourceId, name: &str) -> FmResult<(Sidecar, Option<String>)> {
        let row: Option<(String, Option<String>)> = self
            .conn
            .query_row(
                "SELECT content, stamp FROM sidecar_base WHERE source_id = ?1 AND file = ?2",
                params![source.0, name],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| db_error(&e))?;
        Ok(match row {
            Some((content, stamp)) => match sidecar::parse(&content) {
                Parsed::Sidecar(s) => (s, stamp),
                // A base this build cannot read is as good as none: the
                // merge keeps both sides' values.
                _ => (Sidecar::new(""), None),
            },
            None => (Sidecar::new(""), None),
        })
    }

    fn store_base(
        &self,
        source: SourceId,
        name: &str,
        content: &Sidecar,
        stamp: Option<&str>,
    ) -> FmResult<()> {
        self.conn
            .execute(
                "INSERT INTO sidecar_base (source_id, file, content, stamp) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (source_id, file) DO UPDATE SET
                     content = excluded.content, stamp = excluded.stamp",
                params![source.0, name, content.to_json(), stamp],
            )
            .map_err(|e| db_error(&e))?;
        Ok(())
    }

    /// Records that a sidecar cannot be merged; 1 when that is new.
    fn file_problem(&self, source: SourceId, name: &str, message: &str) -> FmResult<usize> {
        let known: bool = self
            .conn
            .prepare(
                "SELECT 1 FROM sidecar_conflict
                 WHERE source_id = ?1 AND file = ?2 AND field = ?3 AND disk = ?4",
            )
            .and_then(|mut s| s.exists(params![source.0, name, FILE_FIELD, message]))
            .map_err(|e| db_error(&e))?;
        if known {
            return Ok(0);
        }
        self.clear_file_problem(source, name)?;
        self.conn
            .execute(
                "INSERT INTO sidecar_conflict (source_id, file, face_id, field, local, disk)
                 VALUES (?1, ?2, NULL, ?3, NULL, ?4)",
                params![source.0, name, FILE_FIELD, message],
            )
            .map_err(|e| db_error(&e))?;
        Ok(1)
    }

    fn clear_file_problem(&self, source: SourceId, name: &str) -> FmResult<()> {
        self.conn
            .execute(
                "DELETE FROM sidecar_conflict WHERE source_id = ?1 AND file = ?2 AND field = ?3",
                params![source.0, name, FILE_FIELD],
            )
            .map_err(|e| db_error(&e))?;
        Ok(())
    }
}

/// Marks every face of a collection and of its children dirty, members and
/// faces of virtual styles alike: their collection paths change when it is
/// renamed, moved or deleted.
pub(super) fn mark_collection_dirty(tx: &Transaction<'_>, collection: u32) -> FmResult<()> {
    tx.execute(
        "INSERT OR IGNORE INTO sidecar_dirty (face_id)
         WITH RECURSIVE sub (id) AS (
             SELECT ?1
             UNION SELECT collection.id FROM collection JOIN sub ON collection.parent_id = sub.id
         )
         SELECT face_id FROM collection_face WHERE collection_id IN (SELECT id FROM sub)
         UNION SELECT face_id FROM virtual_style WHERE collection_id IN (SELECT id FROM sub)",
        [collection],
    )
    .map_err(|e| db_error(&e))?;
    Ok(())
}

/// Forgets the sidecar values of `field` on `face` that lost a conflict: the
/// user has set the field again.
pub(super) fn resolve_conflicts(tx: &Transaction<'_>, face: u32, field: Field) -> FmResult<()> {
    tx.execute(
        "DELETE FROM sidecar_conflict WHERE face_id = ?1 AND field = ?2",
        params![face, field.as_str()],
    )
    .map_err(|e| db_error(&e))?;
    Ok(())
}

/// Every collection's parent, name and kind.
struct Collections {
    by_id: HashMap<u32, (Option<u32>, String, bool)>,
}

impl Collections {
    fn load(tx: &Transaction<'_>) -> FmResult<Self> {
        let mut stmt = tx
            .prepare("SELECT id, parent_id, name, is_smart FROM collection")
            .map_err(|e| db_error(&e))?;
        let by_id = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, (row.get(1)?, row.get(2)?, row.get(3)?)))
            })
            .map_err(|e| db_error(&e))?
            .collect::<Result<_, _>>()
            .map_err(|e| db_error(&e))?;
        Ok(Self { by_id })
    }

    /// The names from the top level down to `id`.
    fn path(&self, id: u32) -> Option<Vec<String>> {
        let mut names = Vec::new();
        let mut at = Some(id);
        while let Some(id) = at {
            let (parent, name, _) = self.by_id.get(&id)?;
            names.push(name.clone());
            if names.len() > MAX_DEPTH {
                return None;
            }
            at = *parent;
        }
        names.reverse();
        Some(names)
    }

    fn child(&self, parent: Option<u32>, name: &str) -> Option<(u32, bool)> {
        self.by_id
            .iter()
            .find(|(_, (p, n, _))| *p == parent && n == name)
            .map(|(id, (_, _, smart))| (*id, *smart))
    }

    /// The manual collection at `path`; `create` makes the ones missing.
    /// `Err(())` when a collection on the way is smart.
    fn resolve(
        &mut self,
        tx: &Transaction<'_>,
        path: &[String],
        create: bool,
    ) -> FmResult<Result<Option<u32>, ()>> {
        let mut parent = None;
        for name in path {
            parent = match self.child(parent, name) {
                Some((_, true)) => return Ok(Err(())),
                Some((id, false)) => Some(id),
                None if create => {
                    let id = super::collections::insert_collection(tx, parent, name, None)?;
                    self.by_id.insert(id, (parent, name.clone(), false));
                    Some(id)
                }
                None => return Ok(Ok(None)),
            };
        }
        Ok(Ok(parent))
    }
}

/// What the index holds for one face.
fn local_meta(tx: &Transaction<'_>, face: u32, collections: &Collections) -> FmResult<Meta> {
    let mut meta = tx
        .prepare_cached("SELECT favorite, rating, notes FROM user_meta WHERE face_id = ?1")
        .and_then(|mut s| {
            s.query_row([face], |row| {
                Ok(Meta {
                    favorite: row.get(0)?,
                    rating: row.get(1)?,
                    notes: row.get(2)?,
                    ..Meta::default()
                })
            })
            .optional()
        })
        .map_err(|e| db_error(&e))?
        .unwrap_or_default();
    meta.tags = tx
        .prepare_cached(
            "SELECT tag.name FROM face_tag JOIN tag ON tag.id = face_tag.tag_id
             WHERE face_tag.face_id = ?1",
        )
        .and_then(|mut s| s.query_map([face], |row| row.get(0))?.collect())
        .map_err(|e| db_error(&e))?;
    let ids: Vec<u32> = tx
        .prepare_cached("SELECT collection_id FROM collection_face WHERE face_id = ?1")
        .and_then(|mut s| s.query_map([face], |row| row.get(0))?.collect())
        .map_err(|e| db_error(&e))?;
    meta.collections = ids
        .into_iter()
        .filter_map(|id| collections.path(id))
        .collect();
    meta.pins = virtual_styles::face_pins(tx, face, |id| collections.path(id))?;
    meta.normalize();
    Ok(meta)
}

/// What [`apply`] did to one face.
struct Applied {
    /// New conflicts (collections and virtual styles that cannot be joined
    /// here).
    conflicts: usize,
    /// Whether the index changed.
    changed: bool,
}

/// Writes `merged` over `local`, what the index holds, for one face. A
/// collection or virtual style that cannot be joined here is carried; one
/// that can is joined and no longer carried.
fn apply(
    tx: &Transaction<'_>,
    source: SourceId,
    file: &str,
    face: u32,
    local: &Meta,
    merged: &Meta,
    collections: &mut Collections,
) -> FmResult<Applied> {
    let mut changed = false;
    if (local.favorite, local.rating, &local.notes)
        != (merged.favorite, merged.rating, &merged.notes)
    {
        changed = true;
        tx.prepare_cached(
            "INSERT INTO user_meta (face_id, favorite, rating, notes) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (face_id) DO UPDATE SET
                 favorite = excluded.favorite, rating = excluded.rating, notes = excluded.notes",
        )
        .and_then(|mut s| s.execute(params![face, merged.favorite, merged.rating, merged.notes]))
        .map_err(|e| db_error(&e))?;
    }

    let lower = |tags: &[String]| -> BTreeMap<String, String> {
        tags.iter()
            .map(|t| (t.to_ascii_lowercase(), t.clone()))
            .collect()
    };
    let (had, want) = (lower(&local.tags), lower(&merged.tags));
    for (key, name) in &want {
        if had.contains_key(key) {
            continue;
        }
        let id: Option<u32> = tx
            .prepare_cached("SELECT id FROM tag WHERE name = ?1")
            .and_then(|mut s| s.query_row([name], |row| row.get(0)).optional())
            .map_err(|e| db_error(&e))?;
        let id = match id {
            Some(id) => id,
            None => {
                tx.prepare_cached("INSERT INTO tag (name) VALUES (?1)")
                    .and_then(|mut s| s.execute([name]))
                    .map_err(|e| db_error(&e))?;
                u32::try_from(tx.last_insert_rowid())
                    .map_err(|_| FmError::internal("tag id out of range"))?
            }
        };
        changed |= tx
            .prepare_cached("INSERT OR IGNORE INTO face_tag (face_id, tag_id) VALUES (?1, ?2)")
            .and_then(|mut s| s.execute(params![face, id]))
            .map_err(|e| db_error(&e))?
            > 0;
    }
    for (key, name) in &had {
        if want.contains_key(key) {
            continue;
        }
        changed |= tx
            .prepare_cached(
                "DELETE FROM face_tag WHERE face_id = ?1
                 AND tag_id = (SELECT id FROM tag WHERE name = ?2)",
            )
            .and_then(|mut s| s.execute(params![face, name]))
            .map_err(|e| db_error(&e))?
            > 0;
        tx.prepare_cached(
            "DELETE FROM tag WHERE name = ?1
             AND NOT EXISTS (SELECT 1 FROM face_tag WHERE tag_id = tag.id)",
        )
        .and_then(|mut s| s.execute([name]))
        .map_err(|e| db_error(&e))?;
    }

    let mut conflicts = 0;
    for path in merged
        .collections
        .iter()
        .filter(|p| !local.collections.contains(p))
    {
        match collections.resolve(tx, path, true)? {
            Ok(Some(id)) => {
                changed |= tx
                    .prepare_cached(
                        "INSERT OR IGNORE INTO collection_face (collection_id, face_id) VALUES (?1, ?2)",
                    )
                    .and_then(|mut s| s.execute(params![id, face]))
                    .map_err(|e| db_error(&e))?
                    > 0;
                drop_carried(tx, face, path)?;
            }
            Ok(None) => {}
            Err(()) => {
                let disk = serde_json::to_string(path).unwrap_or_default();
                conflicts +=
                    insert_conflict(tx, source, file, Some(face), COLLECTION_FIELD, None, &disk)?;
            }
        }
    }
    for path in local
        .collections
        .iter()
        .filter(|p| !merged.collections.contains(p))
    {
        if let Ok(Some(id)) = collections.resolve(tx, path, false)? {
            changed |= tx
                .prepare_cached(
                    "DELETE FROM collection_face WHERE collection_id = ?1 AND face_id = ?2",
                )
                .and_then(|mut s| s.execute(params![id, face]))
                .map_err(|e| db_error(&e))?
                > 0;
        }
    }

    let had: HashMap<PinKey, &Pin> = local.pins.iter().map(|p| (p.key(), p)).collect();
    let want: HashMap<PinKey, &Pin> = merged.pins.iter().map(|p| (p.key(), p)).collect();
    for (key, pin) in &want {
        if had.get(key) == Some(pin) {
            continue;
        }
        let holder = match (&pin.collection, &pin.set) {
            (Some(path), _) => match collections.resolve(tx, path, true)? {
                Ok(Some(id)) => Holder::Collection(id),
                Ok(None) => continue,
                Err(()) => {
                    let disk = serde_json::to_string(pin).unwrap_or_default();
                    conflicts +=
                        insert_conflict(tx, source, file, Some(face), PIN_FIELD, None, &disk)?;
                    continue;
                }
            },
            (None, Some(set)) => match virtual_styles::set_by_name(tx, set, true)? {
                Some(id) => Holder::Set(id),
                None => continue,
            },
            (None, None) => continue,
        };
        changed |= virtual_styles::put_pin(tx, face, pin, holder)?;
        drop_carried_pin(tx, face, pin)?;
    }
    for (key, pin) in &had {
        if want.contains_key(key) {
            continue;
        }
        let holder = match (&pin.collection, &pin.set) {
            (Some(path), _) => match collections.resolve(tx, path, false)? {
                Ok(Some(id)) => Holder::Collection(id),
                _ => continue,
            },
            (None, Some(set)) => match virtual_styles::set_by_name(tx, set, false)? {
                Some(id) => Holder::Set(id),
                None => continue,
            },
            (None, None) => continue,
        };
        changed |= virtual_styles::remove_pin(tx, face, &pin.name, holder)?;
    }
    Ok(Applied { conflicts, changed })
}

/// The collection paths carried for `face`: on another machine it is in
/// them, here they cannot be joined.
fn carried_paths(tx: &Transaction<'_>, face: u32) -> FmResult<Vec<Vec<String>>> {
    let rows: Vec<String> = tx
        .prepare_cached(
            "SELECT DISTINCT disk FROM sidecar_conflict WHERE face_id = ?1 AND field = ?2",
        )
        .and_then(|mut s| {
            s.query_map(params![face, COLLECTION_FIELD], |row| row.get(0))?
                .collect()
        })
        .map_err(|e| db_error(&e))?;
    Ok(rows
        .iter()
        .filter_map(|json| serde_json::from_str(json).ok())
        .collect())
}

/// Stops carrying `path` for `face`: it was joined, or no machine has it.
fn drop_carried(tx: &Transaction<'_>, face: u32, path: &[String]) -> FmResult<()> {
    let disk = serde_json::to_string(path).unwrap_or_default();
    tx.prepare_cached(
        "DELETE FROM sidecar_conflict WHERE face_id = ?1 AND field = ?2 AND disk = ?3",
    )
    .and_then(|mut s| s.execute(params![face, COLLECTION_FIELD, disk]))
    .map_err(|e| db_error(&e))?;
    Ok(())
}

/// The virtual styles carried for `face`: on another machine it has them,
/// here their collection cannot be joined.
fn carried_pins(tx: &Transaction<'_>, face: u32) -> FmResult<Vec<Pin>> {
    let rows: Vec<String> = tx
        .prepare_cached(
            "SELECT DISTINCT disk FROM sidecar_conflict WHERE face_id = ?1 AND field = ?2",
        )
        .and_then(|mut s| {
            s.query_map(params![face, PIN_FIELD], |row| row.get(0))?
                .collect()
        })
        .map_err(|e| db_error(&e))?;
    Ok(rows
        .iter()
        .filter_map(|json| serde_json::from_str(json).ok())
        .collect())
}

/// Stops carrying the virtual style `pin` (any coordinates) for `face`: it
/// was joined, or no machine has it.
fn drop_carried_pin(tx: &Transaction<'_>, face: u32, pin: &Pin) -> FmResult<()> {
    for carried in carried_pins(tx, face)?
        .into_iter()
        .filter(|c| c.key() == pin.key())
    {
        let disk = serde_json::to_string(&carried).unwrap_or_default();
        tx.prepare_cached(
            "DELETE FROM sidecar_conflict WHERE face_id = ?1 AND field = ?2 AND disk = ?3",
        )
        .and_then(|mut s| s.execute(params![face, PIN_FIELD, disk]))
        .map_err(|e| db_error(&e))?;
    }
    Ok(())
}

/// Keeps a value that lost a conflict; 1 when it was not known yet.
fn insert_conflict(
    tx: &Transaction<'_>,
    source: SourceId,
    file: &str,
    face: Option<u32>,
    field: &str,
    local: Option<&str>,
    disk: &str,
) -> FmResult<usize> {
    tx.prepare_cached(
        "INSERT OR IGNORE INTO sidecar_conflict (source_id, file, face_id, field, local, disk)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .and_then(|mut s| s.execute(params![source.0, file, face, field, local, disk]))
    .map_err(|e| db_error(&e))
}

/// The sidecar restricted to the faces the source has indexed: the base of
/// the next merge. Entries of faces not indexed stay out of it, so when such
/// a face is indexed later its entry is new to the merge and is taken.
fn indexed_only(mut sidecar: Sidecar, faces: &SourceFaces) -> Sidecar {
    sidecar.faces.retain(|e| faces.get(&e.key()).is_some());
    sidecar
}

/// `path` relative to `root`, with `/` separators; `None` outside `root` or
/// when a part is not UTF-8.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Option<Vec<&str>> = rel.components().map(|c| c.as_os_str().to_str()).collect();
    let parts = parts?;
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The `*.json` file names in the sidecar folder; none when it is missing.
fn list_sidecars(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".json") && !n.starts_with('.'))
        .collect();
    names.sort();
    names
}

/// The file's text and stamp; `None` when there is no file. Text that is not
/// UTF-8 reads as text that does not parse.
fn read_file(path: &Path) -> FmResult<Option<(String, String)>> {
    let failed = "A sidecar metadata file could not be read.";
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(FmError::io(failed, path, &e)),
    };
    let bytes = fs::read(path).map_err(|e| FmError::io(failed, path, &e))?;
    Ok(Some((
        String::from_utf8_lossy(&bytes).into_owned(),
        stamp(&meta),
    )))
}

/// Writes the sidecar through a temporary file, so a crash or a sync client
/// never sees half of it; returns the new file's stamp.
fn write_file(src: &SourceDir, name: &str, sidecar: &Sidecar) -> FmResult<String> {
    let failed = "A sidecar metadata file could not be written.";
    let dir = src.dir();
    match fs::create_dir(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(FmError::io(failed, &dir, &e)),
    }
    let path = dir.join(name);
    let tmp = dir.join(format!(".{name}.tmp"));
    fs::write(&tmp, sidecar.to_json()).map_err(|e| FmError::io(failed, &tmp, &e))?;
    fs::rename(&tmp, &path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        FmError::io(failed, &path, &e)
    })?;
    let meta = fs::metadata(&path).map_err(|e| FmError::io(failed, &path, &e))?;
    Ok(stamp(&meta))
}

/// Size and modification time, which change when a sync client replaces
/// the file.
fn stamp(meta: &fs::Metadata) -> String {
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    format!("{}:{mtime}", meta.len())
}

/// One sentence for a value that lost a conflict.
fn conflict_message(field: &str, local: Option<&str>, disk: &str) -> String {
    if field == PIN_FIELD {
        let pin: Option<Pin> = serde_json::from_str(disk).ok();
        let (name, path) = pin.map_or_else(Default::default, |p| {
            (p.name, p.collection.unwrap_or_default().join(" / "))
        });
        return format!(
            "Another machine pinned the style \"{name}\" of this font in the collection \"{path}\", which is a smart collection here, so it was not added here. The other machines keep it."
        );
    }
    if field == COLLECTION_FIELD {
        let path: Vec<String> = serde_json::from_str(disk).unwrap_or_default();
        return format!(
            "Another machine put this font in the collection \"{}\", which is a smart collection here, so it was not added here. The other machines keep it.",
            path.join(" / ")
        );
    }
    let show = |json: Option<&str>| -> String {
        let value: serde_json::Value = json
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or(serde_json::Value::Null);
        match (Field::from_name(field), value) {
            (Some(Field::Favorite), serde_json::Value::Bool(true)) => "a favorite".into(),
            (Some(Field::Favorite), _) => "not a favorite".into(),
            (Some(Field::Rating), serde_json::Value::Number(n)) => format!("{n} stars"),
            (Some(Field::Rating), _) => "no rating".into(),
            (_, serde_json::Value::String(s)) => format!("\"{s}\""),
            _ => "empty".into(),
        }
    };
    let what = match Field::from_name(field) {
        Some(Field::Favorite) => "marked this font",
        Some(Field::Rating) => "rated this font",
        Some(Field::FamilyNotes) => "set the notes of this font's family to",
        _ => "set the notes of this font to",
    };
    format!(
        "Another machine {what} {}; this machine's value ({}) is kept. Set it again to dismiss this.",
        show(Some(disk)),
        show(local)
    )
}

#[cfg(test)]
mod tests;
