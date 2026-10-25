//! Version negotiation and deprecation notes (E25.3, ADR 0015 "Versions").

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{EventStream, Served, header_values, send};
use fm_api::{
    API_VERSION, ApiVersion, DEPRECATION_HEADER, Deprecation, LibraryService, OrganizationService,
    VERSION_HEADER,
};

fn served_version() -> String {
    API_VERSION.to_string()
}

fn get_version(served: &Served, headers: &[(&str, &str)]) -> common::Reply {
    send(served.endpoint.addr(), "GET", "/v1/version", headers, "")
}

#[test]
fn version_needs_no_token_and_names_both_versions_and_the_deprecations() {
    let served = Served::new();
    let reply = get_version(&served, &[("Host", &served.host())]);
    assert_eq!(reply.status, 200, "{}", reply.body);
    let app = served.api.app_info().expect("app info");
    assert_eq!(reply.body["appVersion"], app.version.as_str());
    assert_eq!(
        reply.body["apiVersion"],
        serde_json::to_value(API_VERSION).expect("json")
    );
    assert_eq!(reply.body["deprecations"], serde_json::json!([]));
    assert_eq!(reply.header(VERSION_HEADER), [served_version()]);
}

#[test]
fn version_keeps_the_browser_checks_and_takes_get_only() {
    let served = Served::new();
    let host = served.host();
    let origin = get_version(
        &served,
        &[("Host", &host), ("Origin", "https://evil.example")],
    );
    assert_eq!(origin.status, 403, "{}", origin.body);
    let rebound = get_version(&served, &[("Host", "evil.example:80")]);
    assert_eq!(rebound.status, 403, "{}", rebound.body);
    let post = send(
        served.endpoint.addr(),
        "POST",
        "/v1/version",
        &[("Host", &host)],
        "",
    );
    assert_eq!(post.status, 405, "{}", post.body);
    assert_eq!(post.header("Allow"), ["GET"]);
}

#[test]
fn every_response_carries_the_api_version_header() {
    let served = Served::new();
    let host = served.host();
    let auth = format!("Bearer {}", served.token());
    let addr = served.endpoint.addr();
    let replies = [
        ("ok", served.call("list_sources", "")),
        ("service error", served.call("get_face", "999999")),
        ("bad body", served.call("get_face", "{not json")),
        (
            "no token",
            served.call_with("list_sources", "", &[("Host", &host)]),
        ),
        (
            "origin",
            served.call_with(
                "list_sources",
                "",
                &[
                    ("Host", &host),
                    ("Authorization", &auth),
                    ("Origin", "https://a.example"),
                ],
            ),
        ),
        ("no route", served.call("no_such_method", "")),
        (
            "not post",
            send(
                addr,
                "GET",
                "/v1/list_sources",
                &[("Host", &host), ("Authorization", &auth)],
                "",
            ),
        ),
        (
            "other major",
            served.call_with(
                "list_sources",
                "",
                &[
                    ("Host", &host),
                    ("Authorization", &auth),
                    (VERSION_HEADER, "9.0.0"),
                ],
            ),
        ),
        ("version", get_version(&served, &[("Host", &host)])),
    ];
    for (case, reply) in &replies {
        assert_eq!(
            reply.header(VERSION_HEADER),
            [served_version()],
            "{case}: {}",
            reply.headers
        );
    }
    let stream = EventStream::open(addr, &served.token());
    assert_eq!(
        header_values(&stream.headers, VERSION_HEADER),
        [served_version()],
        "{}",
        stream.headers
    );
}

#[test]
fn a_different_major_is_refused_with_both_versions_and_the_service_is_not_called() {
    let served = Served::new();
    let before = served.api.list_collections().expect("collections").len();
    let host = served.host();
    let auth = format!("Bearer {}", served.token());
    let client = ApiVersion {
        major: API_VERSION.major + 1,
        minor: 0,
        patch: 0,
    };
    let reply = served.call_with(
        "create_collection",
        r#"{"name":"Too new"}"#,
        &[
            ("Host", &host),
            ("Authorization", &auth),
            (VERSION_HEADER, &client.to_string()),
        ],
    );
    assert_eq!(reply.status, 422, "{}", reply.body);
    assert_eq!(reply.body["code"], "unsupported");
    let message = reply.body["message"].as_str().expect("message");
    assert_eq!(
        message,
        format!(
            "The client speaks API {client} but Font Manager serves API {API_VERSION}: \
             update Font Manager to a version that serves API {}.x.",
            client.major
        )
    );
    assert_eq!(
        served.api.list_collections().expect("collections").len(),
        before
    );
}

