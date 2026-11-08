//! Layout service: body-text layouts (E12). A [`Layout`] is a list of text
//! blocks, each with its own size, spacing, columns and alignment; the six
//! presets are the starting points the UI offers. The core clamps every value
//! into its range, so a layout read from a file or sent by a script is always
//! one the UI can render.

use serde::{Deserialize, Serialize};

use crate::ApiResult;
use crate::common::{AxisValue, FeatureSetting};
use crate::ids::FaceId;

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

/// A Pairing Studio context (E45.1): where a pairing is tried out. Each one is a
/// layout template on a page of its own size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum StudioContext {
    /// A magazine spread page: kicker, headline, standfirst, two-column body, caption; A4.
    Magazine,
    /// A website's first screen at 1440 × 900.
    WebHero,
    /// A phone screen at 390 × 844 with the iOS text styles.
    MobileUi,
    /// A video title card at 1920 × 1080 with safe-area guides.
    VideoTitle,
    /// A poster: a very large headline and its details; A4.
    Poster,
}

/// The page a layout is set on (E45.1): the text keeps the page's inner width and
/// the page is shown at its real size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LayoutPage {
    /// What the page is called ("A4", "1440 × 900").
    pub name: String,
    /// Width in CSS pixels (96 per inch).
    pub width_px: u32,
    /// Height in CSS pixels.
    pub height_px: u32,
    /// The margin around the text on every side, in CSS pixels.
    pub margin_px: f32,
    /// Safe-area guides, each an inset from every edge as a share of the page's
    /// width and height (0.05 is title safe); none for a page without guides.
    #[serde(default)]
    pub safe_areas: Vec<f32>,
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

/// The range of one block value: what the core clamps it into and how far the
/// editor and the direct-manipulation gestures move it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BlockRange {
    /// Smallest value.
    pub min: f32,
    /// Largest value.
    pub max: f32,
    /// The value for one that is not a number.
    pub default: f32,
    /// How far one arrow-key press moves the value.
    pub step: f32,
    /// How far one CSS pixel of pointer drag moves the value.
    pub per_px: f32,
}

/// The range of [`TextBlock::columns`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ColumnRange {
    /// Fewest columns.
    pub min: u8,
    /// Most columns.
    pub max: u8,
}

/// The ranges [`LayoutService::clamp_layout`] clamps every block value into,
/// so the properties editor (E12.3) offers exactly those.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LayoutBounds {
    /// Font size in points.
    pub size_pt: BlockRange,
    /// Line height as a multiple of the size.
    pub line_height: BlockRange,
    /// Letter spacing in thousandths of an em.
    pub tracking: BlockRange,
    /// Space after each paragraph in points.
    pub paragraph_spacing_pt: BlockRange,
    /// Indent of each paragraph's first line in em.
    pub first_line_indent_em: BlockRange,
    /// Columns a block flows into.
    pub columns: ColumnRange,
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
    /// The text keeps a fixed width, centered, however wide the window is
    /// (E12.4); false makes it follow the window. A layout saved without it is locked.
    #[serde(default = "width_locked_default")]
    pub width_locked: bool,
    /// The width the text is locked at, in CSS pixels; none is a readable line
    /// length at the body block's size. Kept while the width is unlocked.
    #[serde(default)]
    pub width_px: Option<f32>,
    /// The page the layout is set on (E45.1); none sets the text in the window.
    #[serde(default)]
    pub page: Option<LayoutPage>,
    /// The Pairing Studio context this layout is, or started from (E45.1).
    #[serde(default)]
    pub context: Option<StudioContext>,
}

/// A layout's width is locked unless it says otherwise.
fn width_locked_default() -> bool {
    true
}

/// A layout template file in the layouts folder, as the list shows it (E12.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LayoutEntry {
    /// The file name without `.json` ("book"); stable while the file keeps its name.
    pub id: String,
    /// The layout's name, or the file name when the file cannot be read.
    pub name: String,
    /// The preset the layout is or started from.
    #[serde(default)]
    pub preset: Option<LayoutPreset>,
    /// Why the file cannot be used (not valid JSON, not a layout); none when it can.
    #[serde(default)]
    pub broken: Option<String>,
    /// The Pairing Studio context the layout is or started from (E45.1).
    #[serde(default)]
    pub context: Option<StudioContext>,
}

/// A layout as it is saved in its template file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SavedLayout {
    /// The template file's id.
    pub id: String,
    /// The saved layout, clamped into the block ranges.
    pub layout: Layout,
}

/// Names one layout template file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LayoutRef {
    /// The template file's id.
    pub id: String,
}

/// Writes a layout to its template file; the previous content is kept as a version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SaveLayoutRequest {
    /// The template file's id; a new id makes a new file.
    pub id: String,
    /// The layout; its values are clamped before they are written.
    pub layout: Layout,
}

