//! Runner smoke test (T-B3): the full pipeline (ingest → search → metrics)
//! runs end-to-end and produces a well-formed report.
//!
//! Uses `FakeEmbedder`, so the *values* are not a meaningful baseline (the
//! real baseline runs with ONNX — that's T-B6 / `seele eval`). This only
//! asserts the pipeline shape: every query counted, categories present,
//! metrics in `[0, 1]`.

use seele_embedder::FakeEmbedder;
use seele_eval::{load_suite, run_suite};
use seele_storage::{init_db, ObservationStore};
use tempfile::TempDir;

#[test]
fn run_suite_smoke_produces_wellformed_report() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/coding-memory.json");
    let suite = load_suite(path).unwrap();

    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("eval.db")).unwrap();
    let store = ObservationStore::new(pool);

    let report = run_suite(
        &suite,
        "coding-memory",
        &store,
        Box::new(FakeEmbedder::new()),
        "seele/fake-embedder",
    )
    .unwrap();

    assert_eq!(report.suite, "coding-memory");
    assert_eq!(report.totals.queries, 18, "all queries scored");

    // Categories from the fixture are present.
    for cat in [
        "single-fact",
        "paraphrase",
        "multi-hop",
        "knowledge-update",
        "temporal",
    ] {
        assert!(
            report.by_category.contains_key(cat),
            "missing category {cat}"
        );
    }

    // Metrics are valid fractions.
    let m = &report.totals;
    for v in [m.recall_at_5, m.recall_at_10, m.mrr] {
        assert!((0.0..=1.0).contains(&v), "metric out of range: {v}");
    }
    // recall@10 >= recall@5 by construction.
    assert!(m.recall_at_10 >= m.recall_at_5);
}
