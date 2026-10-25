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
//!
//! The tools (E27.2) are in [`tools`]: one `fm-api` call each. The ones that
//! change something ask a [`ToolGate`] first: by default [`Grants`], the
//! user's per-tool grants and confirmations in `fm-core` (E27.6).
//! The resources and prompts (E27.5) are in [`resources`] and [`prompts`].

pub mod gate;
pub mod prompts;
pub mod resources;
pub mod tools;

use std::sync::{Arc, OnceLock};

use fm_api::settings::MCP_SERVER_NAME;
use fm_api::{API_VERSION, Api};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, GetPromptRequestParams, GetPromptResponse,
    Implementation, ListPromptsResult, ListResourceTemplatesResult, ListToolsResult,
    PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResponse, ServerCapabilities,
    ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, RoleServer, ServerHandler};

pub use gate::{AllowAll, Grants, RefuseMutations, ToolGate};

/// The MCP server over one `fm-api`: the app's own when it hosts the
/// server, or the running app's local API for the stdio shim.
#[derive(Clone)]
pub struct McpServer {
    api: Arc<dyn Api>,
    gate: Arc<dyn ToolGate>,
}

impl std::fmt::Debug for McpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpServer").finish_non_exhaustive()
    }
}

impl McpServer {
    /// A server over `api` whose mutating tools ask the user's grants
    /// ([`Grants`]).
    pub fn new(api: Arc<dyn Api>) -> Self {
        Self::with_gate(api, Arc::new(Grants))
    }

    /// A server over `api` whose mutating tools ask `gate`.
    pub fn with_gate(api: Arc<dyn Api>, gate: Arc<dyn ToolGate>) -> Self {
        Self { api, gate }
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
    /// `font-manager` at the `fm-api` contract version, with tools,
    /// resources and prompts.
    fn get_info(&self) -> ServerConfig {
        let capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_resources()
            .enable_prompts()
            .build();
        ServerConfig::new(capabilities).with_server_info(Implementation::new(
            MCP_SERVER_NAME,
            API_VERSION.to_string(),
        ))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(tools::list()))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        tools::list().into_iter().find(|t| t.name == name)
    }

    /// Runs the tool on a blocking thread: the services are synchronous
    /// (ADR 0006). An unknown tool is a protocol error; everything the API
    /// says no to is a tool error.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let api = Arc::clone(&self.api);
        let gate = Arc::clone(&self.gate);
        let name = request.name.to_string();
        let arguments = request.arguments.unwrap_or_default();
        let unknown = format!("There is no tool named {name}.");
        let result = tokio::task::spawn_blocking(move || {
            tools::call(api.as_ref(), gate.as_ref(), &name, arguments)
        })
        .await
        .map_err(|e| ErrorData::internal_error(format!("The tool stopped: {e}"), None))?;
        result
            .map(CallToolResponse::from)
            .ok_or_else(|| ErrorData::invalid_params(unknown, None))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        Ok(ListResourceTemplatesResult::with_all_items(
            resources::templates(),
        ))
    }

    /// Reads the resource on a blocking thread, like a tool call. An
    /// unknown id is resource-not-found.
    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let api = Arc::clone(&self.api);
        let uri = request.uri;
        tokio::task::spawn_blocking(move || resources::read(api.as_ref(), &uri))
            .await
            .map_err(|e| ErrorData::internal_error(format!("The read stopped: {e}"), None))?
            .map(ReadResourceResponse::from)
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        Ok(ListPromptsResult::with_all_items(prompts::list()))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, ErrorData> {
        prompts::get(&request.name, request.arguments).map(GetPromptResponse::from)
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
    gate: Arc<dyn ToolGate>,
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
    /// The server over `api` whose mutating tools ask the user's grants
    /// ([`Grants`]); nothing is built yet.
    pub fn new(api: Arc<dyn Api>) -> Self {
        Self::with_gate(api, Arc::new(Grants))
    }

    /// The server over `api` whose mutating tools ask `gate`.
    pub fn with_gate(api: Arc<dyn Api>, gate: Arc<dyn ToolGate>) -> Self {
        Self {
            api,
            gate,
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
                let gate = Arc::clone(&self.gate);
                let config = StreamableHttpServerConfig::default()
                    .with_legacy_session_mode(false)
                    .with_json_response(true)
                    .with_sse_keep_alive(None)
                    .with_max_request_body_bytes(MAX_REQUEST_BYTES);
                StreamableHttpService::new(
                    move || Ok(McpServer::with_gate(Arc::clone(&api), Arc::clone(&gate))),
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
    fn the_server_is_font_manager_at_the_api_version_with_tools_resources_and_prompts() {
        let info = McpServer::new(Arc::new(MockApi::new())).get_info();
        assert_eq!(info.server_info.name, "font-manager");
        assert_eq!(info.server_info.version, API_VERSION.to_string());
        assert!(info.capabilities.tools.is_some());
        assert!(info.capabilities.resources.is_some());
        assert!(info.capabilities.prompts.is_some());
    }

    #[test]
    fn a_new_server_builds_nothing_until_asked() {
        let server = HttpServer::new(Arc::new(MockApi::new()));
        assert!(!server.is_built());
        let _ = server.service();
        assert!(server.is_built());
    }
}
