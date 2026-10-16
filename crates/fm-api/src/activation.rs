//! Activation service. The core activates through `fm-platform`'s
//! `FontActivator`; this is the contract adapters see. Results are per face,
//! never all-or-nothing. The app only deactivates faces it registered.

use serde::{Deserialize, Serialize};

use crate::common::Timestamp;
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
    /// Activate anyway for this session.
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
}
