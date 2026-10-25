//! Font Manager local API endpoint (ADR 0015).
//!
//! A thin transport over `Arc<dyn Api>`: every service method is
//! `POST /v1/<method>` on `127.0.0.1:<port the OS picks>`, with the request
//! as the JSON body and the answer as JSON. An [`ApiError`] keeps its JSON
//! shape and gets a fixed HTTP status by its [`ErrorCode`]
//! ([`status_for`]). Services run on `spawn_blocking` (ADR 0006).
//!
//! Every request must carry `Authorization: Bearer <token>`, the token
//! [`fm_api::SettingsService::endpoint_info`] reports at that moment (so a new token
//! works, and the old one fails, at once). A `Host` other than
//! `127.0.0.1:<port>` or `localhost:<port>` and any `Origin` header are
//! refused with 403 before the token is read: a web page in a browser can
//! neither reach the endpoint by DNS rebinding nor send it a request.
//!
//! `GET /v1/events` streams every [`fm_api::Event`] as Server-Sent Events,
//! one [`fm_api::EventSource::subscribe`] per connection ([`events`]).
//!
//! Versions (ADR 0015): every answer carries `Fm-Api-Version` with
//! [`fm_api::API_VERSION`]; `GET /v1/version` tells a client without a token
//! what this app serves ([`fm_api::VersionInfo`]); a client that sends
//! `Fm-Api-Version` with another major is refused
//! ([`fm_api::check_client_version`]); an answer with a deprecated field
//! carries an `Fm-Deprecation` header per field.
//!
//! The endpoint knows nothing about fonts and keeps no state but the socket
//! and its open streams: the token and the discovery file belong to `fm-core`.

mod body;
pub mod events;
pub mod routes;

use std::convert::Infallible;
use std::future::Future as _;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpListener as StdListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::Poll;
use std::thread::JoinHandle;
use std::time::Duration;

use fm_api::{
    API_VERSION, Api, ApiError, ApiVersion, DEPRECATION_HEADER, Deprecation, ErrorCode,
    VERSION_HEADER, VersionInfo,
};
use hyper::body::Incoming;
use hyper::header::{self, HeaderValue};
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::sync::{Notify, oneshot};

use crate::body::{Full, Reply, read_limited};
use crate::events::{EVENTS_ROUTE, StreamOptions};

/// The largest request body, in bytes.
pub const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

/// The path prefix of every route.
pub const ROUTE_PREFIX: &str = "/v1/";

/// What this app serves, without a token (`GET`).
pub const VERSION_ROUTE: &str = "/v1/version";

