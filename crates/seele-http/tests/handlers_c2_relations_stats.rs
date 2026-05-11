//! Bloque C.2 — relations + stats + embedder E2E tests.

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
            legacy_engram_paths: false,
        },
    )
    .router();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (td, format!("http://{addr}"))
}

async fn save_memory(client: &reqwest::Client, base: &str, title: &str) -> String {
    let r = client
        .post(format!("{base}/memories"))
        .json(&json!({
            "title": title,
            "content": format!("content for {title}"),
            "project": "p",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body: serde_json::Value = r.json().await.unwrap();
    body["id"].as_str().unwrap().to_string()
}

// -------- Relations create / list / judge --------

#[tokio::test]
async fn relation_create_list_judge_lifecycle() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let a = save_memory(&client, &base, "a").await;
    let b = save_memory(&client, &base, "b").await;

    let create = client
        .post(format!("{base}/relations"))
        .json(&json!({
            "sync_id": "sync-1",
            "source_id": a,
            "target_id": b,
            "relation": "supersedes",
            "reason": "newer doc replaces older",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 200);
    let rel: serde_json::Value = create.json().await.unwrap();
    let rel_id = rel["id"].as_str().unwrap().to_string();
    assert_eq!(rel["source_id"], a);
    assert_eq!(rel["target_id"], b);
    assert_eq!(rel["relation"], "supersedes");
    assert_eq!(rel["judgment_status"], "pending");

    // List with source_id filter
    let list = client
        .get(format!("{base}/relations?source_id={a}"))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["id"] == rel_id.as_str()));

    // Judge it
    let judge = client
        .put(format!("{base}/relations/{rel_id}/judge"))
        .json(&json!({"status": "judged", "reason": "confirmed"}))
        .send()
        .await
        .unwrap();
    assert_eq!(judge.status(), 204);

    // Verify status changed
    let judged = client
        .get(format!("{base}/relations?source_id={a}&status=judged"))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(judged.as_array().unwrap().len(), 1);
    assert_eq!(judged[0]["judgment_status"], "judged");
}

#[tokio::test]
async fn relation_create_same_source_target_returns_400() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let a = save_memory(&client, &base, "a").await;

    let r = client
        .post(format!("{base}/relations"))
        .json(&json!({
            "sync_id": "sync-1",
            "source_id": a,
            "target_id": a,
            "relation": "related",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
}

#[tokio::test]
async fn relation_create_empty_sync_id_returns_400() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let a = save_memory(&client, &base, "a").await;
    let b = save_memory(&client, &base, "b").await;

    let r = client
        .post(format!("{base}/relations"))
        .json(&json!({
            "sync_id": "",
            "source_id": a,
            "target_id": b,
            "relation": "related",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
}

#[tokio::test]
async fn relation_create_invalid_kind_returns_400() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let a = save_memory(&client, &base, "a").await;
    let b = save_memory(&client, &base, "b").await;

    let r = client
        .post(format!("{base}/relations"))
        .json(&json!({
            "sync_id": "sync-1",
            "source_id": a,
            "target_id": b,
            "relation": "made_up_kind",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
}

#[tokio::test]
async fn relation_judge_unknown_returns_404() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let r = client
        .put(format!("{base}/relations/01HW3X000000000000000000RX/judge"))
        .json(&json!({"status": "judged"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
}

#[tokio::test]
async fn relation_judge_invalid_status_returns_400() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let a = save_memory(&client, &base, "a").await;
    let b = save_memory(&client, &base, "b").await;
    let rel = client
        .post(format!("{base}/relations"))
        .json(&json!({
            "sync_id": "sync-1",
            "source_id": a,
            "target_id": b,
            "relation": "related",
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let id = rel["id"].as_str().unwrap();

    let r = client
        .put(format!("{base}/relations/{id}/judge"))
        .json(&json!({"status": "banana"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
}

// -------- Pending conflicts --------

#[tokio::test]
async fn conflicts_endpoint_returns_only_pending_conflicts_with() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let a = save_memory(&client, &base, "a").await;
    let b = save_memory(&client, &base, "b").await;
    let c = save_memory(&client, &base, "c").await;

    // pending conflict (should appear)
    let pending_conflict = client
        .post(format!("{base}/relations"))
        .json(&json!({
            "sync_id": "sync-1",
            "source_id": a,
            "target_id": b,
            "relation": "conflicts_with",
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let pc_id = pending_conflict["id"].as_str().unwrap().to_string();

    // judged conflict (should NOT appear)
    let judged_conflict = client
        .post(format!("{base}/relations"))
        .json(&json!({
            "sync_id": "sync-2",
            "source_id": a,
            "target_id": c,
            "relation": "conflicts_with",
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let jc_id = judged_conflict["id"].as_str().unwrap();
    client
        .put(format!("{base}/relations/{jc_id}/judge"))
        .json(&json!({"status": "judged"}))
        .send()
        .await
        .unwrap();

    // pending NOT-conflict (should NOT appear)
    client
        .post(format!("{base}/relations"))
        .json(&json!({
            "sync_id": "sync-3",
            "source_id": b,
            "target_id": c,
            "relation": "related",
        }))
        .send()
        .await
        .unwrap();

    let conflicts = client
        .get(format!("{base}/conflicts"))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let ids: Vec<&str> = conflicts
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&pc_id.as_str()));
    assert_eq!(ids.len(), 1, "only pending conflicts_with should surface");
}

// -------- Stats --------

#[tokio::test]
async fn stats_reflects_observations_and_sessions() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();

    // 3 memories: 2 default type, 1 decision
    save_memory(&client, &base, "a").await;
    save_memory(&client, &base, "b").await;
    client
        .post(format!("{base}/memories"))
        .json(&json!({
            "title": "ADR",
            "content": "decision payload",
            "type": "decision",
            "project": "p",
        }))
        .send()
        .await
        .unwrap();

    // 1 session
    client
        .post(format!("{base}/sessions"))
        .json(&json!({"project": "p"}))
        .send()
        .await
        .unwrap();

    let r = client.get(format!("{base}/stats")).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let stats: serde_json::Value = r.json().await.unwrap();

    assert_eq!(stats["observations"]["active"], 3);
    assert_eq!(stats["observations"]["deleted"], 0);
    assert_eq!(stats["observations"]["projects"], 1);
    // by_type contains memory:2 + decision:1
    let by_type = stats["observations"]["by_type"].as_array().unwrap();
    let map: std::collections::HashMap<&str, u64> = by_type
        .iter()
        .map(|b| (b["key"].as_str().unwrap(), b["count"].as_u64().unwrap()))
        .collect();
    assert_eq!(map.get("memory"), Some(&2));
    assert_eq!(map.get("decision"), Some(&1));

    assert_eq!(stats["sessions"]["total"], 1);
    let by_status = stats["sessions"]["by_status"].as_array().unwrap();
    let smap: std::collections::HashMap<&str, u64> = by_status
        .iter()
        .map(|b| (b["key"].as_str().unwrap(), b["count"].as_u64().unwrap()))
        .collect();
    assert_eq!(smap.get("active"), Some(&1));
}

#[tokio::test]
async fn stats_separates_deleted_from_active() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let id = save_memory(&client, &base, "doomed").await;
    save_memory(&client, &base, "survivor").await;
    client
        .delete(format!("{base}/memories/{id}"))
        .send()
        .await
        .unwrap();

    let stats: serde_json::Value = client
        .get(format!("{base}/stats"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(stats["observations"]["active"], 1);
    assert_eq!(stats["observations"]["deleted"], 1);
}

// -------- Embedder info --------

#[tokio::test]
async fn embedder_endpoint_returns_model_dim_and_optional_sha() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let r = client.get(format!("{base}/embedder")).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let info: serde_json::Value = r.json().await.unwrap();
    assert!(info["model_id"].is_string());
    assert!(info["dim"].is_u64());
    // expected_sha256 may be null for FakeEmbedder; the field is omitted
    // by skip_serializing_if when None, so this assertion just confirms
    // payload is well-formed.
    assert!(!info["model_id"].as_str().unwrap().is_empty());
}
