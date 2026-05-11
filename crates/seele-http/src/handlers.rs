//! HTTP handler functions. Each handler is a thin wrapper:
//! - deserialize input from path / query / body
//! - call a method on `SeeleService`
//! - serialize response (or surface `ApiError`)

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;

use crate::dto::{
    parse_id, EmbedderInfo, JudgeRequest, LinkCreateRequest, LinkDto, ListRequest, ObservationDto,
    RelationCreateRequest, RelationDto, RelationListQuery, SaveRequest, SaveResponse,
    SearchRequest, SearchResponse, SessionDto, SessionEndRequest, SessionListQuery,
    SessionStartRequest, StatsResponse,
};
use crate::error::Result;
use crate::service::{enforce_search_query_or_filter, SeeleService};

use serde::Deserialize;

pub async fn save_memory(
    State(svc): State<Arc<SeeleService>>,
    Json(req): Json<SaveRequest>,
) -> Result<Json<SaveResponse>> {
    let resp = svc.save_observation(req)?;
    Ok(Json(resp))
}

pub async fn search_memories(
    State(svc): State<Arc<SeeleService>>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<SearchResponse>> {
    enforce_search_query_or_filter(&req)?;
    let resp = svc.search_observations(req)?;
    Ok(Json(resp))
}

pub async fn get_memory(
    State(svc): State<Arc<SeeleService>>,
    Path(id): Path<String>,
) -> Result<Json<ObservationDto>> {
    let parsed = parse_id(&id, "id")?;
    let dto = svc
        .get_observation(parsed)?
        .ok_or_else(|| crate::ApiError::NotFound(format!("observation {id}")))?;
    Ok(Json(dto))
}

pub async fn list_memories(
    State(svc): State<Arc<SeeleService>>,
    Query(req): Query<ListRequest>,
) -> Result<Json<Vec<ObservationDto>>> {
    let dtos = svc.list_observations(req)?;
    Ok(Json(dtos))
}

pub async fn soft_delete_memory(
    State(svc): State<Arc<SeeleService>>,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    let parsed = parse_id(&id, "id")?;
    svc.soft_delete_observation(parsed)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn restore_memory(
    State(svc): State<Arc<SeeleService>>,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    let parsed = parse_id(&id, "id")?;
    svc.restore_observation(parsed)?;
    Ok(StatusCode::NO_CONTENT)
}

// -------- Sessions --------

pub async fn start_session(
    State(svc): State<Arc<SeeleService>>,
    Json(req): Json<SessionStartRequest>,
) -> Result<Json<SessionDto>> {
    Ok(Json(svc.start_session(req)?))
}

pub async fn list_sessions(
    State(svc): State<Arc<SeeleService>>,
    Query(q): Query<SessionListQuery>,
) -> Result<Json<Vec<SessionDto>>> {
    Ok(Json(svc.list_sessions(q)?))
}

pub async fn get_session(
    State(svc): State<Arc<SeeleService>>,
    Path(id): Path<String>,
) -> Result<Json<SessionDto>> {
    let parsed = parse_id(&id, "id")?;
    let dto = svc
        .get_session(parsed)?
        .ok_or_else(|| crate::ApiError::NotFound(format!("session {id}")))?;
    Ok(Json(dto))
}

pub async fn end_session(
    State(svc): State<Arc<SeeleService>>,
    Path(id): Path<String>,
    Json(req): Json<SessionEndRequest>,
) -> Result<StatusCode> {
    let parsed = parse_id(&id, "id")?;
    svc.end_session(parsed, req)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn abort_session(
    State(svc): State<Arc<SeeleService>>,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    let parsed = parse_id(&id, "id")?;
    svc.abort_session(parsed)?;
    Ok(StatusCode::NO_CONTENT)
}

// -------- Links --------

pub async fn create_link(
    State(svc): State<Arc<SeeleService>>,
    Json(req): Json<LinkCreateRequest>,
) -> Result<Json<LinkDto>> {
    Ok(Json(svc.create_link(req)?))
}

pub async fn list_links_for_memory(
    State(svc): State<Arc<SeeleService>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<LinkDto>>> {
    let parsed = parse_id(&id, "id")?;
    Ok(Json(svc.list_links_for_observation(parsed)?))
}

pub async fn delete_link(
    State(svc): State<Arc<SeeleService>>,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    let parsed = parse_id(&id, "id")?;
    svc.delete_link(parsed)?;
    Ok(StatusCode::NO_CONTENT)
}

// -------- Relations --------

pub async fn create_relation(
    State(svc): State<Arc<SeeleService>>,
    Json(req): Json<RelationCreateRequest>,
) -> Result<Json<RelationDto>> {
    Ok(Json(svc.create_relation(req)?))
}

pub async fn list_relations(
    State(svc): State<Arc<SeeleService>>,
    Query(q): Query<RelationListQuery>,
) -> Result<Json<Vec<RelationDto>>> {
    Ok(Json(svc.list_relations(q)?))
}

pub async fn judge_relation(
    State(svc): State<Arc<SeeleService>>,
    Path(id): Path<String>,
    Json(req): Json<JudgeRequest>,
) -> Result<StatusCode> {
    let parsed = parse_id(&id, "id")?;
    svc.judge_relation(parsed, req)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, Default)]
pub struct ConflictsQuery {
    #[serde(default)]
    pub limit: Option<u32>,
}

pub async fn list_pending_conflicts(
    State(svc): State<Arc<SeeleService>>,
    Query(q): Query<ConflictsQuery>,
) -> Result<Json<Vec<RelationDto>>> {
    Ok(Json(svc.list_pending_conflicts(q.limit)?))
}

// -------- Stats + embedder --------

pub async fn get_stats(State(svc): State<Arc<SeeleService>>) -> Result<Json<StatsResponse>> {
    Ok(Json(svc.stats()?))
}

pub async fn get_embedder_info(State(svc): State<Arc<SeeleService>>) -> Json<EmbedderInfo> {
    Json(svc.embedder_info())
}
