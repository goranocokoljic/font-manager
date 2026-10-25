//! The two body helpers the endpoint needs: a whole-buffer response body and
//! a request body read up to a limit (no extra crate for either).

use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper::body::{Body, Bytes, Frame, Incoming, SizeHint};

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
