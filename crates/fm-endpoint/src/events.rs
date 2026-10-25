//! `GET /v1/events`: every [`Event`] as Server-Sent Events (ADR 0015).
//!
//! Each stream is one [`fm_api::EventSource::subscribe`] and one thread of
//! its own. The thread drains the subscription (an unbounded channel, so a
//! sender never waits) into a bounded queue that the response body reads.
//! When the client stops reading, the queue fills; once it holds
//! [`StreamOptions::buffer`] frames the thread asks the connection to close
//! and ends, which drops the subscription. Nothing it does can make the API,
//! the UI's own subscription or another stream wait.

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use fm_api::Event;
use hyper::body::{Body, Bytes, Frame};
use serde::Deserialize;
use tokio::sync::Notify;
use tokio::sync::mpsc::{self, error::TrySendError};

/// The path of the event stream.
pub const EVENTS_ROUTE: &str = "/v1/events";

/// How long a stream may be quiet before it gets a keep-alive comment.
pub const KEEP_ALIVE: Duration = Duration::from_secs(15);

/// How many frames a stream may hold for a client that does not read them;
/// one more and the server closes that stream.
pub const STREAM_BUFFER: usize = 1024;

/// How often an idle stream thread checks that its client is still there.
const TICK: Duration = Duration::from_millis(200);

/// The first frame of every stream, so a client knows it is subscribed.
const OPENED: &[u8] = b": stream open\n\n";

/// The keep-alive comment.
const KEEP_ALIVE_FRAME: &[u8] = b": keep-alive\n\n";

/// The event stream's limits. [`Endpoint::start`](crate::Endpoint::start)
/// uses [`KEEP_ALIVE`] and [`STREAM_BUFFER`]; tests pass shorter ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamOptions {
    /// Quiet time before a keep-alive comment.
    pub keep_alive: Duration,
    /// Frames held for a client before its stream is closed.
    pub buffer: usize,
}

impl Default for StreamOptions {
    fn default() -> Self {
        Self {
            keep_alive: KEEP_ALIVE,
            buffer: STREAM_BUFFER,
        }
    }
}

/// The response body of one stream: the frames its thread queued.
#[derive(Debug)]
pub struct EventBody(mpsc::Receiver<Bytes>);

impl Body for EventBody {
    type Data = Bytes;
    type Error = std::convert::Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        self.0.poll_recv(cx).map(|b| b.map(|b| Ok(Frame::data(b))))
    }
}

/// Starts the thread of one stream over `events`. `close` is notified when
/// the client fell [`StreamOptions::buffer`] frames behind; `live` counts
/// the stream until its thread has dropped the subscription.
pub fn open(
    events: Receiver<Event>,
    options: StreamOptions,
    close: Arc<Notify>,
    live: Arc<AtomicUsize>,
) -> io::Result<EventBody> {
    let (frames, body) = mpsc::channel(options.buffer.max(1));
    // Cannot fail: the queue is new and has room for one.
    let _ = frames.try_send(Bytes::from_static(OPENED));
    live.fetch_add(1, Ordering::SeqCst);
    let stream = Stream {
        events,
        frames,
        close,
        keep_alive: options.keep_alive,
        _live: Live(live),
    };
    std::thread::Builder::new()
        .name("fm-endpoint-events".into())
        .spawn(move || stream.run())?;
    Ok(EventBody(body))
}

/// One stream's thread state. Fields drop in order: the subscription goes
/// before the stream stops counting as live.
struct Stream {
    events: Receiver<Event>,
    frames: mpsc::Sender<Bytes>,
    close: Arc<Notify>,
    keep_alive: Duration,
    _live: Live,
}

impl Stream {
    fn run(self) {
        let mut sequence = 0u64;
        let mut last_sent = Instant::now();
        loop {
            let wait = TICK.min(self.keep_alive.saturating_sub(last_sent.elapsed()));
            let frame = match self.events.recv_timeout(wait) {
                Ok(event) => {
                    sequence += 1;
                    match frame(&event, sequence) {
                        Some(frame) => frame,
                        None => continue,
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    if self.frames.is_closed() {
                        return;
                    }
                    if last_sent.elapsed() < self.keep_alive {
                        continue;
                    }
                    Bytes::from_static(KEEP_ALIVE_FRAME)
                }
                // The API is gone: end the stream.
                Err(RecvTimeoutError::Disconnected) => return,
            };
            match self.frames.try_send(frame) {
                Ok(()) => last_sent = Instant::now(),
                Err(TrySendError::Full(_)) => {
                    tracing::info!("closed an event stream whose client stopped reading");
                    self.close.notify_one();
                    return;
                }
                Err(TrySendError::Closed(_)) => return,
            }
        }
    }
}

/// Counts a stream while it lives.
struct Live(Arc<AtomicUsize>);

impl Drop for Live {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The `type` tag of an event's JSON.
#[derive(Deserialize)]
struct Tag<'a> {
    #[serde(rename = "type", borrow)]
    kind: &'a str,
}

/// `event`, as one SSE frame: `id: <sequence>`, `event: <type tag>`,
/// `data: <the event as JSON>`. JSON without pretty-printing has no line
/// break, so `data` is one line.
pub fn frame(event: &Event, sequence: u64) -> Option<Bytes> {
    let json = serde_json::to_string(event)
        .inspect_err(|e| tracing::warn!(error = %e, "an event could not be written"))
        .ok()?;
    let tag: Tag<'_> = serde_json::from_str(&json).ok()?;
    Some(Bytes::from(format!(
        "id: {sequence}\nevent: {}\ndata: {json}\n\n",
        tag.kind
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fm_api::{FaceId, JobId};

    #[test]
    fn an_event_is_one_frame_with_id_type_and_json() {
        let e = Event::ScanProgress {
            job: JobId(3),
            done: 1,
            total: 2,
        };
        let frame = frame(&e, 7).expect("frame");
        assert_eq!(
            std::str::from_utf8(&frame).expect("utf-8"),
            "id: 7\nevent: scanProgress\ndata: {\"type\":\"scanProgress\",\"job\":3,\"done\":1,\"total\":2}\n\n"
        );
    }

    #[test]
    fn every_kind_of_event_is_tagged_with_its_type() {
        let e = Event::ActivationChanged {
            activated: vec![FaceId(1)],
            deactivated: Vec::new(),
        };
        let frame = frame(&e, 1).expect("frame");
        let text = std::str::from_utf8(&frame).expect("utf-8");
        assert!(text.contains("\nevent: activationChanged\n"), "{text}");
        assert_eq!(text.matches('\n').count(), 4, "{text}");
    }
}
