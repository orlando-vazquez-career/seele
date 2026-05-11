use std::path::PathBuf;

use clap::Args as ClapArgs;
use seele_http::dto::SearchRequest;
use seele_http::service::enforce_search_query_or_filter;

use crate::app::OutputOpts;
use crate::output;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Free-text query. Empty allowed when at least one filter is set.
    pub query: Option<String>,
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long)]
    pub scope: Option<String>,
    #[arg(long)]
    pub r#type: Option<String>,
    #[arg(long, default_value_t = 20)]
    pub limit: u32,
    /// Include observations whose `meta.context_mode = "purist"` (MNEMA-specific).
    #[arg(long)]
    pub include_purist: bool,
    #[arg(long)]
    pub include_annotations: bool,
}

pub async fn run(
    args: Args,
    db: &Option<PathBuf>,
    fake_embedder: bool,
    out: &OutputOpts,
) -> anyhow::Result<()> {
    let svc = crate::app::build_service(db, fake_embedder)?;
    let req = SearchRequest {
        query: args.query.unwrap_or_default(),
        project: args.project,
        scope: args.scope,
        r#type: args.r#type,
        limit: Some(args.limit),
        include_purist: args.include_purist,
        include_annotations: args.include_annotations,
        score_boost_multiplier: 0.0,
        max_vec_distance: None,
    };
    enforce_search_query_or_filter(&req)
        .map_err(|e| anyhow::anyhow!("{e:?}: empty query requires at least one filter"))?;
    let resp = svc.search_observations(req)?;

    output::emit_split(
        &resp,
        || {
            if resp.hits.is_empty() {
                "no hits".to_string()
            } else {
                let mut lines = Vec::with_capacity(resp.hits.len() + 1);
                lines.push(format!("{} hit(s):", resp.count));
                for h in &resp.hits {
                    lines.push(format!(
                        "  {} [{}] {:.3}  {}",
                        h.id, h.r#type, h.score, h.title
                    ));
                }
                lines.join("\n")
            }
        },
        out.json,
    )
}
