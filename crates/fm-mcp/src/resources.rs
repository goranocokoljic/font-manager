//! The MCP resources (E27.5): one face, one collection, one set, by id.
//!
//! Three URI templates, `font://{faceId}`, `collection://{id}` and
//! `set://{id}`. Reading one answers with the API's own JSON as
//! `application/json` text: the face's `get_face` detail; the collection
//! with its families (`list_families` filtered by the collection); the set
//! with a list row per face. Like the tools, resources carry ids and
//! metadata, never font bytes: the contents are always text, never a blob.
//!
//! An id that names nothing, and a URI that is not one of the three, is
//! the MCP resource-not-found error. Any other API error is an internal
//! error with the API's message and the error itself as data.

use fm_api::common::Page;
use fm_api::library::{FaceSummary, FamilySummary, ListFamiliesRequest};
use fm_api::{Api, ApiError, CollectionId, ErrorCode, FaceId, SetId};
use rmcp::ErrorData;
use rmcp::model::{ReadResourceResult, ResourceContents, ResourceTemplate};
use serde_json::{Value, json};

/// The MIME type of every resource.
pub const MIME_TYPE: &str = "application/json";

/// One face's details.
pub const FONT_TEMPLATE: &str = "font://{faceId}";
/// One collection with its families.
pub const COLLECTION_TEMPLATE: &str = "collection://{id}";
/// One project set with its faces.
pub const SET_TEMPLATE: &str = "set://{id}";

/// Families read per `list_families` call while a collection is read.
const FAMILY_PAGE: u32 = 500;

/// What a resource URI names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resource {
    /// `font://{faceId}`.
    Font(FaceId),
    /// `collection://{id}`.
    Collection(CollectionId),
    /// `set://{id}`.
    Set(SetId),
}

impl Resource {
    /// The resource `uri` names, or `None` when it is not `font://`,
    /// `collection://` or `set://` followed by a decimal id.
    pub fn parse(uri: &str) -> Option<Self> {
        let (scheme, id) = uri.split_once("://")?;
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let id: u32 = id.parse().ok()?;
        match scheme {
            "font" => Some(Self::Font(FaceId(id))),
            "collection" => Some(Self::Collection(CollectionId(id))),
            "set" => Some(Self::Set(SetId(id))),
            _ => None,
        }
    }
}

/// The three templates, in the order `resources/templates/list` gives them.
pub fn templates() -> Vec<ResourceTemplate> {
    let template = |uri: &str, name: &str, title: &str, description: &str| {
        ResourceTemplate::new(uri, name)
            .with_title(title)
            .with_description(description)
            .with_mime_type(MIME_TYPE)
    };
    vec![
        template(
            FONT_TEMPLATE,
            "font",
            "Font face",
            "One face of the library by its face id (from search_fonts): names, \
             file path, metrics, axes, features, scripts, license, tags. No font bytes.",
        ),
        template(
            COLLECTION_TEMPLATE,
            "collection",
            "Collection",
            "One collection by its id (from list_collections), with the families \
             that have a face in it. A smart collection applies its rule.",
        ),
        template(
            SET_TEMPLATE,
            "set",
            "Project set",
            "One project set by its id (from list_sets), with a summary of each of \
             its faces.",
        ),
    ]
}

/// Reads `uri`: its JSON as `application/json` text, or the MCP error.
pub fn read(api: &dyn Api, uri: &str) -> Result<ReadResourceResult, ErrorData> {
    let resource = Resource::parse(uri).ok_or_else(|| not_found(uri))?;
    let value = match resource {
        Resource::Font(face) => to_json(&api.get_face(face).map_err(|e| error(uri, &e))?)?,
        Resource::Collection(id) => {
            let collection = api
                .list_collections()
                .map_err(|e| error(uri, &e))?
                .into_iter()
                .find(|c| c.id == id)
                .ok_or_else(|| not_found(uri))?;
            let families = families_in(api, id).map_err(|e| error(uri, &e))?;
            json!({ "collection": to_json(&collection)?, "families": to_json(&families)? })
        }
        Resource::Set(id) => {
            let set = api
                .list_sets()
                .map_err(|e| error(uri, &e))?
                .into_iter()
                .find(|s| s.id == id)
                .ok_or_else(|| not_found(uri))?;
            let faces = faces_of(api, &set.faces).map_err(|e| error(uri, &e))?;
            json!({ "set": to_json(&set)?, "faces": to_json(&faces)? })
        }
    };
    let text = serde_json::to_string(&value).map_err(|e| internal(&e.to_string()))?;
    Ok(ReadResourceResult::new(vec![
        ResourceContents::text(text, uri).with_mime_type(MIME_TYPE),
    ]))
}

/// Every family with a face in `collection`, page after page.
fn families_in(api: &dyn Api, collection: CollectionId) -> Result<Vec<FamilySummary>, ApiError> {
    let mut families = Vec::new();
    loop {
        let offset = u32::try_from(families.len()).unwrap_or(u32::MAX);
        let page = api.list_families(ListFamiliesRequest {
            collection: Some(collection),
            page: Page {
                offset,
                limit: FAMILY_PAGE,
            },
            ..ListFamiliesRequest::default()
        })?;
        let done = page.items.is_empty()
            || offset.saturating_add(u32::try_from(page.items.len()).unwrap_or(u32::MAX))
                >= page.total;
        families.extend(page.items);
        if done {
            return Ok(families);
        }
    }
}

/// The list rows of `faces`, in set order. A face that is no longer in the
/// library (its file was removed) is left out; the set still lists its id.
fn faces_of(api: &dyn Api, faces: &[FaceId]) -> Result<Vec<FaceSummary>, ApiError> {
    let mut out = Vec::with_capacity(faces.len());
    for &face in faces {
        match api.get_face(face) {
            Ok(detail) => out.push(detail.face),
            Err(e) if e.code == ErrorCode::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<Value, ErrorData> {
    serde_json::to_value(value).map_err(|e| internal(&e.to_string()))
}

fn not_found(uri: &str) -> ErrorData {
    ErrorData::resource_not_found(
        format!("There is no resource {uri}."),
        Some(json!({ "uri": uri })),
    )
}

/// `NotFound` from the API is resource-not-found; anything else is an
/// internal error carrying the API error.
fn error(uri: &str, e: &ApiError) -> ErrorData {
    if e.code == ErrorCode::NotFound {
        return not_found(uri);
    }
    ErrorData::internal_error(e.message.clone(), serde_json::to_value(e).ok())
}

fn internal(detail: &str) -> ErrorData {
    ErrorData::internal_error(
        "The answer could not be written as JSON.",
        Some(json!({ "detail": detail })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_uri_is_a_scheme_and_a_decimal_id() {
        assert_eq!(Resource::parse("font://7"), Some(Resource::Font(FaceId(7))));
        assert_eq!(
            Resource::parse("collection://12"),
            Some(Resource::Collection(CollectionId(12)))
        );
        assert_eq!(Resource::parse("set://1"), Some(Resource::Set(SetId(1))));
        for bad in [
            "font://",
            "font://x",
            "font://-1",
            "font://+1",
            "font://1/2",
            "font://99999999999",
            "file://1",
            "font:1",
            "",
        ] {
            assert_eq!(Resource::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn there_are_three_json_templates() {
        let uris: Vec<String> = templates().into_iter().map(|t| t.uri_template).collect();
        assert_eq!(uris, [FONT_TEMPLATE, COLLECTION_TEMPLATE, SET_TEMPLATE]);
        assert!(
            templates()
                .iter()
                .all(|t| t.mime_type.as_deref() == Some(MIME_TYPE))
        );
    }
}
