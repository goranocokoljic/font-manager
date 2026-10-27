//! The MCP tools (E27.2, E27.3) over `MockApi`: an rmcp client, connected in
//! process, lists the fourteen tools and calls every one of them; results are
//! the API's JSON without font bytes (the preview tools: a PNG and a
//! caption), API errors are tool errors, and the mutating tools are refused
//! until the user grants them (E27.6); a refused call never reaches the API.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use fm_api::activation::{ActivateRequest, ActivationMode, ConflictPolicy, Target};
use fm_api::mock::MockApi;
use fm_api::organization::CreateCollectionRequest;
use fm_api::search::{PairingRequest, SimilarRequest};
use fm_api::settings::{McpConfirmationAnswer, McpGrantUpdate};
use fm_api::{
    ActivationService as _, Api, ApiError, ErrorCode, Event, EventSource as _, FaceId,
    LibraryService as _, OrganizationService as _, SearchService as _, SettingsService as _,
};
use fm_mcp::tools::{MUTATING, NAMES};
use fm_mcp::{AllowAll, McpServer, ToolGate};
use serde_json::{Value, json};

use common::{Client, assert_no_font_bytes, connect};

async fn mock(gate: Arc<dyn ToolGate>) -> (Arc<MockApi>, Client) {
    let api = Arc::new(MockApi::new());
    let dyn_api: Arc<dyn Api> = api.clone();
    let client = connect(McpServer::with_gate(dyn_api, gate)).await;
    (api, client)
}

