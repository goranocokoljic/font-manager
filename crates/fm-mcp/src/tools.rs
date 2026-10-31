//! The MCP tools: one `fm-api` call each (E27.2, E27.3, E27.4).
//!
//! Each tool has an input struct whose JSON schema (`rmcp::schemars`) is the
//! tool's `inputSchema`. A call decodes its arguments into that struct,
//! builds the API request, calls the API once and returns the API's own JSON
//! as the result. Results carry ids and metadata, never font bytes: the API
//! has no call that returns a font file, and `export_web` answers with the
//! paths it wrote and the CSS.
//!
//! The two preview tools answer with a picture instead: the PNG the API
//! rendered, as MCP image content (`image/png`, base64), then a text block
//! naming the faces in row order (one `get_face` per face for the names).
//!
//! The workflow tools (E27.4) are one call too: `pick_fonts_for_brief` and
//! `prepare_for_web` call the API's workflow service, `build_specimen` its
//! specimen export. A workflow that changes something asks the gate for
//! every grant it needs ([`grants`]): `prepare_for_web` activates and
//! exports, `build_specimen` writes a file.
//!
//! An [`ApiError`] becomes a tool error (`isError: true`) whose text is the
//! error's user message and whose structured content is the error itself.
//! The mutating tools ask the [`ToolGate`] first, after their arguments are
//! read: a call with bad arguments is refused before the user is asked.

use std::collections::BTreeMap;
use std::path::Path;

use fm_api::activation::{
    ActivateRequest, ActivationMode, ConflictPolicy, DeactivateRequest, Target,
};
use fm_api::common::{AxisValue, Classification, FeatureSetting, Page};
use fm_api::export::{ExportWebRequest, SpecimenFormat, SpecimenRequest, Subset, WebFormat};
use fm_api::library::LicenseType;
use fm_api::organization::TagFacesRequest;
use fm_api::preview::{
    ComparePreviewRequest, MAX_COMPARE_TEXT_CHARS, PreviewImage, PreviewRequest,
};
use fm_api::search::{
    CoverageRequest, Filters, PairingRequest, SearchRequest, SimilarRequest, Supports,
};
use fm_api::settings::McpToolCall;
use fm_api::workflows::{BriefRequest, PrepareForWebRequest};
use fm_api::{Api, ApiError, CollectionId, ErrorCode, FaceId, FamilyId, SetId};
use rmcp::handler::server::common::schema_for_type;
use rmcp::model::{CallToolResult, ContentBlock, JsonObject, Tool, ToolAnnotations};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::gate::ToolGate;

/// Search the library.
pub const SEARCH_FONTS: &str = "search_fonts";
/// One face's details.
pub const GET_FONT: &str = "get_font";
/// The collections.
pub const LIST_COLLECTIONS: &str = "list_collections";
/// The project sets.
pub const LIST_SETS: &str = "list_sets";
/// Language or text coverage of one face.
pub const CHECK_COVERAGE: &str = "check_coverage";
/// Faces that look like one face.
pub const FIND_SIMILAR: &str = "find_similar";
/// Faces that pair well with one face.
pub const GET_PAIRINGS: &str = "get_pairings";
/// One face's license fields.
pub const GET_LICENSE: &str = "get_license";
/// Activate faces.
pub const ACTIVATE_FONTS: &str = "activate_fonts";
/// Deactivate faces the app activated.
pub const DEACTIVATE_FONTS: &str = "deactivate_fonts";
/// Tag faces.
pub const TAG_FONTS: &str = "tag_fonts";
/// Export web fonts with CSS.
pub const EXPORT_WEB: &str = "export_web";
/// A line of text rendered in one face, as a PNG.
pub const RENDER_PREVIEW: &str = "render_preview";
/// The same line rendered in 2–4 faces, one row each, as one PNG.
pub const COMPARE_PREVIEW: &str = "compare_preview";
/// A shortlist of families for a design brief.
pub const PICK_FONTS_FOR_BRIEF: &str = "pick_fonts_for_brief";
/// Activate a family and export it as subset WOFF2 with CSS.
pub const PREPARE_FOR_WEB: &str = "prepare_for_web";
/// Write a family's HTML specimen into a folder.
pub const BUILD_SPECIMEN: &str = "build_specimen";

/// Every tool, in the order `tools/list` gives them.
pub const NAMES: [&str; 17] = [
    SEARCH_FONTS,
    GET_FONT,
    LIST_COLLECTIONS,
    LIST_SETS,
    CHECK_COVERAGE,
    FIND_SIMILAR,
    GET_PAIRINGS,
    GET_LICENSE,
    ACTIVATE_FONTS,
    DEACTIVATE_FONTS,
    TAG_FONTS,
    EXPORT_WEB,
    RENDER_PREVIEW,
    COMPARE_PREVIEW,
    PICK_FONTS_FOR_BRIEF,
    PREPARE_FOR_WEB,
    BUILD_SPECIMEN,
];

