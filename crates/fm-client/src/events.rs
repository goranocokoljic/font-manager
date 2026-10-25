//! [`fm_api::EventSource`] over `GET /v1/events` (ADR 0015, "Event stream").
//!
//! [`subscribe`] opens the stream and waits for the server's first frame
//! (`: stream open`), so the subscription sees every event sent after it
//! returns, as the trait says. A thread (`fm-client-events`) then moves each
//! event into the receiver. The subscription ends when the stream ends (the
//! app quit, or closed a client that fell 1,024 frames behind) and the
//! receiver reports it as disconnected; there is no replay, so a caller that
//! subscribes again reads again the state it needs. Dropping the receiver
//! ends the thread at the next frame (an event, or the keep-alive every 15 s).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use fm_api::{ApiResult, Event};

use crate::Client;
use crate::http::{self, Body};

/// The path of the event stream.
const EVENTS_PATH: &str = "/v1/events";
/// How long [`subscribe`] waits for the server's first frame.
const OPEN_TIMEOUT: Duration = Duration::from_secs(5);
/// A stream silent this long (three keep-alives) is taken as gone.
const IDLE_TIMEOUT: Duration = Duration::from_secs(45);
/// The longest line of the stream this client reads.
const MAX_LINE_BYTES: u64 = 64 * 1024 * 1024;

/// A new subscription. When the stream cannot be opened, the receiver is
/// disconnected at once and the reason is logged.
pub(crate) fn subscribe(client: &Client) -> Receiver<Event> {
    let (tx, rx) = mpsc::channel();
    match open(client) {
        Ok(frames) => {
            let spawned = std::thread::Builder::new()
                .name("fm-client-events".into())
                .spawn(move || pump(frames, &tx));
            if let Err(e) = spawned {
                tracing::warn!(error = %e, "the event stream thread could not start");
            }
        }
        Err(e) => tracing::warn!(error = %e, "the event stream could not be opened"),
    }
    rx
}

type Frames = SseReader<BufReader<Body<BufReader<TcpStream>>>>;

/// Sends `GET /v1/events`, checks the answer and reads up to the first frame.
fn open(client: &Client) -> ApiResult<Frames> {
    let stream = http::open(client.addr)?;
    stream
        .set_read_timeout(Some(OPEN_TIMEOUT))
        .map_err(http::lost)?;
    let mut req = http::request_head("GET", client.addr, &client.token(), EVENTS_PATH);
    req.push_str("Accept: text/event-stream\r\n\r\n");
    (&stream).write_all(req.as_bytes()).map_err(http::lost)?;
    let timeouts = stream.try_clone().map_err(http::lost)?;
    let mut reader = BufReader::new(stream);
    let head = http::read_head(&mut reader)?;
    if head.status != 200 {
        // The refusal is an error answer like any other.
        let mut body = Vec::new();
        Body::new(&head, reader)?
            .take(64 * 1024)
            .read_to_end(&mut body)
            .map_err(http::lost)?;
        http::Answer { head, body }.decode::<()>()?;
        return Err(http::unreadable(
            "the event stream answered without opening",
        ));
    }
    head.check_version()?;
    let mut frames = SseReader::new(BufReader::new(Body::new(&head, reader)?));
    match frames.next().map_err(http::lost)? {
        Some(Frame::Comment) => {}
        Some(Frame::Data(_)) => return Err(http::unreadable("the event stream did not open")),
        None => {
            return Err(http::lost(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "the event stream ended at once",
            )));
        }
    }
    timeouts
        .set_read_timeout(Some(IDLE_TIMEOUT))
        .map_err(http::lost)?;
    Ok(frames)
}

/// Moves events into `tx` until the stream ends or the receiver is gone.
fn pump(mut frames: Frames, tx: &Sender<Event>) {
    loop {
        match frames.next() {
            Ok(Some(Frame::Data(data))) => match serde_json::from_str::<Event>(&data) {
                Ok(event) => {
                    if tx.send(event).is_err() {
                        return;
                    }
                }
                // An event a newer minor added: not one this client knows.
                Err(e) => tracing::debug!(error = %e, "an unknown event was skipped"),
            },
            Ok(Some(Frame::Comment)) => {}
            Ok(None) => {
                tracing::debug!("the event stream ended");
                return;
            }
            Err(e) => {
                tracing::debug!(error = %e, "the event stream broke");
                return;
            }
        }
    }
}

/// One Server-Sent Events message, as far as this client needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Frame {
    /// A comment only (`: stream open`, `: keep-alive`).
    Comment,
    /// An event's `data` (lines joined with `\n`).
    Data(String),
}

/// Splits a `text/event-stream` into [`Frame`]s.
#[derive(Debug)]
pub(crate) struct SseReader<R> {
    inner: R,
}

impl<R: BufRead> SseReader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner }
    }

    /// The next frame; `None` at the end of the stream.
    pub fn next(&mut self) -> std::io::Result<Option<Frame>> {
        let mut data: Option<String> = None;
        let mut comment = false;
        loop {
            let mut bytes = Vec::new();
            (&mut self.inner)
                .take(MAX_LINE_BYTES)
                .read_until(b'\n', &mut bytes)?;
            if bytes.is_empty() {
                // The end; a message without its blank line is dropped.
                return Ok(None);
            }
            let line = String::from_utf8(bytes)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                if let Some(data) = data.take() {
                    return Ok(Some(Frame::Data(data)));
                }
                if comment {
                    return Ok(Some(Frame::Comment));
                }
                continue;
            }
            if line.starts_with(':') {
                comment = true;
                continue;
            }
            let (field, value) = line.split_once(':').unwrap_or((line, ""));
            let value = value.strip_prefix(' ').unwrap_or(value);
            if field == "data" {
                match &mut data {
                    Some(d) => {
                        d.push('\n');
                        d.push_str(value);
                    }
                    None => data = Some(value.to_owned()),
                }
            }
            // `id`, `event` and `retry` are not needed: `data` carries the type.
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn frames(raw: &str) -> Vec<Frame> {
        let mut reader = SseReader::new(raw.as_bytes());
        let mut out = Vec::new();
        while let Some(frame) = reader.next().unwrap() {
            out.push(frame);
        }
        out
    }

    #[test]
    fn comments_and_events_are_split_by_blank_lines() {
        let raw = ": stream open\n\nid: 1\nevent: scanStarted\ndata: {\"type\":\"scanStarted\",\"job\":1}\n\n: keep-alive\n\n";
        assert_eq!(
            frames(raw),
            vec![
                Frame::Comment,
                Frame::Data(r#"{"type":"scanStarted","job":1}"#.into()),
                Frame::Comment,
            ]
        );
    }

    #[test]
    fn crlf_lines_and_multi_line_data_are_read() {
        let raw = "data: a\r\ndata:b\r\n\r\ndata: tail without end";
        assert_eq!(frames(raw), vec![Frame::Data("a\nb".into())]);
    }
}
