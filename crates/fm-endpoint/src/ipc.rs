//! The IPC endpoint (E29.1, ADR 0016): the same method table as the HTTP
//! routes ([`routes::dispatch`]), over the per-user named pipe or Unix
//! socket of [`fm_platform::ipc`].
//!
//! Messages are newline-delimited JSON, one object per line:
//!
//! - request `{"v":1,"id":<any>,"method":"<service method>","params":<request>}`
//!   (`params` left out, `null` or `{}` for a method without a request);
//! - reply `{"v":1,"id":<the same>,"result":<answer>}` or
//!   `{"v":1,"id":<the same>,"error":<ApiError>}`.
//!
//! `hello` answers [`API_VERSION`]. Another major `v` and an unknown method
//! get `unsupported`; a line that is not such an object, or longer than
//! [`MAX_FRAME_BYTES`], gets `invalidRequest` (with `id` `null` when it could
//! not be read). The connection stays open after every error. There is no
//! token: the OS lets only this user connect.
//!
//! One thread waits for clients and one thread serves each connection, so a
//! slow client never holds up another. Services run on the connection's
//! thread; a service that panics is an `internal` error, not a lost app.

use std::io::{self, Read, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use fm_api::{API_VERSION, Api, ApiError, ErrorCode};
use fm_platform::{IpcListener, IpcName, IpcStream};
use serde_json::Value;

use crate::routes;

/// The envelope version this app speaks (`v`).
pub const ENVELOPE_VERSION: u64 = 1;

/// The longest line, newline not counted.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// The method that answers [`API_VERSION`].
pub const HELLO: &str = "hello";

/// A listening IPC endpoint. Dropping it (or [`IpcEndpoint::stop`]) stops
/// taking clients and removes the socket; open connections end with
/// their client.
#[derive(Debug)]
pub struct IpcEndpoint {
    name: IpcName,
    stopping: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl IpcEndpoint {
    /// Listens at `name` ([`fm_platform::AppPaths::ipc_name`] in the app)
    /// and serves `api` there. Returns once the pipe or socket exists.
    pub fn start(api: Arc<dyn Api>, name: &IpcName) -> io::Result<Self> {
        let listener = IpcListener::bind(name)?;
        let stopping = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stopping);
        let thread = std::thread::Builder::new()
            .name("fm-ipc".into())
            .spawn(move || accept_loop(&listener, &api, &flag))?;
        Ok(Self {
            name: name.clone(),
            stopping,
            thread: Some(thread),
        })
    }

    /// Where it listens.
    pub fn name(&self) -> &IpcName {
        &self.name
    }

    /// Stops listening and waits until the pipe or socket is gone.
    pub fn stop(mut self) {
        self.shut_down();
    }

    fn shut_down(&mut self) {
        let Some(thread) = self.thread.take() else {
            return;
        };
        self.stopping.store(true, Ordering::SeqCst);
        // Wakes the waiting `accept`; the loop sees the flag and ends.
        drop(fm_platform::ipc::connect(&self.name));
        if thread.join().is_err() {
            tracing::warn!("the IPC listener thread panicked");
        }
    }
}

impl Drop for IpcEndpoint {
    fn drop(&mut self) {
        self.shut_down();
    }
}

fn accept_loop(listener: &IpcListener, api: &Arc<dyn Api>, stopping: &AtomicBool) {
    loop {
        let accepted = listener.accept();
        if stopping.load(Ordering::SeqCst) {
            return;
        }
        match accepted {
            Ok(stream) => {
                let api = Arc::clone(api);
                let spawned = std::thread::Builder::new()
                    .name("fm-ipc-client".into())
                    .spawn(move || serve(&*api, stream));
                if let Err(e) = spawned {
                    tracing::warn!(error = %e, "an IPC client was not served");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "IPC accept failed");
                // Not a busy loop when the error repeats.
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

/// Answers every line of one connection until the client closes it.
fn serve(api: &dyn Api, mut stream: IpcStream) {
    let mut frames = Frames::default();
    loop {
        let reply = match frames.next(&mut stream) {
            Ok(Frame::Line(line)) => {
                if line.trim_ascii().is_empty() {
                    continue;
                }
                handle(api, &line)
            }
            Ok(Frame::TooLong) => error_reply(
                &Value::Null,
                &ApiError::invalid("The message is longer than 1 MiB.")
                    .with_detail(format!("frame over {MAX_FRAME_BYTES} bytes")),
            ),
            Ok(Frame::End) | Err(_) => return,
        };
        let sent = stream
            .write_all(&reply)
            .and_then(|()| stream.write_all(b"\n"))
            .and_then(|()| stream.flush());
        if sent.is_err() {
            return;
        }
    }
}

/// The reply to one request line, without the newline.
pub fn handle(api: &dyn Api, line: &[u8]) -> Vec<u8> {
    let request = match serde_json::from_slice::<Value>(line) {
        Ok(Value::Object(map)) => map,
        Ok(_) => return error_reply(&Value::Null, &not_a_request("not a JSON object")),
        Err(e) => {
            return error_reply(
                &Value::Null,
                &ApiError::invalid("The message is not valid JSON.").with_detail(e.to_string()),
            );
        }
    };
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    match request.get("v").map(Value::as_u64) {
        Some(Some(ENVELOPE_VERSION)) => {}
        Some(Some(v)) => {
            return error_reply(
                &id,
                &ApiError::new(
                    ErrorCode::Unsupported,
                    format!(
                        "The client sent message version {v}, but Font Manager speaks version {ENVELOPE_VERSION}."
                    ),
                ),
            );
        }
        _ => return error_reply(&id, &not_a_request("`v` must be a whole number")),
    }
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return error_reply(&id, &not_a_request("`method` must be a string"));
    };
    if method == HELLO {
        return match serde_json::to_vec(&API_VERSION) {
            Ok(json) => result_reply(&id, &json),
            Err(e) => error_reply(&id, &not_encoded(&e)),
        };
    }
    let body = match request.get("params") {
        None => Vec::new(),
        Some(params) => match serde_json::to_vec(params) {
            Ok(body) => body,
            Err(e) => return error_reply(&id, &not_encoded(&e)),
        },
    };
    let dispatched = catch_unwind(AssertUnwindSafe(|| {
        routes::dispatch(api, method, &body, &[])
    }));
    match dispatched {
        Ok(Some(Ok(answer))) => result_reply(&id, &answer.json),
        Ok(Some(Err(e))) => error_reply(&id, &e),
        Ok(None) => error_reply(
            &id,
            &ApiError::new(
                ErrorCode::Unsupported,
                format!("Font Manager has no method `{method}`."),
            ),
        ),
        Err(_) => error_reply(
            &id,
            &ApiError::new(
                ErrorCode::Internal,
                "The request failed inside Font Manager.",
            )
            .with_detail(format!("`{method}` panicked")),
        ),
    }
}

fn not_a_request(detail: &str) -> ApiError {
    ApiError::invalid("The message is not a request.").with_detail(format!(
        "{detail}; expected {{\"v\":{ENVELOPE_VERSION},\"id\",\"method\",\"params\"}}"
    ))
}

fn not_encoded(e: &serde_json::Error) -> ApiError {
    ApiError::new(ErrorCode::Internal, "The answer could not be encoded.")
        .with_detail(e.to_string())
}

fn result_reply(id: &Value, result: &[u8]) -> Vec<u8> {
    let mut reply = envelope_head(id);
    reply.extend_from_slice(b",\"result\":");
    reply.extend_from_slice(result);
    reply.push(b'}');
    reply
}

fn error_reply(id: &Value, error: &ApiError) -> Vec<u8> {
    let mut reply = envelope_head(id);
    reply.extend_from_slice(b",\"error\":");
    // An ApiError is strings and an enum: it always encodes.
    reply.extend(serde_json::to_vec(error).unwrap_or_else(|_| b"null".to_vec()));
    reply.push(b'}');
    reply
}

/// `{"v":1,"id":<id>`; a JSON value always encodes.
fn envelope_head(id: &Value) -> Vec<u8> {
    let id = serde_json::to_vec(id).unwrap_or_else(|_| b"null".to_vec());
    let mut head = format!("{{\"v\":{ENVELOPE_VERSION},\"id\":").into_bytes();
    head.extend(id);
    head
}

enum Frame {
    /// One line, without its newline.
    Line(Vec<u8>),
    /// A line over [`MAX_FRAME_BYTES`], read to its end and dropped.
    TooLong,
    /// The client closed the connection.
    End,
}

/// Splits a byte stream into lines, never holding more than one line of
/// [`MAX_FRAME_BYTES`] plus one read.
#[derive(Default)]
struct Frames {
    buf: Vec<u8>,
    /// Bytes of `buf` already searched for a newline.
    scanned: usize,
    /// The current line went over the limit; its bytes are dropped.
    too_long: bool,
}

impl Frames {
    fn next(&mut self, stream: &mut impl Read) -> io::Result<Frame> {
        let mut chunk = [0u8; 64 * 1024];
        loop {
            if let Some(at) = self.buf[self.scanned..].iter().position(|&b| b == b'\n') {
                let end = self.scanned + at;
                let mut line: Vec<u8> = self.buf.drain(..=end).collect();
                line.pop();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                self.scanned = 0;
                if std::mem::take(&mut self.too_long) || line.len() > MAX_FRAME_BYTES {
                    return Ok(Frame::TooLong);
                }
                return Ok(Frame::Line(line));
            }
            self.scanned = self.buf.len();
            if self.too_long || self.buf.len() > MAX_FRAME_BYTES {
                self.too_long = true;
                self.buf.clear();
                self.scanned = 0;
            }
            let n = match stream.read(&mut chunk) {
                Ok(0) => return Ok(Frame::End),
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            self.buf.extend_from_slice(&chunk[..n]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(input: &[u8]) -> Vec<String> {
        let mut reader = input;
        let mut frames = Frames::default();
        let mut out = Vec::new();
        loop {
            match frames.next(&mut reader).expect("read") {
                Frame::Line(line) => out.push(String::from_utf8(line).expect("utf-8")),
                Frame::TooLong => out.push("<too long>".into()),
                Frame::End => return out,
            }
        }
    }

    #[test]
    fn lines_split_on_newlines_and_crlf() {
        assert_eq!(frames(b"a\nbc\r\n\nd"), ["a", "bc", ""]);
    }

    #[test]
    fn a_line_at_the_limit_passes_and_one_over_is_dropped_whole() {
        let at = vec![b'x'; MAX_FRAME_BYTES];
        let mut input = at.clone();
        input.push(b'\n');
        input.extend(vec![b'y'; MAX_FRAME_BYTES + 1]);
        input.extend(b"\nnext\n");
        let got = frames(&input);
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].len(), MAX_FRAME_BYTES);
        assert_eq!(got[1], "<too long>");
        assert_eq!(got[2], "next");
    }
}
