//! What the client sends, and what it makes of an app that goes away or
//! speaks another major (E25.4 criteria 1 and 2): always an `ApiError`
//! with a plain message, never a panic.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::Duration;

use common::{Served, fake_server};
use fm_api::{API_VERSION, ApiError, ErrorCode, LibraryService as _};
use fm_client::Client;

const LOST: &str = "The connection to Font Manager was lost.";

#[test]
fn every_call_sends_the_token_and_the_api_version() {
    let (addr, head, server) = fake_server(
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]".to_vec(),
    );
    let client = Client::new(addr, "the-token");
    assert_eq!(client.list_sources().unwrap(), Vec::new());
    let head = head.recv_timeout(Duration::from_secs(10)).unwrap();
    server.join().unwrap();
    assert!(
        head.starts_with("POST /v1/list_sources HTTP/1.1\r\n"),
        "{head}"
    );
    assert!(
        head.contains("Authorization: Bearer the-token\r\n"),
        "{head}"
    );
    assert!(
        head.contains(&format!("Fm-Api-Version: {API_VERSION}\r\n")),
        "{head}"
    );
    assert!(
        head.contains(&format!("Host: 127.0.0.1:{}\r\n", addr.port())),
        "{head}"
    );
}

#[test]
fn an_app_of_another_major_is_unsupported() {
    // The app refuses the client's header with 422 (ADR 0015): its error
    // comes back as it is.
    let refusal = ApiError::new(
        ErrorCode::Unsupported,
        "The client speaks API 0.22.0 but Font Manager serves API 1.0.0: update the client to a version built for API 1.x.",
    );
    let json = serde_json::to_string(&refusal).unwrap();
    let (addr, _, server) = fake_server(
        format!(
            "HTTP/1.1 422 Unprocessable Entity\r\nFm-Api-Version: 1.0.0\r\nContent-Length: {}\r\n\r\n{json}",
            json.len()
        )
        .into_bytes(),
    );
    assert_eq!(Client::new(addr, "t").app_info().unwrap_err(), refusal);
    server.join().unwrap();

    // An app that does not check the header still names its version: the
    // client refuses the answer itself.
    let other = format!("{}.0.0", API_VERSION.major + 1);
    let (addr, _, server) = fake_server(
        format!("HTTP/1.1 200 OK\r\nFm-Api-Version: {other}\r\nContent-Length: 2\r\n\r\n[]")
            .into_bytes(),
    );
    let e = Client::new(addr, "t").list_sources().unwrap_err();
    server.join().unwrap();
    assert_eq!(e.code, ErrorCode::Unsupported);
    assert!(e.message.contains(&other), "{}", e.message);
}

#[test]
fn an_app_that_closes_without_answering_is_a_lost_connection() {
    let (addr, _, server) = fake_server(Vec::new());
    let e = Client::new(addr, "t").list_sources().unwrap_err();
    server.join().unwrap();
    assert_eq!(e.code, ErrorCode::Io);
    assert_eq!(e.message, LOST);
}

#[test]
fn an_answer_cut_short_is_a_lost_connection() {
    let cut = [
        &b"HTTP/1.1 200 OK\r\nContent-Le"[..],
        b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n[{\"id\":1",
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n10\r\n[{\"id\"",
    ];
    for answer in cut {
        let (addr, _, server) = fake_server(answer.to_vec());
        let e = Client::new(addr, "t").list_sources().unwrap_err();
        server.join().unwrap();
        assert_eq!(e.code, ErrorCode::Io, "{}", String::from_utf8_lossy(answer));
        assert_eq!(e.message, LOST);
    }
}

#[test]
fn something_that_is_not_the_endpoint_is_an_error_not_a_panic() {
    for answer in [
        &b"SSH-2.0-OpenSSH_9.6\r\n"[..],
        b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nnot json!",
        b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 4\r\n\r\noops",
    ] {
        let (addr, _, server) = fake_server(answer.to_vec());
        let e = Client::new(addr, "t").list_sources().unwrap_err();
        server.join().unwrap();
        assert_eq!(
            e.code,
            ErrorCode::Internal,
            "{}",
            String::from_utf8_lossy(answer)
        );
        assert_eq!(
            e.message,
            "Font Manager sent an answer this program cannot read."
        );
    }
}

#[test]
fn an_app_that_quit_after_connect_is_an_error() {
    let served = Served::new();
    let client = &served.client;
    assert!(client.list_sources().is_ok());
    served.endpoint.stop();
    let e = client.list_sources().unwrap_err();
    assert_eq!(e.code, ErrorCode::Io);
    assert_eq!(
        e.message,
        "Font Manager is not running, or its local API does not answer."
    );
    // The event stream of an app that is gone is a closed subscription.
    let events = fm_api::EventSource::subscribe(client);
    assert!(events.recv_timeout(Duration::from_secs(5)).is_err());
}

#[test]
fn a_wrong_token_is_the_endpoints_refusal() {
    let served = Served::new();
    let client = &Client::new(served.endpoint.addr(), "wrong");
    let e = client.list_sources().unwrap_err();
    assert_eq!(e.code, ErrorCode::PermissionDenied);
    // And a stream the endpoint refuses is a closed subscription.
    let events = fm_api::EventSource::subscribe(client);
    assert!(events.recv_timeout(Duration::from_secs(5)).is_err());
    assert_eq!(served.endpoint.event_streams(), 0);
}

#[test]
fn a_wrong_token_is_permission_denied_and_names_settings_local_api() {
    // A real endpoint over MockApi, asked with a token it does not hold.
    let served = Served::new();
    let client = Client::new(served.endpoint.addr(), "not-the-token");
    let e = client.list_sources().unwrap_err();
    assert_eq!(e.code, ErrorCode::PermissionDenied);
    assert!(e.message.contains("Settings > Local API"), "{}", e.message);
    // The app's own words stay in the detail.
    assert!(
        e.detail.as_deref().is_some_and(|d| d.contains("token")),
        "{e:?}"
    );
    // The right token still works.
    assert!(served.client.list_sources().is_ok());
}
