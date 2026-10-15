//! Font Manager MCP server.
//!
//! A thin adapter: maps MCP tool calls to `fm-api` requests and nothing more.
//! Hosted by the running app over streamable HTTP on localhost, with a stdio
//! shim for clients that need it.
