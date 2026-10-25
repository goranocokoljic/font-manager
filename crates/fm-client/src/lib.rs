//! Font Manager endpoint client (E25.4, ADR 0015).
//!
//! [`Client`] implements [`fm_api::Api`] by calling the running app's local
//! API: every service method is one `POST /v1/<method>` with the request as
//! JSON, and [`fm_api::EventSource::subscribe`] reads `GET /v1/events`. The
//! CLI and the MCP shim hold it as `Arc<dyn Api>`, so neither has HTTP code
//! of its own and both talk to the app the same way.
//!
//! [`connect`] finds the app through `<data>/endpoint.json`. Every request
//! carries the bearer token and `Fm-Api-Version` with [`fm_api::API_VERSION`];
//! an app of another major answers [`fm_api::ErrorCode::Unsupported`], and an
//! answer that names another major is refused the same way
//! ([`fm_api::check_server_version`]).
//!
//! The client is blocking, like the service traits: one connection per call
//! on `std::net`, no async runtime, so it works from any thread, including a
//! `spawn_blocking` thread of an async host. A failed transport is an
//! [`fm_api::ApiError`] with a plain message, never a panic.

mod events;
mod http;
mod services;

use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, PoisonError, RwLock};
use std::time::Duration;

use fm_api::{Api, ApiError, ApiResult, ErrorCode};
use fm_core::endpoint::Discovery;
use fm_platform::AppPaths;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// How long [`connect`] and every call wait for the app to accept the
/// connection.
pub const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);

/// The running app's local API, as an [`fm_api::Api`].
pub struct Client {
    addr: SocketAddr,
    /// Replaced when this client regenerates the token, so it keeps working.
    token: RwLock<String>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the token.
        f.debug_struct("Client").field("addr", &self.addr).finish()
    }
}

/// The client of the app whose `endpoint.json` is in `paths.data`, or `None`
/// when the app is not there to talk to: the file is missing or unreadable,
/// the process that wrote it is gone, its address is not loopback, or its
/// port does not accept a connection within [`CONNECT_TIMEOUT`].
pub fn connect(paths: &AppPaths) -> Option<Client> {
    let discovery = Discovery::read(&paths.data)?;
    if !fm_platform::process::is_running(discovery.pid) {
        tracing::debug!(
            pid = discovery.pid,
            "endpoint.json names a process that is gone"
        );
        return None;
    }
    let addr = loopback_addr(&discovery.url)?;
    match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
        Ok(_) => Some(Client::new(addr, discovery.token)),
        Err(e) => {
            tracing::debug!(%addr, error = %e, "the local API does not accept connections");
            None
        }
    }
}

/// The running app of the current user as `Arc<dyn Api>` ([`connect`] over
/// [`AppPaths::current`]), or `None` when it is not running. What the CLI
/// and the MCP shim hold.
pub fn running_app() -> Option<Arc<dyn Api>> {
    let paths = AppPaths::current().ok()?;
    connect(&paths).map(|client| Arc::new(client) as Arc<dyn Api>)
}

/// The address in `http://127.0.0.1:<port>`: loopback only, so the token
/// never goes to another machine.
fn loopback_addr(url: &str) -> Option<SocketAddr> {
    let addr: SocketAddr = url
        .strip_prefix("http://")?
        .trim_end_matches('/')
        .parse()
        .ok()?;
    addr.ip().is_loopback().then_some(addr)
}

impl Client {
    /// A client of the endpoint at `addr` with `token` (callers that know
    /// both; [`connect`] reads them from the discovery file).
    pub fn new(addr: SocketAddr, token: impl Into<String>) -> Self {
        Self {
            addr,
            token: RwLock::new(token.into()),
        }
    }

    /// Where the endpoint listens.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    fn token(&self) -> String {
        self.token
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn set_token(&self, token: &str) {
        *self.token.write().unwrap_or_else(PoisonError::into_inner) = token.to_owned();
    }

    /// `POST /v1/<method>` with `req` as JSON (`()` sends `null`, which a
    /// method without a request takes), and the answer as `T`.
    fn call<Req: Serialize, T: DeserializeOwned>(&self, method: &str, req: &Req) -> ApiResult<T> {
        let body = serde_json::to_vec(req).map_err(|e| {
            ApiError::new(ErrorCode::Internal, "The request could not be encoded.")
                .with_detail(e.to_string())
        })?;
        let path = format!("{}{method}", http::ROUTE_PREFIX);
        let answer = http::post(self.addr, &self.token(), &path, &body)?;
        answer.decode()
    }
}