/// A running endpoint. Dropping it (or [`Endpoint::stop`]) closes the socket.
#[derive(Debug)]
pub struct Endpoint {
    addr: SocketAddr,
    streams: Arc<AtomicUsize>,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Endpoint {
    /// Binds `127.0.0.1` on a port the OS picks and serves `api` on a
    /// thread of its own. Returns once the socket is bound.
    pub fn start(api: Arc<dyn Api>) -> io::Result<Self> {
        Self::start_with(api, StreamOptions::default())
    }

    /// [`Endpoint::start`] with other event-stream limits (tests).
    pub fn start_with(api: Arc<dyn Api>, options: StreamOptions) -> io::Result<Self> {
        Self::serve(api, options, fm_api::deprecations())
    }

    /// [`Endpoint::start`] with another deprecation list (tests: the real
    /// one, [`fm_api::deprecations`], is empty today).
    pub fn start_with_deprecations(
        api: Arc<dyn Api>,
        deprecations: Vec<Deprecation>,
    ) -> io::Result<Self> {
        Self::serve(api, StreamOptions::default(), deprecations)
    }

    fn serve(
        api: Arc<dyn Api>,
        options: StreamOptions,
        deprecations: Vec<Deprecation>,
    ) -> io::Result<Self> {
        let listener = StdListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()?;
        let (stop, stopped) = oneshot::channel();
        let streams = Arc::new(AtomicUsize::new(0));
        let server = Server {
            api,
            port: addr.port(),
            options,
            streams: Arc::clone(&streams),
            deprecations: Arc::new(deprecations),
            version: version_header(),
        };
        let thread = std::thread::Builder::new()
            .name("fm-endpoint".into())
            .spawn(move || {
                runtime.block_on(async move {
                    let accept = match tokio::net::TcpListener::from_std(listener) {
                        Ok(listener) => tokio::spawn(accept_loop(listener, Arc::new(server))),
                        Err(e) => {
                            tracing::error!(error = %e, "the local API socket could not be used");
                            return;
                        }
                    };
                    let _ = stopped.await;
                    accept.abort();
                });
                // Requests still running get a moment to answer.
                runtime.shutdown_timeout(Duration::from_secs(2));
            })?;
        tracing::info!(%addr, "local API listening");
        Ok(Self {
            addr,
            streams,
            stop: Some(stop),
            thread: Some(thread),
        })
    }

    /// Where it listens: always `127.0.0.1:<port>`.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// How many event streams are open: each holds one subscription and one
    /// thread, and stops counting once both are gone.
    pub fn event_streams(&self) -> usize {
        self.streams.load(Ordering::SeqCst)
    }

    /// Closes the socket and waits for the serving thread to end.
    pub fn stop(mut self) {
        self.shut_down();
    }

    fn shut_down(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::warn!("the local API thread panicked");
        }
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        self.shut_down();
    }
}

/// What every connection shares.
struct Server {
    api: Arc<dyn Api>,
    port: u16,
    options: StreamOptions,
    streams: Arc<AtomicUsize>,
    /// What answers are checked against for `Fm-Deprecation`.
    deprecations: Arc<Vec<Deprecation>>,
    /// `Fm-Api-Version` of every answer.
    version: HeaderValue,
}

fn version_header() -> HeaderValue {
    // Digits and dots only, so always a valid header value.
    HeaderValue::from_str(&API_VERSION.to_string())
        .unwrap_or_else(|_| HeaderValue::from_static("0.0.0"))
}

