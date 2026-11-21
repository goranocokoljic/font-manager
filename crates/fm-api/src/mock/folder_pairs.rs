//! The folder-pair rule of the mock (E56.6): a copy of
//! `fm_core::folder_pairs` (fm-api cannot use fm-core).
//! `fixtures/duplicates/pair-cases.json` runs against both, and against the
//! TypeScript mock, so they stay the same rule. The mock has no files on
//! disk, so its sides have no other files.

use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::common::Paged;
use crate::library::{
    DuplicateLevel, DuplicateScope, FOLDER_PAIR_MIN_SHARED, FolderPair, FolderPairFile,
    FolderPairSide, ListFolderPairFilesRequest, ListFolderPairsRequest, PairSide, ProtectedFolder,
    SharedCounts,
};
use crate::{ApiError, ApiResult};

use super::duplicate_sets::{Part, SetFile, version_key};

/// A folder the user or the OS gave the library: a pair climbs no higher,
/// and such a folder is never the removed side as a whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Root {
    /// Absolute folder path.
    pub(super) path: PathBuf,
    /// What it is.
    pub(super) kind: ProtectedFolder,
}

/// The most pairs, or files, one page holds.
pub(super) const MAX_PAGE: u32 = 500;

/// A version as two copies compare it; `None` for no version.
type VersionKey = Option<Vec<Part>>;

/// One font file, with what a copy of it is known by.
#[derive(Debug)]
struct Font<'a> {
    file: &'a SetFile,
    path: PathBuf,
    dir: PathBuf,
    /// The PostScript names it holds, each with its version key.
    names: Vec<(&'a str, VersionKey)>,
}

/// The font files of the library, by path, with the folders that hold the
/// library's sources.
#[derive(Debug)]
pub(super) struct Pairs<'a> {
    fonts: Vec<Font<'a>>,
    roots: &'a [Root],
}

/// What one side's files have on the other side.
#[derive(Debug)]
struct Matched {
    /// Per file of the side (an index into `fonts`), its best copy.
    files: Vec<(usize, Option<(DuplicateLevel, usize)>)>,
}

