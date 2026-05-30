//! Anti-drift between the router and the OpenAPI spec.
//!
//! axum 0.8 does not expose introspection of registered routes, so we
//! parse `src/server.rs` with a small regex and compare the set of
//! canonical paths against the keys of `/openapi.json`. Legacy ENGRAM
//! aliases (`/save`, `/show/{id}`) are intentionally NOT in the spec
//! (ADR-13) — they're excluded.
//!
//! Failure modes covered:
//! - Adding `.route("/new", ...)` in server.rs and forgetting to add
//!   `paths.path("/new", ...)` in openapi.rs → test reports MISSING.
//! - Adding `paths.path("/new", ...)` to the spec without wiring a
//!   real route → test reports EXTRA.

use std::collections::HashSet;
use std::fs;
use std::sync::Arc;

use seele_embedder::{Embedder, FakeEmbedder};
use seele_http::{SeeleService, Server, ServerConfig};
use seele_storage::init_db;
use tempfile::TempDir;

fn read_server_source() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    fs::read_to_string(format!("{manifest}/src/server.rs")).expect("read server.rs")
}

/// Extract paths from `.route("/<...>", ...)` literals in server.rs,
/// stopping at the `if self.config.legacy_engram_paths` block so the
/// legacy aliases (which are intentionally NOT in the spec) are skipped.
fn canonical_router_paths(source: &str) -> HashSet<String> {
    let cutoff = source
        .find("if self.config.legacy_engram_paths")
        .unwrap_or(source.len());
    let canonical_region = &source[..cutoff];
    let re = regex::Regex::new(r#"\.route\(\s*"([^"]+)""#).unwrap();
    re.captures_iter(canonical_region)
        .map(|c| c[1].to_string())
        .collect()
}

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
async fn openapi_paths_match_router_paths_exactly() {
    let (_td, base) = spawn_test_server().await;
    let spec: serde_json::Value = reqwest::Client::new()
        .get(format!("{base}/openapi.json"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mut spec_paths: HashSet<String> =
        spec["paths"].as_object().unwrap().keys().cloned().collect();
    // `/health` and `/version` are documented in the spec but mounted
    // outside the protected/legacy block in server.rs, so we add them
    // to the router-side set explicitly. They're public.
    let mut router_paths = canonical_router_paths(&read_server_source());
    router_paths.insert("/health".to_string());
    router_paths.insert("/version".to_string());

    // Spec includes `/health` + `/version` already.
    let missing_in_spec: Vec<&String> = router_paths.difference(&spec_paths).collect();
    let extra_in_spec: Vec<&String> = spec_paths.difference(&router_paths).collect();

    assert!(
        missing_in_spec.is_empty(),
        "router has routes not declared in OpenAPI spec: {missing_in_spec:?}.\n\
         Add a `paths.path(\"<path>\", ...)` entry in crates/seele-http/src/openapi.rs."
    );
    assert!(
        extra_in_spec.is_empty(),
        "OpenAPI spec declares paths not present in router: {extra_in_spec:?}.\n\
         Either wire the route in crates/seele-http/src/server.rs or remove from openapi.rs."
    );

    // Sanity: at least the canonical 20-ish paths are present. If the
    // regex returns nothing, the test would pass vacuously — guard.
    spec_paths.remove("/health");
    spec_paths.remove("/version");
    assert!(
        spec_paths.len() >= 15,
        "suspiciously few paths ({}); regex extraction likely broken",
        spec_paths.len()
    );
}
