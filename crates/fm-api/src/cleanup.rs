//! A cleanup as one session (E56.7, ADR 0027): plan what would happen to
//! every copy of the duplicate sets of a scope, apply the plan as one named
//! session, and undo a session whole or file by file.
//!
//! - **The plan touches nothing.** [`crate::LibraryService::plan_cleanup`]
//!   answers, per file, whether it would go to the trash, be hidden, or stay,
//!   and why; a copy that stays names the reason ([`StayReason`]).
//! - **A plan is data.** [`CleanupPlan`] is plain JSON: it can be saved and
//!   applied later. Applying checks every file against the hash of the plan,
//!   so a file that changed since is skipped and reported.
//! - **A session.** [`crate::LibraryService::apply_cleanup`] returns a job at
//!   once; progress arrives as [`crate::Event::CleanupProgress`], the apply
//!   can be cancelled between files, and what was done is one
//!   [`CleanupSession`]. "Keep this one" actions are sessions of one set, so
//!   there is one history.

use serde::{Deserialize, Serialize};

use crate::activation::ActivationMode;
use crate::common::Timestamp;
use crate::library::{DuplicateLevel, TrashMoveStatus};
use crate::settings::MyLanguagesStatus;
use crate::{ApiError, ApiResult, FaceId, ResolutionId, SourceId};

/// The version of the [`CleanupPlan`] format this build writes and reads.
pub const CLEANUP_PLAN_VERSION: u32 = 1;

/// Which duplicate sets a cleanup plans.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum CleanupScope {
    /// Every set of the library.
    #[default]
    Library,
    /// The sets with a copy in one source; only its copies can leave.
    #[serde(rename_all = "camelCase")]
    Source {
        /// The source.
        source: SourceId,
    },
    /// The sets with a copy in one folder and its subfolders; only the copies
    /// in it can leave.
    #[serde(rename_all = "camelCase")]
    Folder {
        /// Absolute folder path.
        path: String,
    },
    /// Two folders: keep `keep`'s copies and plan `remove`'s files that have
    /// a copy there (E56.6).
    #[serde(rename_all = "camelCase")]
    FolderPair {
        /// The folder whose copies stay, absolute.
        keep: String,
        /// The folder whose copies are planned for removal, absolute.
        remove: String,
        /// Also plan copies of the same version and of another version,
        /// through the keeper policy and the language guard; identical
        /// copies only when false.
        #[serde(default)]
        other_levels: bool,
    },
    /// The sets named by id ([`crate::library::DuplicateSet::id`]).
    #[serde(rename_all = "camelCase")]
    Sets {
        /// The set ids.
        ids: Vec<String>,
    },
}

/// The copy the user picked to keep in one set, instead of the policy's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CleanupKeeper {
    /// The set id.
    pub set: String,
    /// The copy to keep, a path of that set.
    pub keep: String,
}

/// How a copy leaves the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RemoveAs {
    /// The file goes to the OS trash (ADR 0009).
    #[default]
    Trash,
    /// The file stays where it is and leaves the library (E56.8).
    Hide,
}

/// Plan a cleanup ([`crate::LibraryService::plan_cleanup`]).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanCleanupRequest {
    /// The sets to plan; the whole library by default.
    #[serde(default)]
    pub scope: CleanupScope,
    /// The keepers the user picked; every other set follows the saved keeper
    /// policy and the language guard.
    #[serde(default)]
    pub keepers: Vec<CleanupKeeper>,
    /// How the copies leave.
    #[serde(default)]
    pub remove_as: RemoveAs,
    /// Move the activation of an active copy of an identical set to the
    /// kept copy, so that copy can leave too. Off: an active copy stays.
    #[serde(default)]
    pub move_activations: bool,
}

/// What the plan does with one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CleanupAction {
    /// It goes to the OS trash.
    Trash,
    /// It is hidden: the file stays where it is (E56.8).
    Hide,
    /// It stays in the library.
    Stays,
}

/// Why a file stays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum StayReason {
    /// It is the copy kept for its set.
    Keeper,
    /// The app activated it (rule 3).
    Active,
    /// It is the only copy that covers one of my languages (the guard).
    MyLanguages,
    /// The policy answered its set "keep both".
    KeepBoth,
    /// It is in a system font folder (rule 4).
    System,
    /// It takes part in another set that keeps it (a collection file).
    KeptByOtherSet,
    /// It holds a font no file outside the removed copies holds.
    LastCopy,
    /// The file is not there any more.
    NotThere,
    /// It is outside the scope: the plan only removes copies in it.
    OutsideScope,
    /// It has no copy on the kept side of a folder pair.
    OnlyHere,
    /// Its copy on the kept side of a folder pair is of another level, and
    /// other levels were not asked for.
    OtherLevel,
}

