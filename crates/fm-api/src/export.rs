//! Export service: web fonts with CSS, and the license inventory.

use serde::{Deserialize, Serialize};

use crate::{ApiResult, CollectionId, FaceId};

/// A web font format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum WebFormat {
    /// WOFF 2.
    Woff2,
    /// WOFF 1.
    Woff,
}

/// Which characters to keep when subsetting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Subset {
    /// Named presets (`latin`, `latin-ext`, `cyrillic`, `greek`, `vietnamese`).
    #[serde(rename_all = "camelCase")]
    Presets {
        /// Preset names.
        names: Vec<String>,
    },
    /// CSS `unicode-range` values (`U+0000-00FF`).
    #[serde(rename_all = "camelCase")]
    Unicode {
        /// Ranges.
        ranges: Vec<String>,
    },
    /// Every character of a sample text.
    #[serde(rename_all = "camelCase")]
    Text {
        /// The text.
        text: String,
    },
}

/// Convert faces to web fonts. Refused with `PermissionDenied` when the
/// embedding bits forbid it and no web license is declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportWebRequest {
    /// The faces.
    pub faces: Vec<FaceId>,
    /// Output formats.
    pub formats: Vec<WebFormat>,
    /// Subset; the full character set when absent.
    #[serde(default)]
    pub subset: Option<Subset>,
    /// Output folder the user chose.
    pub out_dir: String,
    /// Also write `fonts.css`.
    #[serde(default)]
    pub css: bool,
}

/// What the export wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportWebResponse {
    /// Files written, absolute paths.
    pub files: Vec<String>,
    /// The `@font-face` CSS, when asked for.
    pub css: Option<String>,
}

/// A license inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LicenseInventoryRequest {
    /// One collection, or the whole library when absent.
    #[serde(default)]
    pub collection: Option<CollectionId>,
}

/// A license inventory as CSV text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LicenseInventory {
    /// CSV with a header row.
    pub csv: String,
    /// Families listed.
    pub rows: u32,
}

/// Web export and license reports.
pub trait ExportService {
    /// Write web fonts (and CSS) to a folder.
    fn export_web(&self, req: ExportWebRequest) -> ApiResult<ExportWebResponse>;
    /// The license inventory.
    fn license_inventory(&self, req: LicenseInventoryRequest) -> ApiResult<LicenseInventory>;
}
