//! Clap-derive command tree + dispatch.
//!
//! Subcommand bodies live in `commands/<area>.rs` to keep this file
//! focused on the wiring.

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Args, Parser, Subcommand};

use seele_embedder::{Embedder, FakeEmbedder};
use seele_http::SeeleService;
use seele_storage::init_db;

use crate::commands;

/// SEELE — local-first memory engine for AI agents.
#[derive(Parser, Debug)]
#[command(name = "seele", version, about, long_about = None)]
pub struct Cli {
    /// Database file. Defaults to `~/.seele/seele.db`.
    #[arg(long, global = true)]
    pub db: Option<PathBuf>,

    /// Use the deterministic FakeEmbedder instead of ONNX. Testing only.
    /// (Sprint-04 ships FakeEmbedder unconditionally; Sprint-05 swaps in
    /// the real OnnxEmbedder by default and this flag stays for tests.)
    #[arg(long, global = true)]
    pub fake_embedder: bool,

    /// Output as JSON (default: human-friendly text).
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Save an observation.
    Save(commands::save::Args),
    /// Hybrid FTS + vec search.
    Search(commands::search::Args),
    /// Fetch one observation by id.
    Show(commands::show::Args),
    /// List observations with optional filters.
    List(commands::list::Args),
    /// Soft-delete an observation.
    Delete(commands::delete::Args),
    /// Restore a soft-deleted observation.
    Restore(commands::restore::Args),
    /// Create a typed link between two observations.
    Link(commands::link::Args),
    /// Aggregate stats: observations + sessions counters.
    Stats,
    /// Health check + embedder + schema.
    Doctor,
    /// List distinct project names.
    Projects,
    /// Multi-machine sync via gzipped JSON chunks.
    #[command(subcommand)]
    Sync(commands::sync::SyncCmd),
    /// One-shot migration from another memory engine.
    #[command(subcommand)]
    Import(commands::import::ImportCmd),
    /// Install SEELE as MCP server into agent configs.
    Setup(commands::setup::Args),
    /// MCP server over stdio (JSON-RPC 2.0).
    Mcp(commands::mcp::Args),
    /// HTTP REST API server.
    Serve(commands::serve::Args),
}

#[derive(Args, Debug, Clone)]
pub struct OutputOpts {
    pub json: bool,
}

pub async fn run(cli: Cli) -> anyhow::Result<()> {
    let out = OutputOpts { json: cli.json };
    match cli.command {
        Command::Save(args) => commands::save::run(args, &cli.db, cli.fake_embedder, &out).await,
        Command::Search(args) => {
            commands::search::run(args, &cli.db, cli.fake_embedder, &out).await
        }
        Command::Show(args) => commands::show::run(args, &cli.db, cli.fake_embedder, &out).await,
        Command::List(args) => commands::list::run(args, &cli.db, cli.fake_embedder, &out).await,
        Command::Delete(args) => {
            commands::delete::run(args, &cli.db, cli.fake_embedder, &out).await
        }
        Command::Restore(args) => {
            commands::restore::run(args, &cli.db, cli.fake_embedder, &out).await
        }
        Command::Link(args) => commands::link::run(args, &cli.db, cli.fake_embedder, &out).await,
        Command::Stats => commands::stats::run(&cli.db, cli.fake_embedder, &out).await,
        Command::Doctor => commands::doctor::run(&cli.db, cli.fake_embedder, &out).await,
        Command::Projects => commands::projects::run(&cli.db, cli.fake_embedder, &out).await,
        Command::Sync(cmd) => commands::sync::run(cmd, &cli.db, cli.fake_embedder, &out).await,
        Command::Import(cmd) => commands::import::run(cmd, &cli.db, cli.fake_embedder, &out).await,
        Command::Setup(args) => commands::setup::run(args, &out).await,
        Command::Mcp(args) => commands::mcp::run(args, &cli.db, cli.fake_embedder).await,
        Command::Serve(args) => commands::serve::run(args, &cli.db, cli.fake_embedder).await,
    }
}

/// Shared service builder. All subcommands that touch the DB go through
/// this so the FakeEmbedder/OnnxEmbedder switch lives in one place.
///
/// Today only the FakeEmbedder branch ships; the ONNX branch lands in
/// Sprint-05 (or earlier if needed) and will be the default. The
/// `--fake-embedder` flag is kept to force-disable ONNX in tests.
pub fn build_service(
    db_override: &Option<PathBuf>,
    _fake_embedder_flag: bool,
) -> anyhow::Result<SeeleService> {
    let path = db_override.clone().unwrap_or_else(crate::default_db_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let pool = init_db(&path)?;
    let embedder: Arc<dyn Embedder> = Arc::new(FakeEmbedder);
    Ok(SeeleService::new(pool, embedder))
}
