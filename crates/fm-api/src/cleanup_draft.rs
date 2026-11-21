//! The Duplicates module's overview and its draft plan (E56.11).
//!
//! - **The overview** ([`DuplicatesOverview`]) is what a cleanup of a scope
//!   would do with the saved policy, counted: the sets, the extra files, the
//!   folders they are in and the space they take, per level. It is computed
//!   from a plan ([`DuplicatesOverview::from_plan`]), so it says what the
//!   plan would do, and every implementation counts the same way.
//! - **The draft** ([`CleanupDraft`]) collects the user's decisions while
//!   they work through the queue of sets. It touches no file: nothing moves
//!   until a plan is applied. It is stored by the service (fm-core keeps it
//!   in the index), so it survives leaving the module and a restart.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::cleanup::{
    CleanupAction, CleanupKeeper, CleanupPlan, CleanupScope, DuplicateAnswer, PlanCleanupRequest,
    PlannedAnswer, RemoveAs, StayReason,
};
use crate::common::Timestamp;
use crate::library::{DuplicateLevel, DuplicateScope, DuplicateSet, FolderPair};
use crate::settings::MyLanguagesStatus;
use crate::{ApiError, ApiResult};

/// The overview of a scope ([`crate::LibraryService::duplicates_overview`]).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DuplicatesOverviewRequest {
    /// The sets to count; the whole library by default.
    #[serde(default)]
    pub scope: DuplicateScope,
}

/// The sets of one level and what a cleanup would do with them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LevelSummary {
    /// The sets of this level (each one font).
    pub sets: u32,
    /// The files the plan removes from them.
    pub files: u32,
    /// The bytes those files free (saturating).
    pub frees: u32,
    /// The folders those files are in.
    pub folders: u32,
}

/// What a cleanup of a scope would do with the saved policy, counted
/// (E56.11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DuplicatesOverview {
    /// Fonts with more than one copy: the duplicate sets.
    pub fonts: u32,
    /// The files a cleanup would remove.
    pub extra_files: u32,
    /// The folders those files are in.
    pub folders: u32,
    /// The bytes those files free (saturating).
    pub frees: u32,
    /// Identical files: one decision for all of them.
    pub identical: LevelSummary,
    /// The same version in files that differ.
    pub same_version: LevelSummary,
    /// Other versions of one font.
    pub other_version: LevelSummary,
    /// The sets the policy answered "keep both": nothing is removed.
    pub keep_both: u32,
    /// Whether my languages are set; `NotSet` means the guard did nothing,
    /// and the module offers to set them.
    pub my_languages: MyLanguagesStatus,
}

/// One level's sets, files leaving, bytes freed and folders, while counting.
type LevelTally<'a> = (HashSet<&'a str>, u32, u32, BTreeSet<&'a Path>);

impl DuplicatesOverview {
    /// Counts what `plan` does. Every implementation of
    /// [`crate::LibraryService::duplicates_overview`] uses this, so the
    /// counting rule is one.
    pub fn from_plan(plan: &CleanupPlan) -> Self {
        let mut levels: HashMap<DuplicateLevel, LevelTally<'_>> = HashMap::new();
        let mut keep_both: HashSet<&str> = HashSet::new();
        let mut folders: BTreeSet<&Path> = BTreeSet::new();
        let mut extra = 0u32;
        for f in &plan.files {
            let entry = levels.entry(f.level).or_default();
            entry.0.insert(f.set.as_str());
            if f.reason == Some(StayReason::KeepBoth) {
                keep_both.insert(f.set.as_str());
            }
            if f.action == CleanupAction::Stays {
                continue;
            }
            extra = extra.saturating_add(1);
            entry.1 = entry.1.saturating_add(1);
            if f.action == CleanupAction::Trash {
                entry.2 = entry.2.saturating_add(f.size);
            }
            if let Some(dir) = Path::new(&f.path).parent() {
                folders.insert(dir);
                entry.3.insert(dir);
            }
        }
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        let level = |l: DuplicateLevel| {
            levels
                .get(&l)
                .map(|(sets, files, frees, dirs)| LevelSummary {
                    sets: count(sets.len()),
                    files: *files,
                    frees: *frees,
                    folders: count(dirs.len()),
                })
                .unwrap_or_default()
        };
        Self {
            fonts: plan.sets,
            extra_files: extra,
            folders: count(folders.len()),
            frees: plan.frees,
            identical: level(DuplicateLevel::Identical),
            same_version: level(DuplicateLevel::SameVersion),
            other_version: level(DuplicateLevel::OtherVersion),
            keep_both: count(keep_both.len()),
            my_languages: plan.my_languages,
        }
    }
}

impl From<DuplicateScope> for CleanupScope {
    fn from(scope: DuplicateScope) -> Self {
        match scope {
            DuplicateScope::Library => Self::Library,
            DuplicateScope::Source { source } => Self::Source { source },
            DuplicateScope::Folder { path } => Self::Folder { path },
        }
    }
}

/// What the user chose for one copy of a set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CopyChoice {
    /// The copy stays.
    Keep,
    /// The copy goes to the OS trash when the plan is applied.
    Trash,
    /// The copy is hidden when the plan is applied: its file stays (E56.8).
    Hide,
}

/// One copy's choice in a [`SetDecision::Copies`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CopyDecision {
    /// The copy, a path of the set.
    pub path: String,
    /// What happens to it.
    pub choice: CopyChoice,
}

/// The user's answer for one duplicate set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SetDecision {
    /// A choice per copy; at least one copy is kept, and a copy not named
    /// stays.
    #[serde(rename_all = "camelCase")]
    Copies {
        /// The choices.
        copies: Vec<CopyDecision>,
    },
    /// Keep every copy ("keep both").
    KeepAll,
    /// The copies are not duplicates of each other.
    NotDuplicates,
}

