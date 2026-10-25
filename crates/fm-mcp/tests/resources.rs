//! The MCP resources and prompts (E27.5) over `MockApi`: an rmcp client,
//! connected in process, lists the three resource templates and the two
//! prompts, reads each kind of resource as the API's JSON without font
//! bytes, gets resource-not-found for an unknown id, and gets prompt
//! messages that name the tools to call.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::Arc;

use fm_api::library::{ListFamiliesRequest, ScanRequest};
use fm_api::mock::MockApi;
use fm_api::organization::{CollectionFacesRequest, CreateCollectionRequest};
use fm_api::{Api, FaceId, LibraryService as _, OrganizationService as _};
use fm_mcp::McpServer;
use fm_mcp::prompts::{CHECK_LANGUAGE_COVERAGE, CHOOSE_FONTS_FOR_BRIEF};
use fm_mcp::resources::{COLLECTION_TEMPLATE, FONT_TEMPLATE, MIME_TYPE, SET_TEMPLATE};
use rmcp::model::{ErrorCode, ReadResourceResult, ResourceContents};
use rmcp::service::ServiceError;
use serde_json::{Value, json};

use common::{Client, assert_no_font_bytes, connect};

async fn mock() -> (Arc<MockApi>, Client) {
    let api = Arc::new(MockApi::new());
    let dyn_api: Arc<dyn Api> = api.clone();
    let client = connect(McpServer::new(dyn_api)).await;
    (api, client)
}

/// The one content of a resource read: `application/json` text for `uri`,
/// parsed, after checking it carries no font bytes.
fn json_of(result: &ReadResourceResult, uri: &str) -> Value {
    assert_eq!(result.contents.len(), 1, "{uri}: one content");
    let ResourceContents::TextResourceContents {
        uri: got,
        mime_type,
        text,
        ..
    } = &result.contents[0]
    else {
        panic!("{uri}: text contents, never a blob");
    };
    assert_eq!(got, uri);
    assert_eq!(mime_type.as_deref(), Some(MIME_TYPE));
    assert_no_font_bytes(&Value::String(text.clone()));
    let value: Value = serde_json::from_str(text).expect("JSON text");
    assert_no_font_bytes(&value);
    value
}

