//! Only a local program with the token gets in (criteria 2 and 3 of E25.1).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::net::Ipv4Addr;

use common::Served;
use fm_api::{OrganizationService, SettingsService};

fn collection_count(served: &Served) -> usize {
    served.api.list_collections().expect("collections").len()
}

#[test]
fn it_listens_on_loopback_on_a_port_the_os_picked() {
    let a = Served::new();
    let b = Served::new();
    for served in [&a, &b] {
        let addr = served.endpoint.addr();
        assert!(addr.ip().is_loopback(), "{addr}");
        assert_eq!(addr.ip(), Ipv4Addr::LOCALHOST);
        assert_ne!(addr.port(), 0);
    }
    assert_ne!(a.endpoint.addr().port(), b.endpoint.addr().port());
}

#[test]
fn a_missing_or_wrong_token_gets_401_and_the_service_is_not_called() {
    let served = Served::new();
    let before = collection_count(&served);
    let host = served.host();
    let wrong = format!("Bearer {}", "0".repeat(64));
    let short = format!("Bearer {}", &served.token()[..63]);
    let basic = format!("Basic {}", served.token());
    let cases: [&[(&str, &str)]; 5] = [
        &[("Host", &host)],
        &[("Host", &host), ("Authorization", &wrong)],
        &[("Host", &host), ("Authorization", &short)],
        &[("Host", &host), ("Authorization", &basic)],
        &[("Host", &host), ("Authorization", "Bearer")],
    ];
    for headers in cases {
        let reply = served.call_with("create_collection", r#"{"name":"Intruder"}"#, headers);
        assert_eq!(reply.status, 401, "{headers:?}: {}", reply.body);
        assert_eq!(reply.body["code"], "permissionDenied");
        assert!(
            reply
                .headers
                .to_ascii_lowercase()
                .contains("www-authenticate: bearer")
        );
    }
    assert_eq!(
        collection_count(&served),
        before,
        "a refused request created a collection"
    );
    // The right token: the same request does reach the service.
    let ok = served.call("create_collection", r#"{"name":"Welcome"}"#);
    assert_eq!(ok.status, 200, "{}", ok.body);
    assert_eq!(collection_count(&served), before + 1);
}

#[test]
fn a_regenerated_token_works_and_the_old_one_fails_at_once() {
    let served = Served::new();
    let old = served.token();
    let host = served.host();
    let old_auth = format!("Bearer {old}");
    let first = served.call_with(
        "list_sources",
        "",
        &[("Host", &host), ("Authorization", &old_auth)],
    );
    assert_eq!(first.status, 200);
    // Over the endpoint itself, as the Settings button does through the UI.
    let regenerated = served.call_with(
        "regenerate_endpoint_token",
        "",
        &[("Host", &host), ("Authorization", &old_auth)],
    );
    assert_eq!(regenerated.status, 200, "{}", regenerated.body);
    let new = regenerated.body["token"]
        .as_str()
        .expect("token")
        .to_owned();
    assert_ne!(new, old);
    assert_eq!(served.api.endpoint_info().expect("info").token, new);
    let stale = served.call_with(
        "list_sources",
        "",
        &[("Host", &host), ("Authorization", &old_auth)],
    );
    assert_eq!(stale.status, 401);
    let new_auth = format!("Bearer {new}");
    let fresh = served.call_with(
        "list_sources",
        "",
        &[("Host", &host), ("Authorization", &new_auth)],
    );
    assert_eq!(fresh.status, 200);
}

#[test]
fn a_foreign_host_is_refused_with_403_before_the_token_is_read() {
    let served = Served::new();
    let before = collection_count(&served);
    let auth = format!("Bearer {}", served.token());
    let port = served.endpoint.addr().port();
    let other_port = format!("127.0.0.1:{}", port.wrapping_add(1));
    let rebinding = format!("evil.example:{port}");
    let lookalike = format!("localhost.evil.example:{port}");
    let ipv6 = format!("[::1]:{port}");
    for host in [
        rebinding.as_str(),
        lookalike.as_str(),
        other_port.as_str(),
        ipv6.as_str(),
        "127.0.0.1",
    ] {
        let reply = served.call_with(
            "create_collection",
            r#"{"name":"Rebound"}"#,
            &[("Host", host), ("Authorization", &auth)],
        );
        assert_eq!(reply.status, 403, "Host {host}: {}", reply.body);
        assert_eq!(reply.body["code"], "permissionDenied");
    }
    // No Host header at all.
    let none = served.call_with("list_sources", "", &[("Authorization", &auth)]);
    assert_eq!(none.status, 403);
    assert_eq!(collection_count(&served), before);
    // `localhost` with the right port is fine.
    let localhost = format!("localhost:{port}");
    let ok = served.call_with(
        "list_sources",
        "",
        &[("Host", &localhost), ("Authorization", &auth)],
    );
    assert_eq!(ok.status, 200);
}

#[test]
fn a_request_with_a_browser_origin_is_refused_with_403() {
    let served = Served::new();
    let before = collection_count(&served);
    let auth = format!("Bearer {}", served.token());
    let host = served.host();
    let self_origin = format!("http://{host}");
    for origin in ["https://evil.example", "null", self_origin.as_str()] {
        let reply = served.call_with(
            "create_collection",
            r#"{"name":"From a page"}"#,
            &[
                ("Host", &host),
                ("Origin", origin),
                ("Authorization", &auth),
            ],
        );
        assert_eq!(reply.status, 403, "Origin {origin}: {}", reply.body);
    }
    // A CORS preflight gets no allow headers either.
    let preflight = common::send(
        served.endpoint.addr(),
        "OPTIONS",
        "/v1/create_collection",
        &[
            ("Host", &host),
            ("Origin", "https://evil.example"),
            ("Access-Control-Request-Method", "POST"),
        ],
        "",
    );
    assert_eq!(preflight.status, 403);
    assert!(
        !preflight
            .headers
            .to_ascii_lowercase()
            .contains("access-control-allow")
    );
    assert_eq!(collection_count(&served), before);
}

#[test]
fn stopping_the_endpoint_closes_the_socket() {
    let served = Served::new();
    let addr = served.endpoint.addr();
    served.endpoint.stop();
    assert!(
        std::net::TcpStream::connect(addr).is_err(),
        "{addr} still accepts"
    );
}

/// A refusal sent before the body is read still reaches the client: the
/// server reads the body first. Closed on unread bytes, the socket is reset
/// (at once on macOS) and the client loses the answer, or its write fails.
#[test]
fn a_refused_request_with_a_large_body_still_gets_its_answer() {
    let served = Served::new();
    let addr = served.endpoint.addr();
    let host = format!("127.0.0.1:{}", addr.port());
    let body = format!("{{\"padding\":\"{}\"}}", "x".repeat(512 * 1024));
    let wrong = format!("Bearer {}", "0".repeat(64));
    let token = format!("Bearer {}", served.token());
    let (host, wrong, token) = (host.as_str(), wrong.as_str(), token.as_str());
    // Path, Host, Authorization, the expected status.
    let cases = [
        ("/v1/list_sources", host, wrong, 401),
        ("/v1/list_sources", "evil.example", token, 403),
        ("/v1/no_such_route", host, token, 404),
        ("/mcp", host, token, 404),
    ];
    for (path, host, auth, status) in cases {
        let headers = [("Host", host), ("Authorization", auth)];
        let reply = common::send(addr, "POST", path, &headers, &body);
        assert_eq!(reply.status, status, "{path}: {}", reply.body);
    }
}
