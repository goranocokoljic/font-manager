//! Font Manager MCP server.
//!
//! A thin adapter: maps MCP tool calls to `fm-api` requests and nothing more.
//! Hosted by the running app over streamable HTTP on localhost, with a stdio
//! shim for clients that need it. The shim reaches the app through
//! `fm-client` (the local API, ADR 0015), so it has no HTTP code of its own.
//!
//! The app serves [`HttpServer`] at `/mcp` on its local API endpoint
//! (`fm_endpoint::Endpoint::start_with_mcp`, E27.1): the endpoint checks the
//! token and whether the user turned the server on, then hands the request
//! to rmcp's streamable HTTP service ([`HttpServer::service`]). This crate
//! has no HTTP crate of its own (E25.4); the app holds the few lines of glue.

use std::sync::{Arc, OnceLock};

use fm_api::settings::MCP_SERVER_NAME;
use fm_api::{API_VERSION, Api};
use rmcp::ServerHandler;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};

/// The MCP server over one `fm-api`: the app's own when it hosts the
/// server, or the running app's local API for the stdio shim.
#[derive(Clone)]
pub struct McpServer {
    api: Arc<dyn Api>,
}

impl std::fmt::Debug for McpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpServer").finish_non_exhaustive()
    }
}

impl McpServer {
    /// A server over `api`.
    pub fn new(api: Arc<dyn Api>) -> Self {
        Self { api }
    }

    /// A server over the running app ([`fm_client::running_app`]), or `None`
    /// when the app is not running.
    pub fn for_running_app() -> Option<Self> {
        fm_client::running_app().map(Self::new)
    }

    /// The API the tools call.
    pub fn api(&self) -> &Arc<dyn Api> {
        &self.api
    }
}

impl ServerHandler for McpServer {
    /// `font-manager` at the `fm-api` contract version, with tools (none
    /// until E27.2).
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new(MCP_SERVER_NAME, API_VERSION.to_string()),
        )
    }
}

/// rmcp's streamable HTTP service: `handle(request).await` answers one
/// HTTP request.
pub type HttpService = StreamableHttpService<McpServer, NeverSessionManager>;

/// The largest MCP request body, in bytes: the endpoint's own limit
/// (`fm_endpoint::MAX_BODY_BYTES`, ADR 0015).
pub const MAX_REQUEST_BYTES: usize = 32 * 1024 * 1024;

/// The MCP server over streamable HTTP, for the endpoint's `/mcp` mount.
///
/// Stateless: every POST is answered on its own (JSON, or a short event
/// stream when a tool reports progress), so no session or task outlives a
/// request and turning the server off leaves nothing running. The rmcp
/// service is built on the first request, so an app with the server off
/// builds nothing.
pub struct HttpServer {
    api: Arc<dyn Api>,
    service: OnceLock<HttpService>,
}

impl std::fmt::Debug for HttpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpServer")
            .field("built", &self.service.get().is_some())
            .finish_non_exhaustive()
    }
}

impl HttpServer {
    /// The server over `api`; nothing is built yet.
    pub fn new(api: Arc<dyn Api>) -> Self {
        Self {
            api,
            service: OnceLock::new(),
        }
    }

    /// Whether a request built the rmcp service yet.
    pub fn is_built(&self) -> bool {
        self.service.get().is_some()
    }

    /// The rmcp service, built on the first call (a cheap clone after).
    pub fn service(&self) -> HttpService {
        self.service
            .get_or_init(|| {
                let api = Arc::clone(&self.api);
                let config = StreamableHttpServerConfig::default()
                    .with_legacy_session_mode(false)
                    .with_json_response(true)
                    .with_sse_keep_alive(None)
                    .with_max_request_body_bytes(MAX_REQUEST_BYTES);
                StreamableHttpService::new(
                    move || Ok(McpServer::new(Arc::clone(&api))),
                    Arc::new(NeverSessionManager::default()),
                    config,
                )
            })
            .clone()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use fm_api::LibraryService as _;
    use fm_api::mock::MockApi;

    #[test]
    fn the_server_calls_the_api_it_holds() {
        let server = McpServer::new(Arc::new(MockApi::new()));
        assert_eq!(
            server.api().app_info().unwrap(),
            MockApi::new().app_info().unwrap()
        );
    }

    #[test]
    fn the_server_is_font_manager_at_the_api_version_with_tools() {
        let info = McpServer::new(Arc::new(MockApi::new())).get_info();
        assert_eq!(info.server_info.name, "font-manager");
        assert_eq!(info.server_info.version, API_VERSION.to_string());
        assert!(info.capabilities.tools.is_some());
    }

    #[test]
    fn a_new_server_builds_nothing_until_asked() {
        let server = HttpServer::new(Arc::new(MockApi::new()));
        assert!(!server.is_built());
        let _ = server.service();
        assert!(server.is_built());
    }
}
