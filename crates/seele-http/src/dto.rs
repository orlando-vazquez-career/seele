//! Request/response DTOs for the HTTP API.
//!
//! The DTOs are intentionally simpler than the inner core types — strings
//! for IDs (so handlers parse + validate at the boundary), `serde_json::Value`
//! for free-form metadata, no chrono types (Unix epoch ms as i64 instead).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use seele_core::id::SeeleId;
use seele_core::memory::{Observation, ObservationType, Scope};
use seele_core::metadata::Metadata;
use seele_search::{AnnotationKind, SearchHit};

#[derive(Debug, Deserialize, ToSchema)]
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

#[derive(Debug, Serialize, ToSchema)]
pub struct SaveResponse {
    pub id: String,
    /// `"created"` | `"upserted_topic"` | `"duplicate_merged"`.
    pub outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate_count: Option<u32>,
    /// Pre-existing observations whose stored vector sits within the
    /// near-duplicate threshold of the row just saved (Q4). Purely
    /// informational — the save outcome is never altered. Empty when the
    /// embedder failed, the row has no close neighbors, or the write came
    /// through a path that doesn't embed (sync/engram import).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub near_duplicates: Vec<NearDuplicateDto>,
}

/// One near-duplicate candidate reported by the save path (Q4). The
/// distance is vec0's default metric: **L2** over unit-normalized
/// embeddings (0.37 L2 ≈ 0.93 cosine).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NearDuplicateDto {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic_key: Option<String>,
    pub distance: f64,
}

/// One candidate from `find_similar` / `seele_compare` suggest mode (Q6).
/// `signal` names the strongest evidence: `exact` (identical titles),
/// `title` (Jaro-Winkler ≥ 0.92) or `vector` (cosine ≥ 0.93 between
/// stored embeddings). `score` is that signal's similarity in [0, 1].
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SimilarCandidateDto {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic_key: Option<String>,
    pub score: f64,
    pub signal: &'static str,
}

#[derive(Debug, Deserialize, Default, ToSchema)]
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

#[derive(Debug, Serialize, ToSchema)]
pub struct SearchResponse {
    pub hits: Vec<SearchHitDto>,
    pub count: usize,
}

#[derive(Debug, Serialize, ToSchema)]
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
    /// Rank in the bag-of-words FTS rescue path (Q3), when it fired.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fts_loose_rank: Option<usize>,
    pub created_at: i64,
    pub metadata: Value,
    pub annotations: Vec<AnnotationDto>,
}

#[derive(Debug, Serialize, ToSchema)]
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
            fts_loose_rank: h.fts_loose_rank,
            created_at: o.created_at.timestamp_millis(),
            metadata: o.metadata.0.clone(),
            annotations,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
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

#[derive(Debug, Deserialize, Default, ToSchema)]
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

// ---------- Sessions ----------

#[derive(Debug, Deserialize, ToSchema)]
pub struct SessionStartRequest {
    pub project: String,
    #[serde(default)]
    pub directory: Option<String>,
}

