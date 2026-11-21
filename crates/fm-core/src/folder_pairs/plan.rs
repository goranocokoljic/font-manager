//! A folder pair as the scope of a cleanup (E56.6 for E56.7): "keep B's
//! copies" plans the removal of A's font files that have an identical copy
//! in B, and nothing else. Copies of another level are planned only when
//! the user asks for it, and then the keeper policy and the language guard
//! decide each font ([`crate::keeper_policy::recommend`]), over the copies
//! of the pair. A protected folder ([`ProtectedFolder`]) can be the kept
//! side, never the removed side. Nothing here touches a file: the plan of
//! E56.7 carries the answer to the user and applies it.

use std::collections::HashMap;
use std::path::Path;

use fm_api::library::{
    CopyFacts, DuplicateLevel, KeeperPolicy, PairSide, ProtectedFolder, format_from_extension,
};
use fm_api::{ApiError, ApiResult, FaceId};

use super::{Pairs, Root};
use crate::duplicate_sets::SetFile;
use crate::keeper_policy::{Candidate, LanguageNames, holdings, recommend};

/// What a pair plan is asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairPlanRequest {
    /// One folder, absolute.
    pub a: String,
    /// The other folder, absolute.
    pub b: String,
    /// The side whose copies stay; the other side's copies are planned for
    /// removal.
    pub keep: PairSide,
    /// Also plan copies of the same version and of another version, through
    /// the policy and the guard.
    pub other_levels: bool,
}

/// The keeper policy with what it weighs.
#[derive(Debug)]
pub struct PolicyInput<'a> {
    /// The saved policy.
    pub policy: &'a KeeperPolicy,
    /// My languages.
    pub my_languages: &'a [String],
    /// Their names, for the sentences.
    pub names: &'a LanguageNames,
    /// The facts of the faces, by id; a face without facts weighs as none.
    pub facts: &'a HashMap<FaceId, CopyFacts>,
}

/// One file the plan would remove.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairRemoval {
    /// Absolute file path.
    pub path: String,
    /// Size in bytes.
    pub size: u32,
    /// The level of the copy that stays.
    pub level: DuplicateLevel,
    /// The copy on the kept side that stays for it.
    pub kept_copy: String,
}

/// Why a file of the removed side stays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairStay {
    /// It has no copy on the kept side, or holds a font that has none.
    OnlyHere,
    /// Its copy on the kept side is of another level, and the user did not
    /// ask for those.
    OtherLevel(DuplicateLevel),
    /// The policy or the guard keeps it; the keeper the rules chose.
    PolicyKeeps(Option<String>),
    /// It is in a system font folder (rule 4).
    System,
}

/// A file of the removed side that stays, with why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairKept {
    /// Absolute file path.
    pub path: String,
    /// Why it stays.
    pub why: PairStay,
}

/// What "keep one side's copies" would do to the other side's font files.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PairRemovals {
    /// The files to remove, by path.
    pub remove: Vec<PairRemoval>,
    /// The files that stay, by path.
    pub stays: Vec<PairKept>,
}

fn protected_name(kind: ProtectedFolder) -> &'static str {
    match kind {
        ProtectedFolder::SourceRoot => "a folder you added to the library",
        ProtectedFolder::SystemFolder => "a system font folder",
        ProtectedFolder::LibraryFolder => "the library folder",
    }
}

/// What keeping `req.keep`'s copies would remove from the other side.
/// `InvalidRequest` when the folders are the same, or the side to remove
/// is protected. Data that is no font is never planned.
pub fn removals(
    files: &[SetFile],
    roots: &[Root],
    req: &PairPlanRequest,
    policy: &PolicyInput<'_>,
) -> ApiResult<PairRemovals> {
    let (a, b) = (Path::new(&req.a), Path::new(&req.b));
    if a == b {
        return Err(ApiError::invalid("Choose two different folders."));
    }
    let (removed, kept) = match req.keep {
        PairSide::A => (b, a),
        PairSide::B => (a, b),
    };
    let pairs = Pairs::new(files, roots);
    if let Some(kind) = pairs.protection(removed, kept) {
        return Err(ApiError::invalid(format!(
            "{} is {}: it can be the side that is kept, never the side that is removed.",
            removed.display(),
            protected_name(kind)
        )));
    }
    let held = holdings(files);
    let candidate = |file: &SetFile, name: &str| {
        let face = file.faces.iter().find(|f| f.name == name);
        Candidate {
            path: file.path.clone(),
            version: face.and_then(|f| f.version.clone()),
            format: format_from_extension(&file.path),
            in_library_folder: file.in_library_folder,
            system: file.system,
            facts: face.and_then(|f| policy.facts.get(&f.face).cloned()),
            holds: held.get(file.path.as_str()).cloned().unwrap_or_default(),
        }
    };
    let mut out = PairRemovals::default();
    for (file, copy) in pairs.removable(removed, kept) {
        let stay = |why| PairKept {
            path: file.path.clone(),
            why,
        };
        if file.system {
            out.stays.push(stay(PairStay::System));
            continue;
        }
        let Some((level, other)) = copy else {
            out.stays.push(stay(PairStay::OnlyHere));
            continue;
        };
        if level == DuplicateLevel::Identical {
            out.remove.push(PairRemoval {
                path: file.path.clone(),
                size: file.size,
                level,
                kept_copy: other.path.clone(),
            });
            continue;
        }
        if !req.other_levels {
            out.stays.push(stay(PairStay::OtherLevel(level)));
            continue;
        }
        // Every font of the file must go through the policy against the
        // kept side's copies of it, and lose.
        let names: std::collections::BTreeSet<&str> = file
            .faces
            .iter()
            .map(|f| f.name.as_str())
            .filter(|n| !n.is_empty())
            .collect();
        let mut keeper: Option<String> = None;
        let mut why: Option<PairStay> = None;
        for name in names {
            let theirs = pairs.holding(kept, removed, name);
            if theirs.is_empty() {
                why = Some(PairStay::OnlyHere);
                break;
            }
            let mut copies = vec![candidate(file, name)];
            copies.extend(theirs.iter().map(|f| candidate(f, name)));
            let rec = recommend(&copies, policy.policy, policy.my_languages, policy.names);
            if !rec.remove.iter().any(|r| r.path == file.path) {
                why = Some(PairStay::PolicyKeeps(rec.keeper));
                break;
            }
            keeper = keeper.or(rec.keeper);
        }
        match why {
            Some(why) => out.stays.push(stay(why)),
            None => out.remove.push(PairRemoval {
                path: file.path.clone(),
                size: file.size,
                level,
                kept_copy: keeper.unwrap_or_else(|| other.path.clone()),
            }),
        }
    }
    Ok(out)
}
