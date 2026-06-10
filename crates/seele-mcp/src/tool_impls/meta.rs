//! Meta tools: stats, suggest_topic_key, projects, doctor, version.

use seele_http::SeeleService;
use serde_json::Value;

use crate::tools::ToolError;

pub fn stats(svc: &SeeleService, _params: Value) -> Result<Value, ToolError> {
    let s = svc.stats()?;
    Ok(serde_json::to_value(s)?)
}

pub fn projects(svc: &SeeleService, _params: Value) -> Result<Value, ToolError> {
    let projects = svc.list_projects()?;
    Ok(serde_json::json!({ "projects": projects }))
}

pub fn doctor(svc: &SeeleService, _params: Value) -> Result<Value, ToolError> {
    let info = svc.embedder_info();
    let s = svc.stats()?;
    let provenance = svc.embedding_provenance()?;
    let mix_warning = provenance.mix_warning(&info.model_id, info.dim);
    Ok(serde_json::json!({
        "status": "ok",
        "embedder": {
            "model_id": info.model_id,
            "dim": info.dim,
            "expected_sha256": info.expected_sha256,
        },
        "observations_active": s.observations.active,
        "sessions_total": s.sessions.total,
        "embeddings": {
            "models": provenance.models,
            "active_without_vector": provenance.active_without_vector,
            "mix_warning": mix_warning,
        },
        "schema_version": env!("CARGO_PKG_VERSION"),
    }))
}

pub fn version(_svc: &SeeleService, _params: Value) -> Result<Value, ToolError> {
    Ok(serde_json::json!({
        "name": "seele",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// Suggest a topic key for `title + content`. Two signals, in order:
///
/// 1. **Nearest stored neighbor** (Q4): embed the text and KNN against
///    `observations_vec`; if the closest active row sits inside the
///    near-duplicate threshold AND has a topic_key, suggest that key —
///    re-saving the same topic should converge on the same key (that's
///    what makes the upsert fire) instead of minting `<family>/auto`.
/// 2. **Keyword families** (ENGRAM-inherited heuristics) as fallback.
pub fn suggest_topic_key(svc: &SeeleService, params: Value) -> Result<Value, ToolError> {
    let title_raw = params
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadParams("title required".into()))?;
    let content_raw = params.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let project = params.get("project").and_then(|v| v.as_str());

    // Signal 1 — nearest neighbor. Strictly best-effort: any failure
    // (embedder down, empty DB) falls through to the families heuristic.
    if let Ok(vector) = svc.embedder.embed(&format!("{title_raw} {content_raw}")) {
        if let Ok(neighbors) = svc.near_duplicates_for_vector(&vector, project, None, None) {
            if let Some(n) = neighbors.iter().find(|n| n.topic_key.is_some()) {
                return Ok(serde_json::json!({
                    "family": Value::Null,
                    "suggestion": n.topic_key,
                    "source": "neighbor",
                    "neighbor_id": n.id,
                    "neighbor_title": n.title,
                    "distance": n.distance,
                }));
            }
        }
    }

    let title = title_raw.to_lowercase();
    let content = content_raw.to_lowercase();
    let combined = format!("{title} {content}");

    // Signal 2 — keyword families, resolved at boot (Q10): env > project
    // > user > builtin, configurable via topic-families.toml. The match
    // itself stays naive substring voting — this configures the
    // VOCABULARY, not the algorithm.
    let family_set = svc.topic_families();
    let mut best: Option<(&str, usize)> = None;
    for family in &family_set.families {
        let mut score = 0usize;
        for kw in &family.keywords {
            if combined.contains(&kw.to_lowercase()) {
                score += 1;
            }
        }
        if score > 0 && best.map(|(_, s)| score > s).unwrap_or(true) {
            best = Some((family.name.as_str(), score));
        }
    }
    let families_source = family_set.source.as_str();
    Ok(match best {
        Some((family, score)) => serde_json::json!({
            "family": family,
            "score": score,
            "suggestion": format!("{family}/auto"),
            "source": "families",
            "families_source": families_source,
        }),
        None => serde_json::json!({
            "family": Value::Null,
            "suggestion": Value::Null,
            "source": "families",
            "families_source": families_source,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_bug_family_when_keywords_present() {
        let r = suggest_topic_key(
            &dummy_svc(),
            serde_json::json!({"title": "Found a bug in login", "content": "regression in auth"}),
        )
        .unwrap();
        assert_eq!(r["family"], "bug");
    }

    #[test]
    fn returns_null_when_no_signal() {
        let r =
            suggest_topic_key(&dummy_svc(), serde_json::json!({"title": "random stuff"})).unwrap();
        assert!(r["family"].is_null());
        assert_eq!(r["source"], "families");
        // families_source depends on the host (a user-level
        // topic-families.toml is legitimate); the env-pinned E2E in
        // seele-cli covers the custom-file path deterministically.
        assert!(r["families_source"].is_string());
    }

    #[test]
    fn suggests_neighbor_topic_key_when_vector_is_close() {
        let svc = dummy_svc();
        // FakeEmbedder is content-deterministic: store content equal to the
        // exact "{title} {content}" string the tool embeds → distance 0.
        svc.save_observation(seele_http::dto::SaveRequest {
            title: "registro previo".into(),
            content: "login bug detalle".into(),
            r#type: "memory".into(),
            project: Some("p".into()),
            scope: None,
            topic_key: Some("bug/login".into()),
            session_id: None,
            tool_name: None,
            metadata: serde_json::Value::Null,
        })
        .unwrap();

        let r = suggest_topic_key(
            &svc,
            serde_json::json!({"title": "login bug", "content": "detalle", "project": "p"}),
        )
        .unwrap();
        assert_eq!(r["source"], "neighbor");
        assert_eq!(r["suggestion"], "bug/login");
        assert!(r["distance"].as_f64().unwrap() < 1e-6);
    }

    fn dummy_svc() -> SeeleService {
        // Construct a service against an in-memory tempdir DB.
        use seele_embedder::{Embedder, FakeEmbedder};
        use seele_storage::init_db;
        use std::sync::Arc;
        let td = tempfile::TempDir::new().unwrap();
        let pool = init_db(td.path().join("seele.db")).unwrap();
        let e: Arc<dyn Embedder> = Arc::new(FakeEmbedder);
        SeeleService::new(pool, e)
    }
}
