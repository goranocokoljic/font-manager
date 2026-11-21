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

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::cleanup::{CleanupAction, CleanupPlan, CleanupScope, StayReason};
use crate::common::Timestamp;
use crate::library::{DuplicateLevel, DuplicateScope, DuplicateSet};
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
}

impl CleanupDraft {
    /// The ids of the sets with a decision.
    pub fn decided(&self) -> HashSet<String> {
        self.decisions.iter().map(|d| d.set.clone()).collect()
    }

    /// Applies `update`; `set_of` finds a set of the library by id (`None`
    /// when it is not a duplicate set any more). Every implementation of
    /// [`crate::LibraryService::update_cleanup_draft`] uses this.
    pub fn apply(
        &mut self,
        update: CleanupDraftUpdate,
        now: Timestamp,
        set_of: impl FnOnce(&str) -> ApiResult<Option<DuplicateSet>>,
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
            CleanupDraftUpdate::Clear => *self = Self::default(),
        }
        Ok(())
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
    /// Start again: no decision, identical sets out.
    Clear,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleanup::{CLEANUP_PLAN_VERSION, CleanupFile, PlanCleanupRequest};
    use crate::library::DuplicateSetCopy;
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
        );
        assert!(gone.is_err(), "a set that is not there any more");
        draft
            .apply(
                CleanupDraftUpdate::TakeBack {
                    set: "name:A".to_owned(),
                },
                "t4".to_owned(),
                found,
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
}
