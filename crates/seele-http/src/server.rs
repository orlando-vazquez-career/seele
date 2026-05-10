//! HTTP server skeleton: axum App + Router + AppState + middleware base.
//!
//! Real handlers live in `handlers/` modules and are wired here. The skeleton
//! exposes `/health` and `/version`; everything else lands in subsequent
//! Sprint-03 blocks (B/C/D).

use std::net::SocketAddr;
use std::sync::Arc;

use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde_json::json;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::handlers;
use crate::service::SeeleService;

/// Shared state every handler sees via `axum::extract::State<AppState>`.
pub type AppState = Arc<SeeleService>;

#[derive(Clone)]
pub struct ServerConfig {
    pub addr: SocketAddr,
    pub cors_origins: Vec<String>,
    /// When set, every route except `/health` and `/version` requires a
    /// `Authorization: Bearer <token>` header. Wired in block D.
    pub auth_bearer: Option<String>,
}

impl ServerConfig {
    pub fn loopback(port: u16) -> Self {
        Self {
            addr: SocketAddr::from(([127, 0, 0, 1], port)),
            cors_origins: Vec::new(),
            auth_bearer: None,
        }
    }
}

pub struct Server {
    pub service: SeeleService,
    pub config: ServerConfig,
}

impl Server {
    pub fn new(service: SeeleService, config: ServerConfig) -> Self {
        Self { service, config }
    }

    /// Build the `axum::Router`. Public so tests can mount it against an
    /// in-process listener.
    pub fn router(&self) -> Router {
        let state: AppState = Arc::new(self.service.clone());

        let cors = if self.config.cors_origins.is_empty() {
            CorsLayer::new()
        } else {
            // Block D refines per-origin allowlist; today we open up
            // permissively when any origin is requested.
            CorsLayer::new().allow_origin(Any)
        };

        Router::new()
            .route("/health", get(health))
            .route("/version", get(version))
            .route(
                "/memories",
                post(handlers::save_memory).get(handlers::list_memories),
            )
            .route(
                "/memories/{id}",
                get(handlers::get_memory).delete(handlers::soft_delete_memory),
            )
            .route("/memories/{id}/restore", post(handlers::restore_memory))
            .route("/memories/{id}/links", get(handlers::list_links_for_memory))
            .route("/search", post(handlers::search_memories))
            .route(
                "/sessions",
                post(handlers::start_session).get(handlers::list_sessions),
            )
            .route("/sessions/{id}", get(handlers::get_session))
            .route("/sessions/{id}/end", put(handlers::end_session))
            .route("/sessions/{id}/abort", put(handlers::abort_session))
            .route("/links", post(handlers::create_link))
            .route("/links/{id}", delete(handlers::delete_link))
            .with_state(state)
            .layer(TraceLayer::new_for_http())
            .layer(cors)
            .layer(CompressionLayer::new())
    }

    /// Bind + serve. Blocks until the server stops.
    pub async fn run(self) -> anyhow::Result<()> {
        let app = self.router();
        let listener = tokio::net::TcpListener::bind(&self.config.addr).await?;
        let addr = listener.local_addr()?;
        tracing::info!(%addr, "seele http listening");
        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({"status": "ok"}))
}

async fn version() -> Json<serde_json::Value> {
    Json(json!({
        "name": "seele",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
