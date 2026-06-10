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
    svc.restore_observation(parsed)?;
    let payload = RestoreOutcome {
        id: args.id,
        outcome: "restored",
    };
    output::emit_split(&payload, || format!("restored {}", payload.id), out.json)
}

#[derive(serde::Serialize)]
struct RestoreOutcome {
    id: String,
    outcome: &'static str,
}
