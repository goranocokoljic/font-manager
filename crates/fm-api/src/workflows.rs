//! Workflow service: one call instead of many, for the MCP server and the CLI.
//! Each workflow is composed from the other services inside the core.

use serde::{Deserialize, Serialize};

use crate::activation::ActivationOutcome;
use crate::export::{ExportWebResponse, Subset};
use crate::library::{FamilySummary, LicenseType};
use crate::{ApiResult, FamilyId};

/// Pick fonts for a design brief.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BriefRequest {
    /// The brief, in plain words ("a calm bilingual brand").
    pub brief: String,
    /// Language presets every candidate must cover.
    #[serde(default)]
    pub languages: Vec<String>,
    /// Required license use.
    #[serde(default)]
    pub license: Option<LicenseType>,
    /// Candidates to return (default 5).
    #[serde(default)]
    pub limit: Option<u32>,
}

/// One candidate and why it fits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// The family.
    pub family: FamilySummary,
    /// Plain-word reasons, coverage and license checks included.
    pub reasons: Vec<String>,
}

/// The shortlist for a brief.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Shortlist {
    /// Best first.
    pub candidates: Vec<Candidate>,
}

/// Activate a family, subset it and return its CSS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PrepareForWebRequest {
    /// The family.
    pub family: FamilyId,
    /// Subset; the full character set when absent.
    #[serde(default)]
    pub subset: Option<Subset>,
    /// Output folder.
    pub out_dir: String,
}

/// What `prepare_for_web` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PrepareForWebResponse {
    /// Session activation, per face.
    pub activated: Vec<ActivationOutcome>,
    /// The export.
    pub export: ExportWebResponse,
}

/// Composite workflows.
pub trait WorkflowService {
    /// A shortlist for a brief.
    fn pick_fonts_for_brief(&self, req: BriefRequest) -> ApiResult<Shortlist>;
    /// Activate, subset and return the CSS in one call.
    fn prepare_for_web(&self, req: PrepareForWebRequest) -> ApiResult<PrepareForWebResponse>;
}
