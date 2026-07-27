//! T-08 — `SeeleService::reembed_all`: re-embeds active observations
//! whose vector is missing from `observations_vec` or whose provenance
//! (`embeddings_meta`) names a different (model_id, dim) than the active
//! embedder. This is the pass that closes the best-effort embedding hole
//! of the save path: `doctor`'s `mix_warning` detects, this fixes.

use std::sync::Arc;

use seele_embedder::{Embedder, EmbedderError, FakeEmbedder};
use seele_http::dto::SaveRequest;
use seele_http::SeeleService;
use seele_storage::init_db;
use tempfile::TempDir;

/// FakeEmbedder vectors under a caller-chosen model id — simulates a
/// model swap without touching SQL: rows saved under `model/old` are
/// stale the moment the service runs with `model/new`.
struct StubModel(&'static str);

impl Embedder for StubModel {
    fn embed(&self, text: &str) -> seele_embedder::Result<Vec<f32>> {
        FakeEmbedder.embed(text)
    }
    fn dim(&self) -> usize {
        FakeEmbedder.dim()
    }
    fn model_id(&self) -> &str {
        self.0
    }
}

/// Fails on any content containing `poison`; everything else behaves
/// like the FakeEmbedder (same model id, so healthy rows are NOT
/// reported stale by the candidate scan).
struct PoisonEmbedder;

impl Embedder for PoisonEmbedder {
    fn embed(&self, text: &str) -> seele_embedder::Result<Vec<f32>> {
        if text.contains("poison") {
            return Err(EmbedderError::EmptyInput("poison"));
        }
        FakeEmbedder.embed(text)
    }
    fn dim(&self) -> usize {
        FakeEmbedder.dim()
    }
    fn model_id(&self) -> &str {
        FakeEmbedder.model_id()
    }
}

fn service_with(embedder: Arc<dyn Embedder>) -> (TempDir, SeeleService) {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let svc = SeeleService::new(pool, embedder);
    (td, svc)
}

fn save(svc: &SeeleService, title: &str, content: &str, project: &str) -> String {
    svc.save_observation(SaveRequest {
        title: title.to_string(),
        content: content.to_string(),
        r#type: "memory".to_string(),
        project: Some(project.to_string()),
        scope: None,
        topic_key: None,
        session_id: None,
        tool_name: None,
        metadata: serde_json::Value::Null,
    })
    .unwrap()
    .id
}

#[test]
fn reembed_all_fixes_missing_and_stale_then_is_idempotent() {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let old = SeeleService::new(pool.clone(), Arc::new(StubModel("model/old")));
    let a1 = save(&old, "a1", "contenido a1", "a");
    save(&old, "a2", "contenido a2", "a");
    save(&old, "b1", "contenido b1", "b");
    save(&old, "c1", "contenido c1", "c");
    // One row loses its vector+provenance outright (the best-effort hole).
    old.observations
        .delete_embedding(a1.parse().unwrap())
        .unwrap();

    // The active embedder is now a different model (same dim): every
    // row saved under `model/old` is stale, plus a1 has no vector.
    let svc = SeeleService::new(pool, Arc::new(StubModel("model/new")));
    let before = svc.embedding_provenance().unwrap();
    assert_eq!(before.active_without_vector, 1, "a1 lost its vector");
    assert!(
        before.mix_warning("model/new", 384).is_some(),
        "doctor must be warning before the pass: {before:?}"
    );

    // Dry-run counts but writes nothing.
    let dry = svc.reembed_all(None, 64, true).unwrap();
    assert_eq!(dry.candidates, 4, "a1 missing + 3 stale");
    assert_eq!(dry.reembedded, 0);
    assert_eq!(dry.skipped, 0);
    assert!(dry.dry_run);
    assert!(
        svc.observations
            .get_embedding(a1.parse().unwrap())
            .unwrap()
            .is_none(),
        "dry-run must not write vectors"
    );
    let still = svc.embedding_provenance().unwrap();
    assert_eq!(still.active_without_vector, 1, "dry-run must not write");

    // Apply, in two batches (batch_size=2 over 4 candidates).
    let report = svc.reembed_all(None, 2, false).unwrap();
    assert_eq!(report.candidates, 4);
    assert_eq!(report.reembedded, 4);
    assert_eq!(report.skipped, 0);
    assert!(!report.dry_run);
    assert!(
        svc.observations
            .get_embedding(a1.parse().unwrap())
            .unwrap()
            .is_some(),
        "a1 got its vector back"
    );

    // Doctor is clean: single (model_id, dim), nobody without a vector.
    let after = svc.embedding_provenance().unwrap();
    assert_eq!(after.active_without_vector, 0);
    assert_eq!(after.models.len(), 1, "one model only: {:?}", after.models);
    assert_eq!(after.models[0].model_id, "model/new");
    assert_eq!(after.models[0].count, 4);
    assert!(after.mix_warning("model/new", 384).is_none());

    // Idempotent: the second run finds no work.
    let second = svc.reembed_all(None, 64, false).unwrap();
    assert_eq!(second.candidates, 0);
    assert_eq!(second.reembedded, 0);
    assert_eq!(second.skipped, 0);
}

#[test]
fn reembed_all_respects_project_filter() {
    let (_td, svc) = service_with(Arc::new(FakeEmbedder));
    let x = save(&svc, "x1", "contenido x1", "px");
    let y = save(&svc, "y1", "contenido y1", "py");
    svc.observations
        .delete_embedding(x.parse().unwrap())
        .unwrap();
    svc.observations
        .delete_embedding(y.parse().unwrap())
        .unwrap();

    let only_x = svc.reembed_all(Some("px"), 64, false).unwrap();
    assert_eq!(only_x.candidates, 1);
    assert_eq!(only_x.reembedded, 1);
    assert!(
        svc.observations
            .get_embedding(y.parse().unwrap())
            .unwrap()
            .is_none(),
        "py row stays untouched while scoping to px"
    );

    let rest = svc.reembed_all(None, 64, false).unwrap();
    assert_eq!(rest.candidates, 1, "only the py row remains");
    assert_eq!(rest.reembedded, 1);
}

#[test]
fn reembed_all_skips_rows_the_embedder_fails_on() {
    let (_td, svc) = service_with(Arc::new(PoisonEmbedder));
    save(&svc, "sana", "fila sana", "p");
    // Post-save embedding fails for this one → row persisted, no vector
    // (exactly the hole this command exists to close).
    let bad = save(&svc, "mala", "poison row", "p");
    assert!(
        svc.observations
            .get_embedding(bad.parse().unwrap())
            .unwrap()
            .is_none(),
        "poison row never got a vector"
    );
    // A healthy row joins the candidate set so the batch has one
    // embeddable and one failing row.
    let ok = save(&svc, "sana2", "otra fila sana", "p");
    svc.observations
        .delete_embedding(ok.parse().unwrap())
        .unwrap();

    let report = svc.reembed_all(None, 64, false).unwrap();
    assert_eq!(report.candidates, 2);
    assert_eq!(report.reembedded, 1, "the healthy row is re-embedded");
    assert_eq!(report.skipped, 1, "the poison row is counted, not fatal");
    assert!(svc
        .observations
        .get_embedding(ok.parse().unwrap())
        .unwrap()
        .is_some());

    // Re-running does not lose track of the failed row.
    let again = svc.reembed_all(None, 64, false).unwrap();
    assert_eq!(again.candidates, 1);
    assert_eq!(again.skipped, 1);
}
