//! Preview service: rendered thumbnails (CLI, MCP, cache) and the font file
//! the webview loads through the asset protocol. The UI list renders text with
//! the webview's own engine; it does not use `render_preview`. What text a row
//! shows when a face lacks characters of the sample comes from `sample_texts`.

use serde::{Deserialize, Serialize};

use crate::common::{AxisValue, FeatureSetting};
use crate::{ApiResult, FaceId};

/// Render a line of text in one face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRequest {
    /// The face.
    pub face: FaceId,
    /// The text.
    pub text: String,
    /// Size in pixels (8–300).
    pub size_px: f32,
    /// Variation coordinates; defaults for unlisted axes.
    #[serde(default)]
    pub variations: Vec<AxisValue>,
    /// Features switched on or off.
    #[serde(default)]
    pub features: Vec<FeatureSetting>,
    /// Text color, CSS hex (`#1a1a1a`); black when absent.
    #[serde(default)]
    pub foreground: Option<String>,
    /// Background color, CSS hex; transparent when absent.
    #[serde(default)]
    pub background: Option<String>,
}

/// A rendered preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviewImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// PNG bytes.
    pub png: Vec<u8>,
}

/// Where the webview loads a face from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FontSource {
    /// The face.
    pub face: FaceId,
    /// Absolute file path, served by the asset protocol.
    pub path: String,
    /// Index inside a collection file.
    pub collection_index: u32,
    /// blake3 hash, so the UI can cache by content.
    pub hash: String,
}

/// The text to preview some faces with, for one sample text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SampleTextRequest {
    /// The faces.
    pub faces: Vec<FaceId>,
    /// The sample text the user chose.
    pub text: String,
}

/// Why a face previews other text than the sample.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SampleFallback {
    /// Letters of a script the face covers.
    Script {
        /// ISO 15924 script (`Cyrl`).
        script: String,
    },
    /// Characters the face maps, for a face that covers no language.
    Symbols,
}

/// The text one face previews.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SampleText {
    /// The face.
    pub face: FaceId,
    /// The sample text, or text from the face's own coverage.
    pub text: String,
    /// Set when the face lacks characters of the sample and `text` is other text.
    #[serde(default)]
    pub fallback: Option<SampleFallback>,
    /// The characters of the sample the face lacks.
    #[serde(default)]
    pub missing: Vec<String>,
}

/// Thumbnails and font files for the webview.
pub trait PreviewService {
    /// Rasterize text in a face (cached by face hash and settings).
    fn render_preview(&self, req: PreviewRequest) -> ApiResult<PreviewImage>;
    /// The file behind a face, for `@font-face`.
    fn font_source(&self, face: FaceId) -> ApiResult<FontSource>;
    /// The text each face previews: the sample when the face covers it, else
    /// text from the face's own coverage (one answer per face, in order).
    fn sample_texts(&self, req: SampleTextRequest) -> ApiResult<Vec<SampleText>>;
}
