//! `seele tui` — boot the ratatui interactive UI (ADR-07). The TUI
//! talks to the in-process `SeeleService`; no HTTP round trip.

use std::path::PathBuf;

use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {}

pub async fn run(_args: Args, db: &Option<PathBuf>, fake_embedder: bool) -> anyhow::Result<()> {
    let svc = crate::app::build_service(db, fake_embedder)?;
    seele_tui::run_tui(svc)
        .await
        .map_err(|e| anyhow::anyhow!("tui: {e}"))?;
    Ok(())
}
