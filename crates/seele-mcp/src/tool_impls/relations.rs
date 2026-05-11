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

pub fn compare(svc: &SeeleService, params: Value) -> Result<Value, ToolError> {
    let sync_id = params
        .get("sync_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadParams("sync_id required".into()))?
        .to_string();
    let source_id = params
        .get("source_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadParams("source_id required".into()))?
        .to_string();
    let target_id = params
        .get("target_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadParams("target_id required".into()))?
        .to_string();
    let reason = params
        .get("reason")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let dto = svc.create_relation(RelationCreateRequest {
        sync_id,
        source_id,
        target_id,
        relation: "conflicts_with".to_string(),
        reason,
        evidence: None,
        confidence: None,
        marked_by_actor: Some("seele_compare".to_string()),
        marked_by_kind: Some("tool".to_string()),
        marked_by_model: None,
        session_id: None,
    })?;
    Ok(serde_json::to_value(dto)?)
}