#[test]
fn a_newer_minor_is_served_and_unknown_request_fields_are_ignored() {
    let served = Served::new();
    let host = served.host();
    let auth = format!("Bearer {}", served.token());
    let newer = ApiVersion {
        minor: API_VERSION.minor + 1,
        patch: 3,
        ..API_VERSION
    }
    .to_string();
    let reply = served.call_with(
        "create_collection",
        r#"{"name":"From a newer client","addedInANewerMinor":{"x":1}}"#,
        &[
            ("Host", &host),
            ("Authorization", &auth),
            (VERSION_HEADER, &newer),
        ],
    );
    assert_eq!(reply.status, 200, "{}", reply.body);
    assert_eq!(reply.body["name"], "From a newer client");
    assert!(
        served
            .api
            .list_collections()
            .expect("collections")
            .iter()
            .any(|c| c.name == "From a newer client")
    );
}

#[test]
fn an_older_minor_and_no_version_header_are_served() {
    let served = Served::new();
    let host = served.host();
    let auth = format!("Bearer {}", served.token());
    let older = ApiVersion {
        minor: 0,
        patch: 0,
        ..API_VERSION
    }
    .to_string();
    let reply = served.call_with(
        "list_sources",
        "",
        &[
            ("Host", &host),
            ("Authorization", &auth),
            (VERSION_HEADER, &older),
        ],
    );
    assert_eq!(reply.status, 200, "{}", reply.body);
    assert_eq!(served.call("list_sources", "").status, 200);
}

#[test]
fn a_malformed_version_header_is_refused() {
    let served = Served::new();
    let host = served.host();
    let auth = format!("Bearer {}", served.token());
    for bad in ["latest", "1.2", "1.2.3-beta"] {
        let reply = served.call_with(
            "list_sources",
            "",
            &[
                ("Host", &host),
                ("Authorization", &auth),
                (VERSION_HEADER, bad),
            ],
        );
        assert_eq!(reply.status, 400, "{bad}: {}", reply.body);
        assert_eq!(reply.body["code"], "invalidRequest");
        assert!(
            reply.body["message"]
                .as_str()
                .is_some_and(|m| m.contains(VERSION_HEADER)),
            "{}",
            reply.body
        );
    }
}

#[test]
fn the_token_is_checked_before_the_version() {
    let served = Served::new();
    let reply = served.call_with(
        "list_sources",
        "",
        &[("Host", &served.host()), (VERSION_HEADER, "9.0.0")],
    );
    assert_eq!(reply.status, 401, "{}", reply.body);
}

fn test_only_entry() -> Deprecation {
    Deprecation {
        type_name: "AppInfo".into(),
        field: "platformNotes".into(),
        since: API_VERSION,
        note: "Read the notes from a later field (test-only entry).".into(),
    }
}

#[test]
fn a_response_with_a_deprecated_field_carries_the_note() {
    let entry = test_only_entry();
    let served = Served::with_deprecations(vec![entry.clone()]);
    let reply = served.call("app_info", "");
    assert_eq!(reply.status, 200, "{}", reply.body);
    assert!(reply.body.get("platformNotes").is_some(), "{}", reply.body);
    let notes = reply.header(DEPRECATION_HEADER);
    assert_eq!(notes, [entry.header_value()]);
    assert!(notes[0].contains(&entry.note), "{notes:?}");
    // An answer without that type carries no note.
    let other = served.call("list_collections", "");
    assert_eq!(other.status, 200, "{}", other.body);
    assert!(
        other.header(DEPRECATION_HEADER).is_empty(),
        "{}",
        other.headers
    );
    // The version route lists it.
    let version = get_version(&served, &[("Host", &served.host())]);
    assert_eq!(
        version.body["deprecations"],
        serde_json::to_value(vec![entry]).expect("json")
    );
}

#[test]
fn the_served_list_is_the_empty_fm_api_list() {
    let served = Served::new();
    let reply = served.call("app_info", "");
    assert_eq!(reply.status, 200, "{}", reply.body);
    assert!(
        reply.header(DEPRECATION_HEADER).is_empty(),
        "{}",
        reply.headers
    );
}
