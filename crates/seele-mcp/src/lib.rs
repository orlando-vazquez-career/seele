//! SEELE MCP server — stdio transport for Claude Code, Cursor, OpenCode, etc.
//!
//! The crate exposes a JSON-RPC 2.0 server over a generic
//! `AsyncRead + AsyncWrite` pair (stdio by default). 19 canonical SEELE
//! tools are registered; consumers can rename them at the boundary via
//! `McpServerConfig.tool_prefix` (ADR-13: `mnema` is a first-class
//! ENGRAM-compat alias set).

pub mod jsonrpc;
pub mod server;
pub mod tool_impls;
pub mod tools;

pub use server::{McpServer, McpServerConfig};
pub use tools::{all_tools, build_index, Tool, ToolError};
