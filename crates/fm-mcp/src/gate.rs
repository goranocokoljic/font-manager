//! Which mutating tools an MCP client may call.
//!
//! Read tools (search, details, coverage) always run. The tools that change
//! something — activate, deactivate, tag, export — ask a [`ToolGate`] first.
//! The app's gate is [`Grants`]: it asks `fm-core` through the API, which
//! checks the grants the user gave in Settings → AI assistants and, for a
//! destructive tool, waits for the user's Allow (E27.6).

use fm_api::settings::McpToolCall;
use fm_api::{Api, ApiError, ErrorCode};

/// Decides whether a mutating tool call may run.
pub trait ToolGate: Send + Sync {
    /// `Ok` when `call` may run now on `api`; the error is the tool error
    /// the client gets, and the tool is then not called.
    fn check(&self, api: &dyn Api, call: &McpToolCall) -> Result<(), ApiError>;
}

/// The default gate: the grants and confirmations of `fm-core`, read through
/// the API on every call, so a changed grant applies without a restart.
#[derive(Debug, Clone, Copy, Default)]
pub struct Grants;

impl ToolGate for Grants {
    fn check(&self, api: &dyn Api, call: &McpToolCall) -> Result<(), ApiError> {
        api.authorize_mcp_tool(call.clone())
    }
}

/// A gate that refuses every mutating tool.
#[derive(Debug, Clone, Copy, Default)]
pub struct RefuseMutations;

impl ToolGate for RefuseMutations {
    fn check(&self, _api: &dyn Api, call: &McpToolCall) -> Result<(), ApiError> {
        Err(ApiError::new(
            ErrorCode::PermissionDenied,
            "Font Manager does not allow AI assistants to change fonts here.",
        )
        .with_detail(format!("tool {} refused by the tool gate", call.tool)))
    }
}

/// A gate that allows every mutating tool (tests).
#[derive(Debug, Clone, Copy, Default)]
pub struct AllowAll;

impl ToolGate for AllowAll {
    fn check(&self, _api: &dyn Api, _call: &McpToolCall) -> Result<(), ApiError> {
        Ok(())
    }
}
