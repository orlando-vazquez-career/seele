//! Clap-derive command tree + dispatch.
//!
//! Subcommand bodies live in `commands/<area>.rs` to keep this file
//! focused on the wiring.

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Args, Parser, Subcommand};

use seele_embedder::{Embedder, FakeEmbedder, OnnxEmbedder};
use seele_http::SeeleService;
use seele_storage::init_db;

use crate::commands;

/// Env var that forces the FakeEmbedder regardless of CLI flags. Useful
/// for tests, air-gapped environments, and CI where downloading the ONNX
/// model on every invocation would be wasteful or impossible.
const FAKE_EMBEDDER_ENV: &str = "SEELE_FAKE_EMBEDDER";

/// SEELE — local-first memory engine for AI agents.
#[derive(Parser, Debug)]
#[command(name = "seele", version, about, long_about = None)]
pub struct Cli {
    /// Database file. Defaults to `~/.seele/seele.db`.
    #[arg(long, global = true)]
    pub db: Option<PathBuf>,

    /// Force the deterministic FakeEmbedder. Useful for tests, dev
    /// workflows, and air-gapped environments. Equivalent to setting
    /// `SEELE_FAKE_EMBEDDER=1` in the environment.
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
    /// Run a built-in memory-quality evaluation suite (recall@k / MRR).
    Eval(commands::eval::Args),
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
    /// Interactive terminal UI (ratatui).
    Tui(commands::tui::Args),
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
        Command::Eval(args) => commands::eval::run(args, cli.fake_embedder, &out).await,
        Command::Sync(cmd) => commands::sync::run(cmd, &cli.db, cli.fake_embedder, &out).await,
        Command::Import(cmd) => commands::import::run(cmd, &cli.db, cli.fake_embedder, &out).await,
        Command::Setup(args) => commands::setup::run(args, &out).await,
        Command::Mcp(args) => commands::mcp::run(args, &cli.db, cli.fake_embedder).await,
        Command::Serve(args) => commands::serve::run(args, &cli.db, cli.fake_embedder).await,
        Command::Tui(args) => commands::tui::run(args, &cli.db, cli.fake_embedder).await,
    }
}

/// Shared service builder. All subcommands that touch the DB go through
/// this so the embedder choice lives in one place.
///
/// Embedder selection priority (first match wins):
///   1. `--fake-embedder` flag or `SEELE_FAKE_EMBEDDER` env var (non-empty)
///      → `FakeEmbedder`. Use this for tests, dev, air-gapped envs.
///   2. `OnnxEmbedder::new()` → ONNX with `all-MiniLM-L6-v2`. Downloads
///      on first run (~30-90 MB), cached afterwards in
///      `~/.seele/embedder/` (override with `SEELE_EMBEDDER_DIR`).
///   3. Fallback to `FakeEmbedder` with a warning on stderr if ONNX init
///      fails (no network on first run, blocked download, etc). The CLI
///      keeps working but search quality is degraded — vec0 hits become
///      hash-deterministic, not semantic.
pub fn build_service(
    db_override: &Option<PathBuf>,
    fake_embedder_flag: bool,
) -> anyhow::Result<SeeleService> {
    let path = db_override.clone().unwrap_or_else(crate::default_db_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let pool = init_db(&path)?;
    let embedder = pick_embedder(fake_embedder_flag);
    Ok(SeeleService::new(pool, embedder))
}

/// Resolve the embedder per the selection rules documented on
/// [`build_service`]. Split out so unit tests can assert flag behavior
/// without spinning up SQLite.
fn pick_embedder(fake_flag: bool) -> Arc<dyn Embedder> {
    Arc::from(pick_embedder_boxed(fake_flag))
}

/// Boxed variant — the single source of truth for embedder selection,
/// shared with `seele eval` (which needs `Box<dyn Embedder>` for
/// `run_suite`). Previously duplicated in `commands/eval.rs`.
pub(crate) fn pick_embedder_boxed(fake_flag: bool) -> Box<dyn Embedder> {
    if fake_flag || fake_env_set() {
        return Box::new(FakeEmbedder);
    }
    match OnnxEmbedder::new() {
        Ok(emb) => Box::new(emb),
        Err(e) => {
            eprintln!(
                "seele: warning — ONNX embedder unavailable ({e}); falling back \
                 to FakeEmbedder. Search quality is degraded (hash-based, not \
                 semantic). Re-run with network access on first call to \
                 download the model, or set SEELE_FAKE_EMBEDDER=1 to silence \
                 this message."
            );
            // In-band signal for --json consumers: the stderr prose above is
            // invisible to agents; the envelope warning is not.
            crate::output::push_warning(crate::output::WARN_FAKE_EMBEDDER_FALLBACK);
            Box::new(FakeEmbedder)
        }
    }
}

pub(crate) fn fake_env_set() -> bool {
    std::env::var(FAKE_EMBEDDER_ENV)
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_embedder_with_flag_returns_fake() {
        let emb = pick_embedder(true);
        assert_eq!(emb.model_id(), "seele/fake-embedder");
    }
}