async fn accept_loop(listener: tokio::net::TcpListener, server: Arc<Server>) {
    loop {
        let stream = match listener.accept().await {
            Ok((stream, _)) => stream,
            Err(e) => {
                // Out of file handles and the like: wait instead of spinning.
                tracing::warn!(error = %e, "local API accept failed");
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        let server = Arc::clone(&server);
        tokio::spawn(async move {
            // An event stream whose client fell too far behind closes the
            // whole connection: hyper polls a body only when the socket
            // takes bytes, so the stalled stream itself would never end.
            let close = Arc::new(Notify::new());
            let closed = Arc::clone(&close);
            let service = hyper::service::service_fn(move |req| {
                let server = Arc::clone(&server);
                let close = Arc::clone(&close);
                async move { Ok::<_, Infallible>(handle(&server, close, req).await) }
            });
            let connection = hyper::server::conn::http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service);
            let mut connection = std::pin::pin!(connection);
            let mut closed = std::pin::pin!(closed.notified());
            // Whichever ends first; dropping the connection closes the socket.
            let ended = std::future::poll_fn(|cx| {
                if closed.as_mut().poll(cx).is_ready() {
                    return Poll::Ready(None);
                }
                connection.as_mut().poll(cx).map(Some)
            })
            .await;
            match ended {
                Some(Err(e)) => tracing::debug!(error = %e, "local API connection ended"),
                Some(Ok(())) => {}
                None => {
                    tracing::debug!("local API connection closed: its client stopped reading");
                }
            }
        });
    }
}

/// One request, then `Fm-Api-Version` on its answer, whatever it is.
async fn handle(server: &Server, close: Arc<Notify>, req: Request<Incoming>) -> Response<Reply> {
    let mut response = route(server, close, req).await;
    response
        .headers_mut()
        .insert(VERSION_HEADER, server.version.clone());
    response
}

/// In this order: `Host` and `Origin` (403), `GET /v1/version`, the token
/// (401), the client's `Fm-Api-Version` (400, 422), then the event stream or
/// a service call.
async fn route(server: &Server, close: Arc<Notify>, req: Request<Incoming>) -> Response<Reply> {
    if let Err(refused) = check_origin(&req, server.port) {
        return refused.map(Reply::Full);
    }
    if req.uri().path() == VERSION_ROUTE {
        return version(server, &req).await.map(Reply::Full);
    }
    if let Err(refused) = authorize(server, &req).await {
        return refused.map(Reply::Full);
    }
    if let Err(refused) = check_client_version(&req) {
        return refused.map(Reply::Full);
    }
    if req.uri().path() == EVENTS_ROUTE {
        return stream_events(server, close, &req).await;
    }
    call(server, req).await.map(Reply::Full)
}

/// The token (401).
#[allow(clippy::result_large_err)]
async fn authorize(server: &Server, req: &Request<Incoming>) -> Result<(), Response<Full>> {
    let presented = bearer(req).map(str::to_owned);
    let info_api = Arc::clone(&server.api);
    let current = match blocking(move || info_api.endpoint_info()).await {
        Ok(info) => info.token,
        Err(e) => return Err(error(&e)),
    };
    if !presented.is_some_and(|t| same_token(t.as_bytes(), current.as_bytes())) {
        return Err(refuse(
            StatusCode::UNAUTHORIZED,
            "A valid token is required (Authorization: Bearer <token>).",
        ));
    }
    Ok(())
}

/// The client's `Fm-Api-Version`, when it sends one: not
/// `major.minor.patch` → 400; another major → 422 (`unsupported`). A client
/// that sends none is served.
#[allow(clippy::result_large_err)]
fn check_client_version(req: &Request<Incoming>) -> Result<(), Response<Full>> {
    let Some(value) = req.headers().get(VERSION_HEADER) else {
        return Ok(());
    };
    let client = value
        .to_str()
        .map_err(|e| e.to_string())
        .and_then(|v| v.parse::<ApiVersion>().map_err(|e| e.to_string()));
    match client {
        Ok(client) => fm_api::check_client_version(client).map_err(|e| error(&e)),
        Err(why) => Err(error(
            &ApiError::invalid(format!(
                "The {VERSION_HEADER} header must be a version like {API_VERSION}."
            ))
            .with_detail(why),
        )),
    }
}

/// `GET /v1/version` (405 for another method): the app version, the API
/// version and the deprecation list. No token: a client learns here whether
/// it can talk to this app at all.
async fn version(server: &Server, req: &Request<Incoming>) -> Response<Full> {
    if req.method() != Method::GET {
        let mut response = refuse(
            StatusCode::METHOD_NOT_ALLOWED,
            "The version route takes GET.",
        );
        response
            .headers_mut()
            .insert(header::ALLOW, HeaderValue::from_static("GET"));
        return response;
    }
    let api = Arc::clone(&server.api);
    let deprecations = Vec::clone(&server.deprecations);
    let info = blocking(move || {
        Ok(VersionInfo {
            app_version: api.app_info()?.version,
            api_version: API_VERSION,
            deprecations,
        })
    })
    .await
    .and_then(|info| {
        serde_json::to_vec(&info).map_err(|e| {
            ApiError::new(ErrorCode::Internal, "The answer could not be encoded.")
                .with_detail(e.to_string())
        })
    });
    match info {
        Ok(json) => json_response(StatusCode::OK, json),
        Err(e) => error(&e),
    }
}

/// `GET /v1/events` (405 for another method): a new subscription, streamed
/// as `text/event-stream` until the client goes or falls behind.
async fn stream_events(
    server: &Server,
    close: Arc<Notify>,
    req: &Request<Incoming>,
) -> Response<Reply> {
    if req.method() != Method::GET {
        let mut response = refuse(
            StatusCode::METHOD_NOT_ALLOWED,
            "The event stream takes GET.",
        );
        response
            .headers_mut()
            .insert(header::ALLOW, HeaderValue::from_static("GET"));
        return response.map(Reply::Full);
    }
    let api = Arc::clone(&server.api);
    let opened = blocking(move || Ok(api.subscribe()))
        .await
        .and_then(|events| {
            events::open(events, server.options, close, Arc::clone(&server.streams)).map_err(|e| {
                ApiError::new(ErrorCode::Internal, "The event stream could not start.")
                    .with_detail(e.to_string())
            })
        });
    let body = match opened {
        Ok(body) => body,
        Err(e) => return error(&e).map(Reply::Full),
    };
    let mut response = Response::new(Reply::Events(body));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// A service call: the method and route (405, 404), the body (413, 400),
/// then the service.
async fn call(server: &Server, req: Request<Incoming>) -> Response<Full> {
    let Some(method) = req
        .uri()
        .path()
        .strip_prefix(ROUTE_PREFIX)
        .filter(|m| routes::ROUTES.contains(m))
        .map(str::to_owned)
    else {
        return error(&ApiError::not_found(format!(
            "The route {}",
            req.uri().path()
        )));
    };
    if req.method() != Method::POST {
        let mut response = refuse(StatusCode::METHOD_NOT_ALLOWED, "Every route takes POST.");
        response
            .headers_mut()
            .insert(header::ALLOW, HeaderValue::from_static("POST"));
        return response;
    }
    let body = match read_limited(req.into_body(), MAX_BODY_BYTES).await {
        Ok(body) => body,
        Err(body::ReadError::TooLarge) => {
            return refuse(
                StatusCode::PAYLOAD_TOO_LARGE,
                "The request body is too large.",
            );
        }
        Err(body::ReadError::Broken(e)) => {
            return error(&ApiError::invalid("The request body could not be read.").with_detail(e));
        }
    };
    let api = Arc::clone(&server.api);
    let deprecated = Arc::clone(&server.deprecations);
    let answer = blocking(move || {
        routes::dispatch(api.as_ref(), &method, &body, &deprecated).unwrap_or_else(|| {
            Err(ApiError::new(
                ErrorCode::Internal,
                "The route has no method.",
            ))
        })
    })
    .await;
    match answer {
        Ok(answer) => {
            let mut response = json_response(StatusCode::OK, answer.json);
            for note in answer.deprecations {
                match HeaderValue::from_str(&note) {
                    Ok(value) => {
                        response.headers_mut().append(DEPRECATION_HEADER, value);
                    }
                    Err(_) => tracing::warn!(%note, "a deprecation note is not a valid header"),
                }
            }
            response
        }
        Err(e) => error(&e),
    }
}

/// Refuses a request a browser could have sent: `Host` must name this
/// endpoint by its loopback address or `localhost` (DNS rebinding sends the
/// attacker's host name), and there must be no `Origin` (every cross-site
/// `fetch` or form post from a web page carries one).
#[allow(clippy::result_large_err)]
fn check_origin(req: &Request<Incoming>, port: u16) -> Result<(), Response<Full>> {
    if req.headers().contains_key(header::ORIGIN) {
        return Err(refuse(
            StatusCode::FORBIDDEN,
            "Requests from a web page are not allowed.",
        ));
    }
    let host_ok = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| allowed_host(h, port));
    if !host_ok {
        return Err(refuse(
            StatusCode::FORBIDDEN,
            "The Host header must be 127.0.0.1 or localhost with this port.",
        ));
    }
    Ok(())
}

/// `127.0.0.1:<port>` or `localhost:<port>` (any case).
pub fn allowed_host(host: &str, port: u16) -> bool {
    let Some((name, p)) = host.rsplit_once(':') else {
        return false;
    };
    p == port.to_string() && (name == "127.0.0.1" || name.eq_ignore_ascii_case("localhost"))
}

fn bearer(req: &Request<Incoming>) -> Option<&str> {
    let value = req.headers().get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    scheme.eq_ignore_ascii_case("bearer").then(|| token.trim())
}

/// Compares two tokens in time that depends on their length only.
pub fn same_token(presented: &[u8], expected: &[u8]) -> bool {
    if presented.len() != expected.len() {
        return false;
    }
    let diff = presented
        .iter()
        .zip(expected)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b));
    std::hint::black_box(diff) == 0
}

