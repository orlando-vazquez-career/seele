//! HTTP server skeleton: axum App + Router + AppState + middleware base.
//!
//! Real handlers live in `handlers/` modules and are wired here. The skeleton
//! exposes `/health` and `/version`; everything else lands in subsequent
//! Sprint-03 blocks (B/C/D).

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::FromRef;
use axum::middleware;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde_json::json;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::auth::require_bearer;
use crate::handlers;
use crate::openapi;
use crate::service::SeeleService;

/// Shared state every handler sees via `axum::extract::State<AppState>`.
/// Existing handlers use `State<Arc<SeeleService>>` (via the FromRef impl
/// below); the chat handler pulls the whole `AppState`.
#[derive(Clone)]
pub struct AppState {
    pub service: Arc<SeeleService>,
    pub chat: Option<Arc<ChatProviderConfig>>,
}

impl FromRef<AppState> for Arc<SeeleService> {
    fn from_ref(input: &AppState) -> Self {
        input.service.clone()
    }
}

impl FromRef<AppState> for Option<Arc<ChatProviderConfig>> {
    fn from_ref(input: &AppState) -> Self {
        input.chat.clone()
    }
}

#[derive(Clone)]
pub struct ServerConfig {
    pub addr: SocketAddr,
    pub cors_origins: Vec<String>,
    /// When set, every route except `/health`, `/version`, `/docs/*`,
    /// `/openapi.json` requires `Authorization: Bearer <token>`.
    pub auth_bearer: Option<String>,
    /// Expose ENGRAM-compatible aliases (`POST /save`, `GET /show/{id}`)
    /// alongside the canonical SEELE routes. See ADR-13.
    pub legacy_engram_paths: bool,
    /// AI chat provider config (POST /chat). None = endpoint disabled.
    pub chat: Option<ChatProviderConfig>,
}

/// Chat-with-DB provider config. None of these fields ever leave the
/// machine — the API key stays on the box running `seele serve`.
#[derive(Clone, Debug)]
pub struct ChatProviderConfig {
    /// Provider family. Supported: `"minimax"`, `"openai"`, `"openrouter"`,
    /// `"together"`, `"groq"`, `"deepseek"`, `"anthropic"`, or any other
    /// label — anything that's not `"anthropic"` is treated as
    /// OpenAI-compatible and uses `endpoint` as `/v1/chat/completions`.
    pub provider: String,
    pub api_key: String,
    pub model: String,
    /// OpenAI-compatible providers: the full `/v1/chat/completions` URL.
    /// Ignored for `"anthropic"`.
    pub endpoint: Option<String>,
}

impl ServerConfig {
    pub fn loopback(port: u16) -> Self {
        Self {
            addr: SocketAddr::from(([127, 0, 0, 1], port)),
            cors_origins: Vec::new(),
            auth_bearer: None,
            legacy_engram_paths: false,
            chat: None,
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
        let state = AppState {
            service: Arc::new(self.service.clone()),
            chat: self.config.chat.clone().map(Arc::new),
        };

        let cors = if self.config.cors_origins.is_empty() {
            CorsLayer::new()
        } else {
            // Block D refines per-origin allowlist; today we open up
            // permissively when any origin is requested.
            CorsLayer::new().allow_origin(Any)
        };

        // Routes that require auth (when enabled). Built first so the
        // auth middleware wraps only these; `/health`, `/version`, docs
        // stay public.
        let mut protected = Router::new()
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
            .route(
                "/relations",
                post(handlers::create_relation).get(handlers::list_relations),
            )
            .route("/relations/{id}/judge", put(handlers::judge_relation))
            .route("/conflicts", get(handlers::list_pending_conflicts))
            .route("/stats", get(handlers::get_stats))
            .route("/embedder", get(handlers::get_embedder_info))
            .route("/chat", post(handlers::chat))
            .route("/chat/info", get(handlers::chat_info));

        if self.config.legacy_engram_paths {
            // ADR-13: ENGRAM-compatible aliases. Same handlers, alternate
            // paths. New SEELE endpoints (sessions, relations, etc.) are
            // NOT exposed under legacy paths — they don't exist in ENGRAM.
            protected = protected
                .route("/save", post(handlers::save_memory))
                .route("/show/{id}", get(handlers::get_memory));
        }

        let protected = protected.with_state(state);

        let protected = if let Some(token) = self.config.auth_bearer.clone() {
            protected.layer(middleware::from_fn(move |req, next| {
                let token = token.clone();
                async move { require_bearer(token, req, next).await }
            }))
        } else {
            protected
        };

        Router::new()
            .route("/health", get(health))
            .route("/version", get(version))
            .merge(protected)
            .merge(openapi::routes())
            .layer(TraceLayer::new_for_http())
            .layer(cors)
            .layer(CompressionLayer::new())
    }

    /// Bind + serve. Blocks until the server stops.
    ///
    /// Writes a single grepeable line to stderr once the listener is up:
    /// `seele http listening on http://<addr>`. This lets `--port 0`
    /// callers (tests, scripts) discover the OS-chosen port without
    /// the listener-then-drop race.
    pub async fn run(self) -> anyhow::Result<()> {
        let app = self.router();
        let listener = tokio::net::TcpListener::bind(&self.config.addr).await?;
        let addr = listener.local_addr()?;
        eprintln!("seele http listening on http://{addr}");
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
