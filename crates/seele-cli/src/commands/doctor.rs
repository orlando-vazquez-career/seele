//! `seele doctor` — health snapshot. Cloven sight (Sprint-03): the
//! report must grit-shout when the embedder is FakeEmbedder so users
//! don't silently lose vector search quality.

use std::path::PathBuf;

use serde::Serialize;

use crate::app::OutputOpts;
use crate::output;

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
}

pub async fn run(
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
            lines.join("\n")
        },
        out.json,
    )
}
