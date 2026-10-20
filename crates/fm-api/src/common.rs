//! Small types shared by several services.

use serde::{Deserialize, Serialize};

/// A window into a long list. `limit` is capped by the service (500).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    /// Rows to skip.
    pub offset: u32,
    /// Rows to return.
    pub limit: u32,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 100,
        }
    }
}

/// One page of results and the size of the whole list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Paged<T> {
    /// The rows in this page.
    pub items: Vec<T>,
    /// Rows in the whole list.
    pub total: u32,
}

/// The file format of a font.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FontFormat {
    /// TrueType outlines (`.ttf`).
    Ttf,
    /// CFF outlines (`.otf`).
    Otf,
    /// TrueType collection (`.ttc`).
    Ttc,
    /// OpenType collection (`.otc`).
    Otc,
    /// WOFF 1.
    Woff,
    /// WOFF 2.
    Woff2,
    /// PostScript Type 1 (`.pfb`, `.pfa`): detected, not activated.
    Type1,
    /// Mac resource-fork font (`.dfont`): detected, not activated.
    Dfont,
    /// Bitmap-only font: detected, not activated.
    Bitmap,
}

/// Upright, italic or oblique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Slant {
    /// Upright.
    Upright,
    /// Italic.
    Italic,
    /// Oblique (slanted roman).
    Oblique,
}

/// Broad design class, used by filters and smart collections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Classification {
    /// Serif.
    Serif,
    /// Sans serif.
    Sans,
    /// Slab serif.
    Slab,
    /// Script and handwriting.
    Script,
    /// Display and decorative.
    Display,
    /// Monospace.
    Mono,
    /// Symbols, icons, dingbats.
    Symbol,
}

/// Whether a face can be used. Malformed font data is never a panic; it is
/// `Broken` with a reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind", content = "reason")]
pub enum Health {
    /// Parsed and usable.
    Ok,
    /// Truncated, missing required tables, or unreadable.
    Broken(String),
    /// Recognized but not supported for activation (Type 1, bitmap).
    Unsupported(String),
}

/// A value on one variation axis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AxisValue {
    /// Four-letter axis tag (`wght`).
    pub tag: String,
    /// The coordinate.
    pub value: f32,
}

/// A variation axis of a variable font.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Axis {
    /// Four-letter axis tag (`wght`, `wdth`, `opsz`, or a custom one).
    pub tag: String,
    /// Display name from the font.
    pub name: String,
    /// Lowest coordinate.
    pub min: f32,
    /// Default coordinate.
    pub default: f32,
    /// Highest coordinate.
    pub max: f32,
}

/// A named instance of a variable font, listed as a style.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NamedInstance {
    /// Style name ("Semibold").
    pub name: String,
    /// Its axis coordinates.
    pub coordinates: Vec<AxisValue>,
}

/// An OpenType layout feature the face provides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FeatureInfo {
    /// Four-letter feature tag (`smcp`, `ss01`).
    pub tag: String,
    /// Friendly name ("Small capitals"), or the font's own ssXX name.
    pub name: Option<String>,
    /// The shaper applies it by itself (`ccmp`, `mark`, `init`); a user
    /// cannot switch it on or off.
    #[serde(default)]
    pub automatic: bool,
    /// On in default text rendering (`kern`, `liga`, `calt`).
    #[serde(default)]
    pub default_on: bool,
}

/// A feature switched on or off for a preview or CSS output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSetting {
    /// Four-letter feature tag.
    pub tag: String,
    /// On or off.
    pub on: bool,
}

/// Vertical metrics in font units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    /// Units per em.
    pub units_per_em: u16,
    /// Typographic ascender.
    pub ascender: i16,
    /// Typographic descender (negative).
    pub descender: i16,
    /// x-height, when the font declares it.
    pub x_height: Option<i16>,
    /// Cap height, when the font declares it.
    pub cap_height: Option<i16>,
}

/// Embedding permissions from `OS/2.fsType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Embedding {
    /// Installable: no restriction.
    Installable,
    /// Editable embedding.
    Editable,
    /// Preview and print only.
    PreviewAndPrint,
    /// Restricted license embedding.
    Restricted,
}

/// A point in time, as an RFC 3339 string in UTC (`2026-10-16T09:30:00Z`).
pub type Timestamp = String;
