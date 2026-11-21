//! The mock's cleanup plan (E56.7). The mock has no index, no disk and no
//! facts of the copies, so it plans as fm-core does with none: each set's
//! keeper is the user's pick or [`DuplicateGroup::choose_keeper`]'s, a
//! system copy and an active copy stay (an active identical copy can move
//! its activation), a file another set keeps stays, and a folder pair plans
//! the identical copies of the removed side only. fm-core adds the language
//! guard, the policy and the disk.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use crate::activation::ActivationMode;
use crate::cleanup::{
    ActivationMove, CleanupAction, CleanupFile, CleanupScope, PlanCleanupRequest, RemoveAs,
    StayReason,
};
use crate::library::{DuplicateCopy, DuplicateGroup, DuplicateKind, DuplicateLevel, DuplicateSet};
use crate::{ApiError, ApiResult};

/// The keeper of `set` by [`DuplicateGroup::choose_keeper`], among the
/// copies outside the system folders; `None` when every copy is a system copy.
pub(super) fn keeper(set: &DuplicateSet) -> Option<String> {
    let mut group = DuplicateGroup {
        kind: DuplicateKind::Exact,
        postscript_name: set.postscript_name.clone(),
        copies: set
            .copies
            .iter()
            .filter(|c| !c.system)
            .map(|c| DuplicateCopy {
                path: c.path.clone(),
                size: c.size,
                hash: c.hash.clone(),
                face: c.face,
                version: c.version.clone(),
                source: c.source,
                in_library_folder: c.in_library_folder,
                keep: false,
            })
            .collect(),
    };
    group.choose_keeper();
    group.copies.first().map(|c| c.path.clone())
}

fn name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map_or_else(|| path.to_owned(), |n| n.to_string_lossy().into_owned())
}

fn sentence(reason: StayReason) -> &'static str {
    match reason {
        StayReason::Keeper => "Stays: it is the copy that is kept.",
        StayReason::Active => "Stays: this copy is active. Deactivate it first, then move it.",
        StayReason::System => "Stays: in a system font folder. Font Fu never moves system fonts.",
        StayReason::KeptByOtherSet => {
            "Stays: this file also holds another font, and that font's copy here is kept."
        }
        StayReason::OutsideScope => "Stays: it is outside the folder or source you clean up.",
        StayReason::OnlyHere => "Stays: it has no copy in the folder that is kept.",
        _ => "Stays.",
    }
}