/// The tools that change something and have a grant of their own; they go
/// through the [`ToolGate`].
pub const MUTATING: [&str; 4] = [ACTIVATE_FONTS, DEACTIVATE_FONTS, TAG_FONTS, EXPORT_WEB];

/// The workflow tools that change something, with the grants of
/// [`MUTATING`] each needs; every one must be given.
pub const WORKFLOW_GRANTS: [(&str, &[&str]); 2] = [
    (PREPARE_FOR_WEB, &[ACTIVATE_FONTS, EXPORT_WEB]),
    (BUILD_SPECIMEN, &[EXPORT_WEB]),
];

/// The grants tool `name` needs: its own for a [`MUTATING`] tool, those of
/// [`WORKFLOW_GRANTS`] for a workflow, none for a read tool.
pub fn grants(name: &str) -> Vec<&'static str> {
    if let Some(own) = MUTATING.iter().find(|t| **t == name) {
        return vec![*own];
    }
    WORKFLOW_GRANTS
        .iter()
        .find(|(tool, _)| *tool == name)
        .map(|(_, needs)| needs.to_vec())
        .unwrap_or_default()
}

/// A font class.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(crate = "rmcp::schemars")]
pub enum ClassInput {
    /// Serif.
    Serif,
    /// Sans serif.
    Sans,
    /// Slab serif.
    Slab,
    /// Script.
    Script,
    /// Display.
    Display,
    /// Monospaced.
    Mono,
    /// Symbol.
    Symbol,
}

impl From<ClassInput> for Classification {
    fn from(class: ClassInput) -> Self {
        match class {
            ClassInput::Serif => Self::Serif,
            ClassInput::Sans => Self::Sans,
            ClassInput::Slab => Self::Slab,
            ClassInput::Script => Self::Script,
            ClassInput::Display => Self::Display,
            ClassInput::Mono => Self::Mono,
            ClassInput::Symbol => Self::Symbol,
        }
    }
}

/// A web font format.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(crate = "rmcp::schemars")]
pub enum WebFormatInput {
    /// WOFF 2.
    Woff2,
    /// WOFF 1.
    Woff,
}

/// A license use.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(crate = "rmcp::schemars")]
pub enum LicenseUseInput {
    /// Desktop use.
    Desktop,
    /// Web embedding.
    Web,
    /// App embedding.
    App,
}

impl From<LicenseUseInput> for LicenseType {
    fn from(use_: LicenseUseInput) -> Self {
        match use_ {
            LicenseUseInput::Desktop => Self::Desktop,
            LicenseUseInput::Web => Self::Web,
            LicenseUseInput::App => Self::App,
        }
    }
}

impl From<WebFormatInput> for WebFormat {
    fn from(format: WebFormatInput) -> Self {
        match format {
            WebFormatInput::Woff2 => Self::Woff2,
            WebFormatInput::Woff => Self::Woff,
        }
    }
}

/// `search_fonts`: a library search, grouped by family.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SearchFontsInput {
    /// Free text over family, style, PostScript name, foundry, designer, tags
    /// and file name; typo-tolerant. Empty lists everything that passes the
    /// filters.
    pub query: String,
    /// Language preset codes every result must cover (`sr-Cyrl`, `pl`).
    pub languages: Vec<String>,
    /// A text every character of which the results must cover.
    pub text: Option<String>,
    /// Any of these classes.
    pub classification: Vec<ClassInput>,
    /// Variable (true) or static (false) faces only.
    pub variable: Option<bool>,
    /// Active (true) or inactive (false) faces only.
    pub active: Option<bool>,
    /// Faces in this collection only.
    pub collection_id: Option<u32>,
    /// Family groups to return (default 20).
    pub limit: Option<u32>,
    /// Family groups to skip.
    pub offset: u32,
}

/// One face, by id.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct FaceInput {
    /// The face id (from `search_fonts`).
    pub face_id: u32,
}

/// No arguments.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct NoInput {}

/// `check_coverage`: whether one face covers some languages or a text.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct CheckCoverageInput {
    /// The face id.
    pub face_id: u32,
    /// Language preset codes (`sr-Cyrl`, `pl`). Give these or `text`.
    #[serde(default)]
    pub languages: Vec<String>,
    /// A text whose every character the face must cover. Give this or
    /// `languages`.
    #[serde(default)]
    pub text: Option<String>,
}

/// One face and how many results to return.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct FaceLimitInput {
    /// The face id.
    pub face_id: u32,
    /// Results to return (default 10).
    #[serde(default)]
    pub limit: Option<u32>,
}

/// `activate_fonts`: what to activate, and how. Give exactly one of
/// `face_ids`, `family_id`, `collection_id`, `set_id`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ActivateFontsInput {
    /// These faces.
    pub face_ids: Vec<u32>,
    /// Every face of this family.
    pub family_id: Option<u32>,
    /// Every face in this collection.
    pub collection_id: Option<u32>,
    /// Every face in this project set.
    pub set_id: Option<u32>,
    /// Keep the fonts active after a restart (default: this session only).
    pub permanent: bool,
    /// Activate a face that clashes with a system or active font anyway, for
    /// this session (default: leave it inactive and report the conflict).
    pub replace_conflicts: bool,
}

