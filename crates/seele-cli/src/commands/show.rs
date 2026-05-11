use std::path::PathBuf;

use clap::Args as ClapArgs;

use crate::app::OutputOpts;
use crate::output;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// ULID of the observation.
    pub id: String,
}

pub async fn run(
    args: Args,
    db: &Option<PathBuf>,
    fake_embedder: bool,
    out: &OutputOpts,
) -> anyhow::Result<()> {
    let svc = crate::app::build_service(db, fake_embedder)?;
    let parsed = args
        .id
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid id: {e}"))?;
    let dto = svc
        .get_observation(parsed)?
        .ok_or_else(|| anyhow::anyhow!("observation {} not found", args.id))?;
    output::emit_split(
        &dto,
        || {
            format!(
                "{}\n  type: {}\n  scope: {}\n  project: {}\n  title: {}\n  content:\n{}",
                dto.id,
                dto.r#type,
                dto.scope,
                dto.project.as_deref().unwrap_or("-"),
                dto.title,
                indent(&dto.content, "    "),
            )
        },
        out.json,
    )
}

fn indent(s: &str, prefix: &str) -> String {
    s.lines()
        .map(|l| format!("{prefix}{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}
