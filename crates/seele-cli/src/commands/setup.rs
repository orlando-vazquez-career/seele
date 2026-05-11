use std::path::PathBuf;

use clap::Args as ClapArgs;
use seele_setup::{install, InstallOptions};

use crate::app::OutputOpts;
use crate::output;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Agent name (e.g. `claude-code`, `cursor`, `windsurf`). For the
    /// full list, run `seele setup --list`.
    #[arg(long)]
    pub agent: Option<String>,
    /// Install for every implemented agent. Mutually exclusive with
    /// `--agent`. Skeleton agents are skipped with a warning.
    #[arg(long)]
    pub all: bool,
    /// List recognized agent names + their status.
    #[arg(long)]
    pub list: bool,
    /// Show what would change without writing.
    #[arg(long)]
    pub dry_run: bool,
    /// Skip backup of existing config (default: backup ON).
    #[arg(long)]
    pub no_backup: bool,
    /// Override the seele binary path that lands in the agent configs.
    #[arg(long)]
    pub seele_binary: Option<PathBuf>,
}

pub async fn run(args: Args, out: &OutputOpts) -> anyhow::Result<()> {
    if args.list {
        let names = seele_setup::all_agent_names();
        output::emit_split(
            &names,
            || {
                let mut lines = vec![format!("{} known agent(s):", names.len())];
                for n in &names {
                    lines.push(format!("  {n}"));
                }
                lines.join("\n")
            },
            out.json,
        )?;
        return Ok(());
    }

    let opts = InstallOptions {
        dry_run: args.dry_run,
        backup: !args.no_backup,
        home_override: None,
        seele_binary: args.seele_binary,
    };

    if args.all {
        let mut reports = Vec::new();
        let mut errors = Vec::new();
        for name in seele_setup::all_agent_names() {
            match install(name, &opts) {
                Ok(r) => reports.push(r),
                Err(e) => errors.push(format!("{name}: {e}")),
            }
        }
        output::emit_split(
            &reports,
            || {
                let mut lines = Vec::new();
                for r in &reports {
                    lines.push(format!(
                        "  {}: {:?} → {}",
                        r.agent,
                        r.outcome,
                        r.config_path.display()
                    ));
                }
                if !errors.is_empty() {
                    lines.push(String::new());
                    lines.push("errors:".to_string());
                    for e in &errors {
                        lines.push(format!("  {e}"));
                    }
                }
                lines.join("\n")
            },
            out.json,
        )?;
        return Ok(());
    }

    let agent = args
        .agent
        .ok_or_else(|| anyhow::anyhow!("--agent <name> or --all required"))?;
    let report = install(&agent, &opts)?;
    output::emit_split(
        &report,
        || {
            format!(
                "{}: {:?} → {}",
                report.agent,
                report.outcome,
                report.config_path.display()
            )
        },
        out.json,
    )
}
