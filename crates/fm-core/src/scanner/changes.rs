//! Hashing and change detection: what a rescan reads again.
//!
//! Every indexed file carries a blake3 [`FileHash`] of its bytes. The hash is
//! taken from the same read as the parse, so a file is read once per change.
//!
//! [`FileIndex::rescan`] compares a fresh [`Discovery`] with what is indexed:
//!
//! - A file whose size and modification time are both unchanged is skipped:
//!   it is not read, hashed or parsed again.
//! - A new file, or one whose size or modification time differs, is read,
//!   hashed and parsed. A file without a modification time is always read.
//! - An indexed file that is no longer found is marked gone. It stays in the
//!   index with its metadata, so user data attached to it survives a folder
//!   that is unplugged or a file that comes back (ADR 0004). A file under an
//!   entry the walk could not read is not marked gone: it may still be there.
//! - A file that cannot be read keeps its previous entry and is reported in
//!   [`Rescan::failed`]. Its old size and time stay, so the next rescan tries
//!   it again.
//!
//! [`FileIndex::refresh`] applies the same rules to only the paths a watcher
//! event names, so one changed file does not walk the whole source folder.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::parser::{ParsedFile, parse};
use crate::scanner::discovery::is_hidden;
use crate::scanner::{Discovery, FoundFile, Unreadable, discover, is_font_file};
use crate::{FmError, FmResult};

/// The blake3 hash of a file's bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileHash([u8; 32]);

impl FileHash {
    /// Hashes `data`.
    pub fn of(data: &[u8]) -> Self {
        Self(*blake3::hash(data).as_bytes())
    }

    /// The 32 hash bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The hash as 64 lower-case hex digits.
    pub fn to_hex(&self) -> String {
        self.to_string()
    }
}

impl From<[u8; 32]> for FileHash {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl fmt::Display for FileHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl fmt::Debug for FileHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FileHash({self})")
    }
}

/// Reads the font file at `path` once, then hashes and parses those bytes.
///
/// Fails only when the file cannot be read; malformed font data is a
/// [`ParsedFile`] with [`fm_api::common::Health::Broken`].
pub fn hash_and_parse(path: &Path) -> FmResult<(FileHash, ParsedFile)> {
    let data =
        fs::read(path).map_err(|e| FmError::io("The font file could not be read.", path, &e))?;
    Ok((FileHash::of(&data), parse(&data)))
}

/// One font file in the index.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexedFile {
    /// The path as discovered.
    pub path: PathBuf,
    /// The size in bytes when the file was last read.
    pub size: u64,
    /// The modification time when the file was last read.
    pub modified: Option<SystemTime>,
    /// The blake3 hash of the bytes last read.
    pub hash: FileHash,
    /// What the parser found in those bytes.
    pub parsed: ParsedFile,
    /// `true` when the last rescan of its source folder did not find the file.
    pub gone: bool,
}

impl IndexedFile {
    /// Whether `found` has the size and modification time this entry was read
    /// with. No modification time means no proof: the file counts as changed.
    fn same_stamp(&self, found: &FoundFile) -> bool {
        self.size == found.size && self.modified.is_some() && self.modified == found.modified
    }
}

/// The indexed font files, keyed by path. The SQLite index (E3) persists it;
/// this type owns the rescan rules.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileIndex {
    files: BTreeMap<PathBuf, IndexedFile>,
}

/// What one [`FileIndex::rescan`] did. Every list is sorted by path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rescan {
    /// Files not indexed before, now read, hashed and parsed.
    pub added: Vec<PathBuf>,
    /// Indexed files whose size or modification time changed, read again.
    pub changed: Vec<PathBuf>,
    /// Indexed files with the same size and modification time, not read.
    pub unchanged: Vec<PathBuf>,
    /// Indexed files no longer found, now marked gone.
    pub gone: Vec<PathBuf>,
    /// Files that could not be read; their previous entry, if any, is kept.
    pub failed: Vec<Unreadable>,
}

impl FileIndex {
    /// An empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// The entry for `path`, gone or not.
    pub fn get(&self, path: &Path) -> Option<&IndexedFile> {
        self.files.get(path)
    }

