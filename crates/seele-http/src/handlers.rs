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

// -------- Chat (AI provider with tool-use against /search) --------

use seele_chat::{
    AnthropicProvider, ChatConfig, ChatProvider, Message as ChatMessage,
    OpenAICompatibleProvider, ToolHandler, ToolSpec,
};

use crate::server::ChatProviderConfig;

#[derive(Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    /// Optional override for the system prompt (default is provider-baked).
    pub system_prompt: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ChatResponse {
    pub messages: Vec<ChatMessage>,
    pub provider: String,
    pub model: String,
}

#[derive(serde::Serialize)]
pub struct ChatInfoResponse {
    pub enabled: bool,
    pub provider: Option<String>,
    pub model: Option<String>,
}

pub async fn chat_info(
    State(chat): State<Option<Arc<ChatProviderConfig>>>,
) -> Json<ChatInfoResponse> {
    Json(match chat {
        Some(c) => ChatInfoResponse {
            enabled: true,
            provider: Some(c.provider.clone()),
            model: Some(c.model.clone()),
        },
        None => ChatInfoResponse {
            enabled: false,
            provider: None,
            model: None,
        },
    })
}

pub async fn chat(
    State(svc): State<Arc<SeeleService>>,
    State(chat): State<Option<Arc<ChatProviderConfig>>>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>> {
    let cfg = chat.ok_or_else(|| {
        crate::ApiError::BadRequest(
            "chat is not configured; start seele serve with --chat-provider + --chat-key"
                .to_string(),
        )
    })?;

    let tools = vec![ToolSpec {
        name: "seele_search".to_string(),
        description: "Search the local SEELE memory database. Returns up to `limit` memories ranked by hybrid (FTS + vector) score.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Free-text search query" },
                "limit": { "type": "integer", "description": "Max results (default 5, max 20)", "default": 5 },
                "project": { "type": "string", "description": "Filter to one project (optional)" }
            },
            "required": ["query"]
        }),
    }];

    let provider: Box<dyn ChatProvider> = if cfg.provider.eq_ignore_ascii_case("anthropic") {
        Box::new(AnthropicProvider::new(
            cfg.api_key.clone(),
            cfg.model.clone(),
            tools,
        ))
    } else {
        let endpoint = cfg
            .endpoint
            .clone()
            .unwrap_or_else(|| default_endpoint_for(&cfg.provider));
        Box::new(OpenAICompatibleProvider::new(
            cfg.provider.clone(),
            endpoint,
            cfg.api_key.clone(),
            cfg.model.clone(),
            tools,
        ))
    };

    let svc_for_tools = svc.clone();
    let tool_handler: ToolHandler = Box::new(move |args_json: String| {
        let svc = svc_for_tools.clone();
        Box::pin(async move {
            #[derive(Deserialize)]
            struct Args {
                query: String,
                #[serde(default)]
                limit: Option<u32>,
                #[serde(default)]
                project: Option<String>,
            }
            let args: Args = serde_json::from_str(&args_json)
                .map_err(|e| format!("invalid args: {e}"))?;
            let limit = args.limit.unwrap_or(5).min(20);
            let req = SearchRequest {
                query: args.query.clone(),
                project: args.project,
                scope: None,
                r#type: None,
                limit: Some(limit),
                include_purist: false,
                score_boost_multiplier: 1.0,
                max_vec_distance: None,
                include_annotations: false,
            };
            let resp = svc
                .search_observations(req)
                .map_err(|e| format!("search failed: {e}"))?;
            let summary = serde_json::json!({
                "query": args.query,
                "count": resp.count,
                "results": resp.hits.iter().map(|h| serde_json::json!({
                    "id": h.id,
                    "type": h.r#type,
                    "title": h.title,
                    "project": h.project,
                    "score": h.score,
                    "snippet": h.content.chars().take(280).collect::<String>(),
                })).collect::<Vec<_>>(),
            });
            Ok(summary.to_string())
        })
    });

    let mut config = ChatConfig::default();
    if let Some(sp) = req.system_prompt {
        config.system_prompt = sp;
    }

    let history = seele_chat::run_chat(provider.as_ref(), &tool_handler, req.messages, &config)
        .await
        .map_err(|e| crate::ApiError::Internal(format!("chat failed: {e}")))?;

    Ok(Json(ChatResponse {
        messages: history,
        provider: cfg.provider.clone(),
        model: cfg.model.clone(),
    }))
}

fn default_endpoint_for(provider: &str) -> String {
    match provider.to_ascii_lowercase().as_str() {
        "minimax" => "https://api.minimax.io/v1/text/chatcompletion_v2".to_string(),
        "openai" => "https://api.openai.com/v1/chat/completions".to_string(),
        "openrouter" => "https://openrouter.ai/api/v1/chat/completions".to_string(),
        "together" => "https://api.together.xyz/v1/chat/completions".to_string(),
        "groq" => "https://api.groq.com/openai/v1/chat/completions".to_string(),
        "deepseek" => "https://api.deepseek.com/chat/completions".to_string(),
        _ => "https://api.openai.com/v1/chat/completions".to_string(),
    }
}
