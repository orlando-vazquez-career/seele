//! Memory tools: save, search, show, list, update_metadata, soft_delete,
//! restore, link.

use seele_core::id::SeeleId;
use seele_http::dto::{LinkCreateRequest, ListRequest, SaveRequest, SearchRequest};
use seele_http::service::enforce_search_query_or_filter;
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

pub fn save(svc: &SeeleService, params: Value) -> Result<Value, ToolError> {
    let req: SaveRequest = serde_json::from_value(params)?;
    let resp = svc.save_observation(req)?;
    let has_near_dups = !resp.near_duplicates.is_empty();
    let mut value = serde_json::to_value(resp)?;
    // GRAIL-style next-step hint: agents act on directives, not on fields
    // they'd have to interpret.
    if has_near_dups {
        value["hint"] = Value::String(
            "near-duplicates detected: review them and either update the \
             existing memory, or link this one (seele_link supersedes/\
             related) instead of letting paraphrased copies accumulate."
                .to_string(),
        );
    }
    Ok(value)
}

pub fn search(svc: &SeeleService, params: Value) -> Result<Value, ToolError> {
    let req: SearchRequest = serde_json::from_value(params)?;
    enforce_search_query_or_filter(&req)?;
    let resp = svc.search_observations(req)?;
    Ok(serde_json::to_value(resp)?)
}

pub fn show(svc: &SeeleService, params: Value) -> Result<Value, ToolError> {
    let id = parse_id(params.get("id").unwrap_or(&Value::Null))?;
    let dto = svc
        .get_observation(id)?
        .ok_or_else(|| ToolError::NotFound(format!("observation {id}")))?;
    Ok(serde_json::to_value(dto)?)
}

pub fn list(svc: &SeeleService, params: Value) -> Result<Value, ToolError> {
    let req: ListRequest = serde_json::from_value(params)?;
    let dtos = svc.list_observations(req)?;
    Ok(serde_json::to_value(dtos)?)
}

pub fn update_metadata(svc: &SeeleService, params: Value) -> Result<Value, ToolError> {
    let id = parse_id(params.get("id").unwrap_or(&Value::Null))?;
    let patch = params
        .get("metadata_patch")
        .cloned()
        .ok_or_else(|| ToolError::BadParams("metadata_patch required".into()))?;
    if !patch.is_object() {
        return Err(ToolError::BadParams(
            "metadata_patch must be an object".into(),
        ));
    }
    svc.merge_observation_metadata(id, patch)?;
    Ok(serde_json::json!({"ok": true, "id": id.to_string()}))
}

pub fn soft_delete(svc: &SeeleService, params: Value) -> Result<Value, ToolError> {
    let id = parse_id(params.get("id").unwrap_or(&Value::Null))?;
    svc.soft_delete_observation(id)?;
    Ok(serde_json::json!({"ok": true, "id": id.to_string()}))
}

pub fn restore(svc: &SeeleService, params: Value) -> Result<Value, ToolError> {
    let id = parse_id(params.get("id").unwrap_or(&Value::Null))?;
    svc.restore_observation(id)?;
    Ok(serde_json::json!({"ok": true, "id": id.to_string()}))
}

pub fn link(svc: &SeeleService, params: Value) -> Result<Value, ToolError> {
    let req: LinkCreateRequest = serde_json::from_value(params)?;
    let dto = svc.create_link(req)?;
    Ok(serde_json::to_value(dto)?)
}
