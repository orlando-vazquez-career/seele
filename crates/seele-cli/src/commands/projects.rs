use std::path::PathBuf;

use crate::app::OutputOpts;
use crate::output;

pub async fn run(
    db: &Option<PathBuf>,
    fake_embedder: bool,
    out: &OutputOpts,
) -> anyhow::Result<()> {
    let svc = crate::app::build_service(db, fake_embedder)?;
    let projects = svc.list_projects()?;
    output::emit_split(
        &projects,
        || {
            if projects.is_empty() {
                "no projects yet".to_string()
            } else {
                let mut lines = vec![format!("{} project(s):", projects.len())];
                for p in &projects {
                    lines.push(format!("  {p}"));
                }
                lines.join("\n")
            }
        },
        out.json,
    )
}
