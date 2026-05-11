use std::path::PathBuf;

use crate::app::OutputOpts;
use crate::output;

pub async fn run(
    db: &Option<PathBuf>,
    fake_embedder: bool,
    out: &OutputOpts,
) -> anyhow::Result<()> {
    let svc = crate::app::build_service(db, fake_embedder)?;
    let stats = svc.stats()?;
    output::emit_split(
        &stats,
        || {
            let obs = &stats.observations;
            let s = &stats.sessions;
            let by_type = obs
                .by_type
                .iter()
                .map(|b| format!("    {}: {}", b.key, b.count))
                .collect::<Vec<_>>()
                .join("\n");
            let by_scope = obs
                .by_scope
                .iter()
                .map(|b| format!("    {}: {}", b.key, b.count))
                .collect::<Vec<_>>()
                .join("\n");
            let by_status = s
                .by_status
                .iter()
                .map(|b| format!("    {}: {}", b.key, b.count))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "observations:\n  active: {}\n  deleted: {}\n  projects: {}\n  by_type:\n{}\n  by_scope:\n{}\nsessions:\n  total: {}\n  by_status:\n{}",
                obs.active, obs.deleted, obs.projects, by_type, by_scope, s.total, by_status
            )
        },
        out.json,
    )
}
