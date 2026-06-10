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

/// Match `title + content` against a small set of ENGRAM-inherited topic
/// families. Returns the highest-scoring family name, or `null` if no
/// signal beats the noise floor.
pub fn suggest_topic_key(_svc: &SeeleService, params: Value) -> Result<Value, ToolError> {
    let title = params
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadParams("title required".into()))?
        .to_lowercase();
    let content = params
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    let combined = format!("{title} {content}");

    let families: &[(&str, &[&str])] = &[
        (
            "architecture",
            &["architecture", "design", "system", "diagram", "boundary"],
        ),
        (
            "bug",
            &["bug", "fix", "regression", "broken", "fails", "crash"],
        ),
        (
            "decision",
            &["decided", "decision", "we chose", "we picked", "adr"],
        ),
        ("pattern", &["pattern", "convention", "idiom", "approach"]),
        ("config", &["config", "setting", "flag", "env var"]),
        (
            "discovery",
            &["found", "discovered", "noticed", "turns out"],
        ),
        ("learning", &["learning", "lesson", "insight", "takeaway"]),
    ];
    let mut best: Option<(&str, usize)> = None;
    for (family, kws) in families {
        let mut score = 0usize;
        for kw in *kws {
            if combined.contains(kw) {
                score += 1;
            }
        }
        if score > 0 && best.map(|(_, s)| score > s).unwrap_or(true) {
            best = Some((family, score));
        }
    }
    Ok(match best {
        Some((family, score)) => serde_json::json!({
            "family": family,
            "score": score,
            "suggestion": format!("{family}/auto"),
        }),
        None => serde_json::json!({ "family": Value::Null, "suggestion": Value::Null }),
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
