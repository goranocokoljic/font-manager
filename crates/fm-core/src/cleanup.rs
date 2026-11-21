//! The plan of a cleanup (E56.7, ADR 0027): per file of the duplicate sets
//! of a scope, whether it would go to the trash, be hidden, or stay, and why.
//!
//! - **The choice.** Each set follows the saved keeper policy and the
//!   language guard ([`crate::keeper_policy::recommend`]), unless the user
//!   picked its keeper; then the guard still keeps the only copy of one of
//!   my languages, and a file that holds a font no file outside the removed
//!   copies holds is kept too.
//! - **What it never does** (each a [`StayReason`]): a copy the app
//!   activated (rule 3), the only copy of one of my languages, a set
//!   answered "keep both", a system copy (rule 4), a file another set keeps
//!   (a collection file), a file that is not there any more.
//! - **Activation.** An active copy of an identical set can hand its
//!   activation to the kept copy ([`ActivationMove`]); at every other level
//!   an active copy stays.
//! - **Pure.** It reads the files the library shows, their facts and what
//!   is active; `exists` is the one look at the disk. It touches nothing.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use fm_api::activation::ActivationMode;
use fm_api::cleanup::{
    ActivationMove, CleanupAction, CleanupFile, CleanupScope, PlanCleanupRequest, RemoveAs,
    StayReason,
};
use fm_api::library::{
    CopyFacts, DuplicateLevel, DuplicateScope, DuplicateSet, KeeperOutcome, KeeperPolicy,
    KeeperRecommendation, KeptBecause, PairSide,
};
use fm_api::settings::MyLanguagesStatus;
use fm_api::{ApiError, ApiResult, FaceId};

use crate::duplicate_sets::{SetFile, in_scope, sets};
use crate::folder_pairs::Root;
use crate::folder_pairs::plan::{PairPlanRequest, PairStay, PolicyInput, removals};
use crate::keeper_policy::{Candidate, HeldFont, LanguageNames, holdings, recommend};

/// What a plan reads.
pub struct PlanContext<'a> {
    /// The files the library shows.
    pub files: &'a [SetFile],
    /// The sources' folders and the system font folders (folder pairs).
    pub roots: &'a [Root],
    /// The facts of the faces, by id.
    pub facts: &'a HashMap<FaceId, CopyFacts>,
    /// The files the app activated, by path, with the mode.
    pub active: &'a HashMap<String, ActivationMode>,
    /// The saved keeper policy.
    pub policy: &'a KeeperPolicy,
    /// My languages.
    pub my_languages: &'a [String],
    /// Their names, for the sentences.
    pub names: &'a LanguageNames,
    /// Whether a file is still on disk.
    pub exists: &'a dyn Fn(&str) -> bool,
}

impl std::fmt::Debug for PlanContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlanContext").finish_non_exhaustive()
    }
}

/// The files of a plan, before the metadata conflicts are read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFiles {
    /// The sets planned.
    pub sets: u32,
    /// Every file of them, by set, then path.
    pub files: Vec<CleanupFile>,
    /// Whether my languages were set.
    pub my_languages: MyLanguagesStatus,
}

/// What one set keeps and removes.
#[derive(Debug, Clone)]
struct Decision {
    keeper: Option<String>,
    keep_both: bool,
    /// Kept copies with why.
    keep: HashMap<String, (StayReason, String)>,
    /// Copies to remove with the sentence of why.
    remove: HashMap<String, String>,
}

impl Decision {
    fn from_recommendation(rec: &KeeperRecommendation) -> Self {
        let keep_both = rec.outcome == KeeperOutcome::KeepBoth;
        let keep = rec
            .keep
            .iter()
            .map(|k| {
                let reason = match k.because {
                    KeptBecause::Policy | KeptBecause::Languages if keep_both => {
                        StayReason::KeepBoth
                    }
                    KeptBecause::Policy => StayReason::Keeper,
                    KeptBecause::Languages => StayReason::MyLanguages,
                    KeptBecause::System => StayReason::System,
                    KeptBecause::LastCopy => StayReason::LastCopy,
                };
                (k.path.clone(), (reason, k.sentence.clone()))
            })
            .collect();
        let remove = rec
            .remove
            .iter()
            .map(|r| (r.path.clone(), r.reason.sentence.clone()))
            .collect();
        Self {
            keeper: rec.keeper.clone(),
            keep_both,
            keep,
            remove,
        }
    }
}