    /// Every entry, gone ones included, sorted by path.
    pub fn files(&self) -> impl Iterator<Item = &IndexedFile> {
        self.files.values()
    }

    /// How many entries there are, gone ones included.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether the index has no entries.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Adds or replaces an entry, for example one loaded from the database.
    pub fn insert(&mut self, file: IndexedFile) {
        self.files.insert(file.path.clone(), file);
    }

    /// Brings the entries under `root` up to date with `discovery`, the result
    /// of [`crate::scanner::discover`] on `root`. Entries outside `root` are
    /// left alone, so each source folder rescans on its own.
    pub fn rescan(&mut self, root: &Path, discovery: &Discovery) -> Rescan {
        let mut out = Rescan::default();
        let mut found = HashSet::with_capacity(discovery.files.len());

        for file in &discovery.files {
            found.insert(file.path.as_path());
            self.update(file, &mut out);
        }

        for entry in self.files.values_mut() {
            if entry.gone
                || !entry.path.starts_with(root)
                || found.contains(entry.path.as_path())
                || discovery
                    .unreadable
                    .iter()
                    .any(|u| entry.path.starts_with(&u.path))
            {
                continue;
            }
            entry.gone = true;
            out.gone.push(entry.path.clone());
        }

        out.added.sort();
        out.changed.sort();
        out.unchanged.sort();
        out
    }

    /// Brings the entries for `paths` up to date, by the same rules as
    /// [`FileIndex::rescan`]. The paths are what file-system events under the
    /// source folder `root` named:
    ///
    /// - a font file that exists is read again unless its size and time are
    ///   unchanged;
    /// - a folder that exists (created, or renamed into place) is discovered
    ///   and rescanned on its own;
    /// - a path that no longer exists marks its entry, and every entry under
    ///   it (a removed or renamed-away folder), gone.
    ///
    /// Paths outside `root`, inside a hidden folder or without a font
    /// extension are ignored, as [`discover`] would ignore them.
    pub fn refresh(&mut self, root: &Path, paths: &[PathBuf]) -> Rescan {
        let mut out = Rescan::default();
        let mut seen = HashSet::with_capacity(paths.len());
        for path in paths {
            if !seen.insert(path.as_path()) || !Self::watched(root, path) {
                continue;
            }
            match fs::metadata(path) {
                Ok(meta) if meta.is_dir() => match discover(path) {
                    Ok(discovery) => out.extend(self.rescan(path, &discovery)),
                    Err(error) => out.failed.push(Unreadable {
                        path: path.clone(),
                        error,
                    }),
                },
                Ok(meta) if meta.is_file() && is_font_file(path) => {
                    let file = FoundFile {
                        path: path.clone(),
                        size: meta.len(),
                        modified: meta.modified().ok(),
                    };
                    self.update(&file, &mut out);
                }
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => self.mark_gone(path, &mut out),
                Err(e) => out.failed.push(Unreadable {
                    path: path.clone(),
                    error: FmError::io("A font file could not be read.", path, &e),
                }),
            }
        }
        out.sort();
        out
    }

    /// Whether a path under `root` is one [`discover`] would reach: no
    /// folder from `root` down to it is hidden. A file's own name may start
    /// with `.`; a folder's may not.
    fn watched(root: &Path, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(root) else {
            return false;
        };
        let mut names: Vec<_> = relative.iter().collect();
        if !path.is_dir() {
            names.pop();
        }
        !names.into_iter().any(is_hidden)
    }

    /// Reads `file` again unless its entry has the same size and time.
    fn update(&mut self, file: &FoundFile, out: &mut Rescan) {
        if let Some(previous) = self.files.get_mut(&file.path)
            && previous.same_stamp(file)
        {
            previous.gone = false;
            out.unchanged.push(file.path.clone());
            return;
        }
        let known = self.files.contains_key(&file.path);
        match hash_and_parse(&file.path) {
            Ok((hash, parsed)) => {
                self.insert(IndexedFile {
                    path: file.path.clone(),
                    size: file.size,
                    modified: file.modified,
                    hash,
                    parsed,
                    gone: false,
                });
                if known {
                    out.changed.push(file.path.clone());
                } else {
                    out.added.push(file.path.clone());
                }
            }
            Err(error) => out.failed.push(Unreadable {
                path: file.path.clone(),
                error,
            }),
        }
    }

