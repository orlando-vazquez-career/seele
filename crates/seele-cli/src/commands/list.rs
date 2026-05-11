use std::path::PathBuf;

use clap::Args as ClapArgs;
use seele_http::dto::ListRequest;

use crate::app::OutputOpts;
use crate::output;

#[derive(ClapArgs, Debug)]
pub struct Args {
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long)]
    pub scope: Option<String>,
    #[arg(long)]
    pub r#type: Option<String>,
    #[arg(long)]
    pub topic_key: Option<String>,
    #[arg(long)]
    pub session_id: Option<String>,
    #[arg(long, default_value_t = 50)]
    pub limit: u32,
    #[arg(long)]
    pub include_deleted: bool,
}

pub async fn run(
    args: Args,
    db: &Option<PathBuf>,
    fake_embedder: bool,
    out: &OutputOpts,
) -> anyhow::Result<()> {
    let svc = crate::app::build_service(db, fake_embedder)?;
    let req = ListRequest {
        project: args.project,
        scope: args.scope,
        r#type: args.r#type,
        topic_key: args.topic_key,
        session_id: args.session_id,
        limit: Some(args.limit),
        include_deleted: args.include_deleted,
    };
    let dtos = svc.list_observations(req)?;
    output::emit_split(
        &dtos,
        || {
            if dtos.is_empty() {
                "no observations".to_string()
            } else {
                let mut lines = Vec::with_capacity(dtos.len() + 1);
                lines.push(format!("{} observation(s):", dtos.len()));
                for d in &dtos {
                    lines.push(format!("  {} [{}] {}", d.id, d.r#type, d.title));
                }
                lines.join("\n")
            }
        },
        out.json,
    )
}
