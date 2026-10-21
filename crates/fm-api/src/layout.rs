//! Layout service: body-text layouts (E12). A [`Layout`] is a list of text
//! blocks, each with its own size, spacing, columns and alignment; the six
//! presets are the starting points the UI offers. The core clamps every value
//! into its range, so a layout read from a file or sent by a script is always
//! one the UI can render.

use serde::{Deserialize, Serialize};

use crate::ApiResult;

/// A built-in layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LayoutPreset {
    /// A book page: a heading and indented paragraphs in one column.
    Book,
    /// Body text flowed into balanced columns.
    Columns,
    /// A headline, a standfirst and two-column body text.
    Magazine,
    /// A large pull quote and its attribution.
    Quote,
    /// The character set at a large size.
    Characters,
    /// The same text at falling sizes.
    Waterfall,
}

/// How the lines of a block line up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum BlockAlign {
    /// Left in left-to-right text.
    #[default]
    Start,
    /// Centered.
    Center,
    /// Right in left-to-right text.
    End,
    /// Both edges.
    Justify,
}

/// What a block does with text that does not fit its width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum BlockOverflow {
    /// Breaks the text into lines.
    #[default]
    Wrap,
    /// Keeps one line per paragraph and hides what overflows.
    Clip,
}

/// One block of text in a layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TextBlock {
    /// The text; paragraphs are separated by a line break.
    pub text: String,
    /// Font size in points, 8–600.
    pub size_pt: f32,
    /// Line height as a multiple of the size, 0.1–3.0.
    pub line_height: f32,
    /// Letter spacing in thousandths of an em, −200–1000.
    pub tracking: f32,
    /// Space after each paragraph in points, 0–600.
    pub paragraph_spacing_pt: f32,
    /// Indent of each paragraph's first line in em, 0–20.
    pub first_line_indent_em: f32,
    /// Columns the text flows into, 1–3.
    pub columns: u8,
    /// Alignment.
    pub align: BlockAlign,
    /// Wrap or clip.
    pub overflow: BlockOverflow,
}

/// A layout: a list of text blocks, top to bottom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    /// The preset this layout is, or started from; none for a layout of the user's own.
    #[serde(default)]
    pub preset: Option<LayoutPreset>,
    /// Display name ("Book").
    pub name: String,
    /// The blocks, top to bottom.
    pub blocks: Vec<TextBlock>,
}

/// Body-text layouts.
pub trait LayoutService {
    /// The six presets, in the order the UI offers them.
    fn layout_presets(&self) -> ApiResult<Vec<Layout>>;
    /// The layout with every block value clamped into its range (a value that
    /// is not a number takes the default).
    fn clamp_layout(&self, layout: Layout) -> ApiResult<Layout>;
}
