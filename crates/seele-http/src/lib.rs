//! SEELE HTTP API — axum REST server with utoipa OpenAPI docs (utoipa wired
//! in block D).
//!
//! The crate provides:
//! - `SeeleService` — the application-layer service used by both HTTP
//!   handlers (this crate) and MCP tools (`seele-mcp`, block E).
//! - `Server` — thin wrapper that builds the router and binds a TCP
//!   listener.
//! - `ApiError` / `ErrorBody` — uniform JSON error envelope.

pub mod dto;
pub mod error;
pub mod handlers;
pub mod server;
pub mod service;

pub use error::{ApiError, ErrorBody, Result};
pub use server::{AppState, Server, ServerConfig};
pub use service::SeeleService;
