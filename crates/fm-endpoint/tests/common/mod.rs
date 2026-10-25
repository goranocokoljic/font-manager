//! A raw HTTP/1.1 client over `TcpStream`, so a test can send any `Host`,
//! `Origin` or `Authorization` header it likes.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use fm_api::mock::MockApi;
use fm_api::{Api, Event, SettingsService};
use fm_endpoint::Endpoint;
use fm_endpoint::events::StreamOptions;

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

impl Served {
    /// A served `MockApi` with these event-stream limits.
    pub fn with(options: StreamOptions) -> Self {
        let api = Arc::new(MockApi::new());
        let endpoint =
            Endpoint::start_with(Arc::clone(&api) as Arc<dyn Api>, options).expect("start");
        Self { api, endpoint }
    }

    /// `GET /v1/events` with the right Host and token.
    pub fn events(&self) -> EventStream {
        EventStream::open(self.endpoint.addr(), &self.token())
    }

    /// Waits up to 10 s for [`Endpoint::event_streams`] to be `n`.
    pub fn wait_for_streams(&self, n: usize) {
        wait_for_streams(&self.endpoint, n, || {});
    }
}

/// Waits up to 10 s for `endpoint` to have `n` event streams, running
/// `between` (e.g. sending an event) while it waits.
pub fn wait_for_streams(endpoint: &Endpoint, n: usize, mut between: impl FnMut()) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while endpoint.event_streams() != n {
        assert!(
            Instant::now() < deadline,
            "expected {n} event stream(s), have {}",
            endpoint.event_streams()
        );
        between();
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// One Server-Sent Events message: a comment, or an event.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Message {
    pub comment: Option<String>,
    pub id: Option<u64>,
    pub event: Option<String>,
    pub data: Option<String>,
}

impl Message {
    /// The data as an [`Event`].
    pub fn decode(&self) -> Event {
        let data = self.data.as_deref().expect("an event has data");
        serde_json::from_str(data).unwrap_or_else(|e| panic!("event JSON ({e}): {data}"))
    }
}

/// A raw `GET /v1/events` client: it decodes the chunked body and splits it
/// into SSE messages, so a test sees exactly what a client gets.
pub struct EventStream {
    reader: BufReader<TcpStream>,
    pub headers: String,
    text: String,
    ended: bool,
}

impl EventStream {
    pub fn open(addr: SocketAddr, token: &str) -> Self {
        let stream = TcpStream::connect(addr).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .expect("timeout");
        let mut writer = stream.try_clone().expect("clone");
        write!(
            writer,
            "GET /v1/events HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {token}\r\nAccept: text/event-stream\r\n\r\n",
            addr.port()
        )
        .expect("write");
        let mut reader = BufReader::new(stream);
        let mut headers = String::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("header line");
            if line == "\r\n" || line.is_empty() {
                break;
            }
            headers.push_str(&line);
        }
        assert!(headers.starts_with("HTTP/1.1 200"), "{headers}");
        Self {
            reader,
            headers,
            text: String::new(),
            ended: false,
        }
    }

    /// The raw TCP stream (to read it to the end).
    pub fn into_inner(self) -> BufReader<TcpStream> {
        self.reader
    }

    /// The next message; `None` when the server ended the stream.
    pub fn next(&mut self) -> Option<Message> {
        loop {
            if let Some(at) = self.text.find("\n\n") {
                let block: String = self.text.drain(..at + 2).collect();
                return Some(parse(&block));
            }
            if self.ended || !self.read_chunk() {
                self.ended = true;
                return None;
            }
        }
    }

    /// The next message that is an event (comments skipped).
    pub fn next_event(&mut self) -> Message {
        loop {
            let message = self.next().expect("the stream ended");
            if message.comment.is_none() {
                return message;
            }
        }
    }

    /// Reads one chunk of the chunked body; `false` at its end.
    fn read_chunk(&mut self) -> bool {
        let mut size = String::new();
        match self.reader.read_line(&mut size) {
            Ok(0) | Err(_) => return false,
            Ok(_) => {}
        }
        let size = usize::from_str_radix(size.trim().split(';').next().unwrap_or(""), 16)
            .unwrap_or_else(|_| panic!("chunk size line: {size:?}"));
        if size == 0 {
            return false;
        }
        let mut bytes = vec![0; size + 2];
        if self.reader.read_exact(&mut bytes).is_err() {
            return false;
        }
        self.text
            .push_str(std::str::from_utf8(&bytes[..size]).expect("utf-8 stream"));
        true
    }
}

fn parse(block: &str) -> Message {
    let mut message = Message::default();
    for line in block.lines().filter(|l| !l.is_empty()) {
        if let Some(comment) = line.strip_prefix(':') {
            message.comment = Some(comment.trim().to_owned());
        } else if let Some((field, value)) = line.split_once(':') {
            let value = value.strip_prefix(' ').unwrap_or(value).to_owned();
            match field {
                "id" => message.id = Some(value.parse().expect("numeric id")),
                "event" => message.event = Some(value),
                "data" => message.data = Some(value),
                other => panic!("unknown SSE field {other}: {block}"),
            }
        }
    }
    message
}
