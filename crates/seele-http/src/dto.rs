//! Request/response DTOs for the HTTP API.
//!
//! The DTOs are intentionally simpler than the inner core types — strings
//! for IDs (so handlers parse + validate at the boundary), `serde_json::Value`
//! for free-form metadata, no chrono types (Unix epoch ms as i64 instead).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use seele_core::id::SeeleId;
use seele_core::memory::{Observation, ObservationType, Scope};
use seele_core::metadata::Metadata;
use seele_search::{AnnotationKind, SearchHit};

#[derive(Debug, Deserialize)]
pub struct SaveRequest {
    pub title: String,
    pub content: String,
    /// Observation type; one of the canonical 12 (decision, architecture,
    /// bugfix, pattern, config, discovery, learning, memory, skill,
    /// advisor_output, review, verdict) or any custom string.
    #[serde(default = "default_type")]
    pub r#type: String,
    #[serde(default)]
    pub project: Option<String>,
    /// `"project"` | `"personal"` (defaults to `"project"`).
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub topic_key: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

fn default_type() -> String {
    "memory".to_string()
}

#[derive(Debug, Serialize)]
pub struct SaveResponse {
    pub id: String,
    /// `"created"` | `"upserted_topic"` | `"duplicate_merged"`.
    pub outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate_count: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SearchRequest {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub include_purist: bool,
    #[serde(default)]
    pub include_annotations: bool,
    #[serde(default)]
    pub score_boost_multiplier: f64,
    #[serde(default)]
    pub max_vec_distance: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub hits: Vec<SearchHitDto>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct SearchHitDto {
    pub id: String,
    pub title: String,
    pub content: String,
    pub project: Option<String>,
    pub scope: String,
    pub r#type: String,
    pub score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fts_rank: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vec_rank: Option<usize>,
    pub created_at: i64,
    pub metadata: Value,
    pub annotations: Vec<AnnotationDto>,
}

#[derive(Debug, Serialize)]
pub struct AnnotationDto {
    pub kind: &'static str,
    pub other_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl From<&SearchHit> for SearchHitDto {
    fn from(h: &SearchHit) -> Self {
        let o = &h.observation;
        let annotations = h
            .annotations
            .iter()
            .map(|a| AnnotationDto {
                kind: match a.kind {
                    AnnotationKind::Supersedes => "supersedes",
                    AnnotationKind::SupersededBy => "superseded_by",
                    AnnotationKind::ConflictsWith => "conflicts_with",
                    AnnotationKind::ContestedBy => "contested_by",
                },
                other_id: a.other_id.to_string(),
                other_title: a.other_title.clone(),
                reason: a.reason.clone(),
            })
            .collect();
        Self {
            id: o.id.to_string(),
            title: o.title.clone(),
            content: o.content.clone(),
            project: o.project.clone(),
            scope: o.scope.as_str().to_string(),
            r#type: o.kind.as_str().to_string(),
            score: h.score,
            fts_rank: h.fts_rank,
            vec_rank: h.vec_rank,
            created_at: o.created_at.timestamp_millis(),
            metadata: o.metadata.0.clone(),
            annotations,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ObservationDto {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub r#type: String,
    pub title: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic_key: Option<String>,
    pub revision_count: u32,
    pub duplicate_count: u32,
    pub last_seen_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<i64>,
    pub metadata: Value,
}

impl From<Observation> for ObservationDto {
    fn from(o: Observation) -> Self {
        Self {
            id: o.id.to_string(),
            session_id: o.session_id.map(|s| s.to_string()),
            r#type: o.kind.as_str().to_string(),
            title: o.title,
            content: o.content,
            tool_name: o.tool_name,
            project: o.project,
            scope: o.scope.as_str().to_string(),
            topic_key: o.topic_key,
            revision_count: o.revision_count,
            duplicate_count: o.duplicate_count,
            last_seen_at: o.last_seen_at.timestamp_millis(),
            created_at: o.created_at.timestamp_millis(),
            updated_at: o.updated_at.timestamp_millis(),
            deleted_at: o.deleted_at.map(|d| d.timestamp_millis()),
            metadata: o.metadata.0,
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct ListRequest {
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub topic_key: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub include_deleted: bool,
}

/// Helper to parse a `String` id from a DTO into a `SeeleId` with a
/// 400-friendly error message at the boundary.
pub fn parse_id(s: &str, label: &str) -> Result<SeeleId, crate::ApiError> {
    s.parse::<SeeleId>()
        .map_err(|e| crate::ApiError::BadRequest(format!("invalid {label}: {e}")))
}

/// Parse a `"project"` | `"personal"` string into `Scope`. `None` → default.
pub fn parse_scope(s: Option<&str>) -> Result<Scope, crate::ApiError> {
    match s {
        None => Ok(Scope::default()),
        Some("project") => Ok(Scope::Project),
        Some("personal") => Ok(Scope::Personal),
        Some(other) => Err(crate::ApiError::BadRequest(format!(
            "invalid scope '{other}' (expected project | personal)"
        ))),
    }
}

/// Parse an observation type string. Always succeeds (unknown values become
/// `ObservationType::Other(...)`).
pub fn parse_type(s: &str) -> ObservationType {
    ObservationType::from_str_relaxed(s)
}

/// Parse free-form metadata. Empty/null → empty `Metadata`.
pub fn parse_metadata(v: Value) -> Metadata {
    if v.is_null() {
        Metadata::new()
    } else {
        Metadata::from_value(v)
    }
}
