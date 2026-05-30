//! `seele eval --suite <name>` — run a built-in memory-quality evaluation
//! suite against the live hybrid search pipeline and report recall@k / MRR
//! per category (ADR-14).
//!
//! Runs against an **ephemeral** DB (a tempdir), never the user's real DB:
//! the suite's corpus is ingested, queried, then discarded. `--db` is
//! intentionally ignored here.

use clap::Args as ClapArgs;

use seele_embedder::{Embedder, FakeEmbedder, OnnxEmbedder};
use seele_eval::{builtin_suite, run_suite, SuiteReport, BUILTIN_SUITES};
use seele_storage::{init_db, ObservationStore};

use crate::app::OutputOpts;
use crate::output;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Built-in suite to run: `coding-memory` or `longmemeval-subset`.
    #[arg(long)]
    pub suite: String,
}

pub async fn run(args: Args, fake_embedder: bool, out: &OutputOpts) -> anyhow::Result<()> {
    let suite = builtin_suite(&args.suite)?.ok_or_else(|| {
        anyhow::anyhow!(
            "unknown suite '{}'; available: {}",
            args.suite,
            BUILTIN_SUITES.join(", ")
        )
    })?;

    // Ephemeral DB — the eval corpus must never touch the user's real DB.
    let tmp = tempfile::tempdir()?;
    let pool = init_db(tmp.path().join("eval.db"))?;
    let store = ObservationStore::new(pool);

    let embedder = build_embedder(fake_embedder);
    let model_id = embedder.model_id().to_string();

    let report = run_suite(&suite, &args.suite, &store, embedder, &model_id)?;

    output::emit_split(&report, || render(&report), out.json)
}

/// Real ONNX by default (meaningful baseline); `FakeEmbedder` on
/// `--fake-embedder` / `SEELE_FAKE_EMBEDDER`, or as a loud fallback if ONNX
/// init fails — fake numbers are NOT a real baseline.
fn build_embedder(fake_flag: bool) -> Box<dyn Embedder> {
    let fake_env = std::env::var("SEELE_FAKE_EMBEDDER")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    if fake_flag || fake_env {
        return Box::new(FakeEmbedder::new());
    }
    match OnnxEmbedder::new() {
        Ok(e) => Box::new(e),
        Err(e) => {
            eprintln!(
                "seele eval: ONNX embedder unavailable ({e}); falling back to \
                 FakeEmbedder — the resulting numbers are NOT a real baseline."
            );
            Box::new(FakeEmbedder::new())
        }
    }
}

fn render(r: &SuiteReport) -> String {
    let mut lines = vec![
        format!("suite: {}   model: {}", r.suite, r.model_id),
        format!(
            "{:<16} {:>3}  {:>8} {:>9} {:>6}",
            "category", "n", "recall@5", "recall@10", "mrr"
        ),
    ];
    for (cat, m) in &r.by_category {
        lines.push(format!(
            "{:<16} {:>3}  {:>8.3} {:>9.3} {:>6.3}",
            cat, m.queries, m.recall_at_5, m.recall_at_10, m.mrr
        ));
    }
    let t = &r.totals;
    lines.push(format!(
        "{:<16} {:>3}  {:>8.3} {:>9.3} {:>6.3}",
        "TOTAL", t.queries, t.recall_at_5, t.recall_at_10, t.mrr
    ));
    lines.join("\n")
}
