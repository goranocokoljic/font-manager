//! The duplicate-set rule of the mock (E56.1): a copy of
//! `fm_core::duplicate_sets` (fm-api cannot use fm-core).
//! `fixtures/duplicates/set-cases.json` runs against both, and against the
//! TypeScript mock, so they stay the same rule.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;

use crate::common::FontFormat;
use crate::common::Paged;
use crate::library::{
    DuplicateLevel, DuplicateScope, DuplicateSet, DuplicateSetCopy, ListDuplicateSetsRequest,
};
use crate::{FaceId, SourceId};

/// One file the library shows, with its faces in collection order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SetFile {
    /// Absolute path, as shown.
    pub(super) path: String,
    /// Size in bytes.
    pub(super) size: u32,
    /// Hash, hex.
    pub(super) hash: String,
    /// The file format; `None` for data that is no font.
    pub(super) format: Option<FontFormat>,
    /// The source it was found in.
    pub(super) source: SourceId,
    /// The source is the library folder.
    pub(super) in_library_folder: bool,
    /// The source is the system font folders.
    pub(super) system: bool,
    /// Its faces, in collection order.
    pub(super) faces: Vec<SetFace>,
}

/// One face of a [`SetFile`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SetFace {
    /// Id.
    pub(super) face: FaceId,
    /// PostScript name; empty when the font has none.
    pub(super) name: String,
    /// Version string (name ID 5).
    pub(super) version: Option<String>,
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

/// The most sets one page holds.
const MAX_PAGE: u32 = 500;

/// The page `req` asks for of the sets among `files`.
pub(super) fn list(files: &[SetFile], req: &ListDuplicateSetsRequest) -> Paged<DuplicateSet> {
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
fn in_scope(file: &SetFile, scope: &DuplicateScope) -> bool {
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
pub(super) fn sets(files: &[SetFile], scope: &DuplicateScope) -> Vec<DuplicateSet> {
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
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    #[derive(serde::Deserialize)]
    struct Case {
        name: String,
        #[serde(default)]
        scope: DuplicateScope,
        files: Vec<CaseFile>,
        sets: Value,
    }

    #[derive(serde::Deserialize)]
    struct CaseFile {
        path: String,
        hash: String,
        format: Option<FontFormat>,
        source: u32,
        kind: String,
        faces: Vec<CaseFace>,
    }

    #[derive(serde::Deserialize)]
    struct CaseFace {
        face: u32,
        name: String,
        version: Option<String>,
    }

    /// The same table `fm_core::duplicate_sets` and the TypeScript mock run.
    #[test]
    fn the_set_rule_gives_the_core_answer_for_every_shared_case() {
        let cases: Vec<Case> = serde_json::from_str(include_str!(
            "../../../../fixtures/duplicates/set-cases.json"
        ))
        .expect("set cases");
        assert!(cases.len() >= 10, "the table is there");
        for case in cases {
            let files: Vec<SetFile> = case
                .files
                .iter()
                .map(|f| SetFile {
                    path: f.path.clone(),
                    size: 1000,
                    hash: f.hash.clone(),
                    format: f.format,
                    source: SourceId(f.source),
                    in_library_folder: f.kind == "libraryFolder",
                    system: f.kind == "system",
                    faces: f
                        .faces
                        .iter()
                        .map(|x| SetFace {
                            face: FaceId(x.face),
                            name: x.name.clone(),
                            version: x.version.clone(),
                        })
                        .collect(),
                })
                .collect();
            let got: Value = sets(&files, &case.scope)
                .iter()
                .map(|s| {
                    json!({
                        "id": s.id,
                        "level": s.level,
                        "postscriptName": s.postscript_name,
                        "formatsDiffer": s.formats_differ,
                        "copies": s.copies.iter().map(|c| json!({
                            "path": c.path,
                            "face": c.face,
                            "version": c.version,
                            "system": c.system,
                            "inScope": c.in_scope,
                            "identicalWith": c.identical_with,
                            "otherSets": c.other_sets,
                        })).collect::<Vec<_>>(),
                    })
                })
                .collect();
            assert_eq!(got, case.sets, "{}", case.name);
        }
    }
}
