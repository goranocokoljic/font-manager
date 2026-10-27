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

/// Read the fonts a design document uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DocumentFontsRequest {
    /// The document. It is only read, never written or moved.
    pub path: String,
}

/// The kind of document, from its extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DocumentFormat {
    /// Microsoft Word (.docx).
    Docx,
    /// Microsoft PowerPoint (.pptx).
    Pptx,
    /// Sketch (.sketch).
    Sketch,
    /// Figma local copy (.fig).
    Fig,
    /// Adobe InDesign (.indd).
    Indd,
    /// Adobe Illustrator (.ai).
    Ai,
    /// Adobe Photoshop (.psd, .psb).
    Psd,
}

/// One font a document names. Each format stores a different part of the
/// name, so every part is optional; at least one is present.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct DocumentFontRef {
    /// The PostScript name ("Inter-Bold").
    #[serde(default)]
    pub postscript_name: Option<String>,
    /// The family name ("Inter").
    #[serde(default)]
    pub family: Option<String>,
    /// The style name ("Bold").
    #[serde(default)]
    pub style: Option<String>,
}

/// The fonts a document uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DocumentFonts {
    /// What kind of document it is.
    pub format: DocumentFormat,
    /// Every distinct font reference, sorted.
    pub fonts: Vec<DocumentFontRef>,
}

/// Composite workflows.
pub trait WorkflowService {
    /// A shortlist for a brief.
    fn pick_fonts_for_brief(&self, req: BriefRequest) -> ApiResult<Shortlist>;
    /// Activate, subset and return the CSS in one call.
    fn prepare_for_web(&self, req: PrepareForWebRequest) -> ApiResult<PrepareForWebResponse>;
    /// The fonts a design document (.docx, .pptx, .sketch, .fig, .indd, .ai,
    /// .psd) names. The document is only read.
    fn document_fonts(&self, req: DocumentFontsRequest) -> ApiResult<DocumentFonts>;
}