/// `deactivate_fonts`: what to deactivate. Only faces the app activated are
/// touched. Give at most one of `face_ids`, `family_id`, `collection_id`,
/// `set_id`; none deactivates everything the app activated.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct DeactivateFontsInput {
    /// These faces.
    pub face_ids: Vec<u32>,
    /// Every face of this family.
    pub family_id: Option<u32>,
    /// Every face in this collection.
    pub collection_id: Option<u32>,
    /// Every face in this project set.
    pub set_id: Option<u32>,
}

/// `tag_fonts`: tags to add to faces or whole families.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct TagFontsInput {
    /// The faces.
    pub face_ids: Vec<u32>,
    /// Whole families: every face of each.
    pub family_ids: Vec<u32>,
    /// Tag names; a new name creates the tag.
    pub tags: Vec<String>,
}

/// `export_web`: web fonts and `@font-face` CSS written to a folder.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ExportWebInput {
    /// The faces.
    pub face_ids: Vec<u32>,
    /// The folder to write to, an absolute path.
    pub out_dir: String,
    /// Output formats (default `["woff2"]`).
    #[serde(default)]
    pub formats: Vec<WebFormatInput>,
    /// Also write `fonts.css` and return the CSS (default true).
    #[serde(default = "yes")]
    pub css: bool,
    /// Subset presets to keep (`latin`, `latin-ext`, `cyrillic`, `greek`,
    /// `vietnamese`). Give these, `subset_text`, or neither for every
    /// character.
    #[serde(default)]
    pub subset_presets: Vec<String>,
    /// Keep only the characters of this text.
    #[serde(default)]
    pub subset_text: Option<String>,
    /// Replace files of the same name in `out_dir` (default: refuse).
    #[serde(default)]
    pub overwrite: bool,
}

fn yes() -> bool {
    true
}

/// `pick_fonts_for_brief`: a shortlist of families for a design brief.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct PickFontsForBriefInput {
    /// The brief in plain words ("a calm bilingual brand, sans serif").
    /// Words naming a design class, a tag or a name rank families higher.
    pub brief: String,
    /// Language preset codes every family must cover (`sr-Cyrl`, `pl`).
    #[serde(default)]
    pub languages: Vec<String>,
    /// The use every family's license must allow: a recognized open
    /// license or a declared license of this use.
    #[serde(default)]
    pub license: Option<LicenseUseInput>,
    /// Families to return, 1 to 50 (default 5).
    #[serde(default)]
    pub limit: Option<u32>,
}

/// `prepare_for_web`: one family activated for the session and exported as
/// WOFF2 with `fonts.css`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct PrepareForWebInput {
    /// The family id (from `search_fonts` or `pick_fonts_for_brief`).
    pub family_id: u32,
    /// The folder to write to, an absolute path.
    pub out_dir: String,
    /// Subset presets to keep (`latin`, `latin-ext`, `cyrillic`, `greek`,
    /// `vietnamese`). Give these, `subset_text`, or neither for every
    /// character.
    #[serde(default)]
    pub subset_presets: Vec<String>,
    /// Keep only the characters of this text.
    #[serde(default)]
    pub subset_text: Option<String>,
}

/// `build_specimen`: a family's self-contained HTML specimen, written into
/// a folder as `specimen-<family_id>.html`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct BuildSpecimenInput {
    /// The family id.
    pub family_id: u32,
    /// The folder to write to, an absolute path.
    pub out_dir: String,
    /// Replace a specimen of the same name in `out_dir` (default: refuse).
    #[serde(default)]
    pub overwrite: bool,
}

/// How a preview is drawn, beyond the text and size.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct PreviewOptions {
    /// Variation axis values by tag (`{"wght": 700, "wdth": 90}`); axes left
    /// out stay at their default, axes a face lacks are ignored.
    pub variations: BTreeMap<String, f32>,
    /// OpenType features switched on (true) or off (false) by tag
    /// (`{"liga": false, "smcp": true}`).
    pub features: BTreeMap<String, bool>,
    /// Text colour, CSS hex (`#1a1a1a`); black when absent.
    pub foreground: Option<String>,
    /// Background colour, CSS hex; transparent when absent.
    pub background: Option<String>,
}

impl PreviewOptions {
    fn variations(&self) -> Vec<AxisValue> {
        self.variations
            .iter()
            .map(|(tag, value)| AxisValue {
                tag: tag.clone(),
                value: *value,
            })
            .collect()
    }

    fn features(&self) -> Vec<FeatureSetting> {
        self.features
            .iter()
            .map(|(tag, on)| FeatureSetting {
                tag: tag.clone(),
                on: *on,
            })
            .collect()
    }
}

