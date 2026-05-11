//! `seele import` — one-shot migration from other memory engines.
//!
//! Sub-block D.3 fills in `from-engram` per ADR-13. The clap surface is
//! wired here from D.1 so the help text + dispatch shape are stable
//! across commits.

use std::path::PathBuf;

use clap::{Args as ClapArgs, Subcommand};

use crate::app::OutputOpts;

#[derive(Subcommand, Debug)]
pub enum ImportCmd {
    /// Migrate from an ENGRAM SQLite DB (ADR-13).
    FromEngram(FromEngramArgs),
}

#[derive(ClapArgs, Debug)]
pub struct FromEngramArgs {
    /// Path to the ENGRAM DB (typically `~/.mnema/mnema.db`).
    pub path: PathBuf,
    /// Re-compute embeddings with the SEELE embedder. Otherwise the
    /// vec branch stays empty until a manual reindex.
    #[arg(long)]
    pub re_embed: bool,
    /// Show what would be imported without writing.
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn run(
    cmd: ImportCmd,
    _db: &Option<PathBuf>,
    _fake_embedder: bool,
    _out: &OutputOpts,
) -> anyhow::Result<()> {
    match cmd {
        ImportCmd::FromEngram(_args) => Err(anyhow::anyhow!(
            "import from-engram lands in sprint-04 bloque D.3 (next commit)"
        )),
    }
}