/// Copies a layout into a new, independent template file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateLayoutRequest {
    /// The layout to copy.
    pub id: String,
    /// The copy's name; none gives "<name> copy" (then "<name> copy 2", …).
    #[serde(default)]
    pub name: Option<String>,
}

/// Goes back to the saved file, or a preset back to its shipped original.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RevertLayoutRequest {
    /// The template file's id.
    pub id: String,
    /// Writes the shipped original of the layout's preset (the content it
    /// replaces is kept as a version); false only reads the saved file.
    #[serde(default)]
    pub to_original: bool,
}

/// An earlier content of a template file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LayoutVersion {
    /// The version's number; a later save has a higher number.
    pub number: u32,
    /// When the content was replaced, UTC (`2026-10-21T09:30:00Z`).
    pub saved_at: String,
    /// The layout's name in that version, or none when that content was broken.
    #[serde(default)]
    pub name: Option<String>,
    /// Why that content cannot be used; none when it can.
    #[serde(default)]
    pub broken: Option<String>,
}

/// Makes an earlier version current again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RestoreLayoutRequest {
    /// The template file's id.
    pub id: String,
    /// The [`LayoutVersion::number`] to restore.
    pub number: u32,
}

/// The colours a layout is exported on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LayoutSurface {
    /// Near-black type on paper white.
    Paper,
    /// Light type on near-black.
    Ink,
}

/// The font a block is set in: a layout file holds no fonts, so an export names them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LayoutFont {
    /// The face.
    pub face: FaceId,
    /// Variation-axis values; axes not named keep their defaults.
    #[serde(default)]
    pub variations: Vec<AxisValue>,
    /// OpenType features switched on or off.
    #[serde(default)]
    pub features: Vec<FeatureSetting>,
}

/// Renders a saved layout on an A4 page into the app's export folder (E45.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportLayoutPngRequest {
    /// The template file's id.
    pub layout: String,
    /// Device pixels per CSS pixel: 1 or 2.
    pub scale: u8,
    /// The page's colours.
    pub surface: LayoutSurface,
    /// The font of every block without its own.
    pub font: LayoutFont,
    /// The block's own font, by block index; a missing or null entry uses `font`.
    #[serde(default)]
    pub block_fonts: Vec<Option<LayoutFont>>,
}

/// Body-text layouts.
pub trait LayoutService {
    /// The six presets, in the order the UI offers them.
    fn layout_presets(&self) -> ApiResult<Vec<Layout>>;
    /// The layout with every block value clamped into its range (a value that
    /// is not a number takes the default).
    fn clamp_layout(&self, layout: Layout) -> ApiResult<Layout>;
    /// The ranges `clamp_layout` clamps every block value into, with the
    /// steps an editor moves them by (E12.3).
    fn layout_bounds(&self) -> ApiResult<LayoutBounds>;
    /// Every template file in the layouts folder, read again on each call:
    /// the presets first in picker order, then the others by name. The six
    /// presets are written there on first use and never overwritten. A file
    /// that cannot be read is listed with the reason.
    fn list_layouts(&self) -> ApiResult<Vec<LayoutEntry>>;
    /// One template file; a broken one answers `InvalidRequest` with the reason.
    fn get_layout(&self, req: LayoutRef) -> ApiResult<SavedLayout>;
    /// Writes a layout; the content it replaces becomes the newest version.
    fn save_layout(&self, req: SaveLayoutRequest) -> ApiResult<SavedLayout>;
    /// Copies a layout into a new template file with a new name and id.
    fn duplicate_layout(&self, req: DuplicateLayoutRequest) -> ApiResult<SavedLayout>;
    /// The saved file again (drops the editor's unsaved changes), or with
    /// `to_original` the shipped original of the layout's preset.
    fn revert_layout(&self, req: RevertLayoutRequest) -> ApiResult<SavedLayout>;
    /// The kept versions of a template file, newest first.
    fn layout_history(&self, req: LayoutRef) -> ApiResult<Vec<LayoutVersion>>;
    /// Makes a version current; the content it replaces becomes a version too.
    fn restore_layout(&self, req: RestoreLayoutRequest) -> ApiResult<SavedLayout>;
    /// The width in CSS pixels the layout's text is set at: its locked width,
    /// or a readable line length at the body block's size when it has none;
    /// none when the width is unlocked and follows the window (E12.4).
    fn layout_width(&self, layout: Layout) -> ApiResult<Option<f32>>;
    /// Renders a saved layout to a new PNG in the app's export folder with the
    /// core rasterizer, and answers the file's absolute path. An existing
    /// export is never replaced.
    fn export_layout_png(&self, req: ExportLayoutPngRequest) -> ApiResult<String>;
}
