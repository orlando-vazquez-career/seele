//! `seele import` — one-shot migration from other memory engines.
//!
//! Sub-block D.3 wires `from-engram` to `seele-engram-import` per
//! ADR-13. Re-running over the same source is idempotent (preserved
//! ULIDs short-circuit; cuid-style ids are kept distinct via the
//! `engram_id` breadcrumb).

use std::path::PathBuf;

use clap::{Args as ClapArgs, Subcommand};
use seele_engram_import::EngramImporter;
use seele_storage::{init_db, LinkStore, ObservationStore};

use crate::app::OutputOpts;
use crate::output;

#[derive(Subcommand, Debug)]
pub enum ImportCmd {
    /// Migrate from an ENGRAM SQLite DB (ADR-13).
    FromEngram(FromEngramArgs),
}

#[derive(ClapArgs, Debug)]
pub struct FromEngramArgs {
    /// Path to the ENGRAM DB (typically `~/.mnema/mnema.db`).
    pub path: PathBuf,
    /// Re-compute embeddings with the SEELE embedder. v0.1 ships
    /// `FakeEmbedder`, so this flag is recognized but a no-op until
    /// Sprint-05 lands the real ONNX backend.
    #[arg(long)]
    pub re_embed: bool,
    /// Show what would be imported without writing.
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn run(
    cmd: ImportCmd,
    db: &Option<PathBuf>,
    _fake_embedder: bool,
    out: &OutputOpts,
) -> anyhow::Result<()> {
    match cmd {
        ImportCmd::FromEngram(args) => run_from_engram(args, db, out).await,
    }
}

async fn run_from_engram(
    args: FromEngramArgs,
    db: &Option<PathBuf>,
    out: &OutputOpts,
) -> anyhow::Result<()> {
    let path = db.clone().unwrap_or_else(crate::default_db_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let pool = init_db(&path)?;
    let observations = ObservationStore::new(pool.clone());
    let links = LinkStore::new(pool);

    let importer = EngramImporter::new(&observations, &links);
    let report = importer.import_from(&args.path, args.dry_run)?;

    if args.re_embed {
        tracing::warn!(
            "--re-embed recognized but is a no-op in v0.1 (FakeEmbedder is the only backend); \
             ONNX backend lands in Sprint-05."
        );
    }

    output::emit_split(
        &report,
        || {
            let mut lines = Vec::new();
            lines.push(format!(
                "source: {} ({})",
                report.source_path.display(),
                if report.dry_run { "dry-run" } else { "applied" }
            ));
            lines.push(format!("  rows seen:            {}", report.rows_seen));
            lines.push(format!("  rows inserted:        {}", report.rows_inserted));
            lines.push(format!(
                "  rows skipped (exist): {}",
                report.rows_skipped_existing
            ));
            lines.push(format!("  rows invalid:         {}", report.rows_invalid));
            lines.push(format!("  links created:        {}", report.links_created));
            lines.push(format!("  links dangling:       {}", report.links_dangling));
            if !report.errors.is_empty() {
                lines.push(String::new());
                lines.push(format!("errors ({}):", report.errors.len()));
                for e in &report.errors {
                    lines.push(format!("  {e}"));
                }
            }
            lines.join("\n")
        },
        out.json,
    )
}
