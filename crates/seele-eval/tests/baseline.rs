//! Baseline capture (T-B6, #10).
//!
//! `#[ignore]` because it downloads the ONNX model (~30-90 MB, first run) and
//! runs real inference. Capture the v0.2 baseline with:
//!
//! ```text
//! cargo test -p seele-eval --test baseline -- --ignored --nocapture
//! ```
//!
//! The JSON between the BEGIN/END markers is versioned at
//! `docs/plans/tactica/v0.3-calidad-memoria/baseline-v0.2.json`.

use std::collections::BTreeMap;

use seele_embedder::{Embedder, OnnxConfig, OnnxEmbedder};
use seele_eval::{builtin_suite, run_suite, SuiteReport, BUILTIN_SUITES};
use seele_storage::{init_db, ObservationStore};
use tempfile::TempDir;

#[test]
#[ignore = "downloads ONNX model + runs inference; run locally to capture the baseline"]
fn capture_v0_2_baseline() {
    let mut all: BTreeMap<String, SuiteReport> = BTreeMap::new();

    for name in BUILTIN_SUITES {
        let suite = builtin_suite(name)
            .expect("suite parses")
            .expect("suite exists");
        let td = TempDir::new().unwrap();
        let pool = init_db(td.path().join("eval.db")).unwrap();
        let store = ObservationStore::new(pool);
        // `SEELE_EVAL_MODEL_DIR` (set when hf-hub's download is unavailable, e.g.
        // CI/sandbox) loads pre-downloaded model files; otherwise hf-hub fetches.
        let embedder: Box<dyn Embedder> = match std::env::var("SEELE_EVAL_MODEL_DIR") {
            Ok(d) if !d.trim().is_empty() => Box::new(
                OnnxEmbedder::from_local_dir(&d, OnnxConfig::default()).expect("local ONNX model"),
            ),
            _ => Box::new(OnnxEmbedder::new().expect("ONNX init")),
        };
        let model_id = embedder.model_id().to_string();
        let report = run_suite(&suite, name, &store, embedder, &model_id).unwrap();
        all.insert((*name).to_string(), report);
    }

    println!("=== SEELE v0.2 BASELINE (begin) ===");
    println!("{}", serde_json::to_string_pretty(&all).unwrap());
    println!("=== SEELE v0.2 BASELINE (end) ===");
}
