//! Export service: web fonts with CSS, framework snippets, the license
//! inventory, one glyph's outline as SVG, and a family's specimen.

use serde::{Deserialize, Serialize};

use crate::common::AxisValue;
use crate::{ApiResult, CollectionId, FaceId, FamilyId};

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
    /// Replace files of the same name in `out_dir` (`fm export --force`).
    /// Without it, a file that exists refuses the export with
    /// `InvalidRequest` and nothing is written.
    #[serde(default)]
    pub overwrite: bool,
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

/// The file format of a specimen (E21.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SpecimenFormat {
    /// One self-contained HTML page: the faces are embedded, nothing is
    /// loaded from anywhere else.
    Html,
    /// The same page printed by the app's webview to a paginated A4 PDF
    /// (E21.2); `Unsupported` without the app running (the CLI's direct link).
    Pdf,
}

/// Write a specimen of one family to a file the user picked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SpecimenRequest {
    /// The family.
    pub family: FamilyId,
    /// The file format.
    pub format: SpecimenFormat,
    /// The file to write, as the user picked it.
    pub out_path: String,
}

/// Write one specimen of a collection to a file the user picked (E21.3): a
/// cover, then a page per family, for a client to sign off on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSpecimenRequest {
    /// The collection, manual or smart.
    pub collection: CollectionId,
    /// The file format.
    pub format: SpecimenFormat,
    /// The file to write, as the user picked it.
    pub out_path: String,
}

/// What the specimen export wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SpecimenResponse {
    /// The file written, absolute path.
    pub path: String,
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
    /// Write a specimen of the family to `out_path`. A face whose embedding
    /// is restricted while no license is declared for the family refuses
    /// the export with `PermissionDenied`; an unknown family is `NotFound`;
    /// `Pdf` without the app running is `Unsupported`, and a failed print
    /// is `Io` and leaves nothing at `out_path`.
    fn export_specimen(&self, req: SpecimenRequest) -> ApiResult<SpecimenResponse>;
    /// Write a specimen of the collection to `out_path`: a cover (name,
    /// date, family count), then a page per family with a face in it, in
    /// the collection's order; a smart collection applies its rule. An
    /// empty collection is `InvalidRequest`, an unknown one `NotFound`; the
    /// license refusal, `Unsupported` and `Io` are [`Self::export_specimen`]'s.
    fn export_collection_specimen(
        &self,
        req: CollectionSpecimenRequest,
    ) -> ApiResult<SpecimenResponse>;
}