/// The files of a plan of `req` over the library's sets `all`; `in_scope`
/// says whether a copy can leave, `active` which files are active.
pub(super) fn plan(
    all: &[DuplicateSet],
    req: &PlanCleanupRequest,
    active: &HashMap<String, ActivationMode>,
    in_scope: &dyn Fn(&str) -> bool,
) -> ApiResult<(u32, Vec<CleanupFile>)> {
    for k in &req.keepers {
        let set = all.iter().find(|s| s.id == k.set).ok_or_else(|| {
            ApiError::invalid(format!("{} is not a duplicate set any more.", k.set))
        })?;
        if !set.copies.iter().any(|c| c.path == k.keep) {
            return Err(ApiError::invalid(format!(
                "{} is not a copy of {}.",
                k.keep, k.set
            )));
        }
    }
    let keeper_of = |set: &DuplicateSet| {
        req.keepers
            .iter()
            .find(|k| k.set == set.id)
            .map(|k| k.keep.clone())
            .or_else(|| keeper(set))
    };
    let leave = match req.remove_as {
        RemoveAs::Trash => (CleanupAction::Trash, "Goes to the trash"),
        RemoveAs::Hide => (CleanupAction::Hide, "Is hidden"),
    };
    let row = |set: &DuplicateSet, c: &crate::library::DuplicateSetCopy, kept: Option<String>| {
        CleanupFile {
            path: c.path.clone(),
            size: c.size,
            hash: c.hash.clone(),
            faces: c.face.into_iter().collect(),
            set: set.id.clone(),
            level: set.level,
            action: CleanupAction::Stays,
            reason: None,
            kept,
            also_kept: Vec::new(),
            sentence: String::new(),
            activation: None,
        }
    };
    if let CleanupScope::FolderPair { keep, remove, .. } = &req.scope {
        if keep == remove {
            return Err(ApiError::invalid("Choose two different folders."));
        }
        let under = |p: &str, dir: &str| Path::new(p).starts_with(dir);
        let mut files = Vec::new();
        let mut seen = HashSet::new();
        let mut sets = BTreeSet::new();
        for set in all {
            for c in set.copies.iter().filter(|c| under(&c.path, remove)) {
                let twin = set
                    .copies
                    .iter()
                    .find(|o| o.hash == c.hash && under(&o.path, keep));
                if !seen.insert(c.path.clone()) {
                    continue;
                }
                let mut r = row(set, c, twin.map(|t| t.path.clone()));
                match twin {
                    Some(t) if !c.system && !active.contains_key(&c.path) => {
                        r.action = leave.0;
                        r.sentence = format!("{}; {} stays in {keep}.", leave.1, name(&t.path));
                        sets.insert(set.id.clone());
                    }
                    Some(_) => {
                        let why = if c.system {
                            StayReason::System
                        } else {
                            StayReason::Active
                        };
                        r.reason = Some(why);
                        r.sentence = sentence(why).to_owned();
                    }
                    None => {
                        r.reason = Some(StayReason::OnlyHere);
                        r.sentence = sentence(StayReason::OnlyHere).to_owned();
                    }
                }
                files.push(r);
            }
        }
        files.sort_by(|a, b| a.set.cmp(&b.set).then_with(|| a.path.cmp(&b.path)));
        return Ok((u32::try_from(sets.len()).unwrap_or(u32::MAX), files));
    }
    let selected: Vec<&DuplicateSet> = match &req.scope {
        CleanupScope::Sets { ids } => {
            let mut out: Vec<&DuplicateSet> = Vec::new();
            for id in ids {
                let s = all.iter().find(|s| s.id == *id).ok_or_else(|| {
                    ApiError::invalid(format!("{id} is not a duplicate set any more."))
                })?;
                if !out.iter().any(|o| o.id == s.id) {
                    out.push(s);
                }
            }
            out
        }
        _ => all
            .iter()
            .filter(|s| s.copies.iter().any(|c| in_scope(&c.path)))
            .collect(),
    };
    let mut files = Vec::new();
    let mut seen = HashSet::new();
    for set in &selected {
        let kept = keeper_of(set);
        for c in &set.copies {
            if !seen.insert(c.path.clone()) {
                continue;
            }
            let mut r = row(set, c, kept.clone());
            let kept_elsewhere = c.other_sets.iter().any(|o| {
                all.iter()
                    .find(|s| s.id == *o)
                    .is_some_and(|s| keeper_of(s).as_deref() == Some(c.path.as_str()))
            });
            let why = if kept.as_deref() == Some(c.path.as_str()) {
                Some(StayReason::Keeper)
            } else if c.system || kept.is_none() {
                Some(StayReason::System)
            } else if !in_scope(&c.path) {
                Some(StayReason::OutsideScope)
            } else if kept_elsewhere {
                Some(StayReason::KeptByOtherSet)
            } else if let Some(&mode) = active.get(&c.path) {
                if set.level == DuplicateLevel::Identical {
                    r.activation = Some(ActivationMove {
                        to: kept.clone().unwrap_or_default(),
                        mode,
                        planned: req.move_activations,
                    });
                }
                (!(req.move_activations && set.level == DuplicateLevel::Identical))
                    .then_some(StayReason::Active)
            } else {
                None
            };
            match why {
                Some(why) => {
                    r.reason = Some(why);
                    r.sentence = sentence(why).to_owned();
                }
                None => {
                    r.action = leave.0;
                    r.sentence = format!(
                        "{}; {} stays.",
                        leave.1,
                        kept.as_deref().map(name).unwrap_or_default()
                    );
                }
            }
            files.push(r);
        }
    }
    Ok((u32::try_from(selected.len()).unwrap_or(u32::MAX), files))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::FontFormat;
    use crate::library::DuplicateScope;
    use crate::mock::duplicate_sets::{SetFace, SetFile, sets};
    use crate::{FaceId, SourceId};

    fn file(path: &str, hash: &str, face: u32, name: &str, version: &str, system: bool) -> SetFile {
        SetFile {
            path: path.to_owned(),
            size: 10,
            hash: hash.to_owned(),
            format: Some(FontFormat::Ttf),
            source: SourceId(1),
            in_library_folder: false,
            system,
            faces: vec![SetFace {
                face: FaceId(face),
                name: name.to_owned(),
                version: Some(version.to_owned()),
            }],
        }
    }

    #[test]
    fn the_mock_plans_the_keeper_rule_with_active_and_system_copies_staying() {
        let files = vec![
            file("/a/A.ttf", "a", 1, "A", "1.0", false),
            file("/z/A.ttf", "a", 2, "A", "1.0", false),
            file("/a/B.ttf", "b", 3, "B", "1.0", false),
            file("/z/B.ttf", "b", 4, "B", "1.0", false),
            file("/sys/C.ttf", "c", 5, "C", "1.0", true),
            file("/z/C.ttf", "c", 6, "C", "1.0", false),
        ];
        let all = sets(&files, &DuplicateScope::Library);
        let active: HashMap<String, ActivationMode> =
            [("/z/B.ttf".to_owned(), ActivationMode::Session)].into();
        let (n, rows) =
            plan(&all, &PlanCleanupRequest::default(), &active, &|_| true).expect("plan");
        assert_eq!(n, 3);
        let of = |p: &str| {
            let r = rows.iter().find(|r| r.path == p).expect("row");
            (r.action, r.reason)
        };
        assert_eq!(
            of("/a/A.ttf"),
            (CleanupAction::Stays, Some(StayReason::Keeper))
        );
        assert_eq!(of("/z/A.ttf"), (CleanupAction::Trash, None));
        assert_eq!(
            of("/z/B.ttf"),
            (CleanupAction::Stays, Some(StayReason::Active))
        );
        assert!(
            rows.iter()
                .find(|r| r.path == "/z/B.ttf")
                .and_then(|r| r.activation.as_ref())
                .is_some()
        );
        assert_eq!(
            of("/sys/C.ttf"),
            (CleanupAction::Stays, Some(StayReason::System))
        );
        assert_eq!(
            of("/z/C.ttf"),
            (CleanupAction::Stays, Some(StayReason::Keeper))
        );
    }
}
