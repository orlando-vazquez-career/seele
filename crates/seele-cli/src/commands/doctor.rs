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
            if let Some(w) = &report.fake_embedder_warning {
                lines.push(format!("WARNING: {w}"));
            }
            lines.join("\n")
        },
        out.json,
    )
}
