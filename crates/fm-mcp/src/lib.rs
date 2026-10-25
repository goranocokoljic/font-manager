//! Font Manager MCP server.
//!
//! A thin adapter: maps MCP tool calls to `fm-api` requests and nothing more.
//! Hosted by the running app over streamable HTTP on localhost, with a stdio
//! shim for clients that need it. The shim reaches the app through
//! `fm-client` (the local API, ADR 0015), so it has no HTTP code of its own.

use std::sync::Arc;

use fm_api::Api;

/// The MCP server over one `fm-api`: the app's own when it hosts the
/// server, or the running app's local API for the stdio shim.
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
}