/// The activation of an active copy moved to the kept copy (identical sets
/// only): the leaving copy is deactivated, the kept one activated in the same
/// mode; both are the app's own registrations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivationMove {
    /// The kept copy that takes over the activation.
    pub to: String,
    /// The mode of the activation.
    pub mode: ActivationMode,
    /// The plan does it ([`PlanCleanupRequest::move_activations`]); when
    /// false it is only offered and the copy stays.
    pub planned: bool,
}

/// One file of a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CleanupFile {
    /// Absolute file path.
    pub path: String,
    /// Size in bytes.
    pub size: u32,
    /// blake3 hash of the file when planned, hex; applying skips the file
    /// when it no longer has it.
    pub hash: String,
    /// Its faces.
    pub faces: Vec<FaceId>,
    /// The set it is planned in (the first, for a file in several).
    pub set: String,
    /// That set's level.
    pub level: DuplicateLevel,
    /// What happens to it.
    pub action: CleanupAction,
    /// Why it stays; `None` when it leaves.
    pub reason: Option<StayReason>,
    /// The copy that stays for it: its set's keeper; `None` when its set has
    /// none (every copy a system copy).
    pub kept: Option<String>,
    /// The copies kept for the other fonts of this file, when it takes part
    /// in other sets (a collection file): they take over its metadata too.
    #[serde(default)]
    pub also_kept: Vec<String>,
    /// Why, in one sentence, the same in the UI, the CLI and MCP.
    pub sentence: String,
    /// For an active copy of an identical set: the activation move offered
    /// or planned.
    #[serde(default)]
    pub activation: Option<ActivationMove>,
}

/// A field of the user's metadata with one value (E56.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum MetadataField {
    /// The rating.
    Rating,
    /// The face's license and seats, one value.
    License,
    /// The notes.
    Notes,
}

/// A value of a [`MetadataField`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum MetadataValue {
    /// A rating, 1–5 stars.
    #[serde(rename_all = "camelCase")]
    Rating {
        /// Stars.
        stars: u8,
    },
    /// A license type and seats.
    #[serde(rename_all = "camelCase")]
    License {
        /// The license type as stored.
        license: Option<String>,
        /// Seats.
        seats: Option<u32>,
    },
    /// Notes.
    #[serde(rename_all = "camelCase")]
    Notes {
        /// The text.
        text: String,
    },
}

/// Two copies of a set hold different values in one field; nothing is
/// guessed (E56.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetadataConflict {
    /// The set.
    pub set: String,
    /// The kept face the value is for.
    pub kept_face: FaceId,
    /// The field.
    pub field: MetadataField,
    /// The face whose value is the default: the kept face when it has one.
    pub default_from: FaceId,
    /// The default value.
    pub default: MetadataValue,
    /// The face with the other value.
    pub other_face: FaceId,
    /// The other value.
    pub other: MetadataValue,
}

/// The user takes the other value of a [`MetadataConflict`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetadataChoice {
    /// The kept face.
    pub kept_face: FaceId,
    /// The field.
    pub field: MetadataField,
    /// The face whose value is taken: the conflict's `other_face`.
    pub take: FaceId,
}

/// What a cleanup would do (E56.7). Plain data: it can be saved as JSON and
/// applied later ([`ApplyCleanupRequest`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPlan {
    /// The format version ([`CLEANUP_PLAN_VERSION`]).
    pub version: u32,
    /// When it was planned.
    pub planned_at: Timestamp,
    /// What was planned.
    pub request: PlanCleanupRequest,
    /// The sets planned.
    pub sets: u32,
    /// Every file of the planned sets, by set, then path.
    pub files: Vec<CleanupFile>,
    /// The metadata conflicts of the copies that leave, per set.
    pub conflicts: Vec<MetadataConflict>,
    /// The bytes the files that go to the trash free (saturating).
    pub frees: u32,
    /// Whether my languages were set; `NotSet` means the guard did nothing.
    pub my_languages: MyLanguagesStatus,
}

impl CleanupPlan {
    /// `InvalidRequest` when the plan is of a format this build does not
    /// read, or names a file twice.
    pub fn check(&self) -> ApiResult<()> {
        if self.version != CLEANUP_PLAN_VERSION {
            return Err(ApiError::invalid(
                "This plan was made by another version of Font Fu. Plan the cleanup again.",
            )
            .with_detail(format!("plan version {}", self.version)));
        }
        let mut seen = std::collections::HashSet::new();
        for f in &self.files {
            if !seen.insert(f.path.as_str()) {
                return Err(ApiError::invalid(format!(
                    "{} is in the plan twice.",
                    f.path
                )));
            }
        }
        Ok(())
    }

