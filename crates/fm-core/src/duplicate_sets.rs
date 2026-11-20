//! Duplicate sets (E56.1, ADR 0027): the levels of sameness.
//!
//! - **Identical**: files with the same hash at different paths.
//! - **Same version**: one PostScript name and one version in files that
//!   differ.
//! - **Other version**: one PostScript name in files whose versions differ.
//!
//! One name set holds every file with a PostScript name, as long as at least
//! two of them differ; its level is `sameVersion` when every copy has the
//! same version and `otherVersion` otherwise. Versions are compared as the
//! keeper rule compares them (trimmed, digit runs as numbers, other text
//! without case); a copy without a version is its own value. An identical
//! set whose fonts all sit in name sets that hold all its files is not
//! listed again: the name sets show those copies as `identical_with` each
//! other. So a file is listed
//! once per font it duplicates.
//!
//! The rule is pure: [`crate::index::Index::duplicate_sets`] feeds it the
//! files the library shows. `fm_api::mock` and the TypeScript mock carry
//! copies of it; `fixtures/duplicates/set-cases.json` runs against all three.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use fm_api::common::FontFormat;
use fm_api::common::Paged;
use fm_api::library::{
    DuplicateLevel, DuplicateScope, DuplicateSet, DuplicateSetCopy, ListDuplicateSetsRequest,
};
use fm_api::{FaceId, SourceId};
use fm_platform::paths::{Os, is_within, resolve_links, system_font_folders};

/// One file the library shows, with its faces in collection order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetFile {
    /// Absolute path, as shown.
    pub path: String,
    /// Size in bytes.
    pub size: u32,
    /// Hash, hex.
    pub hash: String,
    /// The file format; `None` for data that is no font.
    pub format: Option<FontFormat>,
    /// The source it was found in.
    pub source: SourceId,
    /// The source is the library folder.
    pub in_library_folder: bool,
    /// The source is the system font folders.
    pub system: bool,
    /// Its faces, in collection order.
    pub faces: Vec<SetFace>,
}

/// One face of a [`SetFile`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetFace {
    /// Id.
    pub face: FaceId,
    /// PostScript name; empty when the font has none.
    pub name: String,
    /// Version string (name ID 5).
    pub version: Option<String>,
}

/// A piece of a version key: a run of digits by value, or other text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Part {
    Number(u64),
    Text(String),
}

/// The value two versions are compared by: trimmed, digit runs as numbers,
/// other text lower-cased; `None` for no version (its own value).
fn version_key(version: Option<&str>) -> Option<Vec<Part>> {
    let v = version.map(str::trim).filter(|v| !v.is_empty())?;
    let mut parts = Vec::new();
    let mut rest = v;
    while let Some(c) = rest.chars().next() {
        let digit = c.is_ascii_digit();
        let end = rest
            .find(|ch: char| ch.is_ascii_digit() != digit)
            .unwrap_or(rest.len());
        let (run, tail) = rest.split_at(end);
        parts.push(if digit {
            Part::Number(run.parse().unwrap_or(u64::MAX))
        } else {
            Part::Text(run.to_lowercase())
        });
        rest = tail;
    }
    Some(parts)
}

/// The folders the OS reads fonts from on this computer, each also through
/// its links: a copy inside one is a system copy even when the user watches
/// that folder as a library folder (rule 4). Copies in a system source are
/// marked by their source as well.
pub fn system_folders() -> Vec<PathBuf> {
    with_links(system_font_folders())
}

/// `folders` and, where it differs, each folder with its links resolved.
pub fn with_links(folders: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::with_capacity(folders.len() * 2);
    for dir in folders {
        let real = resolve_links(&dir);
        if real != dir {
            out.push(real);
        }
        out.push(dir);
    }
    out
}

/// Whether `path` is in one of `folders` ([`system_folders`]), compared as
/// the OS compares paths (without case on Windows and macOS).
pub fn in_system_folder(path: &Path, folders: &[PathBuf]) -> bool {
    folders
        .iter()
        .any(|dir| is_within(Os::current(), path, dir))
}

/// The most sets one page holds.
pub const MAX_PAGE: u32 = 500;

/// The page `req` asks for of the sets among `files`.
pub fn list(files: &[SetFile], req: &ListDuplicateSetsRequest) -> Paged<DuplicateSet> {
    let all = sets(files, &req.scope);
    let total = u32::try_from(all.len()).unwrap_or(u32::MAX);
    Paged {
        items: all
            .into_iter()
            .skip(req.page.offset as usize)
            .take(req.page.limit.min(MAX_PAGE) as usize)
            .collect(),
        total,
    }
}

/// Whether `file` is in `scope`.
pub fn in_scope(file: &SetFile, scope: &DuplicateScope) -> bool {
    match scope {
        DuplicateScope::Library => true,
        DuplicateScope::Source { source } => file.source == *source,
        DuplicateScope::Folder { path } => Path::new(&file.path).starts_with(path),
    }
}

/// A set before its copies are built: the level, the name and, per file,
/// the face it is about.
struct Draft {
    id: String,
    level: DuplicateLevel,
    name: Option<String>,
    members: Vec<(usize, Option<usize>)>,
}

