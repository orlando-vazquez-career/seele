//! HTTP handler functions. Each handler is a thin wrapper:
//! - deserialize input from path / query / body
//! - call a method on `SeeleService`
//! - serialize response (or surface `ApiError`)

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;

use crate::dto::{
    parse_id, ListRequest, ObservationDto, SaveRequest, SaveResponse, SearchRequest, SearchResponse,
};
use crate::error::Result;
use crate::service::{enforce_search_query_or_filter, SeeleService};

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
