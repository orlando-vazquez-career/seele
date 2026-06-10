//! `seele eval --suite <name> | --suite-file <path>` — run a memory-quality
//! evaluation suite against the live hybrid search pipeline and report
//! recall@k / MRR / nDCG@10 per category (ADR-14).
//!
//! Runs against an **ephemeral** DB (a tempdir), never the user's real DB:
//! the suite's corpus is ingested, queried, then discarded. `--db` is
//! intentionally ignored here. `--suite-file` accepts any JSON following the
//! Suite contract, so third parties (MNEMA, downstream users) can evaluate
//! their own corpora without recompiling.

use std::path::PathBuf;

use clap::Args as ClapArgs;

use seele_eval::{builtin_suite, load_suite, run_suite, SuiteReport, BUILTIN_SUITES};
use seele_storage::{init_db, ObservationStore};

use crate::app::OutputOpts;
use crate::output;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Built-in suite to run: `coding-memory` or `longmemeval-subset`.
    #[arg(long, conflicts_with = "suite_file")]
    pub suite: Option<String>,
    /// Path to a custom suite JSON (same contract as the built-ins:
    /// `{corpus: [{id, body, metadata}], queries: [{q, expected_ids, category}]}`).
    #[arg(long, value_name = "PATH")]
    pub suite_file: Option<PathBuf>,
}

pub async fn run(args: Args, fake_embedder: bool, out: &OutputOpts) -> anyhow::Result<()> {
    let (suite, label) = match (&args.suite, &args.suite_file) {
        (Some(name), None) => {
            let suite = builtin_suite(name)?.ok_or_else(|| {
                anyhow::anyhow!(
                    "unknown suite '{}'; available: {}",
                    name,
                    BUILTIN_SUITES.join(", ")
                )
            })?;
            (suite, name.clone())
        }
        (None, Some(path)) => {
            let suite = load_suite(path)
                .map_err(|e| anyhow::anyhow!("loading suite file {}: {e}", path.display()))?;
            let label = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            (suite, label)
        }
        _ => anyhow::bail!("pass exactly one of --suite <name> or --suite-file <path>"),
    };

    // Ephemeral DB — the eval corpus must never touch the user's real DB.
    let tmp = tempfile::tempdir()?;
    let pool = init_db(tmp.path().join("eval.db"))?;
    let store = ObservationStore::new(pool);

    // Single source of truth for embedder selection (crate::app). Real
    // ONNX by default — when it degrades to Fake WITHOUT the user asking,
    // the resulting numbers are NOT a real baseline; say so loudly.
    let embedder = crate::app::pick_embedder_boxed(fake_embedder);
    if embedder.model_id().contains("fake") && !fake_embedder && !crate::app::fake_env_set() {
        eprintln!("seele eval: corriendo con FakeEmbedder — los números NO son un baseline real.");
    }
    let model_id = embedder.model_id().to_string();

    let report = run_suite(&suite, &label, &store, embedder, &model_id)?;

    output::emit_split(&report, || render(&report), out.json)
}

fn render(r: &SuiteReport) -> String {
    let mut lines = vec![
        format!("suite: {}   model: {}", r.suite, r.model_id),
        format!(
            "{:<16} {:>3}  {:>8} {:>9} {:>6} {:>7}",
            "category", "n", "recall@5", "recall@10", "mrr", "ndcg@10"
        ),
    ];
    for (cat, m) in &r.by_category {
        lines.push(format!(
            "{:<16} {:>3}  {:>8.3} {:>9.3} {:>6.3} {:>7.3}",
            cat, m.queries, m.recall_at_5, m.recall_at_10, m.mrr, m.ndcg_at_10
        ));
    }
    let t = &r.totals;
    lines.push(format!(
        "{:<16} {:>3}  {:>8.3} {:>9.3} {:>6.3} {:>7.3}",
        "TOTAL", t.queries, t.recall_at_5, t.recall_at_10, t.mrr, t.ndcg_at_10
    ));
    lines.join("\n")
}
