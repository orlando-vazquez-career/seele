//! Bloque C.1 — lifecycle handlers E2E tests.
//!
//! Exercises soft_delete + restore, sessions (start/list/get/end/abort),
//! and links (create/list-for-memory/delete).

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

// -------- soft_delete + restore --------

#[tokio::test]
async fn soft_delete_then_get_returns_observation_with_deleted_at() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let id = save_memory(&client, &base, "doomed").await;

    let r = client
        .delete(format!("{base}/memories/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 204);

    let got = client
        .get(format!("{base}/memories/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(got.status(), 200);
    let body: serde_json::Value = got.json().await.unwrap();
    assert!(
        body["deleted_at"].is_i64(),
        "expected deleted_at timestamp, got {body:?}"
    );
}

#[tokio::test]
async fn soft_delete_excludes_from_default_list() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let id = save_memory(&client, &base, "tombstone").await;
    client
        .delete(format!("{base}/memories/{id}"))
        .send()
        .await
        .unwrap();

    let list = client
        .get(format!("{base}/memories?project=p"))
        .send()
        .await
        .unwrap();
    let arr: serde_json::Value = list.json().await.unwrap();
    let ids: Vec<&str> = arr
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["id"].as_str().unwrap())
        .collect();
    assert!(!ids.contains(&id.as_str()));
}

#[tokio::test]
async fn restore_undoes_soft_delete() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let id = save_memory(&client, &base, "phoenix").await;
    client
        .delete(format!("{base}/memories/{id}"))
        .send()
        .await
        .unwrap();
    let r = client
        .post(format!("{base}/memories/{id}/restore"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 204);

    let got = client
        .get(format!("{base}/memories/{id}"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = got.json().await.unwrap();
    assert!(body["deleted_at"].is_null());
}

#[tokio::test]
async fn soft_delete_unknown_returns_404() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let r = client
        .delete(format!("{base}/memories/01HW3X000000000000000000RX"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
}

// -------- Sessions --------

#[tokio::test]
async fn session_lifecycle_start_list_get_end() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();

    let start = client
        .post(format!("{base}/sessions"))
        .json(&json!({"project": "p", "directory": "/tmp/p"}))
        .send()
        .await
        .unwrap();
    assert_eq!(start.status(), 200);
    let started: serde_json::Value = start.json().await.unwrap();
    let id = started["id"].as_str().unwrap();
    assert_eq!(started["status"], "active");
    assert_eq!(started["project"], "p");

    let list = client
        .get(format!("{base}/sessions?project=p"))
        .send()
        .await
        .unwrap();
    let arr: serde_json::Value = list.json().await.unwrap();
    assert!(arr.as_array().unwrap().iter().any(|s| s["id"] == id));

    let got = client
        .get(format!("{base}/sessions/{id}"))
        .send()
        .await
        .unwrap();
    let s: serde_json::Value = got.json().await.unwrap();
    assert_eq!(s["status"], "active");

    let end = client
        .put(format!("{base}/sessions/{id}/end"))
        .json(&json!({"summary": "wrapped"}))
        .send()
        .await
        .unwrap();
    assert_eq!(end.status(), 204);

    let after = client
        .get(format!("{base}/sessions/{id}"))
        .send()
        .await
        .unwrap();
    let s: serde_json::Value = after.json().await.unwrap();
    assert_eq!(s["status"], "ended");
    assert_eq!(s["summary"], "wrapped");
    assert!(s["ended_at"].is_i64());
}

#[tokio::test]
async fn session_abort_marks_aborted() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let start = client
        .post(format!("{base}/sessions"))
        .json(&json!({"project": "p"}))
        .send()
        .await
        .unwrap();
    let started: serde_json::Value = start.json().await.unwrap();
    let id = started["id"].as_str().unwrap();
    let r = client
        .put(format!("{base}/sessions/{id}/abort"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 204);
    let got = client
        .get(format!("{base}/sessions/{id}"))
        .send()
        .await
        .unwrap();
    let s: serde_json::Value = got.json().await.unwrap();
    assert_eq!(s["status"], "aborted");
}

#[tokio::test]
async fn session_list_filters_by_status() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let s1 = client
        .post(format!("{base}/sessions"))
        .json(&json!({"project": "p"}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let s2 = client
        .post(format!("{base}/sessions"))
        .json(&json!({"project": "p"}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let id2 = s2["id"].as_str().unwrap();
    client
        .put(format!("{base}/sessions/{id2}/abort"))
        .send()
        .await
        .unwrap();

    let active_only = client
        .get(format!("{base}/sessions?status=active"))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let ids: Vec<&str> = active_only
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&s1["id"].as_str().unwrap()));
    assert!(!ids.contains(&id2));
}

#[tokio::test]
async fn session_list_rejects_invalid_status_400() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let r = client
        .get(format!("{base}/sessions?status=banana"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["code"], "BAD_REQUEST");
}

#[tokio::test]
async fn end_session_twice_returns_404_second_call() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let s = client
        .post(format!("{base}/sessions"))
        .json(&json!({"project": "p"}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let id = s["id"].as_str().unwrap();
    let first = client
        .put(format!("{base}/sessions/{id}/end"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), 204);
    let second = client
        .put(format!("{base}/sessions/{id}/end"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    // session was ended already → not active → 404
    assert_eq!(second.status(), 404);
}

// -------- Links --------

#[tokio::test]
async fn link_create_list_for_memory_delete() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let a = save_memory(&client, &base, "a").await;
    let b = save_memory(&client, &base, "b").await;

    let create = client
        .post(format!("{base}/links"))
        .json(&json!({
            "from_id": a,
            "to_id": b,
            "link_type": "derives_from",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 200);
    let link: serde_json::Value = create.json().await.unwrap();
    let link_id = link["id"].as_str().unwrap();
    assert_eq!(link["from_id"], a);
    assert_eq!(link["to_id"], b);

    // list-for-memory: a (source side)
    let from_a = client
        .get(format!("{base}/memories/{a}/links"))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert!(from_a
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l["id"] == link_id));

    // list-for-memory: b (target side)
    let from_b = client
        .get(format!("{base}/memories/{b}/links"))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert!(from_b
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l["id"] == link_id));

    // delete
    let del = client
        .delete(format!("{base}/links/{link_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 204);

    // ensure removed
    let again = client
        .get(format!("{base}/memories/{a}/links"))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert!(!again.as_array().unwrap().iter().any(|l| l["id"] == link_id));
}

#[tokio::test]
async fn link_create_empty_type_returns_400() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let a = save_memory(&client, &base, "a").await;
    let b = save_memory(&client, &base, "b").await;
    let r = client
        .post(format!("{base}/links"))
        .json(&json!({
            "from_id": a,
            "to_id": b,
            "link_type": "",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
}

#[tokio::test]
async fn link_create_invalid_from_id_returns_400() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let b = save_memory(&client, &base, "b").await;
    let r = client
        .post(format!("{base}/links"))
        .json(&json!({
            "from_id": "not-a-ulid",
            "to_id": b,
            "link_type": "related_to",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
}

#[tokio::test]
async fn link_delete_unknown_returns_404() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let r = client
        .delete(format!("{base}/links/01HW3X000000000000000000RX"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
}
