//! Font file discovery: the first step of a scan.
//!
//! [`discover`] walks a source folder and lists every file with a font
//! extension, with its size and modification time. It reads folder listings and file sizes only; it
//! never opens, moves or changes a file (ADR 0004).
//!
//! - Hidden folders (a name that starts with `.`) are skipped. The source
//!   folder itself is walked even when its own name is hidden: the user chose it.
//! - Symbolic links are followed once. Every folder and every file is listed at
//!   most once by its real (canonical) location, so a link back to an ancestor
//!   cannot loop and two links to one folder do not count its fonts twice.
//! - An entry that cannot be read (permission denied, a dangling link) does not
//!   stop the walk; it goes into [`Discovery::unreadable`].

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::{FmError, FmResult};

/// The file extensions of the font formats the scanner knows, lower case.
/// Matching ignores ASCII case.
pub const FONT_EXTENSIONS: [&str; 9] = [
    "ttf", "otf", "ttc", "otc", "woff", "woff2", "pfb", "pfa", "dfont",
];

/// A font file found in a source folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundFile {
    /// The path as reached from the source folder (through links, not
    /// resolved), so the user sees the path they know.
    pub path: PathBuf,
    /// The size in bytes.
    pub size: u64,
    /// The last modification time; `None` when the file system does not
    /// report one. With [`FoundFile::size`] it tells a rescan whether the file
    /// changed.
    pub modified: Option<SystemTime>,
}

/// An entry the walk could not read. The walk goes on without it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable {
    /// The folder, file or link that failed.
    pub path: PathBuf,
    /// Why, with the OS error in the detail.
    pub error: FmError,
}

/// What [`discover`] found in one source folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Discovery {
    /// Every font file, sorted by path.
    pub files: Vec<FoundFile>,
    /// The sum of the sizes of [`Discovery::files`], in bytes.
    pub total_size: u64,
    /// Entries that could not be read, in walk order.
    pub unreadable: Vec<Unreadable>,
}

impl Discovery {
    /// How many font files were found.
    pub fn count(&self) -> usize {
        self.files.len()
    }
}

/// Whether `path` has one of the [`FONT_EXTENSIONS`], ignoring ASCII case.
pub fn is_font_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| FONT_EXTENSIONS.iter().any(|f| ext.eq_ignore_ascii_case(f)))
}

/// Lists every font file under `root`, recursively.
///
/// Fails only when `root` itself cannot be read or is not a folder; problems
/// further down are collected in [`Discovery::unreadable`].
pub fn discover(root: &Path) -> FmResult<Discovery> {
    let meta = fs::metadata(root)
        .map_err(|e| FmError::io("The source folder could not be opened.", root, &e))?;
    if !meta.is_dir() {
        return Err(
            FmError::invalid("The source is not a folder.").with_detail(root.display().to_string())
        );
    }
    let canonical_root = fs::canonicalize(root)
        .map_err(|e| FmError::io("The source folder could not be opened.", root, &e))?;

    let mut walk = Walk::default();
    walk.seen_dirs.insert(canonical_root.clone());
    walk.pending.push((root.to_path_buf(), canonical_root));
    while let Some((dir, canonical_dir)) = walk.pending.pop() {
        walk.read_dir(&dir, &canonical_dir);
    }

    let mut out = walk.out;
    out.files.sort_by(|a, b| a.path.cmp(&b.path));
    out.total_size = out.files.iter().map(|f| f.size).sum();
    Ok(out)
}

/// The state of one walk: folders still to read and what has been seen, both
/// keyed by canonical path.
#[derive(Default)]
struct Walk {
    pending: Vec<(PathBuf, PathBuf)>,
    seen_dirs: HashSet<PathBuf>,
    seen_files: HashSet<PathBuf>,
    out: Discovery,
}

