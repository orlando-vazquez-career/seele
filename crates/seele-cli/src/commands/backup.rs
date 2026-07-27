//! `seele backup <destino>` — consistent one-file copy of the database.
//!
//! T-11. Implementation: SQLite `VACUUM INTO`, chosen over the Online
//! Backup API because a single statement already gives everything a CLI
//! one-shot needs:
//!
//! - **Consistent**: it runs as a plain read transaction, so the copy is
//!   a point-in-time snapshot even while `seele serve` / `seele mcp` are
//!   up. Under WAL a reader never blocks concurrent writers, so those
//!   servers keep accepting writes during the backup.
//! - **Self-contained**: the destination is one complete, defragmented
//!   database file — no `-wal` sidecar to keep next to the copy.
//! - **Safe by default**: SQLite refuses to overwrite an existing
//!   destination, which is exactly the failure mode a backup wants (we
//!   pre-check to report the cause clearly).
//!
//! The Backup API would buy resumable incremental page copies — pointless
//! for a process that exits right after copying.

use std::path::PathBuf;

use clap::Args as ClapArgs;
use serde::Serialize;

use crate::app::OutputOpts;
use crate::output;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Destination file for the backup. Must not exist yet — the command
    /// refuses to overwrite a previous backup. The result is a single
    /// self-contained SQLite database (no -wal sidecar needed).
    pub destination: PathBuf,
}

#[derive(Debug, Serialize)]
struct BackupReport {
    source: PathBuf,
    destination: PathBuf,
    /// Size of the backup file on disk, in bytes.
    bytes: u64,
}

pub async fn run(args: Args, db: &Option<PathBuf>, out: &OutputOpts) -> anyhow::Result<()> {
    let source = db.clone().unwrap_or_else(crate::default_db_path);
    if args.destination.exists() {
        anyhow::bail!(
            "backup destination already exists: {}",
            args.destination.display()
        );
    }
    if let Some(parent) = args.destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let dest_str = args
        .destination
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("destination path is not valid UTF-8"))?;

    // init_db applies the canonical PRAGMAs (WAL, busy_timeout) and runs
    // migrations, like every other DB-touching subcommand. The backup
    // itself is one statement on a pooled connection; no embedder needed.
    let pool = seele_storage::init_db(&source)?;
    let conn = pool.get()?;
    conn.execute("VACUUM INTO ?1", [dest_str])?;

    let bytes = std::fs::metadata(&args.destination)?.len();
    let report = BackupReport {
        source,
        destination: args.destination,
        bytes,
    };
    output::emit_split(
        &report,
        || {
            format!(
                "backup written: {} ({} bytes)\nsource: {}",
                report.destination.display(),
                report.bytes,
                report.source.display()
            )
        },
        out.json,
    )
}