/// The id of the first face of the first family `search_fonts` finds for `query`.
async fn face_of(client: &Client, query: &str) -> u32 {
    let found = client.ok("search_fonts", json!({ "query": query })).await;
    found["families"][0]["faces"][0]
        .as_u64()
        .and_then(|id| u32::try_from(id).ok())
        .unwrap_or_else(|| panic!("no face for {query}: {found}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn an_rmcp_client_lists_the_fourteen_tools_with_object_schemas() {
    let (_api, client) = mock(Arc::new(AllowAll)).await;
    let tools = client.list_all_tools().await.expect("tools/list");
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert_eq!(names, NAMES);
    for tool in &tools {
        assert_eq!(
            tool.input_schema.get("type"),
            Some(&json!("object")),
            "{}",
            tool.name
        );
        assert!(
            tool.input_schema.contains_key("properties"),
            "{}",
            tool.name
        );
        let read_only = tool
            .annotations
            .as_ref()
            .and_then(|a| a.read_only_hint)
            .expect("annotated");
        assert_eq!(
            read_only,
            !MUTATING.contains(&tool.name.as_ref()),
            "{}",
            tool.name
        );
    }
    let face_id = &tools[1].input_schema["properties"]["face_id"];
    assert_eq!(
        face_id["type"],
        json!("integer"),
        "get_font takes a face id"
    );
    client.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn every_tool_answers_over_mock_api_without_font_bytes() {
    let (api, client) = mock(Arc::new(AllowAll)).await;
    let collection = api
        .create_collection(CreateCollectionRequest {
            name: "Picked".into(),
            parent: None,
            rules: None,
            rule_group: None,
        })
        .expect("collection");
    let inter = face_of(&client, "inter").await;
    let mut results = Vec::new();

    let found = client
        .ok("search_fonts", json!({ "query": "inter", "limit": 5 }))
        .await;
    assert_eq!(found["families"][0]["family"]["name"], "Inter", "{found}");
    results.push(found);

    let font = client.ok("get_font", json!({ "face_id": inter })).await;
    assert_eq!(font["face"]["id"], inter);
    assert!(font["path"].is_string(), "a path, not the file: {font}");
    results.push(font);

    let collections = client.ok("list_collections", json!({})).await;
    assert_eq!(collections["collections"][0]["id"], collection.id.0);
    results.push(collections);

    let sets = client.ok("list_sets", json!({})).await;
    assert_eq!(sets["sets"][0]["name"], "Client brand");
    results.push(sets);

    let coverage = client
        .ok(
            "check_coverage",
            json!({ "face_id": inter, "languages": ["pl"] }),
        )
        .await;
    assert_eq!(coverage["face"], inter);
    assert_eq!(coverage["languages"][0]["code"], "pl");
    results.push(coverage);
    let by_text = client
        .ok("check_coverage", json!({ "face_id": inter, "text": "Aa" }))
        .await;
    assert_eq!(
        by_text["languages"],
        json!([]),
        "a text target has no languages"
    );
    results.push(by_text);

    // Both ask for 3 results for the same face, and the API's two answers
    // differ, so a tool that called the other method would fail here.
    let similar_api = api
        .find_similar(SimilarRequest {
            face: FaceId(inter),
            limit: 3,
        })
        .expect("similar");
    let pairings_api = api
        .suggest_pairings(PairingRequest {
            face: FaceId(inter),
            limit: 3,
        })
        .expect("pairings");
    assert_ne!(
        serde_json::to_value(&similar_api).unwrap(),
        serde_json::to_value(&pairings_api).unwrap(),
        "the test data must tell the two methods apart"
    );

    let similar = client
        .ok("find_similar", json!({ "face_id": inter, "limit": 3 }))
        .await;
    assert!(!similar_api.is_empty());
    assert_eq!(
        similar["similar"],
        serde_json::to_value(&similar_api).unwrap()
    );
    results.push(similar);

    let pairings = client
        .ok("get_pairings", json!({ "face_id": inter, "limit": 3 }))
        .await;
    assert!(!pairings_api.is_empty());
    assert_eq!(
        pairings["pairings"],
        serde_json::to_value(&pairings_api).unwrap(),
        "get_pairings is the E23.4 pairing method"
    );
    results.push(pairings);

    let license = client.ok("get_license", json!({ "face_id": inter })).await;
    let detail = api.get_face(FaceId(inter)).expect("face");
    assert_eq!(license["face"], inter);
    assert_eq!(license["familyName"], "Inter");
    assert_eq!(
        license["license"],
        serde_json::to_value(&detail.license).unwrap()
    );
    results.push(license);

    let activated = client
        .ok("activate_fonts", json!({ "face_ids": [inter] }))
        .await;
    assert_eq!(activated["outcomes"][0]["face"], inter);
    assert_eq!(
        activated["outcomes"][0]["outcome"]["status"], "done",
        "{activated}"
    );
    assert_eq!(api.list_active().expect("active")[0].face, FaceId(inter));
    results.push(activated);

    let deactivated = client
        .ok("deactivate_fonts", json!({ "face_ids": [inter] }))
        .await;
    assert_eq!(deactivated["outcomes"][0]["outcome"]["status"], "done");
    assert!(api.list_active().expect("active").is_empty());
    results.push(deactivated);

    let tagged = client
        .ok(
            "tag_fonts",
            json!({ "face_ids": [inter], "tags": ["mcp-picked"] }),
        )
        .await;
    assert_eq!(tagged["tags"][0]["name"], "mcp-picked", "{tagged}");
    assert!(
        api.get_face(FaceId(inter))
            .expect("face")
            .tags
            .contains(&"mcp-picked".into())
    );
    results.push(tagged);

    let out_dir = std::env::temp_dir().join(format!("fm-mcp-tools-{}", std::process::id()));
    let exported = client
        .ok(
            "export_web",
            json!({ "face_ids": [inter], "out_dir": out_dir.display().to_string() }),
        )
        .await;
    let files = exported["files"].as_array().expect("paths");
    assert!(!files.is_empty(), "{exported}");
    let paths: Vec<&str> = files.iter().filter_map(Value::as_str).collect();
    assert_eq!(paths.len(), files.len(), "paths are strings");
    assert!(paths.iter().any(|p| p.ends_with(".woff2")), "{exported}");
    assert!(
        paths.iter().any(|p| p.ends_with("fonts.css")),
        "css by default"
    );
    assert!(
        exported["css"]
            .as_str()
            .is_some_and(|css| css.contains("@font-face"))
    );
    let keys: Vec<&String> = exported.as_object().expect("object").keys().collect();
    assert_eq!(keys, ["css", "files"], "paths and CSS only");
    results.push(exported);

    let (png, _) = client
        .png(
            "render_preview",
            json!({ "face_id": inter, "text": "Ag", "size": 24 }),
        )
        .await;
    assert!(!png.is_empty());
    let other = face_of(&client, "source serif").await;
    client
        .png(
            "compare_preview",
            json!({ "face_ids": [inter, other], "text": "Ag", "size": 24 }),
        )
        .await;

    assert_eq!(results.len(), 13, "the 12 JSON tools, check_coverage twice");
    for result in &results {
        assert_no_font_bytes(result);
    }
    client.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn mutating_tools_are_refused_by_default() {
    let api = Arc::new(MockApi::new());
    let dyn_api: Arc<dyn Api> = api.clone();
    let client = connect(McpServer::new(dyn_api)).await;
    let inter = face_of(&client, "inter").await;
    let calls = [
        ("activate_fonts", json!({ "face_ids": [inter] })),
        ("deactivate_fonts", json!({})),
        (
            "tag_fonts",
            json!({ "face_ids": [inter], "tags": ["refused"] }),
        ),
        (
            "export_web",
            json!({ "face_ids": [inter], "out_dir": "/nowhere" }),
        ),
    ];
    for (tool, args) in calls {
        let e = client.err(tool, args).await;
        assert_eq!(e.code, ErrorCode::PermissionDenied, "{tool}");
        assert!(
            e.message.contains("Settings → AI assistants"),
            "{tool}: the error names the switch: {}",
            e.message
        );
    }
    // The spy: no mutating API call was made.
    assert_eq!(
        api.mutations(),
        Vec::<&str>::new(),
        "the API was not called"
    );
    assert!(
        api.list_active().expect("active").is_empty(),
        "nothing activated"
    );
    assert!(api.list_tags().expect("tags").is_empty(), "nothing tagged");
    // Read tools still run, the preview tools included: no grant needed.
    client.ok("get_font", json!({ "face_id": inter })).await;
    let other = face_of(&client, "source serif").await;
    client
        .png(
            "render_preview",
            json!({ "face_id": inter, "text": "Ag", "size": 24 }),
        )
        .await;
    client
        .png(
            "compare_preview",
            json!({ "face_ids": [inter, other], "text": "Ag", "size": 24 }),
        )
        .await;
    client.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn preview_tools_answer_a_png_and_the_faces_in_order() {
    let (api, client) = mock(Arc::new(AllowAll)).await;
    let inter = face_of(&client, "inter").await;
    let serif = face_of(&client, "source serif").await;
    let name = |face: u32| {
        let f = api.get_face(FaceId(face)).expect("face").face;
        format!("{} {} (face {face})", f.family_name, f.style)
    };

    let (_, caption) = client
        .png(
            "render_preview",
            json!({
                "face_id": inter,
                "text": "Hamburg",
                "size": 48,
                "options": { "variations": { "wght": 700 }, "features": { "liga": false },
                             "foreground": "#112233", "background": "#ffffff" }
            }),
        )
        .await;
    assert!(caption.starts_with(&name(inter)), "{caption}");

    let (_, caption) = client
        .png(
            "compare_preview",
            json!({ "face_ids": [serif, inter], "text": "Hamburg", "size": 48 }),
        )
        .await;
    let first = caption.find(&name(serif)).expect("serif named");
    let second = caption.find(&name(inter)).expect("inter named");
    assert!(first < second, "row order: {caption}");
    assert!(
        caption.contains("1. ") && caption.contains("2. "),
        "{caption}"
    );
    client.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn out_of_range_previews_are_tool_errors_not_panics() {
    let (_api, client) = mock(Arc::new(AllowAll)).await;
    let inter = face_of(&client, "inter").await;
    let serif = face_of(&client, "source serif").await;
    let long = "a".repeat(501);
    let calls = [
        (
            "render_preview",
            json!({ "face_id": inter, "text": "Ag", "size": 7.9 }),
        ),
        (
            "render_preview",
            json!({ "face_id": inter, "text": "Ag", "size": 300.5 }),
        ),
        (
            "render_preview",
            json!({ "face_id": inter, "text": long, "size": 24 }),
        ),
        (
            "compare_preview",
            json!({ "face_ids": [inter], "text": "Ag", "size": 24 }),
        ),
        (
            "compare_preview",
            json!({ "face_ids": [inter, serif, inter, serif, inter], "text": "Ag", "size": 24 }),
        ),
        (
            "compare_preview",
            json!({ "face_ids": [inter, serif], "text": "Ag", "size": 400 }),
        ),
        (
            "compare_preview",
            json!({ "face_ids": [inter, serif], "text": long, "size": 24 }),
        ),
    ];
    for (tool, args) in calls {
        let e = client.err(tool, args.clone()).await;
        assert_eq!(e.code, ErrorCode::InvalidRequest, "{tool} {args}");
    }
    let e = client
        .err(
            "render_preview",
            json!({ "face_id": 99_999, "text": "Ag", "size": 24 }),
        )
        .await;
    assert_eq!(e.code, ErrorCode::NotFound);
    // The server is still up.
    client
        .png(
            "render_preview",
            json!({ "face_id": inter, "text": "a".repeat(500), "size": 24 }),
        )
        .await;
    client.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_api_error_is_a_tool_error_with_the_user_message() {
    let (api, client) = mock(Arc::new(AllowAll)).await;
    let missing = 99_999;
    let expected = api.get_face(FaceId(missing)).expect_err("no such face");
    let (text, e) = client
        .err_text("get_font", json!({ "face_id": missing }))
        .await;
    assert_eq!(e, expected);
    assert_eq!(text, expected.message, "the text is the user message");

    let e = client
        .err(
            "check_coverage",
            json!({ "face_id": 1, "languages": ["pl"], "text": "a" }),
        )
        .await;
    assert_eq!(e.code, ErrorCode::InvalidRequest);
    let e = client.err("get_font", json!({ "face": 1 })).await;
    assert_eq!(e.code, ErrorCode::InvalidRequest, "unknown argument: {e:?}");

    let unknown = client.call("no_such_tool", json!({})).await;
    assert!(unknown.is_err(), "an unknown tool is a protocol error");
    client.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn deactivate_fonts_only_reaches_faces_the_app_registered() {
    let (api, client) = mock(Arc::new(AllowAll)).await;
    let inter = face_of(&client, "inter").await;
    let other = face_of(&client, "jetbrains").await;
    api.activate(ActivateRequest {
        target: Target::Faces {
            faces: vec![FaceId(inter)],
        },
        mode: ActivationMode::Session,
        on_conflict: ConflictPolicy::KeepExisting,
    })
    .expect("activate");

    let answer = client
        .ok("deactivate_fonts", json!({ "face_ids": [inter, other] }))
        .await;
    let outcomes = answer["outcomes"].as_array().expect("outcomes");
    let status = |face: u32| -> Value {
        outcomes
            .iter()
            .find(|o| o["face"] == face)
            .map(|o| o["outcome"]["status"].clone())
            .expect("an outcome per face")
    };
    assert_eq!(status(inter), "done");
    assert_eq!(
        status(other),
        "skipped",
        "the app never registered it: {answer}"
    );
    client.close().await;
}

#[test]
fn an_unsupported_api_error_round_trips_as_tool_error_data() {
    let e = ApiError::new(ErrorCode::Unsupported, "Not built yet.");
    let result = fm_mcp::tools::api_error(&e);
    assert_eq!(result.is_error, Some(true));
    let back: ApiError = serde_json::from_value(result.structured_content.expect("data")).unwrap();
    assert_eq!(back.code, ErrorCode::Unsupported);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_grant_applies_to_the_next_call_and_only_to_its_tool() {
    let api = Arc::new(MockApi::new());
    let dyn_api: Arc<dyn Api> = api.clone();
    let client = connect(McpServer::new(dyn_api)).await;
    let inter = face_of(&client, "inter").await;
    let tag = json!({ "face_ids": [inter], "tags": ["granted"] });
    assert_eq!(
        client.err("tag_fonts", tag.clone()).await.code,
        ErrorCode::PermissionDenied
    );

    api.set_mcp_grant(McpGrantUpdate {
        tool: "tag_fonts".into(),
        granted: true,
    })
    .expect("granted");
    client.ok("tag_fonts", tag.clone()).await;
    assert_eq!(api.mutations(), ["tag_faces"]);
    let e = client
        .err("activate_fonts", json!({ "face_ids": [inter] }))
        .await;
    assert_eq!(e.code, ErrorCode::PermissionDenied, "other tools stay off");
    assert_eq!(api.mutations(), ["tag_faces"], "activate was not called");

    api.set_mcp_grant(McpGrantUpdate {
        tool: "tag_fonts".into(),
        granted: false,
    })
    .expect("refused again");
    assert_eq!(
        client.err("tag_fonts", tag).await.code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(api.mutations(), ["tag_faces"]);
    client.close().await;
}

/// Answers the next confirmation on `events` with `allow`; the faces it
/// names.
fn answer_next(api: &MockApi, events: &Receiver<Event>, allow: bool) -> Option<u32> {
    loop {
        let event = events
            .recv_timeout(Duration::from_secs(10))
            .expect("a confirmation");
        if let Event::McpConfirmationRequested {
            id, tool, faces, ..
        } = event
        {
            assert_eq!(tool, "deactivate_fonts");
            api.answer_mcp_confirmation(McpConfirmationAnswer { id, allow })
                .expect("answered");
            return faces;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn deactivate_fonts_waits_for_the_users_allow_and_deny_changes_nothing() {
    let api = Arc::new(MockApi::new());
    let dyn_api: Arc<dyn Api> = api.clone();
    let client = connect(McpServer::new(dyn_api)).await;
    let inter = face_of(&client, "inter").await;
    api.activate(ActivateRequest {
        target: Target::Faces {
            faces: vec![FaceId(inter)],
        },
        mode: ActivationMode::Session,
        on_conflict: ConflictPolicy::default(),
    })
    .expect("activated");
    api.set_mcp_grant(McpGrantUpdate {
        tool: "deactivate_fonts".into(),
        granted: true,
    })
    .expect("granted");
    let before = api.mutations();

    // Deny: a tool error, and deactivate is never called.
    let events = api.subscribe();
    let answerer = {
        let api = Arc::clone(&api);
        std::thread::spawn(move || answer_next(&api, &events, false))
    };
    let args = json!({ "face_ids": [inter] });
    let e = client.err("deactivate_fonts", args.clone()).await;
    assert_eq!(answerer.join().expect("answered"), Some(1));
    assert_eq!(e.code, ErrorCode::PermissionDenied);
    assert_eq!(api.mutations(), before, "deactivate was not called");
    assert_eq!(api.list_active().expect("active").len(), 1);

    // Allow: the call runs.
    let events = api.subscribe();
    let answerer = {
        let api = Arc::clone(&api);
        std::thread::spawn(move || answer_next(&api, &events, true))
    };
    client.ok("deactivate_fonts", args).await;
    assert_eq!(answerer.join().expect("answered"), Some(1));
    assert!(api.list_active().expect("active").is_empty());
    assert_eq!(api.mutations().last(), Some(&"deactivate"));
    client.close().await;
}
