//! Activation service. The core activates through `fm-platform`'s
//! `FontActivator`; this is the contract adapters see. Results are per face,
//! never all-or-nothing. The app only deactivates faces it registered.

use serde::{Deserialize, Serialize};

use crate::common::Timestamp;
use crate::library::{Problem, ProblemKind};
use crate::{ApiResult, CollectionId, FaceId, FamilyId, SetId};

/// How long an activation lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ActivationMode {
    /// Until the app quits or the user deactivates (the default).
    Session,
    /// Survives reboot; per-user, pointing at the file in place.
    Permanent,
}

/// What to act on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Target {
    /// These faces.
    #[serde(rename_all = "camelCase")]
    Faces {
        /// Face ids.
        faces: Vec<FaceId>,
    },
    /// Every face of a family.
    #[serde(rename_all = "camelCase")]
    Family {
        /// Family id.
        family: FamilyId,
    },
    /// Every face in a collection.
    #[serde(rename_all = "camelCase")]
    Collection {
        /// Collection id.
        collection: CollectionId,
    },
    /// Every face in a project set.
    #[serde(rename_all = "camelCase")]
    Set {
        /// Set id.
        set: SetId,
    },
}

/// What to do when a face clashes with a system or an already-active font.
/// The API never prompts; the UI asks the user and sends the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ConflictPolicy {
    /// Leave the clashing face inactive and report the conflict.
    #[default]
    KeepExisting,
    /// Activate anyway, for this session only: a clashing face is activated
    /// in session mode even when permanent was asked.
    Replace,
}

/// Activate faces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivateRequest {
    /// What to activate.
    pub target: Target,
    /// Session or permanent.
    #[serde(default = "session")]
    pub mode: ActivationMode,
    /// Conflict handling.
    #[serde(default)]
    pub on_conflict: ConflictPolicy,
}

fn session() -> ActivationMode {
    ActivationMode::Session
}

/// Deactivate faces the app registered.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DeactivateRequest {
    /// What to deactivate; everything the app activated when absent.
    #[serde(default)]
    pub target: Option<Target>,
}

/// The other side of a conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ConflictWith {
    /// A system font.
    #[serde(rename_all = "camelCase")]
    System {
        /// Its file.
        path: String,
    },
    /// A face already active from another source.
    #[serde(rename_all = "camelCase")]
    Active {
        /// That face.
        face: FaceId,
    },
}

/// What happened to one face.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum OutcomeStatus {
    /// Now active (or now inactive, for a deactivation).
    Done,
    /// Already in the requested state.
    Unchanged,
    /// Not done because of a clash (with `KeepExisting`).
    #[serde(rename_all = "camelCase")]
    Conflict {
        /// What it clashes with.
        with: ConflictWith,
    },
    /// Not done: the app did not register it, or the format cannot be activated.
    #[serde(rename_all = "camelCase")]
    Skipped {
        /// Why, in plain words.
        reason: String,
    },
    /// The OS call failed.
    #[serde(rename_all = "camelCase")]
    Failed {
        /// The error.
        error: crate::ApiError,
    },
}

/// One face's result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivationOutcome {
    /// The face.
    pub face: FaceId,
    /// What happened.
    pub outcome: OutcomeStatus,
}

/// A face the app has active.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActiveFace {
    /// The face.
    pub face: FaceId,
    /// Session or permanent.
    pub mode: ActivationMode,
    /// Since when.
    pub since: Timestamp,
    /// The document that asked for it (auto-activation), if any.
    pub document: Option<String>,
}

/// A clash found before activating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    /// The face that would be activated.
    pub face: FaceId,
    /// The shared PostScript name.
    pub postscript_name: String,
    /// What it clashes with.
    pub with: ConflictWith,
}

/// Which of the two fonts of a [`Clash`] the app puts in front.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ClashWinner {
    /// The clash's `face`.
    Face,
    /// The font in the clash's `with`.
    With,
}

/// Two active fonts with one PostScript name: one row of the Conflicts view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Clash {
    /// A face the app has active.
    pub face: FaceId,
    /// The shared PostScript name.
    pub postscript_name: String,
    /// What it clashes with: a system or user font, or another face the app
    /// has active (then `face` is the lower face id of the two).
    pub with: ConflictWith,
    /// The winner as the app decided it. The app's face wins over a system
    /// or user font: it is active over that font only because the user chose
    /// "replace for this session" (or it was active before that font came).
    /// Between two of the app's faces the later activation wins.
    pub winner: ClashWinner,
}

impl Clash {
    /// The Problems entry of the clash: `path` is the file of `face`,
    /// `other_path` the file of the other face of an `Active` clash (a
    /// `System` clash carries its own). Every implementation of
    /// [`LibraryService::list_problems`](crate::library::LibraryService::list_problems)
    /// uses this, so the rule is one.
    pub fn problem(&self, path: String, other_path: Option<String>) -> Problem {
        let name = &self.postscript_name;
        let (kind, other, message) = match &self.with {
            ConflictWith::System { path: system } => (
                ProblemKind::SystemConflict,
                Some(system.clone()),
                format!("{name} is active and has the same PostScript name as an installed font."),
            ),
            ConflictWith::Active { .. } => (
                ProblemKind::ActiveConflict,
                other_path,
                format!("{name} is active twice, from two different files."),
            ),
        };
        Problem {
            kind,
            face: Some(self.face),
            path,
            other,
            message,
        }
    }
}

/// Activation and deactivation.
pub trait ActivationService {
    /// Activate; one outcome per face.
    fn activate(&self, req: ActivateRequest) -> ApiResult<Vec<ActivationOutcome>>;
    /// Deactivate; faces the app did not register come back `Skipped`.
    fn deactivate(&self, req: DeactivateRequest) -> ApiResult<Vec<ActivationOutcome>>;
    /// Every face the app has active.
    fn list_active(&self) -> ApiResult<Vec<ActiveFace>>;
    /// Clashes the target would cause, without activating anything.
    fn conflicts(&self, target: Target) -> ApiResult<Vec<Conflict>>;
    /// Every clash of a face the app has active, with its winner (the
    /// Conflicts view). Changes nothing.
    fn list_conflicts(&self) -> ApiResult<Vec<Clash>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_system_clash_is_a_system_conflict_with_the_installed_file() {
        let clash = Clash {
            face: FaceId(6),
            postscript_name: "Helvetica".into(),
            with: ConflictWith::System {
                path: "/System/Library/Fonts/Helvetica.ttc".into(),
            },
            winner: ClashWinner::Face,
        };
        let p = clash.problem("/fonts/Helvetica.otf".into(), Some("ignored".into()));
        assert_eq!(p.kind, ProblemKind::SystemConflict);
        assert_eq!(p.face, Some(FaceId(6)));
        assert_eq!(p.path, "/fonts/Helvetica.otf");
        assert_eq!(
            p.other.as_deref(),
            Some("/System/Library/Fonts/Helvetica.ttc")
        );
        assert!(p.message.starts_with("Helvetica is active"));
    }

    #[test]
    fn an_active_clash_is_an_active_conflict_with_the_other_file() {
        let clash = Clash {
            face: FaceId(2),
            postscript_name: "Inter-Regular".into(),
            with: ConflictWith::Active { face: FaceId(9) },
            winner: ClashWinner::With,
        };
        let p = clash.problem("/a/Inter.ttf".into(), Some("/b/Inter.ttf".into()));
        assert_eq!(p.kind, ProblemKind::ActiveConflict);
        assert_eq!(p.path, "/a/Inter.ttf");
        assert_eq!(p.other.as_deref(), Some("/b/Inter.ttf"));
    }
}