fn assert_not_found(result: Result<ReadResourceResult, ServiceError>, uri: &str) {
    match result {
        Err(ServiceError::McpError(e)) => {
            assert_eq!(e.code, ErrorCode::RESOURCE_NOT_FOUND, "{uri}: {e:?}");
        }
        other => panic!("{uri}: expected resource-not-found, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_three_resource_templates_are_listed_as_json() {
    let (_api, client) = mock().await;
    let templates = client
        .list_all_resource_templates()
        .await
        .expect("resources/templates/list");
    let uris: Vec<&str> = templates.iter().map(|t| t.uri_template.as_str()).collect();
    assert_eq!(uris, [FONT_TEMPLATE, COLLECTION_TEMPLATE, SET_TEMPLATE]);
    assert_eq!(uris, ["font://{faceId}", "collection://{id}", "set://{id}"]);
    for t in &templates {
        assert_eq!(t.mime_type.as_deref(), Some(MIME_TYPE), "{}", t.name);
        assert!(t.description.is_some(), "{}", t.name);
    }
    client.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_font_resource_is_the_get_face_json() {
    let (api, client) = mock().await;
    let result = client.read_resource("font://6").await.expect("read");
    let value = json_of(&result, "font://6");
    let expected = serde_json::to_value(api.get_face(FaceId(6)).unwrap()).unwrap();
    assert_eq!(value, expected);
    assert_eq!(value["face"]["id"], json!(6));
    client.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_collection_resource_lists_the_families_in_it() {
    let (api, client) = mock().await;
    let collection = api
        .create_collection(CreateCollectionRequest {
            name: "Clients".into(),
            parent: None,
            rules: None,
            rule_group: None,
        })
        .unwrap();
    api.add_to_collection(CollectionFacesRequest {
        collection: collection.id,
        faces: vec![FaceId(1), FaceId(6)],
        families: vec![],
    })
    .unwrap();
    let uri = format!("collection://{}", collection.id.0);
    let value = json_of(&client.read_resource(&uri).await.expect("read"), &uri);

    let listed = api.list_collections().unwrap();
    assert_eq!(
        value["collection"],
        serde_json::to_value(&listed[0]).unwrap()
    );
    let families = api
        .list_families(ListFamiliesRequest {
            collection: Some(collection.id),
            ..ListFamiliesRequest::default()
        })
        .unwrap();
    assert_eq!(families.total, 2);
    assert_eq!(
        value["families"],
        serde_json::to_value(&families.items).unwrap()
    );
    let all = api.list_families(ListFamiliesRequest::default()).unwrap();
    assert!(
        value["families"].as_array().unwrap().len() < all.total as usize,
        "only the collection's families"
    );
    client.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_set_resource_carries_the_set_and_a_row_per_face() {
    let (api, client) = mock().await;
    let set = api.list_sets().unwrap().remove(0);
    let uri = format!("set://{}", set.id.0);
    let value = json_of(&client.read_resource(&uri).await.expect("read"), &uri);
    assert_eq!(value["set"], serde_json::to_value(&set).unwrap());
    let rows: Vec<Value> = set
        .faces
        .iter()
        .map(|&f| serde_json::to_value(api.get_face(f).unwrap().face).unwrap())
        .collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(value["faces"], Value::Array(rows));
    client.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_id_or_uri_is_resource_not_found() {
    let (_api, client) = mock().await;
    for uri in [
        "font://999999",
        "collection://999999",
        "set://999999",
        "font://abc",
        "font://",
        "glyph://1",
        "https://example.com/font.ttf",
    ] {
        assert_not_found(client.read_resource(uri).await, uri);
    }
    // The connection is still good after the errors.
    assert!(client.read_resource("font://1").await.is_ok());
    client.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn no_resource_carries_font_bytes_on_a_scanned_library() {
    let (api, client) = mock().await;
    api.scan(ScanRequest::default()).unwrap();
    let collection = api
        .create_collection(CreateCollectionRequest {
            name: "All".into(),
            parent: None,
            rules: None,
            rule_group: None,
        })
        .unwrap();
    let families = api.list_families(ListFamiliesRequest::default()).unwrap();
    api.add_to_collection(CollectionFacesRequest {
        collection: collection.id,
        faces: vec![],
        families: families.items.iter().map(|f| f.id).collect(),
    })
    .unwrap();
    let mut uris = vec![
        format!("collection://{}", collection.id.0),
        "set://1".to_owned(),
    ];
    for family in &families.items {
        for face in api.get_family(family.id).unwrap().faces {
            uris.push(format!("font://{}", face.id.0));
        }
    }
    for uri in &uris {
        let result = client.read_resource(uri).await.expect("read");
        json_of(&result, uri);
    }
    client.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_two_prompts_are_listed_with_their_arguments() {
    let (_api, client) = mock().await;
    let prompts = client.list_all_prompts().await.expect("prompts/list");
    let shape: Vec<(String, Vec<(String, bool)>)> = prompts
        .iter()
        .map(|p| {
            let args = p
                .arguments
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|a| (a.name, a.required.unwrap_or(false)))
                .collect();
            (p.name.clone(), args)
        })
        .collect();
    let owned = |pairs: &[(&str, bool)]| -> Vec<(String, bool)> {
        pairs.iter().map(|(n, r)| ((*n).to_owned(), *r)).collect()
    };
    assert_eq!(
        shape,
        [
            (
                CHOOSE_FONTS_FOR_BRIEF.to_owned(),
                owned(&[("brief", true), ("languages", false)])
            ),
            (
                CHECK_LANGUAGE_COVERAGE.to_owned(),
                owned(&[("text", true), ("font", false)])
            ),
        ]
    );
    client.close().await;
}

/// The text of the one user message of a prompt.
async fn prompt_text(client: &Client, name: &str, args: Value) -> String {
    let result = client.get_prompt(name, args).await.expect("prompts/get");
    assert_eq!(result.messages.len(), 1, "{name}: one message");
    let message = &result.messages[0];
    assert_eq!(message.role, rmcp::model::Role::User);
    message.content.as_text().expect("text").text.clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn choose_fonts_for_brief_names_pick_fonts_for_brief_then_render_preview() {
    let (_api, client) = mock().await;
    let text = prompt_text(
        &client,
        CHOOSE_FONTS_FOR_BRIEF,
        json!({ "brief": "A warm bakery menu", "languages": "sr-Cyrl, pl" }),
    )
    .await;
    assert!(text.contains("A warm bakery menu"), "{text}");
    assert!(text.contains(r#"["sr-Cyrl","pl"]"#), "{text}");
    let pick = text
        .find("`pick_fonts_for_brief`")
        .expect("names the pick tool");
    let render = text
        .find("`render_preview`")
        .expect("names the preview tool");
    assert!(pick < render, "pick first, then look: {text}");

    let bare = prompt_text(
        &client,
        CHOOSE_FONTS_FOR_BRIEF,
        json!({ "brief": "Signage" }),
    )
    .await;
    assert!(bare.contains("`pick_fonts_for_brief`") && !bare.contains("languages"));
    client.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn check_language_coverage_names_check_coverage_with_or_without_a_font() {
    let (_api, client) = mock().await;
    let any = prompt_text(
        &client,
        CHECK_LANGUAGE_COVERAGE,
        json!({ "text": "Ђурђевдан" }),
    )
    .await;
    assert!(any.contains("Ђурђевдан"), "{any}");
    assert!(any.contains("`check_coverage`"), "{any}");
    assert!(any.contains("`search_fonts`"), "finds candidates: {any}");

    let one = prompt_text(
        &client,
        CHECK_LANGUAGE_COVERAGE,
        json!({ "text": "Zażółć", "font": "Inter" }),
    )
    .await;
    assert!(one.contains("Inter") && one.contains("Zażółć"), "{one}");
    assert!(one.contains("`check_coverage`"), "{one}");
    client.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_missing_argument_or_an_unknown_prompt_is_invalid_params() {
    let (_api, client) = mock().await;
    for (name, args) in [
        (CHOOSE_FONTS_FOR_BRIEF, json!({ "languages": "pl" })),
        (CHECK_LANGUAGE_COVERAGE, json!({ "font": "Inter" })),
        ("write_a_poem", json!({})),
    ] {
        match client.get_prompt(name, args).await {
            Err(ServiceError::McpError(e)) => {
                assert_eq!(e.code, ErrorCode::INVALID_PARAMS, "{name}: {e:?}");
            }
            other => panic!("{name}: expected invalid params, got {other:?}"),
        }
    }
    client.close().await;
}