/// `render_preview`: a line of text in one face.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct RenderPreviewInput {
    /// The face id (from `search_fonts`).
    pub face_id: u32,
    /// The text, at most 500 characters.
    pub text: String,
    /// Size in pixels, 8 to 300.
    pub size: f32,
    /// Axis values, features and colours.
    #[serde(default)]
    pub options: PreviewOptions,
}

/// `compare_preview`: the same line of text in 2–4 faces.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ComparePreviewInput {
    /// The face ids, 2 to 4, top to bottom.
    pub face_ids: Vec<u32>,
    /// The text, at most 500 characters.
    pub text: String,
    /// Size in pixels, 8 to 300.
    pub size: f32,
    /// Axis values, features and colours, the same for every face.
    #[serde(default)]
    pub options: PreviewOptions,
}

/// Results per page when a tool is not told.
const DEFAULT_SEARCH_LIMIT: u32 = 20;
const DEFAULT_LIMIT: u32 = 10;

fn tool<T: JsonSchema + 'static>(name: &'static str, description: &'static str) -> Tool {
    let mutating = !grants(name).is_empty();
    let annotations = ToolAnnotations::new()
        .read_only(!mutating)
        .destructive(false)
        .idempotent(![EXPORT_WEB, PREPARE_FOR_WEB, BUILD_SPECIMEN].contains(&name))
        .open_world(false);
    // An argument-less tool still lists `properties`, as MCP clients expect.
    let mut schema = (*schema_for_type::<T>()).clone();
    schema.entry("properties").or_insert_with(|| json!({}));
    Tool::new(name, description, schema).annotate(annotations)
}

/// Every tool's definition, in [`NAMES`] order.
pub fn list() -> Vec<Tool> {
    vec![
        tool::<SearchFontsInput>(
            SEARCH_FONTS,
            "Search the font library by text and filters. Returns family groups with face ids and metadata.",
        ),
        tool::<FaceInput>(
            GET_FONT,
            "Details of one face: names, file path, metrics, axes, features, scripts, license and tags.",
        ),
        tool::<NoInput>(LIST_COLLECTIONS, "The user's collections with their ids."),
        tool::<NoInput>(LIST_SETS, "The user's project sets with their ids."),
        tool::<CheckCoverageInput>(
            CHECK_COVERAGE,
            "Whether one face covers some language presets or every character of a text, and what it misses.",
        ),
        tool::<FaceLimitInput>(
            FIND_SIMILAR,
            "Faces that look like one face, closest first, with plain-word reasons.",
        ),
        tool::<FaceLimitInput>(
            GET_PAIRINGS,
            "Faces that pair well with one face, best first, with plain-word reasons.",
        ),
        tool::<FaceInput>(
            GET_LICENSE,
            "One face's license: the license stated in the font, its embedding permission, the license the user declared for the family, and whether it is OK for web use.",
        ),
        tool::<ActivateFontsInput>(
            ACTIVATE_FONTS,
            "Activate faces so other apps can use them. Reports what happened to each face.",
        ),
        tool::<DeactivateFontsInput>(
            DEACTIVATE_FONTS,
            "Deactivate faces the app activated. Faces it did not activate are left alone and reported as skipped.",
        ),
        tool::<TagFontsInput>(TAG_FONTS, "Add tags to faces or whole families."),
        tool::<ExportWebInput>(
            EXPORT_WEB,
            "Write web fonts (WOFF2/WOFF) and @font-face CSS to a folder, created when missing. Returns the paths written and the CSS.",
        ),
        tool::<RenderPreviewInput>(
            RENDER_PREVIEW,
            "Render a line of text in one face. Returns a PNG image and the face's name.",
        ),
        tool::<ComparePreviewInput>(
            COMPARE_PREVIEW,
            "Render the same line of text in 2 to 4 faces, one row per face, top to bottom in the order given. Returns one PNG image and the faces' names in row order.",
        ),
        tool::<PickFontsForBriefInput>(
            PICK_FONTS_FOR_BRIEF,
            "A shortlist of font families for a design brief, best first. Every family covers the languages asked for and allows the license use asked for; each comes with plain-word reasons.",
        ),
        tool::<PrepareForWebInput>(
            PREPARE_FOR_WEB,
            "Check a family's web license, activate it for this session and export it as subset WOFF2 with fonts.css. Returns what was activated, the paths written and the CSS. A license that does not allow the web activates and writes nothing.",
        ),
        tool::<BuildSpecimenInput>(
            BUILD_SPECIMEN,
            "Write a self-contained HTML specimen of one family into a folder, created when missing. Returns the file's path.",
        ),
    ]
}

