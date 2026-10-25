//! Which mutating tools an MCP client may call.
//!
//! Read tools (search, details, coverage) always run. The tools that change
//! something — activate, deactivate, tag, export — ask a [`ToolGate`] first.
//! The default refuses them all; the grants the user gives in Settings come
//! with E27.6.

/// Decides whether a mutating tool may run.
pub trait ToolGate: Send + Sync {
    /// Whether the mutating tool `tool` (its MCP name) may run now.
    fn allows(&self, tool: &str) -> bool;
}

/// The default gate: refuses every mutating tool.
#[derive(Debug, Clone, Copy, Default)]
pub struct RefuseMutations;

impl ToolGate for RefuseMutations {
    fn allows(&self, _tool: &str) -> bool {
        false
    }
}

/// A gate that allows every mutating tool (tests, and the grants of E27.6).
#[derive(Debug, Clone, Copy, Default)]
pub struct AllowAll;

impl ToolGate for AllowAll {
    fn allows(&self, _tool: &str) -> bool {
        true
    }
}
