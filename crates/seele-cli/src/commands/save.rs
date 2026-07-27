use std::io::Read as _;
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
    /// Content body. Multiline is fine: pass it quoted, or pass `-` to
    /// read the full body from stdin (equivalent: `--content <TEXT>`).
    ///
    /// Embedding limit: only the first ~256 tokens reach the embedder;
    /// longer content is stored in full but embedded truncated, so the
    /// tail is invisible to vector search. Chunking is a deferred
    /// feature — see docs/future-features/03-content-chunking.md.
    pub content: Option<String>,
    /// Content body as a flag; equivalent to the positional argument.
    /// `--content -` reads the full body from stdin.
    #[arg(long = "content", value_name = "CONTENT", conflicts_with = "content")]
    pub content_flag: Option<String>,
    /// Observation type. Defaults to `memory`.
    #[arg(long, default_value = "memory")]
    pub r#type: String,
    /// Project name. If omitted, the CLI autodetects it from the cwd
    /// via `seele_project::detect` and logs the result; if detection
    /// fails the project stays NULL. Servers never autodetect — the
    /// caller's explicit project is mandatory there (ADR-16 D4).
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
    let content = resolve_content(args.content, args.content_flag)?;
    let project = resolve_project(args.project);
    let req = SaveRequest {
        title: args.title,
        content,
        r#type: args.r#type,
        project,
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

/// Merge the positional and `--content` forms (clap already rejects
/// passing both) and expand the stdin sentinel: exactly `-` reads the
/// whole body from stdin; anything else is stored literally.
fn resolve_content(positional: Option<String>, flag: Option<String>) -> anyhow::Result<String> {
    let raw = positional.or(flag).ok_or_else(|| {
        anyhow::anyhow!("missing content: pass it as an argument or via --content")
    })?;
    if raw != "-" {
        return Ok(raw);
    }
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| anyhow::anyhow!("failed to read content from stdin: {e}"))?;
    if buf.trim().is_empty() {
        anyhow::bail!("content from stdin is empty — nothing to save");
    }
    Ok(buf)
}

/// Resolve the effective project: the explicit `--project` flag wins;
/// otherwise autodetect from the cwd (ADR-16 D4 — CLI only, `detect`
/// is forbidden in serve/MCP). A detection failure keeps the previous
/// behavior: project stays NULL.
fn resolve_project(flag: Option<String>) -> Option<String> {
    if flag.is_some() {
        return flag;
    }
    let cwd = std::env::current_dir().ok()?;
    let detected = seele_project::detect(&cwd).ok()?;
    // stderr prose: stdout is reserved for the JSON envelope.
    eprintln!(
        "seele: autodetected project \"{}\" ({:?}); pass --project to override",
        detected.name, detected.source
    );
    Some(detected.name)
}