/// The HTTP status of an [`ApiError`], by its code. Fixed: programs may rely
/// on it (ADR 0015). No wildcard arm, so a new code must be given one.
pub fn status_for(code: ErrorCode) -> StatusCode {
    match code {
        ErrorCode::NotFound => StatusCode::NOT_FOUND,
        ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
        ErrorCode::Conflict => StatusCode::CONFLICT,
        ErrorCode::PermissionDenied => StatusCode::FORBIDDEN,
        ErrorCode::Unsupported => StatusCode::UNPROCESSABLE_ENTITY,
        ErrorCode::Busy => StatusCode::SERVICE_UNAVAILABLE,
        ErrorCode::Io | ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        ErrorCode::Cancelled => StatusCode::GONE,
    }
}

async fn blocking<T, F>(f: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, ApiError> + Send + 'static,
{
    tokio::task::spawn_blocking(f).await.map_err(|e| {
        ApiError::new(ErrorCode::Internal, "The app could not finish the request.")
            .with_detail(e.to_string())
    })?
}

fn error(e: &ApiError) -> Response<Full> {
    let json = serde_json::to_vec(e).unwrap_or_else(|_| b"{}".to_vec());
    json_response(status_for(e.code), json)
}

/// A refusal by the transport itself (not a service), as an [`ApiError`]:
/// `permissionDenied` for 401 and 403, `invalidRequest` otherwise.
fn refuse(status: StatusCode, message: &str) -> Response<Full> {
    let code = match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ErrorCode::PermissionDenied,
        _ => ErrorCode::InvalidRequest,
    };
    let json = serde_json::to_vec(&ApiError::new(code, message)).unwrap_or_else(|_| b"{}".to_vec());
    let mut response = json_response(status, json);
    if status == StatusCode::UNAUTHORIZED {
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    }
    response
}