impl SetDecision {
    /// `InvalidRequest` when the decision does not fit `set`: a path that is
    /// not a copy of it or is named twice, no copy kept, or a copy in a
    /// system font folder that would leave (rule 4).
    pub fn check(&self, set: &DuplicateSet) -> ApiResult<()> {
        let Self::Copies { copies } = self else {
            return Ok(());
        };
        let mut seen = HashSet::new();
        for d in copies {
            let Some(copy) = set.copies.iter().find(|c| c.path == d.path) else {
                return Err(ApiError::invalid(format!(
                    "{} is not a copy of this font.",
                    d.path
                )));
            };
            if !seen.insert(d.path.as_str()) {
                return Err(ApiError::invalid(format!("{} is named twice.", d.path)));
            }
            if copy.system && d.choice != CopyChoice::Keep {
                return Err(ApiError::invalid(
                    "A copy in a system font folder always stays. Font Fu never moves system fonts.",
                ));
            }
        }
        let kept = set.copies.iter().any(|c| {
            copies
                .iter()
                .find(|d| d.path == c.path)
                .is_none_or(|d| d.choice == CopyChoice::Keep)
        });
        if !kept {
            return Err(ApiError::invalid("Keep at least one copy of the font."));
        }
        Ok(())
    }
}

/// One decision of the draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DraftDecision {
    /// The set id ([`DuplicateSet::id`]).
    pub set: String,
    /// The set's level when it was decided.
    pub level: DuplicateLevel,
    /// The set's PostScript name, when it has one.
    pub postscript_name: Option<String>,
    /// The answer.
    pub decision: SetDecision,
    /// When it was decided.
    pub at: Timestamp,
}

/// The decisions collected for the next cleanup plan (E56.11). Touches no
/// file; stored by the service, so it survives a restart.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CleanupDraft {
    /// The identical sets are in the plan, as one decision with the policy's
    /// keepers.
    #[serde(default)]
    pub identical: bool,
    /// The decisions, in the order they were made; one per set.
    #[serde(default)]
    pub decisions: Vec<DraftDecision>,
    /// The folder pairs decided with "Keep this side" (E56.12), in the
    /// order they were added; one per pair of folders.
    #[serde(default)]
    pub pairs: Vec<DraftPair>,
}

/// A folder pair in the draft (E56.12): keep `keep`'s copies and plan the
/// removal of `remove`'s font files that have an identical copy there, and
/// nothing else ([`crate::cleanup::CleanupScope::FolderPair`] without
/// `other_levels`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DraftPair {
    /// The folder whose copies stay, absolute.
    pub keep: String,
    /// The folder whose identical copies are planned for removal, absolute.
    pub remove: String,
    /// The font files of `remove` with an identical copy in `keep` when the
    /// pair was added: what the decision covers.
    pub identical: u32,
    /// Their size in bytes (saturating).
    pub identical_size: u32,
    /// When it was added.
    pub at: Timestamp,
}

impl DraftPair {
    /// Whether this decision is about the folders `a` and `b`, either way round.
    pub fn is_pair(&self, a: &str, b: &str) -> bool {
        (self.keep == a && self.remove == b) || (self.keep == b && self.remove == a)
    }
}

impl CleanupDraft {
    /// The ids of the sets with a decision.
    pub fn decided(&self) -> HashSet<String> {
        self.decisions.iter().map(|d| d.set.clone()).collect()
    }

    /// Applies `update`; `set_of` finds a set of the library by id (`None`
    /// when it is not a duplicate set any more), `pair_of` the folder pair of
    /// two folders (`None` when they are no listed pair). Every
    /// implementation of [`crate::LibraryService::update_cleanup_draft`] uses
    /// this.
    pub fn apply(
        &mut self,
        update: CleanupDraftUpdate,
        now: Timestamp,
        set_of: impl FnOnce(&str) -> ApiResult<Option<DuplicateSet>>,
        pair_of: impl FnOnce(&str, &str) -> ApiResult<Option<FolderPair>>,
    ) -> ApiResult<()> {
        match update {
            CleanupDraftUpdate::Decide { set, decision } => {
                let found = set_of(&set)?.ok_or_else(|| {
                    ApiError::not_found(format!("{set} is not a duplicate set any more."))
                })?;
                decision.check(&found)?;
                let entry = DraftDecision {
                    set: found.id,
                    level: found.level,
                    postscript_name: found.postscript_name,
                    decision,
                    at: now,
                };
                self.decisions.retain(|d| d.set != entry.set);
                self.decisions.push(entry);
            }
            CleanupDraftUpdate::TakeBack { set } => {
                self.decisions.retain(|d| d.set != set);
            }
            CleanupDraftUpdate::Identical { include } => self.identical = include,
            CleanupDraftUpdate::KeepSide { keep, remove } => {
                if keep == remove {
                    return Err(ApiError::invalid("Choose two different folders."));
                }
                let pair = pair_of(&keep, &remove)?.ok_or_else(|| {
                    ApiError::not_found(format!(
                        "{keep} and {remove} do not hold the same fonts any more."
                    ))
                })?;
                let removed = if pair.a.path == remove {
                    &pair.a
                } else if pair.b.path == remove {
                    &pair.b
                } else {
                    return Err(ApiError::not_found(format!(
                        "{remove} is not a side of this folder pair."
                    )));
                };
                if let Some(kind) = removed.protected {
                    return Err(ApiError::invalid(format!(
                        "{remove} is {}: it can be the side that is kept, never the side that is removed.",
                        kind.describe()
                    )));
                }
                let entry = DraftPair {
                    identical: removed.shared.identical,
                    identical_size: removed.identical_size,
                    keep,
                    remove,
                    at: now,
                };
                self.pairs
                    .retain(|p| !p.is_pair(&entry.keep, &entry.remove));
                self.pairs.push(entry);
            }
            CleanupDraftUpdate::TakeBackPair { a, b } => {
                self.pairs.retain(|p| !p.is_pair(&a, &b));
            }
            CleanupDraftUpdate::Clear => *self = Self::default(),
        }
        Ok(())
    }
}

/// Plan the cleanup draft ([`crate::LibraryService::plan_cleanup_draft`],
/// E56.15).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanDraftRequest {
    /// Move the activation of an active copy of an identical set to the
    /// kept copy ([`crate::cleanup::PlanCleanupRequest::move_activations`]).
    #[serde(default)]
    pub move_activations: bool,
}

