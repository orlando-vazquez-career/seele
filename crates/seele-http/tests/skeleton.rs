//! Bloque A — http skeleton tests.
//!
//! Spawns the server on a random port and exercises `/health` + `/version`
//! over real reqwest, validating the middleware stack + handler wiring.

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
            legacy_engram_paths: false,
            chat: None,
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
async fn version_returns_pkg_version_and_name() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let r = client.get(format!("{base}/version")).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["name"], "seele");
    assert!(body["version"].is_string());
}

#[tokio::test]
async fn unknown_route_returns_404() {
    let (_td, base) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let r = client
        .get(format!("{base}/this-path-does-not-exist"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
}
