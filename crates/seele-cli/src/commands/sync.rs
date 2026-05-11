use std::path::PathBuf;

use clap::{Args as ClapArgs, Subcommand};

use crate::app::OutputOpts;
use crate::output;

#[derive(Subcommand, Debug)]
pub enum SyncCmd {
    /// Export observations to a gzipped JSON chunk in `<dir>`.
    Export(ExportArgs),
    /// Import a chunk file into the local DB.
    Import(ImportArgs),
}

#[derive(ClapArgs, Debug)]
pub struct ExportArgs {
    /// Directory to write the chunk into. Created if missing.
    pub dir: PathBuf,
    /// Filter export by project.
    #[arg(long)]
    pub project: Option<String>,
}

#[derive(ClapArgs, Debug)]
pub struct ImportArgs {
    /// Path to a `<chunk_id>.json.gz` file.
    pub path: PathBuf,
    /// Target key identifying this node (hostname, UUID, ...). Same
    /// target_key + chunk_id is a no-op on re-import.
    #[arg(long)]
    pub target_key: String,
}

pub async fn run(
    cmd: SyncCmd,
    db: &Option<PathBuf>,
    fake_embedder: bool,
    out: &OutputOpts,
) -> anyhow::Result<()> {
    let svc = crate::app::build_service(db, fake_embedder)?;
    match cmd {
        SyncCmd::Export(args) => {
            let filter = seele_sync::ExportFilter {
                project: args.project,
            };
            let report = seele_sync::export_to_dir(&svc.observations, &args.dir, filter)?;
            output::emit_split(
                &report,
                || {
                    format!(
                        "exported {} observation(s) → {} ({} bytes)",
                        report.observation_count,
                        report.path.display(),
                        report.bytes_on_disk
                    )
                },
                out.json,
            )
        }
        SyncCmd::Import(args) => {
            let report = seele_sync::import_from_file(
                &svc.observations,
                &svc.chunks,
                &args.target_key,
                &args.path,
            )?;
            output::emit_split(
                &report,
                || {
                    format!(
                        "import {:?}: saved={} already_present={} chunk_skipped_rows={} chunk_id={}",
                        report.outcome,
                        report.observation_count_saved,
                        report.observation_count_already_present,
                        report.observation_count_skipped_chunk_level,
                        report.chunk_id
                    )
                },
                out.json,
            )
        }
    }
}
