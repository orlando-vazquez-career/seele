//! Tool registry + dispatch.
//!
//! Each tool is a thin shim over `SeeleService`. Tools are registered by
//! their canonical SEELE name (`seele_save`, ...); the server may
//! optionally rename them at the boundary via a tool-prefix flag (see
//! `McpServer::with_tool_prefix`).

use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;

use seele_http::SeeleService;

use crate::jsonrpc::{codes, ErrorObject};
use crate::tool_impls;

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("bad params: {0}")]
    BadParams(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("internal: {0}")]
    Internal(String),
}

impl ToolError {
    pub fn to_jsonrpc(&self) -> ErrorObject {
        match self {
            Self::BadParams(m) => ErrorObject::new(codes::INVALID_PARAMS, m.clone()),
            Self::NotFound(m) => ErrorObject::new(codes::TOOL_ERROR, m.clone()),
            Self::Conflict(m) => ErrorObject::new(codes::TOOL_ERROR, m.clone()),
            Self::Internal(m) => ErrorObject::new(codes::INTERNAL_ERROR, m.clone()),
        }
    }
}

impl From<seele_http::ApiError> for ToolError {
    fn from(e: seele_http::ApiError) -> Self {
        use seele_http::ApiError::*;
        match e {
            BadRequest(m) => Self::BadParams(m),
            NotFound(m) => Self::NotFound(m),
            Conflict(m) => Self::Conflict(m),
            Unauthorized => Self::Internal("unauthorized".into()),
            Internal(m) => Self::Internal(m),
        }
    }
}

impl From<serde_json::Error> for ToolError {
    fn from(e: serde_json::Error) -> Self {
        Self::BadParams(format!("json error: {e}"))
    }
}

pub type ToolHandler = fn(&SeeleService, Value) -> Result<Value, ToolError>;

#[derive(Clone)]
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    pub handler: ToolHandler,
}

#[derive(Serialize)]
pub struct ToolDescriptor<'a> {
    pub name: String,
    pub description: &'a str,
    #[serde(rename = "inputSchema")]
    pub input_schema: &'a Value,
}

/// All 19 canonical SEELE tools. Static registry — never mutated at runtime.
pub fn all_tools() -> Vec<Tool> {
    use tool_impls::*;
    vec![
        Tool {
            name: "seele_save",
            description: "Save an observation. Computes embedding, runs privacy strip, applies topic-key upsert + dedup.",
            input_schema: schema_save(),
            handler: memories::save,
        },
        Tool {
            name: "seele_search",
            description: "Hybrid FTS + vec + RRF search. Empty query requires at least one filter.",
            input_schema: schema_search(),
            handler: memories::search,
        },
        Tool {
            name: "seele_show",
            description: "Fetch a single observation by id.",
            input_schema: schema_id_only("id"),
            handler: memories::show,
        },
        Tool {
            name: "seele_list",
            description: "List observations with optional filters.",
            input_schema: schema_list(),
            handler: memories::list,
        },
        Tool {
            name: "seele_update_metadata",
            description: "Merge a metadata patch into an observation's metadata JSON.",
            input_schema: schema_update_metadata(),
            handler: memories::update_metadata,
        },
        Tool {
            name: "seele_delete",
            description: "Soft-delete an observation (sets deleted_at).",
            input_schema: schema_id_only("id"),
            handler: memories::soft_delete,
        },
        Tool {
            name: "seele_restore",
            description: "Restore a soft-deleted observation (clears deleted_at).",
            input_schema: schema_id_only("id"),
            handler: memories::restore,
        },
        Tool {
            name: "seele_link",
            description: "Create a typed link between two observations.",
            input_schema: schema_link(),
            handler: memories::link,
        },
        Tool {
            name: "seele_stats",
            description: "Aggregate stats: observations active/deleted/projects + by_type/by_scope; sessions total + by_status.",
            input_schema: empty_object_schema(),
            handler: meta::stats,
        },
        Tool {
            name: "seele_session_start",
            description: "Start a session.",
            input_schema: schema_session_start(),
            handler: sessions::start,
        },
        Tool {
            name: "seele_session_end",
            description: "End an active session.",
            input_schema: schema_session_end(),
            handler: sessions::end,
        },
        Tool {
            name: "seele_session_summary",
            description: "Save a structured session summary as an observation of type=memory tied to the session.",
            input_schema: schema_session_summary(),
            handler: sessions::summary,
        },
        Tool {
            name: "seele_capture_passive",
            description: "Parse a chat transcript for `## Key Learnings:` blocks and save each as an observation (batch).",
            input_schema: schema_capture_passive(),
            handler: sessions::capture_passive,
        },
        Tool {
            name: "seele_judge",
            description: "Apply a judgment to a memory_relation.",
            input_schema: schema_judge(),
            handler: relations::judge,
        },
        Tool {
            name: "seele_compare",
            description: "Mark two observations as conflicts_with (pending).",
            input_schema: schema_compare(),
            handler: relations::compare,
        },
        Tool {
            name: "seele_suggest_topic_key",
            description: "Suggest a topic_key from default heuristic families given title + content.",
            input_schema: schema_suggest_topic_key(),
            handler: meta::suggest_topic_key,
        },
        Tool {
            name: "seele_projects",
            description: "List distinct projects across active observations.",
            input_schema: empty_object_schema(),
            handler: meta::projects,
        },
        Tool {
            name: "seele_doctor",
            description: "Health check: returns DB pool status, embedder model+dim, schema version.",
            input_schema: empty_object_schema(),
            handler: meta::doctor,
        },
        Tool {
            name: "seele_version",
            description: "Package version + schema version.",
            input_schema: empty_object_schema(),
            handler: meta::version,
        },
    ]
}