/// What of the draft a plan holds (E56.15). Applying the plan takes these
/// decisions out of the draft ([`CleanupDraft::take_planned`]); a decision
/// the plan could not hold (its set is gone, a folder pair is no pair any
/// more) stays in the draft.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlannedDraft {
    /// The identical sets were in the plan.
    #[serde(default)]
    pub identical: bool,
    /// The sets whose decision the plan holds.
    #[serde(default)]
    pub sets: Vec<String>,
    /// The folder pairs the plan holds.
    #[serde(default)]
    pub pairs: Vec<DraftPair>,
}

impl CleanupDraft {
    /// Takes out of the draft what `planned` holds; a decision made after
    /// the plan stays.
    pub fn take_planned(&mut self, planned: &PlannedDraft) {
        if planned.identical {
            self.identical = false;
        }
        self.decisions.retain(|d| !planned.sets.contains(&d.set));
        self.pairs.retain(|p| !planned.pairs.contains(p));
    }
}

const KEPT_BY_CHOICE: &str = "Stays: you chose to keep this copy.";
const KEPT_FOR_ANOTHER: &str = "Stays: another file of this plan leaves for it, so it is kept.";

/// Plans `draft` as one plan (E56.15); every implementation of
/// [`crate::LibraryService::plan_cleanup_draft`] uses this. `sets` are the
/// duplicate sets of the library as listed now; `plan` plans one request
/// ([`crate::LibraryService::plan_cleanup`]).
///
/// - The identical sets (when in) and the sets with a choice per copy are
///   one plan of their ids: each such set keeps the first copy its decision
///   keeps, so the guard and every reason to stay still apply; then a copy
///   the user kept stays, and a copy the user chose to hide is hidden
///   instead of trashed.
/// - Each folder pair is planned as its own scope and adds the files the
///   first part does not hold.
/// - A file that leaves while another leaving file of the plan names it as
///   the copy kept for it stays (`keptByOtherSet`), so the parts never
///   remove each other's keepers.
/// - "Keep both" and "not duplicates" move nothing: they are the plan's
///   [`CleanupPlan::answers`], recorded when it is applied. An answered set
///   is out of the identical sets and out of every folder pair.
pub fn plan_draft(
    draft: &CleanupDraft,
    sets: &[DuplicateSet],
    req: &PlanDraftRequest,
    plan: impl Fn(PlanCleanupRequest) -> ApiResult<CleanupPlan>,
) -> ApiResult<CleanupPlan> {
    let by_id: HashMap<&str, &DuplicateSet> = sets.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut ids: Vec<String> = Vec::new();
    let mut keepers: Vec<CleanupKeeper> = Vec::new();
    // The user's choice per (set, copy); a copy not named stays.
    let mut chosen: HashMap<(String, String), CopyChoice> = HashMap::new();
    let mut by_choice: HashSet<String> = HashSet::new();
    let mut planned = PlannedDraft::default();
    let mut answers: Vec<PlannedAnswer> = Vec::new();
    // The sets answered "keep both" or "not duplicates": none of their copies
    // leaves, whatever else of the draft covers them.
    let mut answered: HashSet<String> = HashSet::new();
    if draft.identical {
        planned.identical = true;
        ids.extend(
            sets.iter()
                .filter(|s| s.level == DuplicateLevel::Identical)
                .map(|s| s.id.clone()),
        );
    }
    for d in &draft.decisions {
        let Some(set) = by_id.get(d.set.as_str()) else {
            continue;
        };
        if d.decision.check(set).is_err() {
            continue;
        }
        match &d.decision {
            SetDecision::Copies { copies } => {
                let choice = |path: &str| {
                    copies
                        .iter()
                        .find(|c| c.path == path)
                        .map_or(CopyChoice::Keep, |c| c.choice)
                };
                let Some(keep) = set
                    .copies
                    .iter()
                    .find(|c| choice(&c.path) == CopyChoice::Keep)
                else {
                    continue;
                };
                keepers.push(CleanupKeeper {
                    set: set.id.clone(),
                    keep: keep.path.clone(),
                });
                for c in &set.copies {
                    chosen.insert((set.id.clone(), c.path.clone()), choice(&c.path));
                }
                by_choice.insert(set.id.clone());
                if !ids.contains(&set.id) {
                    ids.push(set.id.clone());
                }
            }
            SetDecision::KeepAll | SetDecision::NotDuplicates => {
                answered.insert(set.id.clone());
                answers.push(PlannedAnswer {
                    set: set.id.clone(),
                    postscript_name: set.postscript_name.clone(),
                    answer: if d.decision == SetDecision::KeepAll {
                        DuplicateAnswer::KeepBoth
                    } else {
                        DuplicateAnswer::NotDuplicates
                    },
                });
            }
        }
        planned.sets.push(set.id.clone());
    }
    ids.retain(|id| !answered.contains(id));
    let mut out = plan(PlanCleanupRequest {
        scope: CleanupScope::Sets { ids },
        keepers,
        remove_as: RemoveAs::Trash,
        move_activations: req.move_activations,
    })?;
    // The user's choices per copy.
    for f in &mut out.files {
        if !by_choice.contains(&f.set) {
            continue;
        }
        match chosen
            .get(&(f.set.clone(), f.path.clone()))
            .copied()
            .unwrap_or(CopyChoice::Keep)
        {
            CopyChoice::Keep if f.action != CleanupAction::Stays => {
                f.action = CleanupAction::Stays;
                f.reason = Some(StayReason::Keeper);
                KEPT_BY_CHOICE.clone_into(&mut f.sentence);
                if let Some(m) = &mut f.activation {
                    m.planned = false;
                }
            }
            CopyChoice::Hide if f.action == CleanupAction::Trash => {
                f.action = CleanupAction::Hide;
                if let Some(rest) = f.sentence.strip_prefix("Goes to the trash") {
                    f.sentence = format!("Is hidden{rest}");
                }
            }
            _ => {}
        }
    }
    // The folder pairs, each its own scope.
    for p in &draft.pairs {
        let Ok(part) = plan(PlanCleanupRequest {
            scope: CleanupScope::FolderPair {
                keep: p.keep.clone(),
                remove: p.remove.clone(),
                other_levels: false,
            },
            move_activations: req.move_activations,
            ..Default::default()
        }) else {
            continue;
        };
        let have: HashSet<String> = out.files.iter().map(|f| f.path.clone()).collect();
        out.files.extend(
            part.files
                .into_iter()
                .filter(|f| !have.contains(&f.path) && !answered.contains(&f.set)),
        );
        for c in part.conflicts {
            if !out.conflicts.contains(&c) {
                out.conflicts.push(c);
            }
        }
        out.sets = out.sets.saturating_add(part.sets);
        planned.pairs.push(p.clone());
    }
    // No part removes a copy another part keeps for its file.
    loop {
        let kept: HashSet<String> = out
            .files
            .iter()
            .filter(|f| f.action != CleanupAction::Stays)
            .flat_map(|f| f.kept.iter().chain(f.also_kept.iter()).cloned())
            .collect();
        let mut changed = false;
        for f in &mut out.files {
            if f.action != CleanupAction::Stays && kept.contains(&f.path) {
                f.action = CleanupAction::Stays;
                f.reason = Some(StayReason::KeptByOtherSet);
                KEPT_FOR_ANOTHER.clone_into(&mut f.sentence);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // The conflicts of the copies that still leave; the bytes they free.
    let leaving: HashSet<crate::FaceId> = out
        .files
        .iter()
        .filter(|f| f.action != CleanupAction::Stays)
        .flat_map(|f| f.faces.iter().copied())
        .collect();
    out.conflicts
        .retain(|c| leaving.contains(&c.other_face) || leaving.contains(&c.default_from));
    out.frees = out
        .files
        .iter()
        .filter(|f| f.action == CleanupAction::Trash)
        .fold(0u32, |sum, f| sum.saturating_add(f.size));
    out.answers = answers;
    out.draft = Some(planned);
    Ok(out)
}

/// The duplicates of a scope, counted for the ways into the module
/// ([`crate::LibraryService::duplicate_summary`], E56.15): the sidebar row,
/// the notice after a scan, the marks on a family.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateSummaryRequest {
    /// The files to look at; the whole library by default.
    #[serde(default)]
    pub scope: DuplicateScope,
}

/// What [`DuplicateSummaryRequest`] answers.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateSummary {
    /// Fonts with more than one copy: the duplicate sets with a copy in the
    /// scope.
    pub fonts: u32,
    /// The files in the scope that are a copy in one of those sets.
    pub copies: u32,
    /// The families with a face in one of those sets, by family id.
    pub families: Vec<FamilyCopies>,
}

/// One family's duplicates (E56.15).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FamilyCopies {
    /// The family.
    pub family: crate::FamilyId,
    /// The most copies one of its fonts has ("3 copies").
    pub copies: u32,
    /// Its duplicate sets, by id.
    pub sets: Vec<String>,
}

