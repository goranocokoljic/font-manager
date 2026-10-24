//! Export service: web fonts with CSS, framework snippets, the license
//! inventory, and one glyph's outline as SVG.

use serde::{Deserialize, Serialize};

use crate::common::AxisValue;
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

/// One glyph's outline as SVG (E11.5). The SVG text is returned, never
/// written: saving it goes to a path the user picks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GlyphSvgRequest {
    /// The face.
    pub face: FaceId,
    /// The character, as a Unicode code point.
    pub code_point: u32,
    /// Variation coordinates; defaults for unlisted axes.
    #[serde(default)]
    pub variations: Vec<AxisValue>,
}

/// A front-end framework a snippet is written for (E20.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Framework {
    /// A `theme.extend.fontFamily` entry for `tailwind.config.js`.
    Tailwind,
    /// CSS custom properties on `:root`.
    CssVariables,
    /// A `localFont` call from `next/font/local`.
    NextLocalFont,
}

/// A snippet that uses the faces in a framework. Nothing is written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FrameworkSnippetRequest {
    /// The faces; at least one.
    pub faces: Vec<FaceId>,
    /// The framework.
    pub framework: Framework,
    /// The folder the web fonts are in, as the project sees it; `./fonts`
    /// when absent. Only `NextLocalFont` uses it.
    #[serde(default)]
    pub fonts_path: Option<String>,
}

/// A framework snippet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FrameworkSnippet {
    /// The snippet's text.
    pub text: String,
}

/// Web export, framework snippets, license reports and glyph outlines.
pub trait ExportService {
    /// Write web fonts (and CSS) to a folder.
    fn export_web(&self, req: ExportWebRequest) -> ApiResult<ExportWebResponse>;
    /// A Tailwind, CSS custom property or Next.js `localFont` snippet for
    /// the faces' families, as text. An empty face list is `InvalidRequest`,
    /// an unknown face `NotFound`.
    fn framework_snippet(&self, req: FrameworkSnippetRequest) -> ApiResult<FrameworkSnippet>;
    /// The license inventory.
    fn license_inventory(&self, req: LicenseInventoryRequest) -> ApiResult<LicenseInventory>;
    /// The outline of one character as SVG text, upright, in font units: the
    /// viewBox is the advance width by ascender to descender. A character
    /// with no outline (a space) is an SVG with an empty path; a character
    /// the face does not map is `NotFound`.
    fn export_glyph_svg(&self, req: GlyphSvgRequest) -> ApiResult<String>;
}
