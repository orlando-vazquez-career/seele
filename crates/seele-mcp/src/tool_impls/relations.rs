//! Relation tools: judge, compare.

use seele_core::id::SeeleId;
use seele_http::dto::{JudgeRequest, RelationCreateRequest};
use seele_http::SeeleService;
use serde_json::Value;

use crate::tools::ToolError;

fn parse_id(v: &Value) -> Result<SeeleId, ToolError> {
    let s = v
        .as_str()
        .ok_or_else(|| ToolError::BadParams("id must be a string".into()))?;
    s.parse::<SeeleId>()
        .map_err(|e| ToolError::BadParams(format!("invalid id: {e}")))
}

pub fn judge(svc: &SeeleService, params: Value) -> Result<Value, ToolError> {
    let relation_id = parse_id(params.get("relation_id").unwrap_or(&Value::Null))?;
    let status = params
        .get("status")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadParams("status required".into()))?
        .to_string();
    let reason = params
        .get("reason")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let evidence = params
        .get("evidence")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let confidence = params.get("confidence").and_then(|v| v.as_f64());
    svc.judge_relation(
        relation_id,
        JudgeRequest {
            status,
            reason,
            evidence,
            confidence,
        },
    )?;
    Ok(serde_json::json!({"ok": true, "id": relation_id.to_string()}))
}

/// Two modes (Q6, GRAIL suggest→confirm cycle):
///
/// - **suggest** (`source_id` without `target_id`, or `mode:"suggest"`):
///   deterministic candidate scan — title Jaro-Winkler + stored-embedding
///   cosine — returning `{id, title, topic_key, score, signal}`. Creates
///   NOTHING; the caller reviews and re-invokes with a `target_id`.
/// - **create** (`source_id` + `target_id`): creates the `conflicts_with`
///   relation as before, but now with a REAL computed `confidence`
///   (max of the two signals) and `marked_by_kind: "heuristic"` — the
///   pre-Q6 behavior hardcoded a content-blind pending relation.
pub fn compare(svc: &SeeleService, params: Value) -> Result<Value, ToolError> {
    let source_id = params
        .get("source_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadParams("source_id required".into()))?
        .to_string();
    let target = params.get("target_id").and_then(|v| v.as_str());
    let suggest_mode = params.get("mode").and_then(|v| v.as_str()) == Some("suggest");

    if suggest_mode || target.is_none() {
        let id = parse_id(params.get("source_id").unwrap_or(&Value::Null))?;
        let top_k = params
            .get("top_k")
            .and_then(|v| v.as_u64())
            .unwrap_or(5)
            .clamp(1, 20) as usize;
        let candidates = svc.find_similar(id, top_k)?;
        let hint = if candidates.is_empty() {
            "no similar memories above the signal thresholds (JW 0.92 / cos 0.93)"
        } else {
            "review the candidates; to record a conflict, re-call \
             seele_compare with source_id + target_id (+ sync_id)"
        };
        return Ok(serde_json::json!({
            "mode": "suggest",
            "source_id": source_id,
            "candidates": candidates,
            "hint": hint,
        }));
    }

    let sync_id = params
        .get("sync_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadParams("sync_id required".into()))?
        .to_string();
    let target_id = target.unwrap_or_default().to_string();
    let reason = params
        .get("reason")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    // Confidence is the strongest computed signal between the two rows —
    // never invented, never silently 1.0.
    let a = parse_id(params.get("source_id").unwrap_or(&Value::Null))?;
    let b = parse_id(params.get("target_id").unwrap_or(&Value::Null))?;
    let confidence = svc.similarity_between(a, b)?;

    let dto = svc.create_relation(RelationCreateRequest {
        sync_id,
        source_id,
        target_id,
        relation: "conflicts_with".to_string(),
        reason,
        evidence: None,
        confidence,
        marked_by_actor: Some("seele_compare".to_string()),
        marked_by_kind: Some("heuristic".to_string()),
        marked_by_model: None,
        session_id: None,
    })?;
    Ok(serde_json::to_value(dto)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use seele_http::dto::SaveRequest;

    fn dummy_svc() -> SeeleService {
        use seele_embedder::{Embedder, FakeEmbedder};
        use seele_storage::init_db;
        use std::sync::Arc;
        let td = tempfile::TempDir::new().unwrap();
        let pool = init_db(td.path().join("seele.db")).unwrap();
        let e: Arc<dyn Embedder> = Arc::new(FakeEmbedder);
        // Leak the tempdir so the DB outlives the test body.
        std::mem::forget(td);
        SeeleService::new(pool, e)
    }

    fn save(svc: &SeeleService, title: &str, content: &str) -> String {
        svc.save_observation(SaveRequest {
            title: title.into(),
            content: content.into(),
            r#type: "memory".into(),
            project: Some("p".into()),
            scope: None,
            topic_key: None,
            session_id: None,
            tool_name: None,
            metadata: serde_json::Value::Null,
        })
        .unwrap()
        .id
    }

    #[test]
    fn compare_suggest_returns_candidates_without_creating_relations() {
        let svc = dummy_svc();
        let a = save(&svc, "politica de retries", "contenido a");
        // FakeEmbedder: distinct contents → unrelated vectors, so the
        // signal here is the near-identical title (JW > 0.92).
        let b = save(&svc, "politica de retriess", "contenido b");

        let r = compare(&svc, serde_json::json!({"source_id": a})).unwrap();
        assert_eq!(r["mode"], "suggest");
        let cands = r["candidates"].as_array().unwrap();
        assert!(
            cands.iter().any(|c| c["id"] == b.as_str()),
            "near-identical title must be suggested: {r}"
        );
        // Suggest mode creates nothing.
        assert!(svc.list_pending_conflicts(None).unwrap().is_empty());
    }

    #[test]
    fn compare_create_records_heuristic_confidence() {
        let svc = dummy_svc();
        let a = save(&svc, "misma decision", "contenido x");
        let b = save(&svc, "misma decision v2", "contenido y");

        let r = compare(
            &svc,
            serde_json::json!({
                "sync_id": "sync-1",
                "source_id": a,
                "target_id": b,
            }),
        )
        .unwrap();
        let conf = r["confidence"].as_f64().expect("computed confidence");
        assert!(conf >= 0.9, "title JW should dominate: {conf}");
        assert_eq!(svc.list_pending_conflicts(None).unwrap().len(), 1);
    }
}