impl DuplicateSummary {
    /// Counts `sets` (the sets listed for the scope); `in_scope` says whether
    /// a file is in the scope, `family_of` the family of a face. Every
    /// implementation of [`crate::LibraryService::duplicate_summary`] uses
    /// this.
    pub fn from_sets(
        sets: &[DuplicateSet],
        in_scope: impl Fn(&str) -> bool,
        family_of: impl Fn(crate::FaceId) -> Option<crate::FamilyId>,
    ) -> Self {
        let mut paths: HashSet<&str> = HashSet::new();
        let mut families: BTreeMap<crate::FamilyId, FamilyCopies> = BTreeMap::new();
        for s in sets {
            let count = u32::try_from(s.copies.len()).unwrap_or(u32::MAX);
            let mut seen: HashSet<crate::FamilyId> = HashSet::new();
            for c in &s.copies {
                if in_scope(&c.path) {
                    paths.insert(&c.path);
                }
                let Some(family) = c.face.and_then(&family_of) else {
                    continue;
                };
                if !seen.insert(family) {
                    continue;
                }
                let entry = families.entry(family).or_insert_with(|| FamilyCopies {
                    family,
                    copies: 0,
                    sets: Vec::new(),
                });
                entry.copies = entry.copies.max(count);
                entry.sets.push(s.id.clone());
            }
        }
        Self {
            fonts: u32::try_from(sets.len()).unwrap_or(u32::MAX),
            copies: u32::try_from(paths.len()).unwrap_or(u32::MAX),
            families: families.into_values().collect(),
        }
    }
}