    /// The files that leave: [`CleanupAction::Trash`] or
    /// [`CleanupAction::Hide`].
    pub fn leaving(&self) -> impl Iterator<Item = &CleanupFile> {
        self.files
            .iter()
            .filter(|f| f.action != CleanupAction::Stays)
    }
}

/// Apply a plan ([`crate::LibraryService::apply_cleanup`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ApplyCleanupRequest {
    /// The plan, as [`crate::LibraryService::plan_cleanup`] answered it.
    pub plan: CleanupPlan,
    /// The session's name; a name from the date when `None`.
    #[serde(default)]
    pub name: Option<String>,
    /// The other values the user takes in the plan's conflicts.
    #[serde(default)]
    pub choices: Vec<MetadataChoice>,
}

/// Where a session came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SessionKind {
    /// "Keep this one" on one set (E8.3).
    KeepCopy,
    /// An applied cleanup plan.
    Cleanup,
}

/// One file of a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionFile {
    /// The file's path in the library.
    pub path: String,
    /// The set it left, when known (a "Keep this one" action from before
    /// E56.7 names none).
    pub set: Option<String>,
    /// The copy kept for it.
    pub kept: String,
    /// Its size in bytes, when known.
    pub size: Option<u32>,
    /// What happened to it.
    pub status: TrashMoveStatus,
    /// Why it was skipped or failed, in one sentence.
    pub message: Option<String>,
}

/// A cleanup session: one applied plan, or one "Keep this one" action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CleanupSession {
    /// The session, for undo.
    pub id: ResolutionId,
    /// Its name.
    pub name: String,
    /// When it was applied.
    pub at: Timestamp,
    /// Where it came from.
    pub kind: SessionKind,
    /// The apply was cancelled: the session holds what was done before.
    pub cancelled: bool,
    /// Still being applied.
    pub running: bool,
    /// Every moved file was put back.
    pub undone: bool,
    /// The sets it touched.
    pub sets: u32,
    /// The files that are in the trash now.
    pub trashed: u32,
    /// The bytes those files free (saturating).
    pub frees: u32,
    /// Its files, in the order they were done.
    pub files: Vec<SessionFile>,
}

/// Undo a session whole or some of its files
/// ([`crate::LibraryService::undo_cleanup_session`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UndoCleanupRequest {
    /// The session.
    pub session: ResolutionId,
    /// The files to put back; every file of the session when empty.
    #[serde(default)]
    pub paths: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plan_of_another_format_or_with_a_file_twice_is_refused() {
        let file = CleanupFile {
            path: "/a.ttf".into(),
            size: 1,
            hash: "00".into(),
            faces: vec![FaceId(1)],
            set: "identical:00".into(),
            level: DuplicateLevel::Identical,
            action: CleanupAction::Trash,
            reason: None,
            kept: Some("/b.ttf".into()),
            also_kept: Vec::new(),
            sentence: String::new(),
            activation: None,
        };
        let mut plan = CleanupPlan {
            version: CLEANUP_PLAN_VERSION,
            planned_at: "2026-11-21T00:00:00Z".into(),
            request: PlanCleanupRequest::default(),
            sets: 1,
            files: vec![file.clone()],
            conflicts: Vec::new(),
            frees: 1,
            my_languages: MyLanguagesStatus::NotSet,
        };
        assert!(plan.check().is_ok());
        let text = serde_json::to_string(&plan).unwrap();
        let back: CleanupPlan = serde_json::from_str(&text).unwrap();
        assert_eq!(back, plan);
        plan.files.push(file);
        assert!(plan.check().is_err());
        plan.files.pop();
        plan.version = 2;
        assert!(plan.check().is_err());
    }

    #[test]
    fn scopes_are_tagged_by_kind() {
        let s: CleanupScope = serde_json::from_str(
            r#"{"kind":"folderPair","keep":"/a","remove":"/b","otherLevels":true}"#,
        )
        .unwrap();
        assert_eq!(
            s,
            CleanupScope::FolderPair {
                keep: "/a".into(),
                remove: "/b".into(),
                other_levels: true
            }
        );
        let r: PlanCleanupRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(r.scope, CleanupScope::Library);
        assert_eq!(r.remove_as, RemoveAs::Trash);
    }
}