#[derive(Debug, Deserialize, Default, ToSchema)]
pub struct SessionListQuery {
    #[serde(default)]
    pub project: Option<String>,
    /// `"active"` | `"ended"` | `"aborted"`. Omit to include all.
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SessionEndRequest {
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SessionDto {
    pub id: String,
    pub project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
    pub started_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// `"active"` | `"ended"` | `"aborted"`.
    pub status: &'static str,
}

impl From<seele_core::session::Session> for SessionDto {
    fn from(s: seele_core::session::Session) -> Self {
        Self {
            id: s.id.to_string(),
            project: s.project,
            directory: s.directory,
            started_at: s.started_at.timestamp_millis(),
            ended_at: s.ended_at.map(|d| d.timestamp_millis()),
            summary: s.summary,
            status: s.status.as_str(),
        }
    }
}

pub fn parse_session_status(
    s: &str,
) -> Result<seele_core::session::SessionStatus, crate::ApiError> {
    seele_core::session::SessionStatus::from_str_strict(s).ok_or_else(|| {
        crate::ApiError::BadRequest(format!(
            "invalid session status '{s}' (expected active | ended | aborted)"
        ))
    })
}

// ---------- Links ----------

#[derive(Debug, Deserialize, ToSchema)]
pub struct LinkCreateRequest {
    pub from_id: String,
    pub to_id: String,
    pub link_type: String,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LinkDto {
    pub id: String,
    pub from_id: String,
    pub to_id: String,
    pub link_type: String,
    pub metadata: Value,
    pub created_at: i64,
}

impl From<seele_core::link::Link> for LinkDto {
    fn from(l: seele_core::link::Link) -> Self {
        Self {
            id: l.id.to_string(),
            from_id: l.from_id.to_string(),
            to_id: l.to_id.to_string(),
            link_type: l.link_type,
            metadata: l.metadata.0,
            created_at: l.created_at.timestamp_millis(),
        }
    }
}

// ---------- Relations ----------

#[derive(Debug, Deserialize, ToSchema)]
pub struct RelationCreateRequest {
    pub sync_id: String,
    pub source_id: String,
    pub target_id: String,
    /// `"supersedes"` | `"conflicts_with"` | `"scoped"` | `"related"` |
    /// `"compatible"` | `"not_conflict"`.
    pub relation: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub evidence: Option<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub marked_by_actor: Option<String>,
    #[serde(default)]
    pub marked_by_kind: Option<String>,
    #[serde(default)]
    pub marked_by_model: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize, Default, ToSchema)]
pub struct RelationListQuery {
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub target_id: Option<String>,
    #[serde(default)]
    pub relation: Option<String>,
    /// `"pending"` | `"judged"` | `"orphaned"` | `"ignored"`.
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct JudgeRequest {
    /// `"pending"` | `"judged"` | `"orphaned"` | `"ignored"`.
    pub status: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub evidence: Option<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RelationDto {
    pub id: String,
    pub sync_id: String,
    pub source_id: String,
    pub target_id: String,
    pub relation: &'static str,
    pub judgment_status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marked_by_actor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marked_by_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marked_by_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub created_at: i64,
}

impl From<seele_core::relation::MemoryRelation> for RelationDto {
    fn from(r: seele_core::relation::MemoryRelation) -> Self {
        Self {
            id: r.id.to_string(),
            sync_id: r.sync_id,
            source_id: r.source_id.to_string(),
            target_id: r.target_id.to_string(),
            relation: r.relation.as_str(),
            judgment_status: r.judgment_status.as_str(),
            reason: r.reason,
            evidence: r.evidence,
            confidence: r.confidence,
            marked_by_actor: r.marked_by_actor,
            marked_by_kind: r.marked_by_kind,
            marked_by_model: r.marked_by_model,
            session_id: r.session_id.map(|s| s.to_string()),
            created_at: r.created_at.timestamp_millis(),
        }
    }
}

pub fn parse_relation_kind(s: &str) -> Result<seele_core::relation::RelationKind, crate::ApiError> {
    seele_core::relation::RelationKind::from_str_strict(s).ok_or_else(|| {
        crate::ApiError::BadRequest(format!(
            "invalid relation '{s}' (expected supersedes | conflicts_with | scoped | related | compatible | not_conflict)"
        ))
    })
}

pub fn parse_judgment_status(
    s: &str,
) -> Result<seele_core::relation::JudgmentStatus, crate::ApiError> {
    seele_core::relation::JudgmentStatus::from_str_strict(s).ok_or_else(|| {
        crate::ApiError::BadRequest(format!(
            "invalid judgment status '{s}' (expected pending | judged | orphaned | ignored)"
        ))
    })
}

// ---------- Stats + embedder ----------

#[derive(Debug, Serialize, ToSchema)]
pub struct StatsResponse {
    pub observations: ObservationStats,
    pub sessions: SessionStats,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ObservationStats {
    pub active: u64,
    pub deleted: u64,
    pub projects: u64,
    pub by_type: Vec<CountBucket>,
    pub by_scope: Vec<CountBucket>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SessionStats {
    pub total: u64,
    pub by_status: Vec<CountBucket>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CountBucket {
    pub key: String,
    pub count: u64,
}

impl From<(String, u64)> for CountBucket {
    fn from((key, count): (String, u64)) -> Self {
        Self { key, count }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EmbedderInfo {
    pub model_id: String,
    pub dim: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_sha256: Option<String>,
}

// ---------- Helpers ----------

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
