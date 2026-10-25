//! A service mounted on one path of the endpoint, after the endpoint's own
//! checks: the MCP server at [`fm_api::settings::MCP_PATH`] (E27.1). The
//! endpoint stays transport only and has no rmcp: the app implements
//! [`Mount`] over `fm_mcp::HttpServer` and hands it to
//! [`crate::Endpoint::start_with_mcp`].

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper::body::{Body, Bytes, Frame, Incoming, SizeHint};
use hyper::{Request, Response};

/// The request a mounted service gets.
pub type MountRequest = Request<Incoming>;

/// The body a mounted service answers with.
pub type MountBody = Pin<Box<dyn Body<Data = Bytes, Error = Infallible> + Send>>;

/// The answer of a mounted service, once it is ready.
pub type MountFuture = Pin<Box<dyn Future<Output = Response<MountBody>> + Send>>;

/// A service that answers the requests to its path. It sees only requests
/// that passed the `Origin`, `Host`, token and version checks.
pub trait Mount: Send + Sync {
    /// Answers one request.
    fn call(&self, req: MountRequest) -> MountFuture;
}

/// A [`MountBody`] inside a [`crate::body::Reply`].
pub struct Mounted(pub MountBody);

impl std::fmt::Debug for Mounted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Mounted").finish_non_exhaustive()
    }
}

impl Body for Mounted {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        self.0.as_mut().poll_frame(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.0.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.0.size_hint()
    }
}