/// Every duplicate set among `files` with a copy in `scope`, sets by
/// PostScript name (case-insensitive, then exact, then id), copies by path.
pub fn sets(files: &[SetFile], scope: &DuplicateScope) -> Vec<DuplicateSet> {
    // Name sets: one per PostScript name, the first face of each file.
    let mut by_name: BTreeMap<&str, Vec<(usize, usize)>> = BTreeMap::new();
    for (fi, file) in files.iter().enumerate() {
        let mut seen = HashSet::new();
        for (k, face) in file.faces.iter().enumerate() {
            if !face.name.is_empty() && seen.insert(face.name.as_str()) {
                by_name.entry(face.name.as_str()).or_default().push((fi, k));
            }
        }
    }
    let mut drafts: Vec<Draft> = Vec::new();
    for (name, members) in by_name {
        let hashes: BTreeSet<&str> = members
            .iter()
            .map(|&(fi, _)| files[fi].hash.as_str())
            .collect();
        if hashes.len() < 2 {
            continue;
        }
        let versions: BTreeSet<Option<Vec<Part>>> = members
            .iter()
            .map(|&(fi, k)| version_key(files[fi].faces[k].version.as_deref()))
            .collect();
        drafts.push(Draft {
            id: format!("name:{name}"),
            level: if versions.len() == 1 {
                DuplicateLevel::SameVersion
            } else {
                DuplicateLevel::OtherVersion
            },
            name: Some(name.to_owned()),
            members: members.into_iter().map(|(fi, k)| (fi, Some(k))).collect(),
        });
    }
    // Identical sets, unless one name set already holds all their files.
    let mut by_hash: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (fi, file) in files.iter().enumerate() {
        by_hash.entry(file.hash.as_str()).or_default().push(fi);
    }
    let named = drafts.len();
    for (hash, members) in by_hash {
        if members.len() < 2 || covered(files, &drafts[..named], &members) {
            continue;
        }
        let first = |fi: usize| files[fi].faces.iter().position(|f| !f.name.is_empty());
        let name = members
            .iter()
            .find_map(|&fi| first(fi).map(|k| files[fi].faces[k].name.clone()));
        drafts.push(Draft {
            id: format!("identical:{hash}"),
            level: DuplicateLevel::Identical,
            name,
            members: members
                .into_iter()
                .map(|fi| (fi, first(fi).or((!files[fi].faces.is_empty()).then_some(0))))
                .collect(),
        });
    }
    // The sets each file takes part in.
    let mut sets_of: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    for d in &drafts {
        for &(fi, _) in &d.members {
            sets_of.entry(fi).or_default().push(d.id.as_str());
        }
    }
    let mut out: Vec<DuplicateSet> = drafts
        .iter()
        .filter(|d| d.members.iter().any(|&(fi, _)| in_scope(&files[fi], scope)))
        .map(|d| build(files, d, &sets_of, scope))
        .collect();
    out.sort_by_cached_key(|s| {
        (
            s.postscript_name.is_none(),
            s.postscript_name.as_deref().map(str::to_lowercase),
            s.postscript_name.clone(),
            s.id.clone(),
        )
    });
    out
}

/// Whether every font in the identical files `members` already has a name
/// set that holds all of them: then the identical set says nothing new. A
/// collection file with a second font no name set covers stays listed.
fn covered(files: &[SetFile], name_sets: &[Draft], members: &[usize]) -> bool {
    let Some(&first) = members.first() else {
        return false;
    };
    let names: BTreeSet<&str> = files[first]
        .faces
        .iter()
        .map(|f| f.name.as_str())
        .filter(|n| !n.is_empty())
        .collect();
    !names.is_empty()
        && names.iter().all(|name| {
            name_sets.iter().any(|d| {
                d.name.as_deref() == Some(*name)
                    && members
                        .iter()
                        .all(|fi| d.members.iter().any(|m| m.0 == *fi))
            })
        })
}

fn build(
    files: &[SetFile],
    draft: &Draft,
    sets_of: &BTreeMap<usize, Vec<&str>>,
    scope: &DuplicateScope,
) -> DuplicateSet {
    let mut members = draft.members.clone();
    members.sort_by(|a, b| files[a.0].path.cmp(&files[b.0].path));
    let copies: Vec<DuplicateSetCopy> = members
        .iter()
        .map(|&(fi, k)| {
            let file = &files[fi];
            let face = k.and_then(|k| file.faces.get(k));
            let mut other_sets: Vec<String> = sets_of
                .get(&fi)
                .into_iter()
                .flatten()
                .filter(|id| **id != draft.id)
                .map(|id| (*id).to_owned())
                .collect();
            other_sets.sort();
            DuplicateSetCopy {
                path: file.path.clone(),
                size: file.size,
                hash: file.hash.clone(),
                face: face.map(|f| f.face),
                version: face.and_then(|f| f.version.clone()),
                format: file.format,
                source: file.source,
                in_library_folder: file.in_library_folder,
                system: file.system,
                in_scope: in_scope(file, scope),
                identical_with: members
                    .iter()
                    .filter(|&&(other, _)| other != fi && files[other].hash == file.hash)
                    .map(|&(other, _)| files[other].path.clone())
                    .collect(),
                other_sets,
                facts: None,
            }
        })
        .collect();
    let formats: HashSet<Option<FontFormat>> = copies.iter().map(|c| c.format).collect();
    DuplicateSet {
        id: draft.id.clone(),
        level: draft.level,
        postscript_name: draft.name.clone(),
        formats_differ: formats.len() > 1,
        copies,
        recommendation: None,
    }
}

#[cfg(test)]
mod tests;
