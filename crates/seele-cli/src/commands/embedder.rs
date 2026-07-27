//! `seele embedder` — embedder maintenance commands.
//!
//! T-08: `reembed-all` closes the best-effort embedding hole of the
//! save path — rows whose vector never landed (embedder failure
//! post-save, bulk imports) or that were embedded with a different
//! model are invisible to vec search. `doctor` detects them via
//! `EmbeddingProvenance::mix_warning`; this command re-embeds them.

use std::path::PathBuf;

use clap::{Args as ClapArgs, Subcommand};

use crate::app::OutputOpts;
use crate::output;

#[derive(Subcommand, Debug)]
pub enum EmbedderCmd {
    /// Re-embed active observations whose vector is missing or was
    /// produced by a different model than the active embedder.
    /// Idempotent: a second run finds no work.
    ReembedAll(ReembedAllArgs),
}

#[derive(ClapArgs, Debug)]
pub struct ReembedAllArgs {
    /// Rows embedded per embedder batch call.
    #[arg(long, default_value_t = 64)]
    pub batch_size: usize,
    /// Only count what would be re-embedded; write nothing.
    #[arg(long)]
    pub dry_run: bool,
    /// Limit the pass to one project slug.
    #[arg(long)]
    pub project: Option<String>,
}

pub async fn run(
    cmd: EmbedderCmd,
    db: &Option<PathBuf>,
    fake_embedder: bool,
    out: &OutputOpts,
) -> anyhow::Result<()> {
    match cmd {
        EmbedderCmd::ReembedAll(args) => run_reembed_all(args, db, fake_embedder, out).await,
    }
}

async fn run_reembed_all(
    args: ReembedAllArgs,
    db: &Option<PathBuf>,
    fake_embedder: bool,
    out: &OutputOpts,
) -> anyhow::Result<()> {
    let svc = crate::app::build_service(db, fake_embedder)?;
    let project = args.project.clone();
    let report = svc.reembed_all(project.as_deref(), args.batch_size, args.dry_run)?;
    output::emit_split(
        &report,
        || {
            let mut lines = vec![format!("model: {} ({}d)", report.model_id, report.dim)];
            if let Some(p) = &project {
                lines.push(format!("project: {p}"));
            }
            lines.push(format!("candidates: {}", report.candidates));
            if report.dry_run {
                lines.push("dry-run: nothing written".to_string());
            } else {
                lines.push(format!("re-embedded: {}", report.reembedded));
                lines.push(format!("skipped: {}", report.skipped));
            }
            lines.join("\n")
        },
        out.json,
    )
}
