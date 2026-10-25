//! The MCP tools: one `fm-api` call each (E27.2).
//!
//! Each tool has an input struct whose JSON schema (`rmcp::schemars`) is the
//! tool's `inputSchema`. A call decodes its arguments into that struct,
//! builds the API request, calls the API once and returns the API's own JSON
//! as the result. Results carry ids and metadata, never font bytes: the API
//! has no call that returns a font file, and `export_web` answers with the
//! paths it wrote and the CSS.
//!
//! An [`ApiError`] becomes a tool error (`isError: true`) whose text is the
//! error's user message and whose structured content is the error itself.
//! The mutating tools ask the [`ToolGate`] first.

use fm_api::activation::{
    ActivateRequest, ActivationMode, ConflictPolicy, DeactivateRequest, Target,
};
use fm_api::common::{Classification, Page};
use fm_api::export::{ExportWebRequest, Subset, WebFormat};
use fm_api::organization::TagFacesRequest;
use fm_api::search::{
    CoverageRequest, Filters, PairingRequest, SearchRequest, SimilarRequest, Supports,
};
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

/// Every tool, in the order `tools/list` gives them.
pub const NAMES: [&str; 12] = [
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
];

/// The tools that change something; they go through the [`ToolGate`].
pub const MUTATING: [&str; 4] = [ACTIVATE_FONTS, DEACTIVATE_FONTS, TAG_FONTS, EXPORT_WEB];

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

/// Results per page when a tool is not told.
const DEFAULT_SEARCH_LIMIT: u32 = 20;
const DEFAULT_LIMIT: u32 = 10;

fn tool<T: JsonSchema + 'static>(name: &'static str, description: &'static str) -> Tool {
    let mutating = MUTATING.contains(&name);
    let annotations = ToolAnnotations::new()
        .read_only(!mutating)
        .destructive(false)
        .idempotent(name != EXPORT_WEB)
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
            "Write web fonts (WOFF2/WOFF) and @font-face CSS to a folder. Returns the paths written and the CSS.",
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
    if MUTATING.contains(&name) && !gate.allows(name) {
        return Some(api_error(&refused(name)));
    }
    let answer = dispatch(api, name, Value::Object(arguments));
    Some(match answer {
        Ok(value) => CallToolResult::structured(value),
        Err(e) => api_error(&e),
    })
}

/// A tool error for `e`: its user message as text, the error as
/// structured content.
pub fn api_error(e: &ApiError) -> CallToolResult {
    let mut result = CallToolResult::error(vec![ContentBlock::text(e.message.clone())]);
    result.structured_content = serde_json::to_value(e).ok();
    result
}

/// The error a mutating tool gets while the gate refuses it.
fn refused(name: &str) -> ApiError {
    ApiError::new(
        ErrorCode::PermissionDenied,
        "Font Manager does not allow AI assistants to change fonts yet.",
    )
    .with_detail(format!("tool {name} refused by the tool gate"))
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
            let subset = match (a.subset_presets.is_empty(), a.subset_text) {
                (false, Some(_)) => {
                    return Err(ApiError::invalid(
                        "Give either subset_presets or subset_text, not both.",
                    ));
                }
                (false, None) => Some(Subset::Presets {
                    names: a.subset_presets,
                }),
                (true, Some(text)) => Some(Subset::Text { text }),
                (true, None) => None,
            };
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
        _ => Err(ApiError::not_found(format!("The tool {name}"))),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn there_are_twelve_tools_in_order_with_object_schemas() {
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
            assert_eq!(
                read_only,
                !MUTATING.contains(&t.name.as_ref()),
                "{}",
                t.name
            );
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
