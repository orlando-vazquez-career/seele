# Sprint-03 Bloque B — HTTP handlers básicos

**Tema**: 4 handlers HTTP más usados: save, search (con regla anti-empty-query), get-by-id, list.

**Pre-requisitos**: Bloque A cerrado.

## Tareas atómicas

### B.1 — Service methods

Agregar a `crates/seele-http/src/service.rs` los métodos:

```rust
impl SeeleService {
    pub fn save_observation(&self, input: SaveRequest) -> Result<SaveResponse, ServiceError>;
    pub fn search_observations(&self, req: SearchRequest) -> Result<SearchResponse, ServiceError>;
    pub fn get_observation(&self, id: SeeleId) -> Result<Option<Observation>, ServiceError>;
    pub fn list_observations(&self, q: ListRequest) -> Result<Vec<Observation>, ServiceError>;
}
```

`ServiceError` enum nuevo o reuso `ApiError` directamente. Decisión: dejar `ServiceError` aparte para que MCP también lo use sin depender de HTTP types. `impl From<ServiceError> for ApiError`.

### B.2 — Request/Response DTOs

`crates/seele-http/src/dto.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SaveRequest {
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub topic_key: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct SaveResponse {
    pub id: String,
    pub outcome: &'static str, // "created" | "upserted_topic" | "duplicate_merged"
    pub revision_count: Option<u32>,
    pub duplicate_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub include_purist: bool,
    #[serde(default)]
    pub include_annotations: bool,
    #[serde(default)]
    pub score_boost_multiplier: f64,
}

#[derive(Debug, Serialize)]
pub struct SearchHitDto {
    pub id: String,
    pub title: String,
    pub content: String,
    pub project: Option<String>,
    pub scope: String,
    pub r#type: String,
    pub score: f64,
    pub fts_rank: Option<usize>,
    pub vec_rank: Option<usize>,
    pub created_at: i64,
    pub metadata: serde_json::Value,
    pub annotations: Vec<AnnotationDto>,
}

#[derive(Debug, Serialize)]
pub struct AnnotationDto {
    pub kind: &'static str, // "supersedes" | "superseded_by" | "conflicts_with" | "contested_by"
    pub other_id: String,
    pub other_title: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub hits: Vec<SearchHitDto>,
}

#[derive(Debug, Deserialize)]
pub struct ListRequest {
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub include_deleted: bool,
}
```

### B.3 — Handlers HTTP

`crates/seele-http/src/handlers/memories.rs`:

```rust
pub async fn save(
    State(svc): State<Arc<SeeleService>>,
    Json(req): Json<SaveRequest>,
) -> Result<Json<SaveResponse>> {
    let resp = svc.save_observation(req)?;
    Ok(Json(resp))
}

pub async fn search(
    State(svc): State<Arc<SeeleService>>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<SearchResponse>> {
    // ANTI-empty-query (Cloven 2026-05-10): rechazar empty + no filters
    if req.query.trim().is_empty()
        && req.project.is_none()
        && req.scope.is_none()
        && req.kind.is_none()
    {
        return Err(ApiError::BadRequest(
            "empty query requires at least one filter (project, scope, or kind)".into(),
        ));
    }
    let resp = svc.search_observations(req)?;
    Ok(Json(resp))
}

pub async fn get_by_id(
    State(svc): State<Arc<SeeleService>>,
    Path(id): Path<String>,
) -> Result<Json<Observation>> {
    let parsed: SeeleId = id.parse().map_err(|e: SeeleError| {
        ApiError::BadRequest(format!("invalid id: {e}"))
    })?;
    let obs = svc.get_observation(parsed)?
        .ok_or_else(|| ApiError::NotFound(format!("observation {id}")))?;
    Ok(Json(obs))
}

pub async fn list(
    State(svc): State<Arc<SeeleService>>,
    Query(req): Query<ListRequest>,
) -> Result<Json<Vec<Observation>>> {
    Ok(Json(svc.list_observations(req)?))
}
```

### B.4 — Wire handlers en Router

En `server.rs::router()`, agregar:

```rust
.route("/memories", post(handlers::memories::save).get(handlers::memories::list))
.route("/memories/{id}", get(handlers::memories::get_by_id))
.route("/search", post(handlers::memories::search))
```

### B.5 — Tests E2E

`crates/seele-http/tests/handlers_basicos.rs`:

```rust
#[tokio::test]
async fn save_and_get_roundtrip() { ... }

#[tokio::test]
async fn search_returns_saved_observation() { ... }

#[tokio::test]
async fn list_returns_recent_in_project() { ... }

#[tokio::test]
async fn search_empty_query_without_filters_rejected_400() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let r = client.post(format!("{base}/search"))
        .json(&serde_json::json!({"query": ""}))
        .send().await.unwrap();
    assert_eq!(r.status(), 400);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["code"], "BAD_REQUEST");
    assert!(body["message"].as_str().unwrap().contains("filter"));
}

#[tokio::test]
async fn search_empty_query_with_project_accepted() {
    // empty query + project = list recent in project (Sprint-02 path).
    // ...
}

#[tokio::test]
async fn get_by_id_not_found_returns_404() { ... }

#[tokio::test]
async fn save_invalid_scope_returns_400() { ... }
```

## Criterios de aceptación del bloque B

1. `cargo test -p seele-http` verde con ~6 tests nuevos.
2. POST `/memories` retorna 200 + JSON con ID.
3. POST `/search` con query no vacío retorna hits.
4. POST `/search` con query vacío + sin filters retorna 400.
5. POST `/search` con query vacío + project filter retorna list por created_at desc.
6. GET `/memories/{invalid-id}` retorna 400.
7. GET `/memories/{unknown-valid-ulid}` retorna 404.

## Commit del bloque B

```
git add -A
git commit -m "sprint-03 bloque-B — http handlers basicos (save, search, get, list + anti-empty-query)"
```