impl Walk {
    /// Reads one folder. `canonical_dir` is its real location; a plain entry's
    /// real location is `canonical_dir` joined with its name, so only links
    /// need a `canonicalize` call.
    fn read_dir(&mut self, dir: &Path, canonical_dir: &Path) {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => return self.unreadable(dir, "A folder could not be read.", &e),
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    self.unreadable(dir, "A folder could not be read to the end.", &e);
                    break;
                }
            };
            let name = entry.file_name();
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(t) => t,
                Err(e) => {
                    self.unreadable(&path, "A file could not be read.", &e);
                    continue;
                }
            };
            if file_type.is_symlink() {
                self.follow_link(path, &name);
            } else if file_type.is_dir() {
                if !is_hidden(&name) {
                    self.enter(path, canonical_dir.join(&name));
                }
            } else if file_type.is_file() && is_font_file(&path) {
                match entry.metadata() {
                    Ok(meta) => self.add(path, canonical_dir.join(&name), &meta),
                    Err(e) => self.unreadable(&path, "A font file could not be read.", &e),
                }
            }
        }
    }

    /// Follows a link once: to a folder (walked unless already seen) or to a
    /// font file (listed unless already seen).
    fn follow_link(&mut self, path: PathBuf, name: &OsStr) {
        let target = match fs::canonicalize(&path) {
            Ok(target) => target,
            Err(e) => return self.unreadable(&path, "A link points to nothing.", &e),
        };
        let meta = match fs::metadata(&target) {
            Ok(meta) => meta,
            Err(e) => return self.unreadable(&path, "A link could not be followed.", &e),
        };
        if meta.is_dir() {
            if !is_hidden(name) {
                self.enter(path, target);
            }
        } else if meta.is_file() && is_font_file(&path) {
            self.add(path, target, &meta);
        }
    }

    fn enter(&mut self, path: PathBuf, canonical: PathBuf) {
        if self.seen_dirs.insert(canonical.clone()) {
            self.pending.push((path, canonical));
        }
    }

    fn add(&mut self, path: PathBuf, canonical: PathBuf, meta: &fs::Metadata) {
        if self.seen_files.insert(canonical) {
            self.out.files.push(FoundFile {
                path,
                size: meta.len(),
                modified: meta.modified().ok(),
            });
        }
    }

    fn unreadable(&mut self, path: &Path, message: &str, err: &std::io::Error) {
        self.out.unreadable.push(Unreadable {
            path: path.to_path_buf(),
            error: FmError::io(message, path, err),
        });
    }
}