fn sat(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl<'a> Pairs<'a> {
    /// The font files among `files` (data that is no font is left out), in
    /// path order.
    pub(super) fn new(files: &'a [SetFile], roots: &'a [Root]) -> Self {
        let mut fonts: Vec<Font<'a>> = files
            .iter()
            .filter(|f| f.format.is_some())
            .map(|file| {
                let path = PathBuf::from(&file.path);
                let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
                let mut seen = BTreeSet::new();
                let names = file
                    .faces
                    .iter()
                    .filter(|f| !f.name.is_empty() && seen.insert(f.name.as_str()))
                    .map(|f| (f.name.as_str(), version_key(f.version.as_deref())))
                    .collect();
                Font {
                    file,
                    path,
                    dir,
                    names,
                }
            })
            .collect();
        fonts.sort_by(|a, b| a.path.cmp(&b.path));
        Self { fonts, roots }
    }

    /// The fonts in `dir` and its subfolders: one run of the path order,
    /// because paths sort component by component.
    fn subtree(&self, dir: &Path) -> Range<usize> {
        let start = self.fonts.partition_point(|f| f.path.as_path() < dir);
        let len = self.fonts[start..].partition_point(|f| f.path.starts_with(dir));
        start..start + len
    }

    /// The fonts of side `dir`, without those of `other` when `dir` holds it.
    fn side(&self, dir: &Path, other: &Path) -> Vec<usize> {
        let skip = if other.starts_with(dir) {
            self.subtree(other)
        } else {
            0..0
        };
        self.subtree(dir).filter(|i| !skip.contains(i)).collect()
    }

    /// The best copy among `ys` of each of `xs`: identical before the same
    /// version before another version, the first by path at that level.
    fn matched(&self, xs: &[usize], ys: &[usize]) -> Matched {
        let mut by_hash: HashMap<&str, usize> = HashMap::new();
        let mut by_name: HashMap<&str, Vec<(&VersionKey, usize)>> = HashMap::new();
        for &y in ys {
            let font = &self.fonts[y];
            by_hash.entry(font.file.hash.as_str()).or_insert(y);
            for (name, key) in &font.names {
                by_name.entry(name).or_default().push((key, y));
            }
        }
        let files = xs
            .iter()
            .map(|&x| {
                let font = &self.fonts[x];
                if let Some(&y) = by_hash.get(font.file.hash.as_str()) {
                    return (x, Some((DuplicateLevel::Identical, y)));
                }
                let mut best: Option<(DuplicateLevel, usize)> = None;
                for (name, key) in &font.names {
                    for &(other, y) in by_name.get(name).into_iter().flatten() {
                        let level = if other == key {
                            DuplicateLevel::SameVersion
                        } else {
                            DuplicateLevel::OtherVersion
                        };
                        let better = match best {
                            None => true,
                            Some((l, b)) => rank(level) < rank(l) || (level == l && y < b),
                        };
                        if better {
                            best = Some((level, y));
                        }
                    }
                }
                (x, best)
            })
            .collect();
        Matched { files }
    }

    /// Whether every font of `a` has an identical copy in `b`, and the other
    /// way round; a side without fonts is not contained.
    fn contained(&self, a: &Path, b: &Path) -> (bool, bool) {
        let (xs, ys) = (self.side(a, b), self.side(b, a));
        let all = |m: &Matched| {
            !m.files.is_empty()
                && m.files
                    .iter()
                    .all(|(_, c)| matches!(c, Some((DuplicateLevel::Identical, _))))
        };
        (all(&self.matched(&xs, &ys)), all(&self.matched(&ys, &xs)))
    }

    /// Whether `dir` is a root or inside one: a pair climbs no higher.
    fn in_roots(&self, dir: &Path) -> bool {
        self.roots.iter().any(|r| dir.starts_with(&r.path))
    }

    /// Why `dir` can never be the removed side, if it cannot: a system font
    /// folder or a folder inside one, a root itself, or a folder that holds
    /// a root (outside `other`, when `dir` holds `other`); a side holding a
    /// system copy counts as a system folder.
    fn protected(&self, dir: &Path, other: &Path, fonts: &[usize]) -> Option<ProtectedFolder> {
        let system = self
            .roots
            .iter()
            .any(|r| r.kind == ProtectedFolder::SystemFolder && dir.starts_with(&r.path))
            || fonts.iter().any(|&i| self.fonts[i].file.system);
        if system {
            return Some(ProtectedFolder::SystemFolder);
        }
        let held = |r: &&Root| {
            r.path.starts_with(dir) && !(other.starts_with(dir) && r.path.starts_with(other))
        };
        self.roots
            .iter()
            .find(|r| r.path == dir)
            .or_else(|| self.roots.iter().find(held))
            .map(|r| r.kind)
    }

    /// Whether `dir` is a root: a pair climbs no higher than a root.
    fn is_root(&self, dir: &Path) -> bool {
        self.roots.iter().any(|r| r.path == dir)
    }

    /// The pair `(a, b)` climbed as far as it stays contained.
    fn climb(&self, mut a: PathBuf, mut b: PathBuf) -> (PathBuf, PathBuf, bool) {
        let (mut ca, mut cb) = self.contained(&a, &b);
        if !ca && !cb {
            return (a, b, false);
        }
        while let (Some(pa), Some(pb)) = (a.parent(), b.parent()) {
            if a.file_name() != b.file_name()
                || self.is_root(&a)
                || self.is_root(&b)
                || pa == pb
                || pa.starts_with(pb)
                || pb.starts_with(pa)
                || !self.in_roots(pa)
                || !self.in_roots(pb)
            {
                break;
            }
            let (xa, xb) = self.contained(pa, pb);
            if !((ca && xa) || (cb && xb)) {
                break;
            }
            (ca, cb) = (ca && xa, cb && xb);
            (a, b) = (pa.to_path_buf(), pb.to_path_buf());
        }
        (a, b, true)
    }

    /// Every pair of folders whose files share a copy, before the threshold:
    /// the folders that directly hold two copies of one file or one font.
    fn base_pairs(&self) -> BTreeSet<(PathBuf, PathBuf)> {
        let mut groups: HashMap<&str, BTreeSet<&Path>> = HashMap::new();
        for font in &self.fonts {
            groups
                .entry(font.file.hash.as_str())
                .or_default()
                .insert(&font.dir);
        }
        let mut names: HashMap<&str, BTreeSet<&Path>> = HashMap::new();
        for font in &self.fonts {
            for (name, _) in &font.names {
                names.entry(name).or_default().insert(&font.dir);
            }
        }
        let mut out = BTreeSet::new();
        for dirs in groups.values().chain(names.values()) {
            let dirs: Vec<&&Path> = dirs.iter().collect();
            for (i, a) in dirs.iter().enumerate() {
                for b in &dirs[i + 1..] {
                    out.insert((a.to_path_buf(), b.to_path_buf()));
                }
            }
        }
        out
    }

    /// The pairs to list: base pairs climbed, the ones inside a reported
    /// contained pair left out, each as a [`FolderPair`] (`other_files` 0),
    /// filtered by `scope` and `min_shared`, most freed bytes first.
    pub(super) fn pairs(&self, scope: &DuplicateScope, min_shared: u32) -> Vec<FolderPair> {
        let mut tops: BTreeSet<(PathBuf, PathBuf, bool)> = BTreeSet::new();
        for (a, b) in self.base_pairs() {
            let (a, b, contained) = self.climb(a, b);
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            tops.insert((a, b, contained));
        }
        let held: Vec<&(PathBuf, PathBuf, bool)> = tops.iter().filter(|t| t.2).collect();
        let inside = |a: &Path, b: &Path| {
            held.iter().any(|(ta, tb, _)| {
                (ta.as_path(), tb.as_path()) != (a, b)
                    && ((a.starts_with(ta) && b.starts_with(tb))
                        || (a.starts_with(tb) && b.starts_with(ta)))
            })
        };
        let mut seen = BTreeSet::new();
        let min = min_shared.max(1);
        let mut out: Vec<FolderPair> = tops
            .iter()
            .filter(|(a, b, _)| seen.insert((a.clone(), b.clone())) && !inside(a, b))
            .map(|(a, b, _)| self.pair(a, b))
            .filter(|p| {
                let shared = |s: &FolderPairSide| {
                    s.shared.identical + s.shared.same_version + s.shared.other_version
                };
                shared(&p.a).max(shared(&p.b)) >= min && self.pair_in_scope(p, scope)
            })
            .collect();
        out.sort_by(|x, y| {
            y.frees
                .cmp(&x.frees)
                .then_with(|| x.a.path.cmp(&y.a.path))
                .then_with(|| x.b.path.cmp(&y.b.path))
        });
        out
    }

    fn pair_in_scope(&self, pair: &FolderPair, scope: &DuplicateScope) -> bool {
        match scope {
            DuplicateScope::Library => true,
            DuplicateScope::Source { source } => [&pair.a.path, &pair.b.path].iter().any(|p| {
                let (a, b) = (Path::new(&pair.a.path), Path::new(&pair.b.path));
                let other = if Path::new(p) == a { b } else { a };
                self.side(Path::new(p), other)
                    .iter()
                    .any(|&i| self.fonts[i].file.source == *source)
            }),
            DuplicateScope::Folder { path } => [&pair.a.path, &pair.b.path]
                .iter()
                .any(|p| Path::new(p).starts_with(path)),
        }
    }

    /// The pair of `a` and `b`, `a` first.
    pub(super) fn pair(&self, a: &Path, b: &Path) -> FolderPair {
        let (xs, ys) = (self.side(a, b), self.side(b, a));
        let side_a = self.side_of(a, b, &xs, &self.matched(&xs, &ys));
        let side_b = self.side_of(b, a, &ys, &self.matched(&ys, &xs));
        let smaller = if (side_b.size, side_b.font_files) < (side_a.size, side_a.font_files) {
            &side_b
        } else {
            &side_a
        };
        FolderPair {
            frees: smaller.identical_size,
            a: side_a,
            b: side_b,
        }
    }

    fn side_of(&self, dir: &Path, other: &Path, fonts: &[usize], m: &Matched) -> FolderPairSide {
        let size = |i: usize| self.fonts[i].file.size;
        let mut shared = SharedCounts::default();
        let (mut total, mut shared_size, mut identical_size) = (0_u32, 0_u32, 0_u32);
        for &(x, copy) in &m.files {
            total = total.saturating_add(size(x));
            let Some((level, _)) = copy else { continue };
            shared_size = shared_size.saturating_add(size(x));
            match level {
                DuplicateLevel::Identical => {
                    shared.identical += 1;
                    identical_size = identical_size.saturating_add(size(x));
                }
                DuplicateLevel::SameVersion => shared.same_version += 1,
                DuplicateLevel::OtherVersion => shared.other_version += 1,
            }
        }
        let font_files = sat(fonts.len());
        FolderPairSide {
            path: dir.to_string_lossy().into_owned(),
            font_files,
            size: total,
            only_here: font_files - shared.identical - shared.same_version - shared.other_version,
            contained: font_files > 0 && shared.identical == font_files,
            shared,
            shared_size,
            identical_size,
            other_files: 0,
            protected: self.protected(dir, other, fonts),
        }
    }

    /// The font files of `a` and `b` with their best copies, `a`'s first,
    /// each side by path.
    pub(super) fn files(&self, a: &Path, b: &Path) -> Vec<FolderPairFile> {
        let (xs, ys) = (self.side(a, b), self.side(b, a));
        let row = |side: PairSide| {
            move |&(x, copy): &(usize, Option<(DuplicateLevel, usize)>)| FolderPairFile {
                path: self.fonts[x].file.path.clone(),
                side,
                size: self.fonts[x].file.size,
                level: copy.map(|c| c.0),
                copy: copy.map(|c| self.fonts[c.1].file.path.clone()),
            }
        };
        let mut out: Vec<FolderPairFile> = self
            .matched(&xs, &ys)
            .files
            .iter()
            .map(row(PairSide::A))
            .collect();
        out.extend(self.matched(&ys, &xs).files.iter().map(row(PairSide::B)));
        out
    }
}

/// Identical before the same version before another version.
fn rank(level: DuplicateLevel) -> u8 {
    match level {
        DuplicateLevel::Identical => 0,
        DuplicateLevel::SameVersion => 1,
        DuplicateLevel::OtherVersion => 2,
    }
}

/// The page `req` asks for of the pairs among `files`.
pub(super) fn list(
    files: &[SetFile],
    roots: &[Root],
    req: &ListFolderPairsRequest,
) -> Paged<FolderPair> {
    let all = Pairs::new(files, roots)
        .pairs(&req.scope, req.min_shared.unwrap_or(FOLDER_PAIR_MIN_SHARED));
    Paged {
        total: sat(all.len()),
        items: all
            .into_iter()
            .skip(req.page.offset as usize)
            .take(req.page.limit.min(MAX_PAGE) as usize)
            .collect(),
    }
}

/// The listed pair of the folders `a` and `b` (either way round), at the
/// default threshold over the whole library; `None` when they are no pair.
/// The draft's "Keep this side" (E56.12) checks a pair with it.
pub(super) fn find(files: &[SetFile], roots: &[Root], a: &str, b: &str) -> Option<FolderPair> {
    Pairs::new(files, roots)
        .pairs(&DuplicateScope::Library, FOLDER_PAIR_MIN_SHARED)
        .into_iter()
        .find(|p| (p.a.path == a && p.b.path == b) || (p.a.path == b && p.b.path == a))
}

/// The page `req` asks for of the files of two folders.
pub(super) fn list_files(
    files: &[SetFile],
    roots: &[Root],
    req: &ListFolderPairFilesRequest,
) -> ApiResult<Paged<FolderPairFile>> {
    let (a, b) = (Path::new(&req.a), Path::new(&req.b));
    if a == b {
        return Err(ApiError::invalid("Choose two different folders."));
    }
    let all: Vec<FolderPairFile> = Pairs::new(files, roots)
        .files(a, b)
        .into_iter()
        .filter(|f| req.keeps(f))
        .collect();
    Ok(Paged {
        total: sat(all.len()),
        items: all
            .into_iter()
            .skip(req.page.offset as usize)
            .take(req.page.limit.min(MAX_PAGE) as usize)
            .collect(),
    })
}

#[cfg(test)]
mod tests;
