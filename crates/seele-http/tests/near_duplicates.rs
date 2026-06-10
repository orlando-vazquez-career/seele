//! Q4 — near-duplicate detection on the save path.
//!
//! Uses a stub embedder with controllable vectors: the production
//! FakeEmbedder is hash-based, so distinct contents land on unrelated
//! vectors and could never sit inside the near-dup threshold.

use std::sync::Arc;

use seele_embedder::Embedder;
use seele_http::dto::SaveRequest;
use seele_http::SeeleService;
use seele_storage::init_db;
use tempfile::TempDir;

/// First word of the text picks the vector: `dup*` → e0, `far*` → e2.
/// Unit-norm by construction, so L2 distances are exactly 0 or sqrt(2).
struct StubEmbedder;

impl Embedder for StubEmbedder {
    fn embed(&self, text: &str) -> seele_embedder::Result<Vec<f32>> {
        let mut v = vec![0.0f32; 384];
        if text.trim_start().starts_with("dup") {
            v[0] = 1.0;
        } else {
            v[2] = 1.0;
        }
        Ok(v)
    }
    fn dim(&self) -> usize {
        384
    }
    fn model_id(&self) -> &str {
        "seele/stub-embedder"
    }
}

fn service() -> (TempDir, SeeleService) {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let svc = SeeleService::new(pool, Arc::new(StubEmbedder));
    (td, svc)
}

fn save(svc: &SeeleService, title: &str, content: &str) -> seele_http::dto::SaveResponse {
    svc.save_observation(SaveRequest {
        title: title.to_string(),
        content: content.to_string(),
        r#type: "memory".to_string(),
        project: Some("p".to_string()),
        scope: None,
        topic_key: None,
        session_id: None,
        tool_name: None,
        metadata: serde_json::Value::Null,
    })
    .unwrap()
}

#[test]
fn save_reports_near_duplicate_within_threshold() {
    let (_td, svc) = service();
    let first = save(&svc, "deploy v1", "dup el deploy fallo por dns");
    assert!(
        first.near_duplicates.is_empty(),
        "first save has no neighbors"
    );

    // Different content (different hash → no dedup merge), same stub
    // vector → L2 distance 0 → must be reported.
    let second = save(&svc, "deploy v2", "dup el deploy se cayo por el dns");
    assert_eq!(second.outcome, "created");
    assert_eq!(second.near_duplicates.len(), 1);
    let nd = &second.near_duplicates[0];
    assert_eq!(nd.id, first.id);
    assert_eq!(nd.title, "deploy v1");
    assert!(nd.distance < 1e-6, "identical vectors → distance ~0");
}

#[test]
fn distant_vectors_are_not_reported() {
    let (_td, svc) = service();
    save(&svc, "deploy", "dup contenido cercano");
    let far = save(&svc, "otra cosa", "far contenido lejano");
    assert!(
        far.near_duplicates.is_empty(),
        "orthogonal vector (l2 = sqrt(2)) must stay out: {:?}",
        far.near_duplicates
    );
}

#[test]
fn near_duplicates_respect_project_boundary() {
    let (_td, svc) = service();
    save(&svc, "en p", "dup misma cosa");
    // Same vector but saved under another project: the scan is scoped to
    // the project of the row being saved.
    let resp = svc
        .save_observation(SaveRequest {
            title: "en q".to_string(),
            content: "dup misma cosa pero en otro proyecto".to_string(),
            r#type: "memory".to_string(),
            project: Some("q".to_string()),
            scope: None,
            topic_key: None,
            session_id: None,
            tool_name: None,
            metadata: serde_json::Value::Null,
        })
        .unwrap();
    assert!(resp.near_duplicates.is_empty());
}
