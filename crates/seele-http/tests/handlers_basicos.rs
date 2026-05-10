//! Bloque B — HTTP handlers basicos E2E tests.
//!
//! Spawns the server with a tempdir DB + FakeEmbedder and exercises the
//! 4 core endpoints: save / search / get-by-id / list. Also exercises the
//! anti-empty-query gate flagged by Cloven (2026-05-10).

use std::sync::Arc;

use seele_embedder::{Embedder, FakeEmbedder};
use seele_http::{SeeleService, Server, ServerConfig};
use seele_storage::init_db;
use serde_json::json;
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
async fn save_and_get_roundtrip() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();

    let save = client
        .post(format!("{base}/memories"))
        .json(&json!({
            "title": "hello",
            "content": "the body of the memory",
            "project": "p",
            "type": "decision",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(save.status(), 200);
    let saved: serde_json::Value = save.json().await.unwrap();
    let id = saved["id"].as_str().expect("id in response");
    assert_eq!(saved["outcome"], "created");

    let got = client
        .get(format!("{base}/memories/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(got.status(), 200);
    let body: serde_json::Value = got.json().await.unwrap();
    assert_eq!(body["title"], "hello");
    assert_eq!(body["content"], "the body of the memory");
    assert_eq!(body["project"], "p");
    assert_eq!(body["type"], "decision");
}

#[tokio::test]
async fn search_returns_saved_observation() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    client
        .post(format!("{base}/memories"))
        .json(&json!({
            "title": "wal",
            "content": "Postgres uses WAL for crash recovery and high availability",
            "project": "p",
        }))
        .send()
        .await
        .unwrap();

    let r = client
        .post(format!("{base}/search"))
        .json(&json!({"query": "postgres", "project": "p"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body: serde_json::Value = r.json().await.unwrap();
    let hits = body["hits"].as_array().expect("hits array");
    assert!(!hits.is_empty(), "expected at least one hit");
    assert!(hits.iter().any(|h| h["title"].as_str() == Some("wal")));
}

#[tokio::test]
async fn list_returns_recent_in_project() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    for i in 0..5 {
        client
            .post(format!("{base}/memories"))
            .json(&json!({
                "title": format!("memo {i}"),
                "content": format!("content {i}"),
                "project": "p",
            }))
            .send()
            .await
            .unwrap();
    }

    let r = client
        .get(format!("{base}/memories?project=p&limit=3"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body: serde_json::Value = r.json().await.unwrap();
    let arr = body.as_array().expect("array");
    assert!(arr.len() <= 3);
    for o in arr {
        assert_eq!(o["project"], "p");
    }
}

#[tokio::test]
async fn search_empty_query_without_filters_rejected_400() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let r = client
        .post(format!("{base}/search"))
        .json(&json!({"query": ""}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["code"], "BAD_REQUEST");
    assert!(body["message"]
        .as_str()
        .unwrap()
        .to_lowercase()
        .contains("filter"));
}

#[tokio::test]
async fn search_empty_query_with_project_accepted() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    client
        .post(format!("{base}/memories"))
        .json(&json!({
            "title": "marker",
            "content": "marker content",
            "project": "p",
        }))
        .send()
        .await
        .unwrap();

    let r = client
        .post(format!("{base}/search"))
        .json(&json!({"query": "", "project": "p"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body: serde_json::Value = r.json().await.unwrap();
    let hits = body["hits"].as_array().expect("hits array");
    assert!(!hits.is_empty());
}

#[tokio::test]
async fn get_by_id_invalid_returns_400() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let r = client
        .get(format!("{base}/memories/not-a-ulid"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["code"], "BAD_REQUEST");
}

#[tokio::test]
async fn get_by_id_valid_but_unknown_returns_404() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    // A syntactically valid ULID that does not exist in the DB.
    let r = client
        .get(format!("{base}/memories/01HW3X000000000000000000RX"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["code"], "NOT_FOUND");
}

#[tokio::test]
async fn save_invalid_scope_returns_400() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let r = client
        .post(format!("{base}/memories"))
        .json(&json!({
            "title": "t",
            "content": "c",
            "scope": "weird",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["code"], "BAD_REQUEST");
}
