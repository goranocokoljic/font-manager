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
//! The endpoint knows nothing about fonts and keeps no state but the socket:
//! the token and the discovery file belong to `fm-core`.

mod body;
pub mod routes;

use std::convert::Infallible;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpListener as StdListener};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use fm_api::{Api, ApiError, ErrorCode};
use hyper::body::Incoming;
use hyper::header::{self, HeaderValue};
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::sync::oneshot;

use crate::body::{Full, read_limited};

/// The largest request body, in bytes.
pub const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

/// The path prefix of every route.
pub const ROUTE_PREFIX: &str = "/v1/";

/// A running endpoint. Dropping it (or [`Endpoint::stop`]) closes the socket.
#[derive(Debug)]
pub struct Endpoint {
    addr: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Endpoint {
    /// Binds `127.0.0.1` on a port the OS picks and serves `api` on a
    /// thread of its own. Returns once the socket is bound.
    pub fn start(api: Arc<dyn Api>) -> io::Result<Self> {
        let listener = StdListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()?;
        let (stop, stopped) = oneshot::channel();
        let thread = std::thread::Builder::new()
            .name("fm-endpoint".into())
            .spawn(move || {
                runtime.block_on(async move {
                    let accept = match tokio::net::TcpListener::from_std(listener) {
                        Ok(listener) => tokio::spawn(accept_loop(listener, api, addr.port())),
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
            stop: Some(stop),
            thread: Some(thread),
        })
    }

    /// Where it listens: always `127.0.0.1:<port>`.
    pub fn addr(&self) -> SocketAddr {
        self.addr
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

async fn accept_loop(listener: tokio::net::TcpListener, api: Arc<dyn Api>, port: u16) {
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
        let api = Arc::clone(&api);
        tokio::spawn(async move {
            let service = hyper::service::service_fn(move |req| {
                let api = Arc::clone(&api);
                async move { Ok::<_, Infallible>(handle(api, port, req).await) }
            });
            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await
            {
                tracing::debug!(error = %e, "local API connection ended");
            }
        });
    }
}

/// One request, in this order: `Host` and `Origin` (403), the token (401),
/// the method and route (405, 404), the body (413, 400), then the service.
async fn handle(api: Arc<dyn Api>, port: u16, req: Request<Incoming>) -> Response<Full> {
    if let Err(refused) = check_origin(&req, port) {
        return refused;
    }
    let presented = bearer(&req).map(str::to_owned);
    let info_api = Arc::clone(&api);
    let current = match blocking(move || info_api.endpoint_info()).await {
        Ok(info) => info.token,
        Err(e) => return error(&e),
    };
    if !presented.is_some_and(|t| same_token(t.as_bytes(), current.as_bytes())) {
        return refuse(
            StatusCode::UNAUTHORIZED,
            "A valid token is required (Authorization: Bearer <token>).",
        );
    }
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
    let answer = blocking(move || {
        routes::dispatch(api.as_ref(), &method, &body).unwrap_or_else(|| {
            Err(ApiError::new(
                ErrorCode::Internal,
                "The route has no method.",
            ))
        })
    })
    .await;
    match answer {
        Ok(json) => json_response(StatusCode::OK, json),
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
