//! `seele tui` — boot the ratatui interactive UI (ADR-07). The TUI
//! talks to the in-process `SeeleService`; no HTTP round trip.

use std::path::PathBuf;

use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Headless smoke mode: render one frame off-screen and exit 0.
    /// Used by the v0.1.0 acceptance smoke (MVP criterion 8). Hidden
    /// from `--help` because it's not for interactive use.
    #[arg(long, hide = true)]
    pub smoke: bool,
}

pub async fn run(args: Args, db: &Option<PathBuf>, fake_embedder: bool) -> anyhow::Result<()> {
    let svc = crate::app::build_service(db, fake_embedder)?;
    if args.smoke {
        seele_tui::run_tui_smoke(svc).map_err(|e| anyhow::anyhow!("tui smoke: {e}"))?;
        println!("tui smoke ok");
        return Ok(());
    }
    seele_tui::run_tui(svc)
        .await
        .map_err(|e| anyhow::anyhow!("tui: {e}"))?;
    Ok(())
}