    /// Marks the entry for `path`, and every entry under it, gone.
    fn mark_gone(&mut self, path: &Path, out: &mut Rescan) {
        for entry in self.files.values_mut() {
            if !entry.gone && entry.path.starts_with(path) {
                entry.gone = true;
                out.gone.push(entry.path.clone());
            }
        }
    }
}

impl Rescan {
    /// Whether nothing was added, changed, marked gone or failed.
    pub fn is_quiet(&self) -> bool {
        self.added.is_empty()
            && self.changed.is_empty()
            && self.gone.is_empty()
            && self.failed.is_empty()
    }

    fn extend(&mut self, other: Rescan) {
        self.added.extend(other.added);
        self.changed.extend(other.changed);
        self.unchanged.extend(other.unchanged);
        self.gone.extend(other.gone);
        self.failed.extend(other.failed);
    }

    fn sort(&mut self) {
        for list in [
            &mut self.added,
            &mut self.changed,
            &mut self.unchanged,
            &mut self.gone,
        ] {
            list.sort();
            list.dedup();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use fm_api::ErrorCode;
    use fm_api::common::Health;

    use super::*;
    use crate::scanner::discover;
    use crate::test_support::TempDir;

    const REGULAR: &str = "google-static/asap/Asap_Condensed-ExtraLightItalic.ttf";
    const OTHER: &str = "google-static/anybody/Anybody_Expanded-MediumItalic.ttf";

    fn fixture(rel: &str) -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts")
            .join(rel);
        fs::read(path).expect("read fixture")
    }

    fn put(path: &Path, data: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create folder");
        }
        fs::write(path, data).expect("write font");
    }

    fn modified(path: &Path) -> SystemTime {
        fs::metadata(path)
            .and_then(|m| m.modified())
            .expect("modification time")
    }

    fn set_modified(path: &Path, time: SystemTime) {
        fs::File::options()
            .write(true)
            .open(path)
            .and_then(|f| f.set_modified(time))
            .expect("set modification time");
    }

    fn rescan(index: &mut FileIndex, root: &Path) -> Rescan {
        index.rescan(root, &discover(root).expect("discover"))
    }

