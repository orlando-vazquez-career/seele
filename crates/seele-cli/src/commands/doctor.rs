//! `seele doctor` — health snapshot. Cloven sight (Sprint-03): the
//! report must grit-shout when the embedder is FakeEmbedder so users
//! don't silently lose vector search quality.
//!
//! T-11: the report also checks FTS5 index health (see
//! `seele_core::doctor` for the heuristic and its honesty note). Plain
//! `seele doctor` only reads; `--fix` is the opt-in remediation that
//! runs FTS5 'optimize' when invoked and reports the segment count
//! before and after.

use std::path::PathBuf;

use clap::Args as ClapArgs;
use serde::Serialize;

use crate::app::OutputOpts;
use crate::output;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Apply the FTS remediation: run FTS5 'optimize' on
    /// `observations_fts`, merging all index segments into one. Opt-in
    /// because it rewrites the index in a single transaction — always
    /// safe, but not free on very large indexes.
    #[arg(long)]
    pub fix: bool,
}

/// FTS5 index health section of the doctor report (T-11). Flat fields
/// (rather than a nested `FtsHealth`) so JSON consumers read one object.
#[derive(Debug, Serialize)]
struct FtsReport {
    /// Documents in the FTS index (includes pending soft-delete entries).
    docs: u64,
    /// Estimated on-disk segment count before any `--fix` pass.
    segments: u64,
    /// True when `segments` is past `FTS_OPTIMIZE_SEGMENT_THRESHOLD`.
    optimize_recommended: bool,
    /// True when this invocation ran FTS5 'optimize' (`--fix`).
    optimize_applied: bool,
    /// Segment count measured after the `--fix` pass (`None` without it).
    segments_after: Option<u64>,
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    status: &'static str,
    seele_version: String,
    db_path: PathBuf,
    embedder_model_id: String,
    embedder_dim: usize,
    embedder_expected_sha256: Option<String>,
    fake_embedder_warning: Option<String>,
    observations_active: u64,
    sessions_total: u64,
    /// (model_id, dim) combos present in `embeddings_meta` (ADR-14).
    embedding_models: Vec<seele_storage::EmbeddingModelCount>,
    /// Active observations invisible to the vec branch (no vector stored).
    observations_active_without_vector: u64,
    /// ADR-14 anti-mix guard: mixed models, or stored ≠ active embedder.
    embedding_mix_warning: Option<String>,
    /// FTS5 index health; `optimize_applied`/`segments_after` reflect
    /// the `--fix` pass when given.
    fts: FtsReport,
}

/// Read the FTS health snapshot through the service's pooled connection.
fn fts_health(svc: &seele_http::SeeleService) -> anyhow::Result<seele_core::doctor::FtsHealth> {
    let conn = svc.pool.get()?;
    let docs: u64 = conn.query_row(seele_core::doctor::FTS_DOCS_SQL, [], |r| r.get(0))?;
    let segments: u64 = conn.query_row(seele_core::doctor::FTS_SEGMENTS_SQL, [], |r| r.get(0))?;
    Ok(seele_core::doctor::FtsHealth { docs, segments })
}

pub async fn run(
    args: Args,
    db: &Option<PathBuf>,
    fake_embedder: bool,
    out: &OutputOpts,
) -> anyhow::Result<()> {
    let svc = crate::app::build_service(db, fake_embedder)?;
    let stats = svc.stats()?;
    let info = svc.embedder_info();
    let db_path = db.clone().unwrap_or_else(crate::default_db_path);

    // v0.1 always uses FakeEmbedder (real ONNX in Sprint-05+). The
    // warning fires whenever the active model identifies as fake (the
    // `seele/fake-embedder` literal returned by `FakeEmbedder::model_id`)
    // so users don't silently lose vector search quality.
    let fake_embedder_warning = if info.model_id.contains("fake") {
        Some(
            "FakeEmbedder is active — vector search returns deterministic \
             zeros. For real semantic search install the ONNX model \
             (Sprint-05+) or pass observations through `seele search` \
             with FTS-only queries."
                .to_string(),
        )
    } else {
        None
    };

    let provenance = svc.embedding_provenance()?;
    let embedding_mix_warning = provenance.mix_warning(&info.model_id, info.dim);

    // FTS health (T-11). Plain doctor only reads; `--fix` is the opt-in
    // remediation: run FTS5 'optimize' (one transaction) and re-measure.
    let before = fts_health(&svc)?;
    let mut segments_after = None;
    if args.fix {
        let conn = svc.pool.get()?;
        conn.execute(seele_core::doctor::FTS_OPTIMIZE_SQL, [])?;
        segments_after = Some(fts_health(&svc)?.segments);
    }
    let fts = FtsReport {
        docs: before.docs,
        segments: before.segments,
        optimize_recommended: before.optimize_recommended(),
        optimize_applied: args.fix,
        segments_after,
    };

    let report = DoctorReport {
        status: "ok",
        seele_version: env!("CARGO_PKG_VERSION").to_string(),
        db_path,
        embedder_model_id: info.model_id,
        embedder_dim: info.dim,
        embedder_expected_sha256: info.expected_sha256,
        fake_embedder_warning: fake_embedder_warning.clone(),
        observations_active: stats.observations.active,
        sessions_total: stats.sessions.total,
        embedding_models: provenance.models,
        observations_active_without_vector: provenance.active_without_vector,
        embedding_mix_warning,
        fts,
    };

    output::emit_split(
        &report,
        || {
            let mut lines = vec![
                format!("status: {}", report.status),
                format!("seele version: {}", report.seele_version),
                format!("db path: {}", report.db_path.display()),
                format!(
                    "embedder: {} (dim={})",
                    report.embedder_model_id, report.embedder_dim
                ),
                format!("observations (active): {}", report.observations_active),
                format!("sessions (total): {}", report.sessions_total),
            ];
            if report.embedding_models.is_empty() {
                lines.push("embeddings: none stored yet".to_string());
            } else {
                for m in &report.embedding_models {
                    lines.push(format!(
                        "embeddings: {} ({}d) — {} row(s)",
                        m.model_id, m.dim, m.count
                    ));
                }
            }
            if report.observations_active_without_vector > 0 {
                lines.push(format!(
                    "observations without vector (invisible to vec search): {}",
                    report.observations_active_without_vector
                ));
            }
            if let Some(w) = &report.fake_embedder_warning {
                lines.push(format!("WARNING: {w}"));
            }
            if let Some(w) = &report.embedding_mix_warning {
                lines.push(format!("WARNING: {w}"));
            }
            lines.push(format!(
                "fts: {} docs, {} segment(s)",
                report.fts.docs, report.fts.segments
            ));
            if let Some(after) = report.fts.segments_after {
                lines.push(format!(
                    "fts optimize: applied ({} -> {} segment(s))",
                    report.fts.segments, after
                ));
            } else if report.fts.optimize_recommended {
                lines.push(format!(
                    "WARNING: FTS index fragmented ({} segments > {}) — \
                     run 'seele doctor --fix' to merge them (FTS5 'optimize')",
                    report.fts.segments,
                    seele_core::doctor::FTS_OPTIMIZE_SEGMENT_THRESHOLD
                ));
            }
            lines.join("\n")
        },
        out.json,
    )
}
