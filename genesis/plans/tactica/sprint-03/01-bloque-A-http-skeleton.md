# Sprint-03 Bloque A — HTTP server skeleton + service layer

**Tema**: Crear el skeleton de `seele-http` (axum App + Router + AppState + middleware) y el service layer compartido que MCP también va a usar.

**Pre-requisitos**: Sprint-02 cerrado.

## Tareas atómicas

### A.1 — Cargo.toml de seele-http

Agregar deps reales (hoy es stub):

```toml
[dependencies]
seele-core = { path = "../seele-core" }
seele-storage = { path = "../seele-storage" }
seele-search = { path = "../seele-search" }
seele-embedder = { path = "../seele-embedder" }
tokio.workspace = true
axum.workspace = true
tower.workspace = true
tower-http.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true
chrono.workspace = true

[dev-dependencies]
tempfile.workspace = true
reqwest = { version = "0.12", features = ["json"] }
```

(utoipa va en Bloque D.)

Agregar `reqwest` al `Cargo.toml` workspace si no está.

### A.2 — `seele-http::service` (service layer compartido)

`crates/seele-http/src/service.rs`:

```rust
//! Service layer used by both HTTP handlers and MCP tools. Keeps
//! application logic out of the transport-specific code.

use std::sync::Arc;

use seele_core::id::SeeleId;
use seele_core::memory::Observation;
use seele_embedder::Embedder;
use seele_search::{SearchEngine, SearchHit, SearchQuery};
use seele_storage::{
    ChunkStore, LinkStore, ObservationStore, Pool, PromptStore, RelationStore, SaveInput,
    SaveOutcome, SessionStore,
};

#[derive(Clone)]
pub struct SeeleService {
    pub observations: ObservationStore,
    pub sessions: SessionStore,
    pub links: LinkStore,
    pub relations: RelationStore,
    pub prompts: PromptStore,
    pub chunks: ChunkStore,
    pub search: Arc<SearchEngine>,
    pub embedder: Arc<dyn Embedder>,
    pub pool: Pool,
}

impl SeeleService {
    pub fn new(pool: Pool, embedder: Arc<dyn Embedder>) -> Self {
        let observations = ObservationStore::new(pool.clone());
        let sessions = SessionStore::new(pool.clone());
        let links = LinkStore::new(pool.clone());
        let relations = RelationStore::new(pool.clone());
        let prompts = PromptStore::new(pool.clone());
        let chunks = ChunkStore::new(pool.clone());
        let search_embedder: Box<dyn Embedder> = boxed_embedder(embedder.clone());
        let search = Arc::new(SearchEngine::new(pool.clone(), search_embedder));
        Self {
            observations,
            sessions,
            links,
            relations,
            prompts,
            chunks,
            search,
            embedder,
            pool,
        }
    }
}

fn boxed_embedder(arc: Arc<dyn Embedder>) -> Box<dyn Embedder> {
    Box::new(ArcEmbedder(arc))
}

/// Bridge an `Arc<dyn Embedder>` into a `Box<dyn Embedder>` so the same
/// embedder instance is shared by both the SearchEngine and direct callers.
struct ArcEmbedder(Arc<dyn Embedder>);

impl Embedder for ArcEmbedder {
    fn embed(&self, text: &str) -> seele_embedder::Result<Vec<f32>> {
        self.0.embed(text)
    }
    fn embed_batch(&self, texts: &[&str]) -> seele_embedder::Result<Vec<Vec<f32>>> {
        self.0.embed_batch(texts)
    }
    fn dim(&self) -> usize {
        self.0.dim()
    }
    fn model_id(&self) -> &str {
        self.0.model_id()
    }
    fn expected_sha256(&self) -> Option<&str> {
        self.0.expected_sha256()
    }
}
```

Esto es el bottom del service layer. Las operaciones concretas (`save`, `search`, etc) llegan en bloques B/C.

### A.3 — `seele-http::error::ApiError`

`crates/seele-http/src/error.rs`:

```rust
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "CONFLICT"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL"),
        };
        (
            status,
            Json(ErrorBody {
                code,
                message: self.to_string(),
            }),
        )
            .into_response()
    }
}

impl From<seele_storage::StorageError> for ApiError {
    fn from(e: seele_storage::StorageError) -> Self {
        use seele_storage::StorageError::*;
        match e {
            NotFound(msg) => Self::NotFound(msg),
            InvalidInput(msg) => Self::BadRequest(msg),
            other => Self::Internal(other.to_string()),
        }
    }
}

impl From<seele_search::SearchError> for ApiError {
    fn from(e: seele_search::SearchError) -> Self {
        use seele_search::SearchError::*;
        match e {
            InvalidInput(msg) => Self::BadRequest(msg),
            other => Self::Internal(other.to_string()),
        }
    }
}

pub type Result<T> = std::result::Result<T, ApiError>;
```

### A.4 — `seele-http::server::Server`

`crates/seele-http/src/server.rs`:

```rust
use std::net::SocketAddr;
use std::sync::Arc;

use axum::routing::get;
use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::service::SeeleService;

#[derive(Clone)]
pub struct ServerConfig {
    pub addr: SocketAddr,
    pub cors_origins: Vec<String>,
    pub auth_bearer: Option<String>,  // used in Bloque D
}

pub struct Server {
    pub service: SeeleService,
    pub config: ServerConfig,
}

impl Server {
    pub fn new(service: SeeleService, config: ServerConfig) -> Self {
        Self { service, config }
    }

    pub fn router(&self) -> Router {
        let cors = if self.config.cors_origins.is_empty() {
            CorsLayer::new()
        } else {
            CorsLayer::new().allow_origin(Any) // refined in Bloque D
        };

        Router::new()
            .route("/health", get(handlers::health))
            .route("/version", get(handlers::version))
            // Handlers reales se montan en bloques B/C/D
            .with_state(Arc::new(self.service.clone()))
            .layer(TraceLayer::new_for_http())
            .layer(cors)
            .layer(CompressionLayer::new())
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let app = self.router();
        let listener = tokio::net::TcpListener::bind(&self.config.addr).await?;
        let addr = listener.local_addr()?;
        tracing::info!(%addr, "seele serve listening");
        axum::serve(listener, app).await?;
        Ok(())
    }
}

mod handlers {
    use axum::Json;
    use serde_json::json;

    pub async fn health() -> Json<serde_json::Value> {
        Json(json!({"status": "ok"}))
    }

    pub async fn version() -> Json<serde_json::Value> {
        Json(json!({
            "name": "seele",
            "version": env!("CARGO_PKG_VERSION"),
        }))
    }
}
```

### A.5 — `lib.rs` re-exports

`crates/seele-http/src/lib.rs`:

```rust
//! SEELE HTTP API — axum REST server.

pub mod error;
pub mod server;
pub mod service;

pub use error::{ApiError, ErrorBody};
pub use server::{Server, ServerConfig};
pub use service::SeeleService;
```

### A.6 — Tests skeleton

`crates/seele-http/tests/skeleton.rs`:

```rust
use std::net::SocketAddr;
use std::sync::Arc;

use seele_embedder::{Embedder, FakeEmbedder};
use seele_http::{SeeleService, Server, ServerConfig};
use seele_storage::init_db;
use tempfile::TempDir;

async fn spawn_test_server() -> (TempDir, String) {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let embedder: Arc<dyn Embedder> = Arc::new(FakeEmbedder);
    let service = SeeleService::new(pool, embedder);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Server::new(
        service,
        ServerConfig {
            addr,
            cors_origins: vec![],
            auth_bearer: None,
        },
    )
    .router();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (td, format!("http://{addr}"))
}

#[tokio::test]
async fn health_returns_ok() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let r = client.get(format!("{base}/health")).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn version_returns_pkg_version() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let r = client.get(format!("{base}/version")).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["name"], "seele");
}
```

## Criterios de aceptación del bloque A

1. `cargo build -p seele-http` verde con deps reales.
2. `cargo test -p seele-http` verde con los 2 tests skeleton.
3. `cargo clippy -p seele-http -- -D warnings` verde.
4. Service layer compartido (`SeeleService`) clonable y safe-to-share entre handlers.
5. `Server::router()` retorna un `axum::Router` con middleware base + endpoints `/health` y `/version`.

## Commit del bloque A

```
git add -A
git commit -m "sprint-03 bloque-A — http skeleton (server + service layer + /health + /version)"
```