fn json_response(status: StatusCode, json: Vec<u8>) -> Response<Full> {
    let mut response = Response::new(Full::new(json));
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_hosts_with_this_port_are_allowed() {
        assert!(allowed_host("127.0.0.1:4100", 4100));
        assert!(allowed_host("localhost:4100", 4100));
        assert!(allowed_host("LocalHost:4100", 4100));
        assert!(!allowed_host("127.0.0.1:4101", 4100));
        assert!(!allowed_host("127.0.0.1", 4100));
        assert!(!allowed_host("evil.example:4100", 4100));
        assert!(!allowed_host("localhost.evil.example:4100", 4100));
        assert!(!allowed_host("[::1]:4100", 4100));
        assert!(!allowed_host("", 4100));
    }

    #[test]
    fn tokens_match_only_when_equal() {
        assert!(same_token(b"abcd", b"abcd"));
        assert!(!same_token(b"abce", b"abcd"));
        assert!(!same_token(b"abc", b"abcd"));
        assert!(!same_token(b"", b"abcd"));
    }

    #[test]
    fn every_error_code_has_a_fixed_status() {
        let table = [
            (ErrorCode::NotFound, 404),
            (ErrorCode::InvalidRequest, 400),
            (ErrorCode::Conflict, 409),
            (ErrorCode::PermissionDenied, 403),
            (ErrorCode::Unsupported, 422),
            (ErrorCode::Busy, 503),
            (ErrorCode::Io, 500),
            (ErrorCode::Cancelled, 410),
            (ErrorCode::Internal, 500),
        ];
        for (code, status) in table {
            assert_eq!(status_for(code).as_u16(), status, "{code:?}");
        }
    }
}