/// Calls tool `name` with `arguments`; `None` when there is no such tool.
pub fn call(
    api: &dyn Api,
    gate: &dyn ToolGate,
    name: &str,
    arguments: JsonObject,
) -> Option<CallToolResult> {
    if !NAMES.contains(&name) {
        return None;
    }
    let arguments = Value::Object(arguments);
    if let Err(e) = tool_calls(name, &arguments)
        .and_then(|calls| calls.iter().try_for_each(|c| gate.check(api, c)))
    {
        return Some(api_error(&e));
    }
    let answer = match name {
        RENDER_PREVIEW | COMPARE_PREVIEW => preview(api, name, arguments),
        _ => dispatch(api, name, arguments).map(CallToolResult::structured),
    };
    Some(answer.unwrap_or_else(|e| api_error(&e)))
}

/// A tool error for `e`: its user message as text, the error as
/// structured content.
pub fn api_error(e: &ApiError) -> CallToolResult {
    let mut result = CallToolResult::error(vec![ContentBlock::text(e.message.clone())]);
    result.structured_content = serde_json::to_value(e).ok();
    result
}

/// What the gate is asked before tool `name` runs, one call per grant it
/// needs ([`grants`]; none for a read tool): the faces the call acts on,
/// read from the arguments as the call reads them.
fn tool_calls(name: &str, arguments: &Value) -> Result<Vec<McpToolCall>, ApiError> {
    let needs = grants(name);
    if needs.is_empty() {
        return Ok(Vec::new());
    }
    let target = match name {
        ACTIVATE_FONTS => {
            let a: ActivateFontsInput = args(name, arguments.clone())?;
            target(&a.face_ids, a.family_id, a.collection_id, a.set_id)?
        }
        DEACTIVATE_FONTS => {
            let a: DeactivateFontsInput = args(name, arguments.clone())?;
            target(&a.face_ids, a.family_id, a.collection_id, a.set_id)?
        }
        PREPARE_FOR_WEB => {
            let a: PrepareForWebInput = args(name, arguments.clone())?;
            Some(Target::Family {
                family: FamilyId(a.family_id),
            })
        }
        BUILD_SPECIMEN => {
            let _: BuildSpecimenInput = args(name, arguments.clone())?;
            None
        }
        _ => None,
    };
    // Only an activation grant is about faces; an export's is not.
    Ok(needs
        .into_iter()
        .map(|tool| McpToolCall {
            tool: tool.to_owned(),
            target: [ACTIVATE_FONTS, DEACTIVATE_FONTS]
                .contains(&tool)
                .then(|| target.clone())
                .flatten(),
        })
        .collect())
}

