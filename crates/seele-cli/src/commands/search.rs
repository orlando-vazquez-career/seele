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
    /// Attach the execution trace (Q8): candidate counts per path, the
    /// exact FTS MATCH expressions, vec distances, effective parameters.
    #[arg(long)]
    pub explain: bool,
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

    if args.explain {
        let (resp, trace) = svc.search_observations_explain(req)?;
        let payload = ExplainedSearch { resp, trace };
        return output::emit_split(
            &payload,
            || {
                let mut s = render_hits(&payload.resp);
                s.push('\n');
                s.push_str(&render_trace(&payload.trace));
                s
            },
            out.json,
        );
    }

    let resp = svc.search_observations(req)?;
    output::emit_split(&resp, || render_hits(&resp), out.json)
}

#[derive(serde::Serialize)]
struct ExplainedSearch {
    #[serde(flatten)]
    resp: seele_http::dto::SearchResponse,
    trace: seele_search::SearchTrace,
}

fn render_hits(resp: &seele_http::dto::SearchResponse) -> String {
    if resp.hits.is_empty() {
        return "no hits".to_string();
    }
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

fn render_trace(t: &seele_search::SearchTrace) -> String {
    let dists = t
        .vec_distances
        .iter()
        .take(5)
        .map(|d| format!("{d:.3}"))
        .collect::<Vec<_>>()
        .join(", ");
    [
        format!("--- trace (v{}) ---", t.version),
        format!("fts match:        {}", t.fts_match),
        format!(
            "fts loose match:  {}",
            t.fts_loose_match.as_deref().unwrap_or("(skipped: <2 tokens)")
        ),
        format!(
            "candidates:       fts={} fts_loose={} vec={}",
            t.fts_candidates,
            t.fts_loose_candidates
                .map(|n| n.to_string())
                .unwrap_or_else(|| "-".into()),
            t.vec_candidates
        ),
        format!("vec distances:    [{dists}{}]", if t.vec_distances.len() > 5 { ", …" } else { "" }),
        format!(
            "params:           rrf_k={} per_method_limit={} limit={} boost={} max_vec_distance={:?}",
            t.rrf_k, t.per_method_limit, t.final_limit, t.score_boost_multiplier, t.max_vec_distance
        ),
        format!(
            "embedder:         {} ({}d)",
            t.embedder_model_id, t.embedder_dim
        ),
    ]
    .join("\n")
}