/// A change to the draft ([`crate::LibraryService::update_cleanup_draft`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum CleanupDraftUpdate {
    /// Decide a set, or decide it again. `NotFound` when the set is not a
    /// duplicate set any more; `InvalidRequest` when the decision does not
    /// fit it ([`SetDecision::check`]).
    #[serde(rename_all = "camelCase")]
    Decide {
        /// The set id.
        set: String,
        /// The answer.
        decision: SetDecision,
    },
    /// Take a set's decision back: it is in the queue again.
    #[serde(rename_all = "camelCase")]
    TakeBack {
        /// The set id.
        set: String,
    },
    /// Put the identical sets in the plan, or take them out.
    #[serde(rename_all = "camelCase")]
    Identical {
        /// In the plan.
        include: bool,
    },
    /// Keep one folder's copies and plan the removal of the other folder's
    /// identical copies (E56.12); replaces a decision about the same two
    /// folders. `NotFound` when the folders are no folder pair any more;
    /// `InvalidRequest` when `remove` is protected
    /// ([`crate::library::FolderPairSide::protected`]).
    #[serde(rename_all = "camelCase")]
    KeepSide {
        /// The folder whose copies stay, absolute.
        keep: String,
        /// The folder whose identical copies leave, absolute.
        remove: String,
    },
    /// Take back the decision about two folders, either way round.
    #[serde(rename_all = "camelCase")]
    TakeBackPair {
        /// One folder, absolute.
        a: String,
        /// The other folder, absolute.
        b: String,
    },
    /// Start again: no decision, identical sets out, no folder pair.
    Clear,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleanup::{CLEANUP_PLAN_VERSION, CleanupFile, PlanCleanupRequest};
    use crate::library::{DuplicateSetCopy, FolderPairSide, ProtectedFolder, SharedCounts};
    use crate::{FaceId, SourceId};

    fn copy(path: &str, system: bool) -> DuplicateSetCopy {
        DuplicateSetCopy {
            path: path.to_owned(),
            size: 10,
            hash: "h".to_owned(),
            face: None,
            version: None,
            format: None,
            source: SourceId(1),
            in_library_folder: false,
            system,
            in_scope: true,
            identical_with: Vec::new(),
            other_sets: Vec::new(),
            facts: None,
        }
    }

    fn set() -> DuplicateSet {
        DuplicateSet {
            id: "name:A".to_owned(),
            level: DuplicateLevel::OtherVersion,
            postscript_name: Some("A".to_owned()),
            formats_differ: false,
            copies: vec![
                copy("/a/A.otf", false),
                copy("/b/A.otf", false),
                copy("/sys/A.otf", true),
            ],
            recommendation: None,
        }
    }

    fn no_pair(_: &str, _: &str) -> ApiResult<Option<FolderPair>> {
        Ok(None)
    }

    fn side(path: &str, identical: u32, protected: Option<ProtectedFolder>) -> FolderPairSide {
        FolderPairSide {
            path: path.to_owned(),
            font_files: 10,
            size: 1000,
            shared: SharedCounts {
                identical,
                same_version: 1,
                other_version: 0,
            },
            shared_size: 700,
            identical_size: identical * 100,
            only_here: 10 - identical - 1,
            other_files: 0,
            contained: false,
            protected,
        }
    }

    fn pair(protected_b: Option<ProtectedFolder>) -> FolderPair {
        FolderPair {
            a: side("/a", 6, None),
            b: side("/b", 4, protected_b),
            frees: 400,
        }
    }

    fn keep_side(keep: &str, remove: &str) -> CleanupDraftUpdate {
        CleanupDraftUpdate::KeepSide {
            keep: keep.to_owned(),
            remove: remove.to_owned(),
        }
    }

    #[test]
    fn keep_this_side_adds_the_pair_with_what_it_covers_and_replaces_the_other_way_round() {
        let mut draft = CleanupDraft::default();
        let open = |_: &str, _: &str| Ok(Some(pair(None)));
        draft
            .apply(keep_side("/b", "/a"), "t1".to_owned(), |_| Ok(None), open)
            .expect("keep /b");
        assert_eq!(draft.pairs.len(), 1);
        assert_eq!(draft.pairs[0].remove, "/a");
        assert_eq!(
            draft.pairs[0].identical, 6,
            "/a's identical copies, nothing else"
        );
        assert_eq!(draft.pairs[0].identical_size, 600);
        draft
            .apply(keep_side("/a", "/b"), "t2".to_owned(), |_| Ok(None), open)
            .expect("keep /a instead");
        assert_eq!(draft.pairs.len(), 1, "one decision per pair of folders");
        assert_eq!(draft.pairs[0].remove, "/b");
        assert_eq!(draft.pairs[0].identical, 4);

        draft
            .apply(
                CleanupDraftUpdate::TakeBackPair {
                    a: "/b".to_owned(),
                    b: "/a".to_owned(),
                },
                "t3".to_owned(),
                |_| Ok(None),
                no_pair,
            )
            .expect("take back");
        assert!(draft.pairs.is_empty());
    }

    #[test]
    fn keep_this_side_never_removes_a_protected_side_or_a_pair_that_is_gone() {
        let mut draft = CleanupDraft::default();
        let system = |_: &str, _: &str| Ok(Some(pair(Some(ProtectedFolder::SystemFolder))));
        let refused = draft
            .apply(keep_side("/a", "/b"), "t1".to_owned(), |_| Ok(None), system)
            .expect_err("the system folder is never the removed side");
        assert_eq!(refused.code, crate::ErrorCode::InvalidRequest);
        assert!(
            refused.message.contains("a system font folder"),
            "{}",
            refused.message
        );
        assert!(draft.pairs.is_empty());
        draft
            .apply(keep_side("/b", "/a"), "t2".to_owned(), |_| Ok(None), system)
            .expect("the system folder can be the kept side");
        assert_eq!(draft.pairs.len(), 1);

        let gone = draft
            .apply(
                keep_side("/a", "/c"),
                "t3".to_owned(),
                |_| Ok(None),
                no_pair,
            )
            .expect_err("no pair");
        assert_eq!(gone.code, crate::ErrorCode::NotFound);
        let same = draft
            .apply(
                keep_side("/a", "/a"),
                "t4".to_owned(),
                |_| Ok(None),
                no_pair,
            )
            .expect_err("one folder");
        assert_eq!(same.code, crate::ErrorCode::InvalidRequest);
        draft
            .apply(
                CleanupDraftUpdate::Clear,
                "t5".to_owned(),
                |_| Ok(None),
                no_pair,
            )
            .expect("clear");
        assert!(draft.pairs.is_empty(), "clear drops the pairs too");
    }

    fn copies(choices: &[(&str, CopyChoice)]) -> SetDecision {
        SetDecision::Copies {
            copies: choices
                .iter()
                .map(|(p, c)| CopyDecision {
                    path: (*p).to_owned(),
                    choice: *c,
                })
                .collect(),
        }
    }

    #[test]
    fn a_decision_must_keep_a_copy_and_never_move_a_system_copy() {
        let s = set();
        assert!(copies(&[("/a/A.otf", CopyChoice::Trash)]).check(&s).is_ok());
        assert!(
            copies(&[
                ("/a/A.otf", CopyChoice::Trash),
                ("/b/A.otf", CopyChoice::Hide)
            ])
            .check(&s)
            .is_ok(),
            "the system copy is kept"
        );
        assert!(
            copies(&[("/sys/A.otf", CopyChoice::Trash)])
                .check(&s)
                .is_err()
        );
        assert!(copies(&[("/c/A.otf", CopyChoice::Keep)]).check(&s).is_err());
        assert!(
            copies(&[
                ("/a/A.otf", CopyChoice::Keep),
                ("/a/A.otf", CopyChoice::Trash)
            ])
            .check(&s)
            .is_err()
        );
        let mut two = set();
        two.copies.pop();
        assert!(
            copies(&[
                ("/a/A.otf", CopyChoice::Trash),
                ("/b/A.otf", CopyChoice::Trash)
            ])
            .check(&two)
            .is_err(),
            "no copy kept"
        );
        assert!(SetDecision::NotDuplicates.check(&two).is_ok());
    }

    #[test]
    fn deciding_a_set_again_replaces_its_decision_and_take_back_removes_it() {
        let mut draft = CleanupDraft::default();
        let found = |_: &str| Ok(Some(set()));
        draft
            .apply(
                CleanupDraftUpdate::Decide {
                    set: "name:A".to_owned(),
                    decision: SetDecision::KeepAll,
                },
                "t1".to_owned(),
                found,
                no_pair,
            )
            .expect("decide");
        draft
            .apply(
                CleanupDraftUpdate::Decide {
                    set: "name:A".to_owned(),
                    decision: SetDecision::NotDuplicates,
                },
                "t2".to_owned(),
                found,
                no_pair,
            )
            .expect("decide again");
        assert_eq!(draft.decisions.len(), 1);
        assert_eq!(draft.decisions[0].decision, SetDecision::NotDuplicates);
        assert_eq!(draft.decisions[0].level, DuplicateLevel::OtherVersion);
        let gone = draft.apply(
            CleanupDraftUpdate::Decide {
                set: "name:B".to_owned(),
                decision: SetDecision::KeepAll,
            },
            "t3".to_owned(),
            |_| Ok(None),
            no_pair,
        );
        assert!(gone.is_err(), "a set that is not there any more");
        draft
            .apply(
                CleanupDraftUpdate::TakeBack {
                    set: "name:A".to_owned(),
                },
                "t4".to_owned(),
                found,
                no_pair,
            )
            .expect("take back");
        assert!(draft.decisions.is_empty());
    }

    fn file(
        path: &str,
        set: &str,
        level: DuplicateLevel,
        action: CleanupAction,
        reason: Option<StayReason>,
    ) -> CleanupFile {
        CleanupFile {
            path: path.to_owned(),
            size: 100,
            hash: "h".to_owned(),
            faces: vec![FaceId(1)],
            set: set.to_owned(),
            level,
            action,
            reason,
            kept: None,
            also_kept: Vec::new(),
            sentence: String::new(),
            activation: None,
        }
    }

    #[test]
    fn the_overview_counts_what_the_plan_removes_per_level() {
        use CleanupAction::{Hide, Stays, Trash};
        use DuplicateLevel::{Identical, OtherVersion, SameVersion};
        let plan = CleanupPlan {
            version: CLEANUP_PLAN_VERSION,
            planned_at: "t0".to_owned(),
            request: PlanCleanupRequest::default(),
            sets: 4,
            files: vec![
                file(
                    "/a/x.otf",
                    "identical:1",
                    Identical,
                    Stays,
                    Some(StayReason::Keeper),
                ),
                file("/b/x.otf", "identical:1", Identical, Trash, None),
                file("/c/x.otf", "identical:1", Identical, Trash, None),
                file(
                    "/a/y.otf",
                    "name:Y",
                    OtherVersion,
                    Stays,
                    Some(StayReason::Keeper),
                ),
                file("/b/y.otf", "name:Y", OtherVersion, Hide, None),
                file(
                    "/a/z.otf",
                    "name:Z",
                    OtherVersion,
                    Stays,
                    Some(StayReason::KeepBoth),
                ),
                file(
                    "/b/z.otf",
                    "name:Z",
                    OtherVersion,
                    Stays,
                    Some(StayReason::KeepBoth),
                ),
                file(
                    "/a/w.otf",
                    "name:W",
                    SameVersion,
                    Stays,
                    Some(StayReason::Active),
                ),
            ],
            conflicts: Vec::new(),
            frees: 200,
            my_languages: MyLanguagesStatus::NotSet,
            answers: Vec::new(),
            draft: None,
        };
        let o = DuplicatesOverview::from_plan(&plan);
        assert_eq!(o.fonts, 4);
        assert_eq!(o.extra_files, 3);
        assert_eq!(o.folders, 2, "/b and /c");
        assert_eq!(o.frees, 200);
        assert_eq!(
            o.identical,
            LevelSummary {
                sets: 1,
                files: 2,
                frees: 200,
                folders: 2
            }
        );
        assert_eq!(
            o.other_version,
            LevelSummary {
                sets: 2,
                files: 1,
                frees: 0,
                folders: 1
            },
            "a hidden file frees nothing"
        );
        assert_eq!(
            o.same_version,
            LevelSummary {
                sets: 1,
                ..LevelSummary::default()
            },
            "an active copy stays: nothing leaves"
        );
        assert_eq!(o.keep_both, 1);
        assert_eq!(o.my_languages, MyLanguagesStatus::NotSet);
    }

    #[test]
    fn the_recommendation_filter_lists_a_keep_both_set_and_a_keeper_set_apart() {
        use crate::library::{
            DuplicateSetFilter, KeeperOutcome, KeeperRecommendation, RecommendationFilter,
        };
        use crate::settings::MyLanguagesStatus;
        use std::collections::HashSet;

        let with = |outcome| {
            let mut s = set();
            s.recommendation = Some(KeeperRecommendation {
                outcome,
                keeper: Some("/a/A.otf".to_owned()),
                reason: None,
                keep: Vec::new(),
                remove: Vec::new(),
                my_languages: MyLanguagesStatus::Set,
            });
            s
        };
        let filter = |r| DuplicateSetFilter {
            recommendation: Some(r),
            ..Default::default()
        };
        let none = HashSet::new();
        let keep_both = with(KeeperOutcome::KeepBoth);
        let keeper = with(KeeperOutcome::Keeper);

        assert!(filter(RecommendationFilter::KeepBoth).matches(&keep_both, &none));
        assert!(!filter(RecommendationFilter::KeepBoth).matches(&keeper, &none));
        assert!(!filter(RecommendationFilter::KeepBoth).matches(&set(), &none));
        // A keeper with nothing to remove is no recommendation to act on.
        assert!(!filter(RecommendationFilter::HasRecommendation).matches(&keeper, &none));
        assert!(!filter(RecommendationFilter::HasRecommendation).matches(&keep_both, &none));
    }

    // E56.15: the draft as one plan.

    fn decision(set: &str, decision: SetDecision) -> DraftDecision {
        DraftDecision {
            set: set.to_owned(),
            level: DuplicateLevel::OtherVersion,
            postscript_name: None,
            decision,
            at: "t0".to_owned(),
        }
    }

    fn named(id: &str, paths: &[&str]) -> DuplicateSet {
        DuplicateSet {
            id: id.to_owned(),
            copies: paths.iter().map(|p| copy(p, false)).collect(),
            ..set()
        }
    }

    /// A plan as the services make one: the keeper the request names (else
    /// the first copy) stays, every other copy goes to the trash. A folder
    /// pair removes `<remove>/A.otf` for `<keep>/A.otf`.
    fn fake_plan(sets: Vec<DuplicateSet>) -> impl Fn(PlanCleanupRequest) -> ApiResult<CleanupPlan> {
        move |req| {
            let mut files = Vec::new();
            if let CleanupScope::Sets { ids } = &req.scope {
                for id in ids {
                    let s = sets.iter().find(|s| &s.id == id).unwrap();
                    let keep = req
                        .keepers
                        .iter()
                        .find(|k| &k.set == id)
                        .map_or(s.copies[0].path.clone(), |k| k.keep.clone());
                    for c in &s.copies {
                        let mut f = file(&c.path, id, s.level, CleanupAction::Trash, None);
                        f.kept = Some(keep.clone());
                        if c.path == keep {
                            f.action = CleanupAction::Stays;
                            f.reason = Some(StayReason::Keeper);
                        } else {
                            f.sentence = "Goes to the trash; A.otf stays.".to_owned();
                        }
                        files.push(f);
                    }
                }
            }
            if let CleanupScope::FolderPair { keep, remove, .. } = &req.scope {
                let mut f = file(
                    &format!("{remove}/A.otf"),
                    "name:A",
                    DuplicateLevel::Identical,
                    CleanupAction::Trash,
                    None,
                );
                f.kept = Some(format!("{keep}/A.otf"));
                files.push(f);
            }
            Ok(CleanupPlan {
                version: CLEANUP_PLAN_VERSION,
                planned_at: "t0".to_owned(),
                request: req,
                sets: 1,
                files,
                conflicts: Vec::new(),
                frees: 0,
                my_languages: MyLanguagesStatus::NotSet,
                answers: Vec::new(),
                draft: None,
            })
        }
    }

    fn choose(path: &str, choice: CopyChoice) -> CopyDecision {
        CopyDecision {
            path: path.to_owned(),
            choice,
        }
    }

    fn draft_pair(keep: &str, remove: &str) -> DraftPair {
        DraftPair {
            keep: keep.into(),
            remove: remove.into(),
            identical: 1,
            identical_size: 10,
            at: "t0".into(),
        }
    }

    #[test]
    fn the_draft_plans_each_copy_as_the_user_chose_and_answers_the_rest() {
        let a = named("name:A", &["/a/A.otf", "/b/A.otf", "/c/A.otf", "/d/A.otf"]);
        let b = named("name:B", &["/a/B.otf", "/b/B.otf"]);
        let draft = CleanupDraft {
            identical: false,
            decisions: vec![
                decision(
                    "name:A",
                    SetDecision::Copies {
                        copies: vec![
                            choose("/a/A.otf", CopyChoice::Trash),
                            choose("/b/A.otf", CopyChoice::Keep),
                            choose("/c/A.otf", CopyChoice::Hide),
                            // /d/A.otf is not named: it stays.
                        ],
                    },
                ),
                decision("name:B", SetDecision::KeepAll),
                decision("name:Gone", SetDecision::NotDuplicates),
            ],
            pairs: Vec::new(),
        };
        let sets = vec![a, b];
        let plan = plan_draft(
            &draft,
            &sets,
            &PlanDraftRequest::default(),
            fake_plan(sets.clone()),
        )
        .unwrap();
        assert_eq!(
            plan.request.keepers,
            vec![CleanupKeeper {
                set: "name:A".into(),
                keep: "/b/A.otf".into()
            }]
        );
        let action = |p: &str| {
            plan.files
                .iter()
                .find(|f| f.path == p)
                .map(|f| (f.action, f.reason))
        };
        assert_eq!(action("/a/A.otf"), Some((CleanupAction::Trash, None)));
        assert_eq!(
            action("/b/A.otf"),
            Some((CleanupAction::Stays, Some(StayReason::Keeper)))
        );
        assert_eq!(action("/c/A.otf"), Some((CleanupAction::Hide, None)));
        assert_eq!(
            action("/d/A.otf"),
            Some((CleanupAction::Stays, Some(StayReason::Keeper))),
            "a copy not named stays"
        );
        let hidden = plan.files.iter().find(|f| f.path == "/c/A.otf").unwrap();
        assert!(
            hidden.sentence.starts_with("Is hidden"),
            "{}",
            hidden.sentence
        );
        assert_eq!(plan.frees, 100, "only the trashed copy frees space");
        assert_eq!(
            plan.answers,
            vec![PlannedAnswer {
                set: "name:B".into(),
                postscript_name: Some("A".into()),
                answer: DuplicateAnswer::KeepBoth
            }]
        );
        let planned = plan.draft.unwrap();
        assert_eq!(
            planned.sets,
            vec!["name:A".to_owned(), "name:B".to_owned()],
            "a set that is gone stays in the draft"
        );
        assert!(!planned.identical);
    }

    #[test]
    fn an_answered_set_loses_no_copy_to_the_identical_sets_or_a_folder_pair() {
        // The family queue lists identical sets too, so one can be answered
        // "keep both" while the identical copies are in the plan; a folder
        // pair can cover an answered set as well.
        let same = DuplicateSet {
            level: DuplicateLevel::Identical,
            ..named("identical:h", &["/a/S.otf", "/b/S.otf"])
        };
        let other = DuplicateSet {
            level: DuplicateLevel::Identical,
            ..named("identical:k", &["/a/K.otf", "/b/K.otf"])
        };
        let draft = CleanupDraft {
            identical: true,
            decisions: vec![
                decision("identical:h", SetDecision::KeepAll),
                decision("name:A", SetDecision::NotDuplicates),
            ],
            pairs: vec![draft_pair("/z", "/a")],
        };
        let sets = vec![same, other, named("name:A", &["/a/A.otf", "/z/A.otf"])];
        let plan = plan_draft(
            &draft,
            &sets,
            &PlanDraftRequest::default(),
            fake_plan(sets.clone()),
        )
        .unwrap();
        let leaving: Vec<&str> = plan.leaving().map(|f| f.path.as_str()).collect();
        assert_eq!(
            leaving,
            vec!["/b/K.otf"],
            "only the unanswered identical set loses a copy"
        );
        assert!(
            plan.files
                .iter()
                .all(|f| f.set != "identical:h" && f.set != "name:A"),
            "an answered set has no row"
        );
        assert_eq!(
            plan.request.scope,
            CleanupScope::Sets {
                ids: vec!["identical:k".into()]
            }
        );
        assert_eq!(plan.answers.len(), 2);
        let planned = plan.draft.unwrap();
        assert!(planned.identical);
        assert_eq!(planned.pairs.len(), 1);
    }

    #[test]
    fn a_folder_pair_never_removes_the_copy_the_sets_keep() {
        let a = named("name:A", &["/a/A.otf", "/z/A.otf"]);
        let draft = CleanupDraft {
            identical: false,
            decisions: vec![decision(
                "name:A",
                SetDecision::Copies {
                    copies: vec![
                        choose("/a/A.otf", CopyChoice::Keep),
                        choose("/z/A.otf", CopyChoice::Trash),
                    ],
                },
            )],
            pairs: vec![draft_pair("/z", "/a")],
        };
        let sets = vec![a];
        let plan = plan_draft(
            &draft,
            &sets,
            &PlanDraftRequest::default(),
            fake_plan(sets.clone()),
        )
        .unwrap();
        // The sets part keeps /a/A.otf; the pair would remove it. The file is
        // in the plan once, as the sets part planned it.
        let rows: Vec<_> = plan.files.iter().filter(|f| f.path == "/a/A.otf").collect();
        assert_eq!(rows.len(), 1, "a file is in the plan once");
        assert_eq!(rows[0].action, CleanupAction::Stays);
        assert!(plan.check().is_ok());
        assert_eq!(plan.draft.unwrap().pairs.len(), 1);
    }

    #[test]
    fn a_part_that_would_remove_another_parts_keeper_keeps_it() {
        // Two pairs: the first removes /a for /b, the second /b for /a.
        let draft = CleanupDraft {
            identical: false,
            decisions: Vec::new(),
            pairs: vec![draft_pair("/b", "/a"), draft_pair("/a", "/b")],
        };
        let plan = plan_draft(
            &draft,
            &[],
            &PlanDraftRequest::default(),
            fake_plan(Vec::new()),
        )
        .unwrap();
        let leaving: Vec<&str> = plan.leaving().map(|f| f.path.as_str()).collect();
        assert!(
            leaving.is_empty(),
            "each is the other one's keeper: {leaving:?}"
        );
        assert!(
            plan.files
                .iter()
                .all(|f| f.reason == Some(StayReason::KeptByOtherSet))
        );
    }

    #[test]
    fn applying_takes_out_of_the_draft_only_what_the_plan_held() {
        let mut draft = CleanupDraft {
            identical: true,
            decisions: vec![
                decision("name:A", SetDecision::KeepAll),
                decision("name:B", SetDecision::KeepAll),
            ],
            pairs: vec![draft_pair("/b", "/a")],
        };
        draft.take_planned(&PlannedDraft {
            identical: true,
            sets: vec!["name:A".into()],
            pairs: Vec::new(),
        });
        assert!(!draft.identical);
        assert_eq!(draft.decided(), HashSet::from(["name:B".to_owned()]));
        assert_eq!(draft.pairs.len(), 1, "a pair decided after the plan stays");
    }

    #[test]
    fn the_summary_counts_fonts_copies_in_scope_and_families() {
        let mut a = named("name:A", &["/a/A.otf", "/b/A.otf", "/c/A.otf"]);
        for (i, c) in (1u32..).zip(a.copies.iter_mut()) {
            c.face = Some(FaceId(i));
        }
        let mut b = named("name:B", &["/a/B.otf", "/b/B.otf"]);
        b.copies[0].face = Some(FaceId(10));
        let families = HashMap::from([
            (FaceId(1), crate::FamilyId(1)),
            (FaceId(2), crate::FamilyId(1)),
            (FaceId(10), crate::FamilyId(1)),
        ]);
        let s = DuplicateSummary::from_sets(
            &[a, b],
            |p| p.starts_with("/a/") || p.starts_with("/b/"),
            |f| families.get(&f).copied(),
        );
        assert_eq!(s.fonts, 2);
        assert_eq!(s.copies, 4, "the copy in /c is outside the scope");
        assert_eq!(
            s.families,
            vec![FamilyCopies {
                family: crate::FamilyId(1),
                copies: 3,
                sets: vec!["name:A".into(), "name:B".into()]
            }]
        );
    }
}