fn args<T: DeserializeOwned>(name: &str, value: Value) -> Result<T, ApiError> {
    serde_json::from_value(value)
        .map_err(|e| ApiError::invalid(format!("The arguments for {name} are not valid: {e}.")))
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<Value, ApiError> {
    serde_json::to_value(value).map_err(|e| {
        ApiError::new(
            ErrorCode::Internal,
            "The answer could not be written as JSON.",
        )
        .with_detail(e.to_string())
    })
}

fn ids<T>(raw: &[u32], wrap: fn(u32) -> T) -> Vec<T> {
    raw.iter().copied().map(wrap).collect()
}

/// What the caller asked for coverage of: languages or text, not both.
fn supports(
    languages: Vec<String>,
    text: Option<String>,
    required: bool,
) -> Result<Option<Supports>, ApiError> {
    match (languages.is_empty(), text) {
        (false, Some(_)) => Err(ApiError::invalid(
            "Give either languages or text, not both.",
        )),
        (false, None) => Ok(Some(Supports::Languages { codes: languages })),
        (true, Some(text)) => Ok(Some(Supports::Text { text })),
        (true, None) if required => Err(ApiError::invalid("Give languages or text to check.")),
        (true, None) => Ok(None),
    }
}

/// The one activation target the caller named, or `None` when none.
fn target(
    faces: &[u32],
    family: Option<u32>,
    collection: Option<u32>,
    set: Option<u32>,
) -> Result<Option<Target>, ApiError> {
    let mut named = Vec::new();
    if !faces.is_empty() {
        named.push(Target::Faces {
            faces: ids(faces, FaceId),
        });
    }
    if let Some(family) = family {
        named.push(Target::Family {
            family: FamilyId(family),
        });
    }
    if let Some(collection) = collection {
        named.push(Target::Collection {
            collection: CollectionId(collection),
        });
    }
    if let Some(set) = set {
        named.push(Target::Set { set: SetId(set) });
    }
    if named.len() > 1 {
        return Err(ApiError::invalid(
            "Give only one of face_ids, family_id, collection_id or set_id.",
        ));
    }
    Ok(named.pop())
}

/// Text over the tools' cap is refused here; the API checks the rest (size,
/// colours, tags, face count) and its error becomes the tool error.
fn short_text(text: &str) -> Result<(), ApiError> {
    if text.chars().count() > MAX_COMPARE_TEXT_CHARS {
        return Err(ApiError::invalid(format!(
            "Preview text must be at most {MAX_COMPARE_TEXT_CHARS} characters."
        )));
    }
    Ok(())
}

/// A preview tool: the image, then the faces in row order.
fn preview(api: &dyn Api, name: &str, value: Value) -> Result<CallToolResult, ApiError> {
    let (image, faces) = if name == RENDER_PREVIEW {
        let a: RenderPreviewInput = args(name, value)?;
        short_text(&a.text)?;
        let face = FaceId(a.face_id);
        let image = api.render_preview(PreviewRequest {
            face,
            text: a.text,
            size_px: a.size,
            variations: a.options.variations(),
            features: a.options.features(),
            foreground: a.options.foreground,
            background: a.options.background,
        })?;
        (image, vec![face])
    } else {
        let a: ComparePreviewInput = args(name, value)?;
        short_text(&a.text)?;
        let faces = ids(&a.face_ids, FaceId);
        let image = api.compare_preview(ComparePreviewRequest {
            faces: faces.clone(),
            text: a.text,
            size_px: a.size,
            variations: a.options.variations(),
            features: a.options.features(),
            foreground: a.options.foreground,
            background: a.options.background,
        })?;
        (image, faces)
    };
    let names = faces
        .iter()
        .map(|face| {
            let f = api.get_face(*face)?.face;
            Ok(format!("{} {} (face {})", f.family_name, f.style, f.id.0))
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(CallToolResult::success(vec![
        ContentBlock::image(base64(&image.png), "image/png"),
        ContentBlock::text(caption(&image, &names)),
    ]))
}

/// `Inter Regular (face 12), 640×58 px`; for several faces their names
/// numbered top to bottom.
fn caption(image: &PreviewImage, names: &[String]) -> String {
    let size = format!("{}×{} px", image.width, image.height);
    match names {
        [one] => format!("{one}, {size}"),
        _ => {
            let rows: Vec<String> = names
                .iter()
                .enumerate()
                .map(|(i, n)| format!("{}. {n}", i + 1))
                .collect();
            format!("Rows top to bottom: {}; {size}", rows.join("; "))
        }
    }
}

/// Standard base64 with padding (RFC 4648 §4), as MCP image content wants.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let byte = |i: usize| u32::from(chunk.get(i).copied().unwrap_or(0));
        let n = (byte(0) << 16) | (byte(1) << 8) | byte(2);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// What to subset to: presets or text, not both; neither keeps every
/// character.
fn subset(presets: Vec<String>, text: Option<String>) -> Result<Option<Subset>, ApiError> {
    match (presets.is_empty(), text) {
        (false, Some(_)) => Err(ApiError::invalid(
            "Give either subset_presets or subset_text, not both.",
        )),
        (false, None) => Ok(Some(Subset::Presets { names: presets })),
        (true, Some(text)) => Ok(Some(Subset::Text { text })),
        (true, None) => Ok(None),
    }
}

/// The file `build_specimen` writes for family `family`.
pub fn specimen_file(family: u32) -> String {
    format!("specimen-{family}.html")
}

fn dispatch(api: &dyn Api, name: &str, value: Value) -> Result<Value, ApiError> {
    match name {
        SEARCH_FONTS => {
            let a: SearchFontsInput = args(name, value)?;
            let filters = Filters {
                classification: a.classification.into_iter().map(Into::into).collect(),
                variable: a.variable,
                active: a.active,
                collection: a.collection_id.map(CollectionId),
                supports: supports(a.languages, a.text, false)?,
                ..Filters::default()
            };
            to_json(&api.search(SearchRequest {
                query: a.query,
                filters,
                rule: None,
                page: Page {
                    offset: a.offset,
                    limit: a.limit.unwrap_or(DEFAULT_SEARCH_LIMIT),
                },
            })?)
        }
        GET_FONT => {
            let a: FaceInput = args(name, value)?;
            to_json(&api.get_face(FaceId(a.face_id))?)
        }
        LIST_COLLECTIONS => {
            let _: NoInput = args(name, value)?;
            Ok(json!({ "collections": to_json(&api.list_collections()?)? }))
        }
        LIST_SETS => {
            let _: NoInput = args(name, value)?;
            Ok(json!({ "sets": to_json(&api.list_sets()?)? }))
        }
        CHECK_COVERAGE => {
            let a: CheckCoverageInput = args(name, value)?;
            let Some(target) = supports(a.languages, a.text, true)? else {
                return Err(ApiError::invalid("Give languages or text to check."));
            };
            to_json(&api.check_coverage(CoverageRequest {
                face: FaceId(a.face_id),
                target,
            })?)
        }
        FIND_SIMILAR => {
            let a: FaceLimitInput = args(name, value)?;
            let similar = api.find_similar(SimilarRequest {
                face: FaceId(a.face_id),
                limit: a.limit.unwrap_or(DEFAULT_LIMIT),
            })?;
            Ok(json!({ "similar": to_json(&similar)? }))
        }
        GET_PAIRINGS => {
            let a: FaceLimitInput = args(name, value)?;
            let pairings = api.suggest_pairings(PairingRequest {
                face: FaceId(a.face_id),
                limit: a.limit.unwrap_or(DEFAULT_LIMIT),
            })?;
            Ok(json!({ "pairings": to_json(&pairings)? }))
        }
        GET_LICENSE => {
            let a: FaceInput = args(name, value)?;
            let detail = api.get_face(FaceId(a.face_id))?;
            Ok(json!({
                "face": detail.face.id,
                "family": detail.face.family,
                "familyName": detail.face.family_name,
                "style": detail.face.style,
                "license": to_json(&detail.license)?,
            }))
        }
        ACTIVATE_FONTS => {
            let a: ActivateFontsInput = args(name, value)?;
            let Some(target) = target(&a.face_ids, a.family_id, a.collection_id, a.set_id)? else {
                return Err(ApiError::invalid(
                    "Give face_ids, family_id, collection_id or set_id to activate.",
                ));
            };
            let outcomes = api.activate(ActivateRequest {
                target,
                mode: if a.permanent {
                    ActivationMode::Permanent
                } else {
                    ActivationMode::Session
                },
                on_conflict: if a.replace_conflicts {
                    ConflictPolicy::Replace
                } else {
                    ConflictPolicy::KeepExisting
                },
            })?;
            Ok(json!({ "outcomes": to_json(&outcomes)? }))
        }
        DEACTIVATE_FONTS => {
            let a: DeactivateFontsInput = args(name, value)?;
            let outcomes = api.deactivate(DeactivateRequest {
                target: target(&a.face_ids, a.family_id, a.collection_id, a.set_id)?,
                except: None,
            })?;
            Ok(json!({ "outcomes": to_json(&outcomes)? }))
        }
        TAG_FONTS => {
            let a: TagFontsInput = args(name, value)?;
            let tags = api.tag_faces(TagFacesRequest {
                faces: ids(&a.face_ids, FaceId),
                tags: a.tags,
                families: ids(&a.family_ids, FamilyId),
            })?;
            Ok(json!({ "tags": to_json(&tags)? }))
        }
        EXPORT_WEB => {
            let a: ExportWebInput = args(name, value)?;
            let subset = subset(a.subset_presets, a.subset_text)?;
            let formats = if a.formats.is_empty() {
                vec![WebFormat::Woff2]
            } else {
                a.formats.into_iter().map(Into::into).collect()
            };
            to_json(&api.export_web(ExportWebRequest {
                faces: ids(&a.face_ids, FaceId),
                formats,
                subset,
                out_dir: a.out_dir,
                css: a.css,
                overwrite: a.overwrite,
            })?)
        }
        PICK_FONTS_FOR_BRIEF => {
            let a: PickFontsForBriefInput = args(name, value)?;
            to_json(&api.pick_fonts_for_brief(BriefRequest {
                brief: a.brief,
                languages: a.languages,
                license: a.license.map(Into::into),
                limit: a.limit,
            })?)
        }
        PREPARE_FOR_WEB => {
            let a: PrepareForWebInput = args(name, value)?;
            to_json(&api.prepare_for_web(PrepareForWebRequest {
                family: FamilyId(a.family_id),
                subset: subset(a.subset_presets, a.subset_text)?,
                out_dir: a.out_dir,
            })?)
        }
        BUILD_SPECIMEN => {
            let a: BuildSpecimenInput = args(name, value)?;
            if a.out_dir.trim().is_empty() {
                return Err(ApiError::invalid("Choose a folder for the specimen."));
            }
            let out = Path::new(&a.out_dir).join(specimen_file(a.family_id));
            if !a.overwrite && out.exists() {
                return Err(ApiError::new(
                    ErrorCode::Conflict,
                    format!(
                        "{} is already there. Pass overwrite to replace it.",
                        out.display()
                    ),
                ));
            }
            to_json(&api.export_specimen(SpecimenRequest {
                family: FamilyId(a.family_id),
                format: SpecimenFormat::Html,
                out_path: out.display().to_string(),
            })?)
        }
        _ => Err(ApiError::not_found(format!("The tool {name}"))),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn the_mutating_tools_are_the_ones_fm_core_grants() {
        assert_eq!(MUTATING, fm_api::settings::MCP_MUTATING_TOOLS);
        for tool in fm_api::settings::MCP_CONFIRM_TOOLS {
            assert!(MUTATING.contains(&tool), "{tool}");
        }
    }

    #[test]
    fn workflows_ask_for_every_grant_they_need() {
        assert_eq!(grants(PICK_FONTS_FOR_BRIEF), Vec::<&str>::new());
        assert_eq!(grants(TAG_FONTS), [TAG_FONTS]);
        let calls = tool_calls(
            PREPARE_FOR_WEB,
            &json!({ "family_id": 4, "out_dir": "/web" }),
        )
        .unwrap();
        let asked: Vec<(&str, Option<Target>)> = calls
            .iter()
            .map(|c| (c.tool.as_str(), c.target.clone()))
            .collect();
        assert_eq!(
            asked,
            [
                (
                    ACTIVATE_FONTS,
                    Some(Target::Family {
                        family: FamilyId(4)
                    })
                ),
                (EXPORT_WEB, None)
            ]
        );
        let specimen = tool_calls(BUILD_SPECIMEN, &json!({ "family_id": 4, "out_dir": "/s" }));
        assert_eq!(specimen.unwrap().len(), 1);
        let bad = tool_calls(PREPARE_FOR_WEB, &json!({ "out_dir": "/web" }));
        assert_eq!(bad.unwrap_err().code, ErrorCode::InvalidRequest);
        for (_, needs) in WORKFLOW_GRANTS {
            assert!(needs.iter().all(|n| MUTATING.contains(n)));
        }
    }

    #[test]
    fn subsets_take_presets_or_text_not_both() {
        assert!(subset(vec!["latin".into()], Some("x".into())).is_err());
        assert_eq!(subset(Vec::new(), None).unwrap(), None);
    }

    #[test]
    fn the_gate_is_asked_about_the_faces_the_call_names() {
        let call = tool_calls(DEACTIVATE_FONTS, &json!({ "family_id": 7 }))
            .unwrap()
            .remove(0);
        assert_eq!(call.tool, DEACTIVATE_FONTS);
        assert_eq!(
            call.target,
            Some(Target::Family {
                family: FamilyId(7)
            })
        );
        let all = tool_calls(DEACTIVATE_FONTS, &json!({})).unwrap().remove(0);
        assert_eq!(all.target, None);
        let tag = tool_calls(TAG_FONTS, &json!({ "face_ids": [1], "tags": ["x"] }))
            .unwrap()
            .remove(0);
        assert_eq!(tag.target, None);
        let bad = tool_calls(DEACTIVATE_FONTS, &json!({ "face_ids": [1], "set_id": 2 }));
        assert_eq!(bad.unwrap_err().code, ErrorCode::InvalidRequest);
    }

    #[test]
    fn there_are_seventeen_tools_in_order_with_object_schemas() {
        let tools = list();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(names, NAMES);
        for t in &tools {
            assert_eq!(
                t.input_schema.get("type"),
                Some(&json!("object")),
                "{}",
                t.name
            );
            let read_only = t.annotations.as_ref().unwrap().read_only_hint.unwrap();
            assert_eq!(read_only, grants(t.name.as_ref()).is_empty(), "{}", t.name);
        }
    }

    #[test]
    fn an_unsupported_error_is_a_tool_error_with_its_message() {
        let e = ApiError::new(ErrorCode::Unsupported, "Pairing is not available yet.")
            .with_detail("E23.4 not built");
        let result = api_error(&e);
        assert_eq!(result.is_error, Some(true));
        assert_eq!(
            result.content[0].as_text().unwrap().text,
            "Pairing is not available yet."
        );
        let back: ApiError = serde_json::from_value(result.structured_content.unwrap()).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn base64_matches_the_rfc_4648_vectors() {
        let vectors = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (raw, encoded) in vectors {
            assert_eq!(base64(raw.as_bytes()), encoded, "{raw:?}");
        }
        assert_eq!(base64(&[0xFB, 0xFF, 0xBF]), "+/+/");
        assert_eq!(base64(b"\x89PNG\r\n\x1a\n"), "iVBORw0KGgo=");
    }

    #[test]
    fn preview_text_is_capped_at_500_characters() {
        assert!(short_text(&"ž".repeat(500)).is_ok());
        let e = short_text(&"ž".repeat(501)).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidRequest);
    }

    #[test]
    fn options_become_api_axis_values_and_features() {
        let options: PreviewOptions = serde_json::from_value(
            json!({ "variations": { "wght": 700.0 }, "features": { "liga": false } }),
        )
        .unwrap();
        assert_eq!(
            options.variations(),
            vec![AxisValue {
                tag: "wght".into(),
                value: 700.0
            }]
        );
        assert_eq!(
            options.features(),
            vec![FeatureSetting {
                tag: "liga".into(),
                on: false
            }]
        );
    }

    #[test]
    fn coverage_takes_languages_or_text_not_both() {
        assert!(supports(vec!["pl".into()], Some("x".into()), false).is_err());
        assert!(supports(Vec::new(), None, true).is_err());
        assert_eq!(supports(Vec::new(), None, false).unwrap(), None);
        assert_eq!(
            supports(Vec::new(), Some("ž".into()), true).unwrap(),
            Some(Supports::Text { text: "ž".into() })
        );
    }

    #[test]
    fn a_target_is_one_of_faces_family_collection_or_set() {
        assert_eq!(target(&[], None, None, None).unwrap(), None);
        assert_eq!(
            target(&[], Some(3), None, None).unwrap(),
            Some(Target::Family {
                family: FamilyId(3)
            })
        );
        let e = target(&[1], Some(3), None, None).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidRequest);
    }
}
