use std::path::PathBuf;

use clap::Args as ClapArgs;
use seele_http::dto::LinkCreateRequest;
use serde_json::Value;

use crate::app::OutputOpts;
use crate::output;

#[derive(ClapArgs, Debug)]
pub struct Args {
    pub from_id: String,
    pub to_id: String,
    /// Free-form link type, e.g. `derives_from`, `related_to`,
    /// `supersedes`, `contests`.
    pub link_type: String,
    #[arg(long)]
    pub metadata: Option<String>,
}

pub async fn run(
    args: Args,
    db: &Option<PathBuf>,
    fake_embedder: bool,
    out: &OutputOpts,
) -> anyhow::Result<()> {
    let svc = crate::app::build_service(db, fake_embedder)?;
    let metadata: Value = match args.metadata {
        Some(raw) => serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("invalid --metadata JSON: {e}"))?,
        None => Value::Null,
    };
    let dto = svc.create_link(LinkCreateRequest {
        from_id: args.from_id,
        to_id: args.to_id,
        link_type: args.link_type,
        metadata,
    })?;
    output::emit_split(
        &dto,
        || {
            format!(
                "linked {} -[{}]-> {} (id={})",
                dto.from_id, dto.link_type, dto.to_id, dto.id
            )
        },
        out.json,
    )
}
