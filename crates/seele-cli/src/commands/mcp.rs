use std::path::PathBuf;

use clap::Args as ClapArgs;
use seele_mcp::{McpServer, McpServerConfig};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Rename exposed tool names. `mnema` activates ADR-13 ENGRAM-compat
    /// (mnema_save, mnema_recall, ...).
    #[arg(long)]
    pub tool_prefix: Option<String>,
}

pub async fn run(args: Args, db: &Option<PathBuf>, fake_embedder: bool) -> anyhow::Result<()> {
    let svc = crate::app::build_service(db, fake_embedder)?;
    let server = McpServer::new(
        svc,
        McpServerConfig {
            tool_prefix: args.tool_prefix,
        },
    );
    server.run_stdio().await
}