/// Build a name → Tool index, optionally rewriting names with a prefix.
///
/// ENGRAM compatibility (ADR-13): when `prefix = "mnema"`, tools are
/// renamed `seele_save` → `mnema_save` etc. Plus one special-case:
/// ENGRAM used `recall` instead of `search`, so `seele_search` becomes
/// `mnema_recall` rather than `mnema_search`.
pub fn build_index(prefix: Option<&str>) -> HashMap<String, Tool> {
    let mut map = HashMap::new();
    for tool in all_tools() {
        let name = match prefix {
            None => tool.name.to_string(),
            Some("seele") => tool.name.to_string(),
            Some(p) => translate_name(tool.name, p),
        };
        map.insert(name, tool);
    }
    map
}

fn translate_name(canonical: &str, prefix: &str) -> String {
    // canonical is always `seele_<suffix>`.
    let suffix = canonical.strip_prefix("seele_").unwrap_or(canonical);
    // ADR-13: ENGRAM used `recall` for search.
    let suffix = if prefix == "mnema" && suffix == "search" {
        "recall"
    } else {
        suffix
    };
    format!("{prefix}_{suffix}")
}

// ---------- input_schema helpers ----------

fn empty_object_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false,
    })
}

fn schema_id_only(field: &str) -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            field: { "type": "string", "description": "ULID" },
        },
        "required": [field],
    })
}

fn schema_save() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" },
            "content": { "type": "string" },
            "type": { "type": "string", "description": "One of the 12 canonical types or any custom string. Defaults to 'memory'." },
            "project": { "type": "string" },
            "scope": { "type": "string", "enum": ["project", "personal"] },
            "topic_key": { "type": "string" },
            "session_id": { "type": "string" },
            "tool_name": { "type": "string" },
            "metadata": { "type": "object" },
        },
        "required": ["title", "content"],
    })
}

fn schema_search() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "query": { "type": "string" },
            "project": { "type": "string" },
            "scope": { "type": "string", "enum": ["project", "personal"] },
            "type": { "type": "string" },
            "limit": { "type": "integer", "minimum": 1 },
            "include_purist": { "type": "boolean" },
            "include_annotations": { "type": "boolean" },
            "score_boost_multiplier": { "type": "number" },
            "max_vec_distance": { "type": "number" },
        },
    })
}

fn schema_list() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "project": { "type": "string" },
            "scope": { "type": "string" },
            "type": { "type": "string" },
            "topic_key": { "type": "string" },
            "session_id": { "type": "string" },
            "limit": { "type": "integer" },
            "include_deleted": { "type": "boolean" },
        },
    })
}

fn schema_update_metadata() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "metadata_patch": { "type": "object" },
        },
        "required": ["id", "metadata_patch"],
    })
}

fn schema_link() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "from_id": { "type": "string" },
            "to_id": { "type": "string" },
            "link_type": { "type": "string" },
            "metadata": { "type": "object" },
        },
        "required": ["from_id", "to_id", "link_type"],
    })
}

fn schema_session_start() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "project": { "type": "string" },
            "directory": { "type": "string" },
        },
        "required": ["project"],
    })
}

fn schema_session_end() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "summary": { "type": "string" },
        },
        "required": ["id"],
    })
}

fn schema_session_summary() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "session_id": { "type": "string" },
            "project": { "type": "string" },
            "title": { "type": "string" },
            "summary": { "type": "string" },
        },
        "required": ["session_id", "title", "summary"],
    })
}

fn schema_capture_passive() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "transcript": { "type": "string" },
            "project": { "type": "string" },
            "session_id": { "type": "string" },
        },
        "required": ["transcript"],
    })
}

fn schema_judge() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "relation_id": { "type": "string" },
            "status": { "type": "string", "enum": ["pending", "judged", "orphaned", "ignored"] },
            "reason": { "type": "string" },
            "evidence": { "type": "string" },
            "confidence": { "type": "number" },
        },
        "required": ["relation_id", "status"],
    })
}

fn schema_compare() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "sync_id": { "type": "string" },
            "source_id": { "type": "string" },
            "target_id": { "type": "string" },
            "reason": { "type": "string" },
        },
        "required": ["sync_id", "source_id", "target_id"],
    })
}

fn schema_suggest_topic_key() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" },
            "content": { "type": "string" },
        },
        "required": ["title"],
    })
}
