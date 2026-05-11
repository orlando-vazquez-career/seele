//! SEELE CLI binary.
//!
//! Subcommands:
//! - `seele save / search / show / list / delete / restore / link` — local memory ops.
//! - `seele stats / doctor / projects` — introspection.
//! - `seele sync export / sync import` — chunk-based multi-machine sync.
//! - `seele import --from-engram <path>` — one-shot migration from ENGRAM (ADR-13).
//! - `seele setup --agent <name>` — install MCP entry into agent configs.
//! - `seele mcp` — JSON-RPC stdio server.
//! - `seele serve` — HTTP REST API server.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

mod app;
mod commands;
mod output;

use app::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let rt = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("failed to start tokio runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    match rt.block_on(app::run(cli)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("seele error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

/// Default DB path used when `--db` is not provided.
pub fn default_db_path() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".seele").join("seele.db")
    } else {
        PathBuf::from(".seele/seele.db")
    }
}