    #[test]
    fn the_hash_is_blake3_as_hex() {
        // The blake3 test vector for empty input.
        assert_eq!(
            FileHash::of(b"").to_hex(),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
        let hash = FileHash::of(b"font");
        assert_eq!(FileHash::from(*hash.as_bytes()), hash);
        assert_eq!(format!("{hash:?}"), format!("FileHash({hash})"));
    }

    #[test]
    fn each_file_gets_a_blake3_hash_of_its_bytes() {
        let tmp = TempDir::new("changes-hash");
        let root = tmp.path();
        let regular = fixture(REGULAR);
        put(&root.join("a.ttf"), &regular);
        put(&root.join("copy/Schrift-ü.ttf"), &regular);
        put(&root.join("b.ttf"), &fixture(OTHER));

        let mut index = FileIndex::new();
        let report = rescan(&mut index, root);

        assert_eq!(report.added.len(), 3);
        assert_eq!(index.len(), 3);
        for file in index.files() {
            let bytes = fs::read(&file.path).expect("read");
            assert_eq!(file.hash, FileHash::of(&bytes), "{}", file.path.display());
            assert_eq!(file.hash.to_hex(), blake3::hash(&bytes).to_hex().as_str());
            assert_eq!(file.parsed.health, Health::Ok);
            assert!(!file.gone);
        }
        let hash = |p: &str| index.get(&root.join(p)).expect("indexed").hash;
        assert_eq!(hash("a.ttf"), hash("copy/Schrift-ü.ttf"));
        assert_ne!(hash("a.ttf"), hash("b.ttf"));
    }

    #[test]
    fn a_rescan_skips_a_file_whose_size_and_time_are_unchanged() {
        let tmp = TempDir::new("changes-skip");
        let root = tmp.path();
        let path = root.join("a.ttf");
        let regular = fixture(REGULAR);
        put(&path, &regular);
        let mut index = FileIndex::new();
        rescan(&mut index, root);
        let before = index.get(&path).expect("indexed").clone();

        // Same size, same time, other bytes: only a read would notice.
        let time = modified(&path);
        put(&path, &vec![0u8; regular.len()]);
        set_modified(&path, time);
        let report = rescan(&mut index, root);

        assert_eq!(report.unchanged, vec![path.clone()]);
        assert!(report.added.is_empty() && report.changed.is_empty());
        assert_eq!(index.get(&path), Some(&before));
    }

    #[test]
    fn a_rescan_reparses_a_file_whose_time_changed() {
        let tmp = TempDir::new("changes-time");
        let root = tmp.path();
        let path = root.join("a.ttf");
        let regular = fixture(REGULAR);
        put(&path, &regular);
        let mut index = FileIndex::new();
        rescan(&mut index, root);

        let time = modified(&path) + Duration::from_secs(10);
        put(&path, &vec![0u8; regular.len()]);
        set_modified(&path, time);
        let report = rescan(&mut index, root);

        assert_eq!(report.changed, vec![path.clone()]);
        let entry = index.get(&path).expect("indexed");
        assert_eq!(entry.hash, FileHash::of(&vec![0u8; regular.len()]));
        assert_eq!(entry.modified, Some(time));
        assert!(matches!(entry.parsed.health, Health::Broken(_)));
    }

    #[test]
    fn a_rescan_reparses_a_file_whose_size_changed() {
        let tmp = TempDir::new("changes-size");
        let root = tmp.path();
        let path = root.join("a.ttf");
        put(&path, &fixture(REGULAR));
        let mut index = FileIndex::new();
        rescan(&mut index, root);

        let time = modified(&path);
        let other = fixture(OTHER);
        put(&path, &other);
        set_modified(&path, time);
        let report = rescan(&mut index, root);

        assert_eq!(report.changed, vec![path.clone()]);
        let entry = index.get(&path).expect("indexed");
        assert_eq!(entry.hash, FileHash::of(&other));
        assert_eq!(entry.size, other.len() as u64);
        assert_eq!(entry.parsed, parse(&other));
    }

    #[test]
    fn a_file_without_a_time_is_always_read() {
        let tmp = TempDir::new("changes-no-time");
        let root = tmp.path();
        let path = root.join("a.ttf");
        put(&path, &fixture(REGULAR));
        let mut found = discover(root).expect("discover");
        found.files[0].modified = None;
        let mut index = FileIndex::new();
        index.rescan(root, &found);

        let report = index.rescan(root, &found);

        assert_eq!(report.changed, vec![path]);
        assert!(report.unchanged.is_empty());
    }

    #[test]
    fn a_removed_file_is_marked_gone_not_deleted() {
        let tmp = TempDir::new("changes-gone");
        let root = tmp.path();
        let kept = root.join("kept.ttf");
        let removed = root.join("sub/removed.ttf");
        put(&kept, &fixture(REGULAR));
        put(&removed, &fixture(OTHER));
        let mut index = FileIndex::new();
        rescan(&mut index, root);
        let before = index.get(&removed).expect("indexed").clone();

        fs::remove_file(&removed).expect("remove");
        let report = rescan(&mut index, root);

        assert_eq!(report.gone, vec![removed.clone()]);
        assert_eq!(report.unchanged, vec![kept.clone()]);
        assert_eq!(index.len(), 2);
        let entry = index.get(&removed).expect("still indexed");
        assert!(entry.gone);
        assert_eq!(entry.hash, before.hash);
        assert_eq!(entry.parsed, before.parsed);
        assert!(!index.get(&kept).expect("indexed").gone);

        // A second rescan does not report it again.
        assert!(rescan(&mut index, root).gone.is_empty());
    }

    #[test]
    fn an_emptied_folder_marks_every_file_gone() {
        let tmp = TempDir::new("changes-empty");
        let root = tmp.path();
        put(&root.join("a.ttf"), &fixture(REGULAR));
        put(&root.join("b.ttf"), &fixture(OTHER));
        let mut index = FileIndex::new();
        rescan(&mut index, root);

        fs::remove_file(root.join("a.ttf")).expect("remove");
        fs::remove_file(root.join("b.ttf")).expect("remove");
        let report = rescan(&mut index, root);

        assert_eq!(report.gone, vec![root.join("a.ttf"), root.join("b.ttf")]);
        assert_eq!(index.len(), 2);
        assert!(index.files().all(|f| f.gone));
    }

    #[test]
    fn a_file_that_comes_back_is_no_longer_gone() {
        let tmp = TempDir::new("changes-back");
        let root = tmp.path();
        let path = root.join("a.ttf");
        let regular = fixture(REGULAR);
        put(&path, &regular);
        let mut index = FileIndex::new();
        rescan(&mut index, root);
        let time = modified(&path);

        fs::remove_file(&path).expect("remove");
        rescan(&mut index, root);
        assert!(index.get(&path).expect("indexed").gone);

        put(&path, &regular);
        set_modified(&path, time);
        let report = rescan(&mut index, root);

        assert_eq!(report.unchanged, vec![path.clone()]);
        assert!(!index.get(&path).expect("indexed").gone);
    }

    #[test]
    fn a_rescan_leaves_other_source_folders_alone() {
        let tmp = TempDir::new("changes-roots");
        let one = tmp.path().join("one");
        let two = tmp.path().join("one-more");
        put(&one.join("a.ttf"), &fixture(REGULAR));
        put(&two.join("b.ttf"), &fixture(OTHER));
        let mut index = FileIndex::new();
        rescan(&mut index, &one);
        rescan(&mut index, &two);

        fs::remove_file(two.join("b.ttf")).expect("remove");
        let report = rescan(&mut index, &one);

        assert!(report.gone.is_empty());
        assert!(!index.get(&two.join("b.ttf")).expect("indexed").gone);
        assert_eq!(rescan(&mut index, &two).gone, vec![two.join("b.ttf")]);
    }

    #[test]
    fn a_file_under_an_unreadable_folder_is_not_marked_gone() {
        let tmp = TempDir::new("changes-unreadable");
        let root = tmp.path();
        let hidden = root.join("locked/a.ttf");
        put(&hidden, &fixture(REGULAR));
        let mut index = FileIndex::new();
        rescan(&mut index, root);

        // The walk could not open `locked` this time (permission denied).
        let found = Discovery {
            unreadable: vec![Unreadable {
                path: root.join("locked"),
                error: FmError::new(ErrorCode::PermissionDenied, "A folder could not be read."),
            }],
            ..Discovery::default()
        };
        let report = index.rescan(root, &found);

        assert!(report.gone.is_empty());
        assert!(!index.get(&hidden).expect("indexed").gone);
    }

    #[test]
    fn a_file_that_cannot_be_read_keeps_its_entry() {
        let tmp = TempDir::new("changes-failed");
        let root = tmp.path();
        let path = root.join("a.ttf");
        put(&path, &fixture(REGULAR));
        let mut index = FileIndex::new();
        rescan(&mut index, root);
        let before = index.get(&path).expect("indexed").clone();

        // Listed with a new size, but the read fails (here: it vanished).
        let mut found = discover(root).expect("discover");
        found.files[0].size += 1;
        let missing = root.join("missing.ttf");
        found.files.push(FoundFile {
            path: missing.clone(),
            size: 1,
            modified: None,
        });
        fs::remove_file(&path).expect("remove");
        let report = index.rescan(root, &found);

        let failed: Vec<_> = report.failed.iter().map(|u| u.path.clone()).collect();
        assert_eq!(failed, vec![path.clone(), missing.clone()]);
        assert!(
            report
                .failed
                .iter()
                .all(|u| u.error.code() == ErrorCode::NotFound)
        );
        assert!(report.changed.is_empty() && report.added.is_empty());
        assert_eq!(index.get(&path), Some(&before));
        assert_eq!(index.get(&missing), None);
    }

    #[test]
    fn a_broken_font_is_indexed_with_its_hash() {
        let tmp = TempDir::new("changes-broken");
        let root = tmp.path();
        let path = root.join("broken.ttf");
        put(&path, b"not a font");
        let mut index = FileIndex::new();

        let report = rescan(&mut index, root);

        assert_eq!(report.added, vec![path.clone()]);
        let entry = index.get(&path).expect("indexed");
        assert_eq!(entry.hash, FileHash::of(b"not a font"));
        assert!(matches!(entry.parsed.health, Health::Broken(_)));
    }

    #[test]
    fn a_refresh_reads_only_the_named_files() {
        let tmp = TempDir::new("refresh-files");
        let root = tmp.path();
        let a = root.join("a.ttf");
        let b = root.join("Schrift-ü/b.ttf");
        put(&a, &fixture(REGULAR));
        put(&b, &fixture(OTHER));
        let mut index = FileIndex::new();
        rescan(&mut index, root);
        let new = root.join("new.otf");
        put(&new, &fixture(REGULAR));
        let time = modified(&b) + Duration::from_secs(10);
        put(&b, &fixture(REGULAR));
        set_modified(&b, time);
        fs::remove_file(&a).expect("remove");

        let report = index.refresh(root, &[new.clone(), b.clone(), a.clone(), new.clone()]);

        assert_eq!(report.added, vec![new.clone()]);
        assert_eq!(report.changed, vec![b.clone()]);
        assert_eq!(report.gone, vec![a.clone()]);
        assert!(report.unchanged.is_empty() && report.failed.is_empty());
        assert!(!report.is_quiet());
        assert_eq!(
            index.get(&b).expect("indexed").hash,
            FileHash::of(&fixture(REGULAR))
        );
        assert!(index.get(&a).expect("still indexed").gone);

        // Nothing changed since: the same paths are a quiet refresh.
        let again = index.refresh(root, &[new.clone(), b.clone(), a.clone()]);
        assert_eq!(again.unchanged, vec![b, new]);
        assert!(again.is_quiet());
    }

    #[test]
    fn a_refresh_of_a_folder_rescans_it_and_a_removed_folder_is_gone() {
        let tmp = TempDir::new("refresh-folder");
        let root = tmp.path();
        let mut index = FileIndex::new();
        rescan(&mut index, root);
        let folder = root.join("incoming");
        put(&folder.join("a.ttf"), &fixture(REGULAR));
        put(&folder.join("deep/b.ttf"), &fixture(OTHER));

        let report = index.refresh(root, std::slice::from_ref(&folder));
        assert_eq!(
            report.added,
            vec![folder.join("a.ttf"), folder.join("deep/b.ttf")]
        );

        fs::remove_dir_all(&folder).expect("remove folder");
        let report = index.refresh(root, std::slice::from_ref(&folder));
        assert_eq!(
            report.gone,
            vec![folder.join("a.ttf"), folder.join("deep/b.ttf")]
        );
        assert_eq!(index.len(), 2);
    }

    #[test]
    fn a_refresh_ignores_what_discovery_would_skip() {
        let tmp = TempDir::new("refresh-skip");
        let root = tmp.path().join("source");
        let outside = tmp.path().join("other/a.ttf");
        let hidden_folder = root.join(".cache");
        let in_hidden = hidden_folder.join("b.ttf");
        let not_a_font = root.join("readme.txt");
        let hidden_file = root.join(".c.ttf");
        put(&outside, &fixture(REGULAR));
        put(&in_hidden, &fixture(REGULAR));
        put(&not_a_font, b"text");
        put(&hidden_file, &fixture(REGULAR));
        let mut index = FileIndex::new();

        let report = index.refresh(
            &root,
            &[
                outside,
                hidden_folder,
                in_hidden,
                not_a_font,
                hidden_file.clone(),
            ],
        );

        // A hidden file is listed by discovery too; only hidden folders are skipped.
        assert_eq!(report.added, vec![hidden_file]);
        assert_eq!(index.len(), 1);
        assert_eq!(report.added, rescan(&mut FileIndex::new(), &root).added);
    }

    #[test]
    fn a_refresh_of_a_broken_file_indexes_it_as_broken() {
        let tmp = TempDir::new("refresh-broken");
        let root = tmp.path();
        let path = root.join("broken.ttf");
        put(&path, b"not a font");
        let mut index = FileIndex::new();

        let report = index.refresh(root, std::slice::from_ref(&path));

        assert_eq!(report.added, vec![path.clone()]);
        assert!(matches!(
            index.get(&path).expect("indexed").parsed.health,
            Health::Broken(_)
        ));
    }
}