fn languages_of(c: &Candidate, mine: &[String]) -> BTreeSet<String> {
    c.facts
        .as_ref()
        .map(|f| {
            f.languages_full
                .iter()
                .filter(|l| mine.contains(l))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// The decision when the user picked `keep`: the guard and the last-copy
/// rule still apply.
fn picked(
    copies: &[Candidate],
    keep: &str,
    my_languages: &[String],
    names: &LanguageNames,
) -> Decision {
    let mut out = Decision {
        keeper: Some(keep.to_owned()),
        keep_both: false,
        keep: HashMap::new(),
        remove: HashMap::new(),
    };
    out.keep.insert(
        keep.to_owned(),
        (StayReason::Keeper, "Kept: you chose this copy.".to_owned()),
    );
    let mut covered: BTreeSet<String> = copies
        .iter()
        .filter(|c| c.path == keep)
        .flat_map(|c| languages_of(c, my_languages))
        .collect();
    let mut others: Vec<&Candidate> = copies.iter().filter(|c| c.path != keep).collect();
    others.sort_by(|a, b| a.path.cmp(&b.path));
    for c in &others {
        if c.system {
            out.keep.insert(
                c.path.clone(),
                (
                    StayReason::System,
                    "Kept: in a system font folder; Font Fu never removes it.".to_owned(),
                ),
            );
            continue;
        }
        let only: Vec<String> = languages_of(c, my_languages)
            .into_iter()
            .filter(|l| !covered.contains(l))
            .collect();
        if only.is_empty() {
            out.remove.insert(
                c.path.clone(),
                format!("You chose to keep {}.", file_name(keep)),
            );
        } else {
            let list = names.list(&only);
            covered.extend(only);
            out.keep.insert(
                c.path.clone(),
                (
                    StayReason::MyLanguages,
                    format!("Kept: the only copy that covers {list} of your languages."),
                ),
            );
        }
    }
    // The last copy of a font: while the copies to remove hold a font no
    // other file holds, the first of them by path stays.
    loop {
        let removed: HashSet<&str> = out.remove.keys().map(String::as_str).collect();
        let last = others.iter().find(|c| {
            removed.contains(c.path.as_str())
                && c.holds
                    .iter()
                    .any(|h: &HeldFont| h.holders.iter().all(|p| removed.contains(p.as_str())))
        });
        let Some(c) = last else { break };
        out.remove.remove(&c.path);
        out.keep.insert(
            c.path.clone(),
            (
                StayReason::LastCopy,
                "Kept: this file holds a font no other file holds.".to_owned(),
            ),
        );
    }
    out
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map_or_else(|| path.to_owned(), |n| n.to_string_lossy().into_owned())
}

/// The candidates of a set, with the facts and holdings of its copies.
fn candidates(
    set: &DuplicateSet,
    held: &HashMap<&str, Vec<HeldFont>>,
    facts: &HashMap<FaceId, CopyFacts>,
) -> Vec<Candidate> {
    set.copies
        .iter()
        .map(|c| Candidate {
            path: c.path.clone(),
            version: c.version.clone(),
            format: fm_api::library::format_from_extension(&c.path),
            in_library_folder: c.in_library_folder,
            system: c.system,
            facts: c.face.and_then(|f| facts.get(&f).cloned()),
            holds: held.get(c.path.as_str()).cloned().unwrap_or_default(),
        })
        .collect()
}

fn stay_sentence(reason: StayReason, identical: bool) -> String {
    match reason {
        StayReason::Active if identical => "Stays: this copy is active. Deactivate it first, or move its activation to the copy that is kept.".to_owned(),
        StayReason::Active => "Stays: this copy is active. Deactivate it first, then move it.".to_owned(),
        StayReason::KeptByOtherSet => "Stays: this file also holds another font, and that font's copy here is kept.".to_owned(),
        StayReason::NotThere => "Stays: the file is not there any more.".to_owned(),
        StayReason::OutsideScope => "Stays: it is outside the folder or source you clean up.".to_owned(),
        StayReason::System => "Stays: in a system font folder. Font Fu never moves system fonts.".to_owned(),
        StayReason::KeepBoth => "Stays: no copy covers every one of your languages the others cover, so both are kept.".to_owned(),
        StayReason::Keeper | StayReason::MyLanguages | StayReason::LastCopy => "Stays.".to_owned(),
        StayReason::OnlyHere => "Stays: it has no copy in the folder that is kept.".to_owned(),
        StayReason::OtherLevel => "Stays: its copy in the folder that is kept is another version or file; include other versions to plan it.".to_owned(),
    }
}

/// Plans `req` over `ctx`. `InvalidRequest` for a set id that is not a set,
/// a keeper that is not a copy of its set, or a folder pair whose removed
/// side is protected.
pub fn plan(ctx: &PlanContext<'_>, req: &PlanCleanupRequest) -> ApiResult<PlannedFiles> {
    let status = crate::my_languages::apply(ctx.my_languages, <[String]>::to_vec).status;
    if let CleanupScope::FolderPair {
        keep,
        remove,
        other_levels,
    } = &req.scope
    {
        return plan_pair(ctx, req, keep, remove, *other_levels).map(|(sets, files)| {
            PlannedFiles {
                sets,
                files,
                my_languages: status,
            }
        });
    }
    let mut all = sets(ctx.files, &DuplicateScope::Library);
    let by_path: HashMap<&str, &SetFile> = ctx.files.iter().map(|f| (f.path.as_str(), f)).collect();
    let scope_of = |path: &str| -> bool {
        let Some(file) = by_path.get(path) else {
            return false;
        };
        match &req.scope {
            CleanupScope::Source { source } => {
                in_scope(file, &DuplicateScope::Source { source: *source })
            }
            CleanupScope::Folder { path } => {
                in_scope(file, &DuplicateScope::Folder { path: path.clone() })
            }
            _ => true,
        }
    };
    let selected: Vec<usize> = match &req.scope {
        CleanupScope::Sets { ids } => {
            let mut out = Vec::new();
            for id in ids {
                let i = all.iter().position(|s| s.id == *id).ok_or_else(|| {
                    ApiError::invalid(format!("{id} is not a duplicate set any more."))
                })?;
                if !out.contains(&i) {
                    out.push(i);
                }
            }
            out
        }
        _ => (0..all.len())
            .filter(|&i| all[i].copies.iter().any(|c| scope_of(&c.path)))
            .collect(),
    };
    for k in &req.keepers {
        let Some(set) = all.iter().find(|s| s.id == k.set) else {
            return Err(ApiError::invalid(format!(
                "{} is not a duplicate set any more.",
                k.set
            )));
        };
        if !set.copies.iter().any(|c| c.path == k.keep) {
            return Err(ApiError::invalid(format!(
                "{} is not a copy of {}.",
                k.keep, k.set
            )));
        }
    }
    // Every set a planned file takes part in is decided, so a collection
    // file never leaves for one set while another keeps it.
    let ids: HashMap<String, usize> = all
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.clone(), i))
        .collect();
    let mut needed: BTreeSet<usize> = selected.iter().copied().collect();
    for &i in &selected {
        for c in &all[i].copies {
            needed.extend(c.other_sets.iter().filter_map(|o| ids.get(o).copied()));
        }
    }
    let held = holdings(ctx.files);
    let mut decisions: HashMap<usize, Decision> = HashMap::new();
    for &i in &needed {
        let set = &mut all[i];
        for c in &mut set.copies {
            c.facts = c.face.and_then(|f| ctx.facts.get(&f).cloned());
        }
        let cands = candidates(set, &held, ctx.facts);
        let d = match req.keepers.iter().find(|k| k.set == set.id) {
            Some(k) => picked(&cands, &k.keep, ctx.my_languages, ctx.names),
            None => Decision::from_recommendation(&recommend(
                &cands,
                ctx.policy,
                ctx.my_languages,
                ctx.names,
            )),
        };
        decisions.insert(i, d);
    }

    let mut files: Vec<CleanupFile> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for &i in &selected {
        let set = &all[i];
        let d = &decisions[&i];
        let identical = set.level == DuplicateLevel::Identical;
        for copy in &set.copies {
            if !seen.insert(copy.path.clone()) {
                continue;
            }
            let faces = by_path
                .get(copy.path.as_str())
                .map(|f| f.faces.iter().map(|x| x.face).collect())
                .unwrap_or_default();
            // The keepers of the other sets this file takes part in.
            let others: Vec<(&DuplicateSet, &Decision)> = copy
                .other_sets
                .iter()
                .filter_map(|o| ids.get(o))
                .filter_map(|&j| decisions.get(&j).map(|d| (&all[j], d)))
                .collect();
            let also_kept: Vec<String> = others
                .iter()
                .filter_map(|(_, d)| d.keeper.clone())
                .filter(|k| *k != copy.path && d.keeper.as_ref() != Some(k))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let mut row = CleanupFile {
                path: copy.path.clone(),
                size: copy.size,
                hash: copy.hash.clone(),
                faces,
                set: set.id.clone(),
                level: set.level,
                action: CleanupAction::Stays,
                reason: None,
                kept: d.keeper.clone(),
                also_kept,
                sentence: String::new(),
                activation: None,
            };
            let stay = |row: &mut CleanupFile, reason: StayReason, sentence: Option<String>| {
                row.action = CleanupAction::Stays;
                row.reason = Some(reason);
                row.sentence = sentence.unwrap_or_else(|| stay_sentence(reason, identical));
            };
            if let Some((reason, sentence)) = d.keep.get(&copy.path) {
                let sentence = if *reason == StayReason::KeepBoth {
                    stay_sentence(StayReason::KeepBoth, identical)
                } else {
                    sentence.clone()
                };
                stay(&mut row, *reason, Some(sentence));
            } else if d.keep_both {
                stay(&mut row, StayReason::KeepBoth, None);
            } else if copy.system {
                stay(&mut row, StayReason::System, None);
            } else if !scope_of(&copy.path) {
                stay(&mut row, StayReason::OutsideScope, None);
            } else if others
                .iter()
                .any(|(_, o)| o.keep.contains_key(&copy.path) || o.keep_both)
            {
                stay(&mut row, StayReason::KeptByOtherSet, None);
            } else if !(ctx.exists)(&copy.path) {
                stay(&mut row, StayReason::NotThere, None);
            } else {
                let why = d.remove.get(&copy.path).cloned().unwrap_or_default();
                let kept = d.keeper.as_deref().map(file_name).unwrap_or_default();
                let leave = |row: &mut CleanupFile| {
                    row.action = match req.remove_as {
                        RemoveAs::Trash => CleanupAction::Trash,
                        RemoveAs::Hide => CleanupAction::Hide,
                    };
                    let verb = match req.remove_as {
                        RemoveAs::Trash => "Goes to the trash",
                        RemoveAs::Hide => "Is hidden",
                    };
                    row.sentence = format!("{verb}; {kept} stays. {why}").trim_end().to_owned();
                };
                match ctx.active.get(&copy.path) {
                    Some(&mode) if identical && d.keeper.is_some() => {
                        let planned = req.move_activations;
                        row.activation = Some(ActivationMove {
                            to: d.keeper.clone().unwrap_or_default(),
                            mode,
                            planned,
                        });
                        if planned {
                            leave(&mut row);
                            row.sentence =
                                format!("{} Its activation moves to {kept}.", row.sentence);
                        } else {
                            stay(&mut row, StayReason::Active, None);
                        }
                    }
                    Some(_) => stay(&mut row, StayReason::Active, None),
                    None => leave(&mut row),
                }
            }
            files.push(row);
        }
    }
    Ok(PlannedFiles {
        sets: u32::try_from(selected.len()).unwrap_or(u32::MAX),
        files,
        my_languages: status,
    })
}

/// A folder-pair plan: "keep `keep`'s copies" (E56.6), with the checks of
/// every plan (active, not there).
fn plan_pair(
    ctx: &PlanContext<'_>,
    req: &PlanCleanupRequest,
    keep: &str,
    remove: &str,
    other_levels: bool,
) -> ApiResult<(u32, Vec<CleanupFile>)> {
    let pair = PairPlanRequest {
        a: remove.to_owned(),
        b: keep.to_owned(),
        keep: PairSide::B,
        other_levels,
    };
    let policy = PolicyInput {
        policy: ctx.policy,
        my_languages: ctx.my_languages,
        names: ctx.names,
        facts: ctx.facts,
    };
    let answer = removals(ctx.files, ctx.roots, &pair, &policy)?;
    let all = sets(ctx.files, &DuplicateScope::Library);
    let by_path: HashMap<&str, &SetFile> = ctx.files.iter().map(|f| (f.path.as_str(), f)).collect();
    // The set that holds both the file and the copy kept for it.
    let set_of = |path: &str, kept: Option<&str>| -> (String, DuplicateLevel) {
        all.iter()
            .find(|s| {
                s.copies.iter().any(|c| c.path == path)
                    && kept.is_none_or(|k| s.copies.iter().any(|c| c.path == k))
            })
            .map_or_else(
                || {
                    let hash = by_path
                        .get(path)
                        .map(|f| f.hash.clone())
                        .unwrap_or_default();
                    (format!("identical:{hash}"), DuplicateLevel::Identical)
                },
                |s| (s.id.clone(), s.level),
            )
    };
    let base = |path: &str, set: String, level: DuplicateLevel, kept: Option<String>| {
        let file = by_path.get(path);
        CleanupFile {
            path: path.to_owned(),
            size: file.map_or(0, |f| f.size),
            hash: file.map(|f| f.hash.clone()).unwrap_or_default(),
            faces: file
                .map(|f| f.faces.iter().map(|x| x.face).collect())
                .unwrap_or_default(),
            set,
            level,
            action: CleanupAction::Stays,
            reason: None,
            kept,
            also_kept: Vec::new(),
            sentence: String::new(),
            activation: None,
        }
    };
    let mut files: Vec<CleanupFile> = Vec::new();
    let mut keepers: BTreeSet<String> = BTreeSet::new();
    let mut sets_seen: BTreeSet<String> = BTreeSet::new();
    for r in &answer.remove {
        let (set, _) = set_of(&r.path, Some(&r.kept_copy));
        sets_seen.insert(set.clone());
        let identical = r.level == DuplicateLevel::Identical;
        let mut row = base(&r.path, set, r.level, Some(r.kept_copy.clone()));
        let kept = file_name(&r.kept_copy);
        let (action, verb) = match req.remove_as {
            RemoveAs::Trash => (CleanupAction::Trash, "Goes to the trash"),
            RemoveAs::Hide => (CleanupAction::Hide, "Is hidden"),
        };
        let leave_sentence = format!("{verb}; {kept} stays in {keep}.");
        if !(ctx.exists)(&r.path) {
            row.reason = Some(StayReason::NotThere);
            row.sentence = stay_sentence(StayReason::NotThere, identical);
        } else if let Some(&mode) = ctx.active.get(&r.path) {
            if identical {
                row.activation = Some(ActivationMove {
                    to: r.kept_copy.clone(),
                    mode,
                    planned: req.move_activations,
                });
            }
            if identical && req.move_activations {
                row.action = action;
                row.sentence = format!("{leave_sentence} Its activation moves to {kept}.");
            } else {
                row.reason = Some(StayReason::Active);
                row.sentence = stay_sentence(StayReason::Active, identical);
            }
        } else {
            row.action = action;
            row.sentence = leave_sentence;
        }
        keepers.insert(r.kept_copy.clone());
        files.push(row);
    }
    for s in &answer.stays {
        let (reason, kept) = match &s.why {
            PairStay::OnlyHere => (StayReason::OnlyHere, None),
            PairStay::OtherLevel(_) => (StayReason::OtherLevel, None),
            PairStay::PolicyKeeps(k) => (StayReason::Keeper, k.clone()),
            PairStay::System => (StayReason::System, None),
        };
        let (set, level) = set_of(&s.path, kept.as_deref());
        let mut row = base(&s.path, set, level, kept);
        row.reason = Some(reason);
        row.sentence = if reason == StayReason::Keeper {
            "Stays: the keeper policy keeps this copy.".to_owned()
        } else {
            stay_sentence(reason, level == DuplicateLevel::Identical)
        };
        files.push(row);
    }
    for k in keepers {
        let (set, level) = set_of(&k, None);
        let mut row = base(&k, set, level, Some(k.clone()));
        row.reason = Some(StayReason::Keeper);
        row.sentence = format!("Stays: the copies of {keep} are kept.");
        files.push(row);
    }
    files.sort_by(|a, b| a.set.cmp(&b.set).then_with(|| a.path.cmp(&b.path)));
    Ok((u32::try_from(sets_seen.len()).unwrap_or(u32::MAX), files))
}

#[cfg(test)]
mod tests;
