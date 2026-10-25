//! The body helpers the endpoint needs: a whole-buffer response body, the
//! [`Reply`] that is it, an event stream or a mounted service's body, and
//! a request body read up to a limit (no extra crate for any).

use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper::body::{Body, Bytes, Frame, Incoming, SizeHint};

use crate::events::EventBody;
use crate::mount::Mounted;

/// A response body sent in one frame.
#[derive(Debug)]
pub struct Full(Option<Bytes>);

impl Full {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(Some(Bytes::from(bytes)))
    }
}

impl Body for Full {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        Poll::Ready(self.0.take().map(|b| Ok(Frame::data(b))))
    }

    fn is_end_stream(&self) -> bool {
        self.0.is_none()
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.0.as_ref().map_or(0, |b| b.len() as u64))
    }
}

/// Every response body: a whole answer, an event stream, or the answer of
/// a mounted service.
#[derive(Debug)]
pub enum Reply {
    /// A service answer or a refusal.
    Full(Full),
    /// `GET /v1/events`.
    Events(EventBody),
    /// `/mcp` ([`crate::mount`]).
    Mounted(Mounted),
}

impl Body for Reply {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        match self.get_mut() {
            Self::Full(full) => Pin::new(full).poll_frame(cx),
            Self::Events(events) => Pin::new(events).poll_frame(cx),
            Self::Mounted(mounted) => Pin::new(mounted).poll_frame(cx),
        }
    }

    fn is_end_stream(&self) -> bool {
        match self {
            Self::Full(full) => full.is_end_stream(),
            Self::Events(events) => events.is_end_stream(),
            Self::Mounted(mounted) => mounted.is_end_stream(),
        }
    }

    fn size_hint(&self) -> SizeHint {
        match self {
            Self::Full(full) => full.size_hint(),
            Self::Events(events) => events.size_hint(),
            Self::Mounted(mounted) => mounted.size_hint(),
        }
    }
}

/// Why a request body was not read.
#[derive(Debug)]
pub enum ReadError {
    /// It is longer than the limit.
    TooLarge,
    /// The connection broke.
    Broken(String),
}

/// The whole body, or [`ReadError::TooLarge`] once it passes `limit` bytes.
pub async fn read_limited(mut body: Incoming, limit: usize) -> Result<Vec<u8>, ReadError> {
    if body.size_hint().lower() > limit as u64 {
        return Err(ReadError::TooLarge);
    }
    let mut out = Vec::new();
    while let Some(frame) = std::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await {
        let frame = frame.map_err(|e| ReadError::Broken(e.to_string()))?;
        if let Ok(data) = frame.into_data() {
            if out.len() + data.len() > limit {
                return Err(ReadError::TooLarge);
            }
            out.extend_from_slice(&data);
        }
    }
    Ok(out)
}
