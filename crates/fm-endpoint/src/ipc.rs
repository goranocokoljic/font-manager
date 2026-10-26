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
//! slow client never holds up another. A stop ends every open connection
//! and serves no request after it returns. Services run on the connection's
//! thread; a service that panics is an `internal` error, not a lost app.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use fm_api::{API_VERSION, Api, ApiError, ErrorCode};
use fm_platform::{IpcCloser, IpcListener, IpcName, IpcStream};
use serde_json::Value;

use crate::routes;

/// The envelope version this app speaks (`v`).
pub const ENVELOPE_VERSION: u64 = 1;

/// The longest line, newline not counted.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// The method that answers [`API_VERSION`].
pub const HELLO: &str = "hello";

/// A listening IPC endpoint. Dropping it (or [`IpcEndpoint::stop`]) stops
/// taking clients, ends every open connection and removes the socket.
#[derive(Debug)]
pub struct IpcEndpoint {
    name: IpcName,
    connections: Arc<Connections>,
    thread: Option<JoinHandle<()>>,
}

/// How long [`IpcEndpoint::stop`] waits for a request still running.
const STOP_WAIT: Duration = Duration::from_secs(2);

impl IpcEndpoint {
    /// Listens at `name` ([`fm_platform::AppPaths::ipc_name`] in the app)
    /// and serves `api` there. Returns once the pipe or socket exists.
    pub fn start(api: Arc<dyn Api>, name: &IpcName) -> io::Result<Self> {
        let listener = IpcListener::bind(name)?;
        let connections = Arc::new(Connections::default());
        let open = Arc::clone(&connections);
        let thread = std::thread::Builder::new()
            .name("fm-ipc".into())
            .spawn(move || accept_loop(&listener, &api, &open))?;
        Ok(Self {
            name: name.clone(),
            connections,
            thread: Some(thread),
        })
    }

    /// Where it listens.
    pub fn name(&self) -> &IpcName {
        &self.name
    }

    /// Stops listening, ends every open connection, and waits until the
    /// pipe or socket is gone and no request runs any more (at most
    /// [`STOP_WAIT`] for one still running). No request is served after it
    /// returns, so the quit path can end the session fonts after it.
    pub fn stop(mut self) {
        self.shut_down();
    }

    fn shut_down(&mut self) {
        let Some(thread) = self.thread.take() else {
            return;
        };
        self.connections.stop();
        // Wakes the waiting `accept`; the loop sees the stop and ends.
        drop(fm_platform::ipc::connect(&self.name));
        if thread.join().is_err() {
            tracing::warn!("the IPC listener thread panicked");
        }
        if !self.connections.wait_until_ended(STOP_WAIT) {
            tracing::warn!("an IPC request was still running when the endpoint stopped");
        }
    }
}

impl Drop for IpcEndpoint {
    fn drop(&mut self) {
        self.shut_down();
    }
}

/// The open connections, so a stop can end them.
#[derive(Debug, Default)]
struct Connections {
    state: Mutex<Open>,
    ended: Condvar,
}

#[derive(Debug, Default)]
struct Open {
    stopping: bool,
    next: u64,
    closers: HashMap<u64, IpcCloser>,
}

impl Connections {
    fn lock(&self) -> MutexGuard<'_, Open> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Records a new connection; `None` once the endpoint is stopping.
    fn add(&self, closer: IpcCloser) -> Option<u64> {
        let mut open = self.lock();
        if open.stopping {
            return None;
        }
        let id = open.next;
        open.next += 1;
        open.closers.insert(id, closer);
        Some(id)
    }

    fn remove(&self, id: u64) {
        self.lock().closers.remove(&id);
        self.ended.notify_all();
    }

    fn is_stopping(&self) -> bool {
        self.lock().stopping
    }

    /// No new connection or request from now on; every open one is closed.
    fn stop(&self) {
        let mut open = self.lock();
        open.stopping = true;
        for closer in open.closers.values() {
            closer.close();
        }
    }

    /// Whether every connection ended within `wait`.
    fn wait_until_ended(&self, wait: Duration) -> bool {
        let open = self.lock();
        let (open, _) = self
            .ended
            .wait_timeout_while(open, wait, |open| !open.closers.is_empty())
            .unwrap_or_else(PoisonError::into_inner);
        open.closers.is_empty()
    }
}

fn accept_loop(listener: &IpcListener, api: &Arc<dyn Api>, connections: &Arc<Connections>) {
    loop {
        let accepted = listener.accept();
        if connections.is_stopping() {
            return;
        }
        let stream = match accepted {
            Ok(stream) => stream,
            Err(e) => {
                tracing::warn!(error = %e, "IPC accept failed");
                // Not a busy loop when the error repeats.
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        // A connection a stop could not end is not served.
        let closer = match stream.closer() {
            Ok(closer) => closer,
            Err(e) => {
                tracing::warn!(error = %e, "an IPC client was not served");
                continue;
            }
        };
        let Some(id) = connections.add(closer) else {
            return;
        };
        let api = Arc::clone(api);
        let open = Arc::clone(connections);
        let spawned = std::thread::Builder::new()
            .name("fm-ipc-client".into())
            .spawn(move || {
                serve(&*api, stream, &open);
                open.remove(id);
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "an IPC client was not served");
            connections.remove(id);
        }
    }
}

/// Answers every line of one connection until the client closes it or the
/// endpoint stops.
fn serve(api: &dyn Api, mut stream: IpcStream, connections: &Connections) {
    let mut frames = Frames::default();
    loop {
        let frame = frames.next(&mut stream);
        // A line that arrived while the endpoint stopped is not served.
        if connections.is_stopping() {
            return;
        }
        let reply = match frame {
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
