use std::path::PathBuf;

use clap::Args as ClapArgs;
use seele_http::dto::SaveRequest;
use serde_json::Value;

use crate::app::OutputOpts;
use crate::output;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Title of the observation.
    pub title: String,
    /// Content body. Can be multiline; pass with quotes or `--content -`
    /// for stdin (Sprint-05 wires the stdin path).
    pub content: String,
    /// Observation type. Defaults to `memory`.
    #[arg(long, default_value = "memory")]
    pub r#type: String,
    /// Project name. If omitted, the caller is expected to set
    /// `--project ""` explicitly when this matters; project detection
    /// (`seele-project`) wires in Sprint-04 Bloque D.2.
    #[arg(long)]
    pub project: Option<String>,
    /// `project` or `personal`. Defaults to `project`.
    #[arg(long)]
    pub scope: Option<String>,
    /// Topic key (e.g. `decision/auth-flow`).
    #[arg(long)]
    pub topic_key: Option<String>,
    /// Inline metadata as a JSON object.
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
    let req = SaveRequest {
        title: args.title,
        content: args.content,
        r#type: args.r#type,
        project: args.project,
        scope: args.scope,
        topic_key: args.topic_key,
        session_id: None,
        tool_name: Some("seele-cli".to_string()),
        metadata,
    };
    let resp = svc.save_observation(req)?;
    output::emit_split(
        &resp,
        || format!("saved {} ({})", resp.id, resp.outcome),
        out.json,
    )
}
