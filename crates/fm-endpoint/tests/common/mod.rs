//! A raw HTTP/1.1 client over `TcpStream`, so a test can send any `Host`,
//! `Origin` or `Authorization` header it likes.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;

use fm_api::mock::MockApi;
use fm_api::{Api, SettingsService};
use fm_endpoint::Endpoint;

/// An answer: status, the raw header block and the body as JSON
/// (`Null` when it is empty).
#[derive(Debug)]
pub struct Reply {
    pub status: u16,
    pub headers: String,
    pub body: serde_json::Value,
}

/// Sends one request; `headers` are sent as given (no `Host` is added).
pub fn send(
    addr: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> Reply {
    let mut stream = TcpStream::connect(addr).expect("connect");
    let mut req = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
    if !headers
        .iter()
        .any(|(n, _)| n.eq_ignore_ascii_case("content-length"))
    {
        req.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    for (name, value) in headers {
        req.push_str(&format!("{name}: {value}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(body);
    stream.write_all(req.as_bytes()).expect("write");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read");
    let text = String::from_utf8(raw).expect("utf-8 answer");
    let (head, body) = text.split_once("\r\n\r\n").expect("header block");
    let status = head
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("status line");
    let body = if body.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(body).unwrap_or_else(|e| panic!("JSON body ({e}): {body}"))
    };
    Reply {
        status,
        headers: head.to_owned(),
        body,
    }
}

/// A served `MockApi` and the token it expects now.
pub struct Served {
    pub api: Arc<MockApi>,
    pub endpoint: Endpoint,
}

impl Served {
    pub fn new() -> Self {
        let api = Arc::new(MockApi::new());
        let endpoint = Endpoint::start(Arc::clone(&api) as Arc<dyn Api>).expect("start");
        Self { api, endpoint }
    }

    pub fn token(&self) -> String {
        self.api.endpoint_info().expect("info").token
    }

    pub fn host(&self) -> String {
        format!("127.0.0.1:{}", self.endpoint.addr().port())
    }

    /// `POST /v1/<method>` with the right Host and token.
    pub fn call(&self, method: &str, body: &str) -> Reply {
        let auth = format!("Bearer {}", self.token());
        self.call_with(
            method,
            body,
            &[("Host", &self.host()), ("Authorization", &auth)],
        )
    }

    pub fn call_with(&self, method: &str, body: &str, headers: &[(&str, &str)]) -> Reply {
        let mut all = vec![("Content-Type", "application/json")];
        all.extend_from_slice(headers);
        send(
            self.endpoint.addr(),
            "POST",
            &format!("/v1/{method}"),
            &all,
            body,
        )
    }
}
