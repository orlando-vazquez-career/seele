//! SEELE CLI binary.
//!
//! Subcommands:
//! - `seele save / search / show / list / delete / restore / link` — local memory ops.
//! - `seele stats / doctor / projects` — introspection.
//! - `seele backup <destino>` — consistent one-file DB copy (VACUUM INTO).
//! - `seele sync export / sync import` — chunk-based multi-machine sync.
//! - `seele import from-engram <path>` — one-shot migration from ENGRAM (ADR-13).
//! - `seele embedder reembed-all` — re-embed missing/stale vectors.
//! - `seele setup --agent <name>` — install MCP entry into agent configs.
//! - `seele mcp` — JSON-RPC stdio server.
//! - `seele serve` — HTTP REST API server (incl. POST /chat).
//! - `seele eval` — memory-quality suites (cargo feature `eval`, off by default).
//! - `seele tui` — interactive ratatui UI (cargo feature `tui`, off by default).
//!
//! The default build is slim (T-13): `tui` and `eval` are cargo features,
//! re-enabled together with `--features full`. `chat` is not a feature —
//! it has no subcommand of its own; it lives in `seele serve --chat-*`
//! and inside `seele-http`.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

mod app;
mod commands;
mod output;

use app::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.json;
    let rt = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            let err = anyhow::anyhow!("failed to start tokio runtime: {e}");
            report_error(&err, json);
            return ExitCode::FAILURE;
        }
    };
    match rt.block_on(app::run(cli)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            report_error(&e, json);
            ExitCode::FAILURE
        }
    }
}

/// Errors honor the same `--json` contract as successes: an envelope on
/// stdout for machines, prose on stderr for humans. Exit code is
/// non-zero either way.
fn report_error(err: &anyhow::Error, json: bool) {
    if json {
        output::emit_error_json(err);
    } else {
        eprintln!("seele error: {err:#}");
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
