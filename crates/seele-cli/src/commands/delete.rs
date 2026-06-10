use std::path::PathBuf;

use clap::Args as ClapArgs;

use crate::app::OutputOpts;
use crate::output;

#[derive(ClapArgs, Debug)]
pub struct Args {
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
    svc.soft_delete_observation(parsed)?;
    let payload = DeleteOutcome {
        id: args.id,
        outcome: "soft-deleted",
    };
    output::emit_split(
        &payload,
        || format!("soft-deleted {}", payload.id),
        out.json,
    )
}

#[derive(serde::Serialize)]
struct DeleteOutcome {
    id: String,
    outcome: &'static str,
}
