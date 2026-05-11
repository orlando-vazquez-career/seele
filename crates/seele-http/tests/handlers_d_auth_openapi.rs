//! Bloque D — auth bearer + OpenAPI + legacy ENGRAM paths.

use std::sync::Arc;

use seele_embedder::{Embedder, FakeEmbedder};
use seele_http::{SeeleService, Server, ServerConfig};
use seele_storage::init_db;
use serde_json::json;
use tempfile::TempDir;

async fn spawn(auth_bearer: Option<&str>, legacy_engram_paths: bool) -> (TempDir, String) {
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
            auth_bearer: auth_bearer.map(str::to_string),
            legacy_engram_paths,
        },
    )
    .router();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (td, format!("http://{addr}"))
}

// -------- Auth --------

#[tokio::test]
async fn auth_disabled_by_default_no_token_required() {
    let (_td, base) = spawn(None, false).await;
    let r = reqwest::Client::new()
        .post(format!("{base}/memories"))
        .json(&json!({"title": "a", "content": "b"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
}

#[tokio::test]
async fn auth_enabled_rejects_missing_token_401() {
    let (_td, base) = spawn(Some("s3cret"), false).await;
    let r = reqwest::Client::new()
        .post(format!("{base}/memories"))
        .json(&json!({"title": "a", "content": "b"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test]
async fn auth_enabled_rejects_wrong_token_401() {
    let (_td, base) = spawn(Some("s3cret"), false).await;
    let r = reqwest::Client::new()
        .post(format!("{base}/memories"))
        .bearer_auth("wrong")
        .json(&json!({"title": "a", "content": "b"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test]
async fn auth_enabled_accepts_valid_token() {
    let (_td, base) = spawn(Some("s3cret"), false).await;
    let r = reqwest::Client::new()
        .post(format!("{base}/memories"))
        .bearer_auth("s3cret")
        .json(&json!({"title": "a", "content": "b"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
}

#[tokio::test]
async fn auth_enabled_health_remains_public() {
    let (_td, base) = spawn(Some("s3cret"), false).await;
    let r = reqwest::Client::new()
        .get(format!("{base}/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
}

#[tokio::test]
async fn auth_enabled_version_remains_public() {
    let (_td, base) = spawn(Some("s3cret"), false).await;
    let r = reqwest::Client::new()
        .get(format!("{base}/version"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
}

#[tokio::test]
async fn auth_enabled_openapi_json_remains_public() {
    let (_td, base) = spawn(Some("s3cret"), false).await;
    let r = reqwest::Client::new()
        .get(format!("{base}/openapi.json"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
}

// -------- OpenAPI --------

#[tokio::test]
async fn openapi_json_exposes_canonical_paths() {
    let (_td, base) = spawn(None, false).await;
    let r = reqwest::Client::new()
        .get(format!("{base}/openapi.json"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let spec: serde_json::Value = r.json().await.unwrap();
    assert_eq!(spec["info"]["title"], "SEELE HTTP API");
    let paths = spec["paths"].as_object().unwrap();
    // Spot check a handful of paths across domains.
    for p in [
        "/memories",
        "/memories/{id}",
        "/search",
        "/sessions",
        "/links",
        "/relations",
        "/conflicts",
        "/stats",
        "/embedder",
    ] {
        assert!(paths.contains_key(p), "missing path {p} in OpenAPI spec");
    }
}

#[tokio::test]
async fn openapi_json_registers_dto_schemas() {
    let (_td, base) = spawn(None, false).await;
    let spec: serde_json::Value = reqwest::Client::new()
        .get(format!("{base}/openapi.json"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let schemas = spec["components"]["schemas"].as_object().unwrap();
    for s in [
        "SaveRequest",
        "SaveResponse",
        "SearchRequest",
        "SearchResponse",
        "ObservationDto",
        "SessionDto",
        "LinkDto",
        "RelationDto",
        "StatsResponse",
        "EmbedderInfo",
        "ErrorBody",
    ] {
        assert!(schemas.contains_key(s), "missing schema {s}");
    }
}

#[tokio::test]
async fn swagger_ui_serves_html() {
    let (_td, base) = spawn(None, false).await;
    let r = reqwest::Client::new()
        .get(format!("{base}/docs/"))
        .send()
        .await
        .unwrap();
    assert!(
        r.status().is_success(),
        "expected 2xx from /docs/, got {}",
        r.status()
    );
    let body = r.text().await.unwrap();
    assert!(body.contains("swagger") || body.contains("Swagger"));
}

// -------- Legacy ENGRAM paths (ADR-13) --------

#[tokio::test]
async fn legacy_paths_disabled_by_default_returns_404() {
    let (_td, base) = spawn(None, false).await;
    let r = reqwest::Client::new()
        .post(format!("{base}/save"))
        .json(&json!({"title": "a", "content": "b"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
}

#[tokio::test]
async fn legacy_save_alias_works_when_enabled() {
    let (_td, base) = spawn(None, true).await;
    let r = reqwest::Client::new()
        .post(format!("{base}/save"))
        .json(&json!({"title": "a", "content": "b"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body: serde_json::Value = r.json().await.unwrap();
    assert!(body["id"].is_string());
}

#[tokio::test]
async fn legacy_show_alias_works_when_enabled() {
    let (_td, base) = spawn(None, true).await;
    // Create via canonical POST /memories
    let saved: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/memories"))
        .json(&json!({"title": "x", "content": "y"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = saved["id"].as_str().unwrap();
    // Read via legacy GET /show/{id}
    let r = reqwest::Client::new()
        .get(format!("{base}/show/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["id"], id);
}

#[tokio::test]
async fn legacy_paths_respect_auth_bearer() {
    let (_td, base) = spawn(Some("s3cret"), true).await;
    // No token → 401
    let r = reqwest::Client::new()
        .post(format!("{base}/save"))
        .json(&json!({"title": "a", "content": "b"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
    // Valid token → 200
    let r = reqwest::Client::new()
        .post(format!("{base}/save"))
        .bearer_auth("s3cret")
        .json(&json!({"title": "a", "content": "b"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
}
