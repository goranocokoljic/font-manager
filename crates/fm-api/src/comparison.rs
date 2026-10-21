//! Named comparisons (E13.2): a Compare sheet's faces, mode and sample text
//! kept under a name, so it can be reopened later.

use serde::{Deserialize, Serialize};

use crate::ApiResult;
use crate::ids::{ComparisonId, FaceId};
use crate::library::FaceSummary;

/// Fewest faces a comparison holds.
pub const MIN_COMPARISON_FACES: usize = 2;
/// Most faces a comparison holds (the Compare sheet's pins).
pub const MAX_COMPARISON_FACES: usize = 4;

/// How the Compare sheet shows its faces.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CompareMode {
    /// One column per face.
    #[default]
    SideBySide,
    /// One row per face.
    Stacked,
    /// Two faces drawn on one baseline.
    Overlay,
}

/// A saved comparison as the list shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonEntry {
    /// Id.
    pub id: ComparisonId,
    /// The name the user gave it.
    pub name: String,
    /// The mode it was saved in.
    pub mode: CompareMode,
    /// How many faces it was saved with, missing ones included.
    pub face_count: u32,
}

/// A face a saved comparison names that the library no longer shows (its
/// file was removed, or its folder is paused).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MissingFace {
    /// The face's id when it was saved.
    pub id: FaceId,
    /// Its family and style name when it was saved ("Roboto Bold").
    pub name: String,
}

/// A saved comparison, ready to reopen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    /// Id.
    pub id: ComparisonId,
    /// The name the user gave it.
    pub name: String,
    /// The mode it was saved in.
    pub mode: CompareMode,
    /// The sample text it was saved with; empty means each face shows its
    /// own name.
    pub sample_text: String,
    /// The faces the library still shows, in the saved order.
    pub faces: Vec<FaceSummary>,
    /// The saved faces the library no longer shows, in the saved order.
    pub missing: Vec<MissingFace>,
}

/// Saves a comparison: a new one, or over an existing one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SaveComparisonRequest {
    /// The comparison to overwrite; none saves a new one.
    #[serde(default)]
    pub id: Option<ComparisonId>,
    /// Its name; must not be empty (spaces around it are dropped).
    pub name: String,
    /// 2–4 distinct faces, in the order the sheet shows them.
    pub faces: Vec<FaceId>,
    /// The mode.
    #[serde(default)]
    pub mode: CompareMode,
    /// The sample text; empty means each face shows its own name.
    #[serde(default)]
    pub sample_text: String,
}

/// Names one saved comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonRef {
    /// Id.
    pub id: ComparisonId,
}

/// Gives a saved comparison a new name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RenameComparisonRequest {
    /// Id.
    pub id: ComparisonId,
    /// The new name; must not be empty.
    pub name: String,
}

/// Named comparisons, kept in the index database.
pub trait ComparisonService {
    /// Every saved comparison, by name (ignoring case), then by id.
    fn list_comparisons(&self) -> ApiResult<Vec<ComparisonEntry>>;
    /// One saved comparison: the faces the library still shows, and the
    /// ones it no longer shows by the name they were saved with.
    fn get_comparison(&self, req: ComparisonRef) -> ApiResult<Comparison>;
    /// Saves a comparison. Fewer than 2 or more than 4 faces, a face named
    /// twice, or an empty name is `InvalidRequest`; an unknown face or
    /// comparison is `NotFound`. Nothing is saved on an error.
    fn save_comparison(&self, req: SaveComparisonRequest) -> ApiResult<Comparison>;
    /// Renames a saved comparison; an empty name is `InvalidRequest`, an
    /// unknown comparison `NotFound`.
    fn rename_comparison(&self, req: RenameComparisonRequest) -> ApiResult<ComparisonEntry>;
    /// Deletes a saved comparison; the faces are not touched. An unknown
    /// comparison is `NotFound`.
    fn delete_comparison(&self, req: ComparisonRef) -> ApiResult<()>;
}