/// A hidden folder by the cross-platform rule: its name starts with `.`.
fn is_hidden(name: &OsStr) -> bool {
    name.as_encoded_bytes().first() == Some(&b'.')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use fm_api::ErrorCode;

    fn write(path: &Path, bytes: usize) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, vec![0u8; bytes]).unwrap();
    }

    /// The found paths relative to `root`, with `/` separators.
    fn relative(found: &Discovery, root: &Path) -> Vec<String> {
        found
            .files
            .iter()
            .map(|f| {
                f.path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    /// Makes a link to a folder; see [`made`].
    fn link_dir(original: &Path, link: &Path) -> bool {
        made(fm_platform::fs::symlink_dir(original, link))
    }

    /// Makes a link to a file; see [`made`].
    fn link_file(original: &Path, link: &Path) -> bool {
        made(fm_platform::fs::symlink_file(original, link))
    }

    /// `true` when the link was made. A desktop may not allow links (Windows
    /// without Developer Mode): the caller then skips its link checks. CI
    /// runners allow them, so there a failure fails the test instead of
    /// skipping it silently.
    fn made(result: std::io::Result<()>) -> bool {
        match result {
            Ok(()) => true,
            Err(e) if std::env::var_os("CI").is_none() => {
                eprintln!("skipping link checks: cannot create a symbolic link here: {e}");
                false
            }
            Err(e) => panic!("cannot create a symbolic link on CI: {e}"),
        }
    }

    #[test]
    fn lists_every_font_extension_recursively_and_nothing_else() {
        let tmp = TempDir::new("discover-ext");
        let root = tmp.path();
        for (i, ext) in FONT_EXTENSIONS.iter().enumerate() {
            write(&root.join(format!("a/b/c/{i}.{ext}")), 1);
        }
        write(&root.join("Upper.TTF"), 1);
        write(&root.join("mixed.WoFf2"), 1);
        write(&root.join("Schrift-ü.otf"), 1);
        for other in [
            "readme.txt",
            "OFL.txt",
            "font.ttf.bak",
            "ttf",
            "font.svg",
            "a/b/x.zip",
        ] {
            write(&root.join(other), 1);
        }
        fs::create_dir_all(root.join("folder.ttf")).unwrap();

        let found = discover(root).unwrap();
        assert_eq!(
            relative(&found, root),
            [
                "Schrift-ü.otf",
                "Upper.TTF",
                "a/b/c/0.ttf",
                "a/b/c/1.otf",
                "a/b/c/2.ttc",
                "a/b/c/3.otc",
                "a/b/c/4.woff",
                "a/b/c/5.woff2",
                "a/b/c/6.pfb",
                "a/b/c/7.pfa",
                "a/b/c/8.dfont",
                "mixed.WoFf2",
            ]
        );
        assert!(found.unreadable.is_empty());
    }

    #[test]
    fn reports_count_and_total_size() {
        let tmp = TempDir::new("discover-size");
        let root = tmp.path();
        write(&root.join("a.ttf"), 100);
        write(&root.join("sub/b.otf"), 2_500);
        write(&root.join("sub/deeper/c.woff2"), 7);
        write(&root.join("sub/not-a-font.txt"), 1_000_000);

        let found = discover(root).unwrap();
        assert_eq!(found.count(), 3);
        assert_eq!(found.total_size, 2_607);
        let sizes: Vec<u64> = found.files.iter().map(|f| f.size).collect();
        assert_eq!(sizes, [100, 2_500, 7]);
    }

    #[test]
    fn records_each_file_modification_time() {
        let tmp = TempDir::new("discover-time");
        let root = tmp.path();
        write(&root.join("a.ttf"), 1);
        let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
        fs::File::options()
            .write(true)
            .open(root.join("a.ttf"))
            .and_then(|f| f.set_modified(time))
            .unwrap();

        let found = discover(root).unwrap();
        assert_eq!(found.files[0].modified, Some(time));
    }

    #[test]
    fn finds_the_whole_fixture_corpus_with_its_manifest_sizes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts");
        let manifest = fs::read_to_string(root.join("manifest.tsv")).unwrap();
        let mut expected: Vec<(String, u64)> = manifest
            .lines()
            .skip(1)
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let cols: Vec<&str> = l.split('\t').collect();
                (cols[4].to_owned(), cols[1].parse().unwrap())
            })
            .collect();
        expected.sort();

        let found = discover(&root).unwrap();
        let mut got: Vec<(String, u64)> = relative(&found, &root)
            .into_iter()
            .zip(found.files.iter().map(|f| f.size))
            .collect();
        got.sort();

        assert_eq!(found.count(), 200);
        assert_eq!(got, expected);
        assert_eq!(
            found.total_size,
            expected.iter().map(|(_, s)| s).sum::<u64>()
        );
    }

    #[test]
    fn skips_hidden_folders_but_not_a_hidden_source_folder() {
        let tmp = TempDir::new("discover-hidden");
        let root = tmp.path().join(".my-fonts");
        write(&root.join("visible.ttf"), 1);
        write(&root.join(".git/objects/x.ttf"), 1);
        write(&root.join(".Trash/old.otf"), 1);
        write(&root.join("sub/.cache/c.woff"), 1);
        write(&root.join("sub/kept.woff"), 1);

        let found = discover(&root).unwrap();
        assert_eq!(relative(&found, &root), ["sub/kept.woff", "visible.ttf"]);
    }

    #[test]
    fn follows_a_folder_link_once_and_does_not_loop() {
        let tmp = TempDir::new("discover-links");
        let root = tmp.path().join("source");
        let outside = tmp.path().join("outside");
        write(&root.join("own.ttf"), 10);
        write(&outside.join("linked.otf"), 20);

        if !link_dir(&outside, &root.join("to-outside")) {
            return;
        }
        // A second link to the same folder, and a link back to the source.
        assert!(link_dir(&outside, &root.join("again")));
        assert!(link_dir(&root, &root.join("loop")));

        let found = discover(&root).unwrap();
        let paths = relative(&found, &root);
        // The linked font once, through whichever link the walk met first.
        assert_eq!(found.count(), 2, "{paths:?}");
        assert!(
            paths[0] == "again/linked.otf" || paths[0] == "to-outside/linked.otf",
            "{paths:?}"
        );
        assert_eq!(paths[1], "own.ttf");
        assert_eq!(found.total_size, 30);
        assert!(found.unreadable.is_empty(), "{:?}", found.unreadable);
    }

    #[test]
    fn follows_a_file_link_and_lists_a_file_reached_twice_once() {
        let tmp = TempDir::new("discover-file-link");
        let root = tmp.path().join("source");
        let outside = tmp.path().join("outside");
        write(&root.join("own.ttf"), 10);
        write(&outside.join("real.otf"), 20);

        if !link_file(&outside.join("real.otf"), &root.join("alias.otf")) {
            return;
        }
        // The same font again through its own folder.
        assert!(link_file(
            &root.join("own.ttf"),
            &root.join("own-alias.ttf")
        ));

        let found = discover(&root).unwrap();
        let paths = relative(&found, &root);
        assert_eq!(paths.len(), 2, "{paths:?}");
        assert!(paths.contains(&"alias.otf".to_owned()), "{paths:?}");
        assert_eq!(found.total_size, 30);
    }

    #[test]
    fn a_dangling_link_is_reported_and_the_walk_goes_on() {
        let tmp = TempDir::new("discover-dangling");
        let root = tmp.path().join("source");
        write(&root.join("ok.ttf"), 5);
        let gone = tmp.path().join("gone");
        fs::create_dir_all(&gone).unwrap();
        if !link_dir(&gone, &root.join("broken")) {
            return;
        }
        fs::remove_dir(&gone).unwrap();

        let found = discover(&root).unwrap();
        assert_eq!(relative(&found, &root), ["ok.ttf"]);
        assert_eq!(found.unreadable.len(), 1);
        assert_eq!(found.unreadable[0].path, root.join("broken"));
        assert_eq!(found.unreadable[0].error.code(), ErrorCode::NotFound);
    }

    #[test]
    fn an_empty_folder_finds_nothing() {
        let tmp = TempDir::new("discover-empty");
        fs::create_dir_all(tmp.path().join("a/b")).unwrap();
        let found = discover(tmp.path()).unwrap();
        assert_eq!(found, Discovery::default());
        assert_eq!(found.count(), 0);
    }

    #[test]
    fn a_missing_source_is_not_found() {
        let tmp = TempDir::new("discover-missing");
        let err = discover(&tmp.path().join("nope")).unwrap_err();
        assert_eq!(err.code(), ErrorCode::NotFound);
        assert_eq!(err.message(), "The source folder could not be opened.");
        assert!(err.detail().unwrap().contains("nope"));
    }

    #[test]
    fn a_file_as_source_is_an_invalid_request() {
        let tmp = TempDir::new("discover-file");
        let file = tmp.path().join("a.ttf");
        write(&file, 1);
        let err = discover(&file).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidRequest);
    }

    #[test]
    fn extension_matching_ignores_case_only() {
        assert!(is_font_file(Path::new("x/Font.TTF")));
        assert!(is_font_file(Path::new("Font.dFont")));
        assert!(!is_font_file(Path::new("Font.ttf2")));
        assert!(!is_font_file(Path::new("Font")));
        assert!(!is_font_file(Path::new(".ttf")));
    }
}
