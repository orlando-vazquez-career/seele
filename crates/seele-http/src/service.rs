//! Service layer used by both HTTP handlers and MCP tools.
//!
//! Application logic lives here. Transport-specific code (axum handlers,
//! JSON-RPC dispatch) only deserializes the request, calls a method on
//! `SeeleService`, and serializes the response. This keeps logic DRY across
//! the two transports — when an operation needs to change, it changes in
//! exactly one place.

use std::sync::Arc;

use seele_core::id::SeeleId;
use seele_core::memory::Observation;
use seele_embedder::Embedder;
use seele_search::{SearchEngine, SearchQuery};
use seele_storage::{
    ChunkStore, LinkInput, LinkQuery, LinkStore, ObservationQuery, ObservationStore, Pool,
    PromptStore, RelationStore, SaveInput, SaveOutcome, SessionFilter, SessionInput, SessionStore,
};

use crate::dto::{
    parse_id, parse_metadata, parse_scope, parse_session_status, parse_type, LinkCreateRequest,
    LinkDto, ListRequest, ObservationDto, SaveRequest, SaveResponse, SearchHitDto, SearchRequest,
    SearchResponse, SessionDto, SessionEndRequest, SessionListQuery, SessionStartRequest,
};
use crate::error::{ApiError, Result};

#[derive(Clone)]
pub struct SeeleService {
    pub observations: ObservationStore,
    pub sessions: SessionStore,
    pub links: LinkStore,
    pub relations: RelationStore,
    pub prompts: PromptStore,
    pub chunks: ChunkStore,
    pub search: Arc<SearchEngine>,
    pub embedder: Arc<dyn Embedder>,
    pub pool: Pool,
}

impl SeeleService {
    pub fn new(pool: Pool, embedder: Arc<dyn Embedder>) -> Self {
        let observations = ObservationStore::new(pool.clone());
        let sessions = SessionStore::new(pool.clone());
        let links = LinkStore::new(pool.clone());
        let relations = RelationStore::new(pool.clone());
        let prompts = PromptStore::new(pool.clone());
        let chunks = ChunkStore::new(pool.clone());
        let search_embedder: Box<dyn Embedder> = Box::new(ArcEmbedder(embedder.clone()));
        let search = Arc::new(SearchEngine::new(pool.clone(), search_embedder));
        Self {
            observations,
            sessions,
            links,
            relations,
            prompts,
            chunks,
            search,
            embedder,
            pool,
        }
    }

    /// Save an observation + compute & store its embedding atomically (best
    /// effort: the embedding write is post-save, so if the embedder fails
    /// the observation row is still persisted — search will skip it from the
    /// vec branch until a reindex pass).
    pub fn save_observation(&self, req: SaveRequest) -> Result<SaveResponse> {
        let session_id = req
            .session_id
            .as_deref()
            .map(|s| parse_id(s, "session_id"))
            .transpose()?;
        let scope = parse_scope(req.scope.as_deref())?;
        let kind = parse_type(&req.r#type);
        let metadata = parse_metadata(req.metadata);

        let input = SaveInput {
            session_id,
            kind,
            title: req.title.clone(),
            content: req.content.clone(),
            tool_name: req.tool_name,
            project: req.project,
            scope,
            topic_key: req.topic_key,
            metadata,
        };
        let outcome = self.observations.save(input)?;

        // Best-effort embedding write. Failure logs but does not surface
        // as a 5xx — the row is persisted and a reindex pass can fix it.
        match self.embedder.embed(&req.content) {
            Ok(v) => {
                if let Err(e) = self.observations.set_embedding(outcome.id(), &v) {
                    tracing::warn!(error=%e, id=%outcome.id(), "embedding write failed post-save");
                }
            }
            Err(e) => {
                tracing::warn!(error=%e, id=%outcome.id(), "embedder failed post-save");
            }
        }

        Ok(match outcome {
            SaveOutcome::Created(id) => SaveResponse {
                id: id.to_string(),
                outcome: "created",
                revision_count: None,
                duplicate_count: None,
            },
            SaveOutcome::UpsertedTopic { id, revision_count } => SaveResponse {
                id: id.to_string(),
                outcome: "upserted_topic",
                revision_count: Some(revision_count),
                duplicate_count: None,
            },
            SaveOutcome::DuplicateMerged {
                id,
                duplicate_count,
            } => SaveResponse {
                id: id.to_string(),
                outcome: "duplicate_merged",
                revision_count: None,
                duplicate_count: Some(duplicate_count),
            },
        })
    }

    /// Run the search engine. Caller is responsible for the anti-empty-query
    /// gate at the transport layer (HTTP handler / MCP tool both apply it).
    pub fn search_observations(&self, req: SearchRequest) -> Result<SearchResponse> {
        let scope = match req.scope.as_deref() {
            None => None,
            Some(s) => Some(parse_scope(Some(s))?),
        };
        let q = SearchQuery {
            text: req.query,
            project: req.project,
            scope,
            kind: req.r#type,
            per_method_limit: None,
            limit: req.limit,
            include_purist: req.include_purist,
            score_boost_multiplier: req.score_boost_multiplier,
            max_vec_distance: req.max_vec_distance,
            include_annotations: req.include_annotations,
        };
        let hits = self.search.search(q)?;
        let dtos: Vec<SearchHitDto> = hits.iter().map(SearchHitDto::from).collect();
        let count = dtos.len();
        Ok(SearchResponse { hits: dtos, count })
    }

    pub fn get_observation(&self, id: SeeleId) -> Result<Option<ObservationDto>> {
        Ok(self.observations.get(id)?.map(ObservationDto::from))
    }

    pub fn list_observations(&self, req: ListRequest) -> Result<Vec<ObservationDto>> {
        let scope = match req.scope.as_deref() {
            None => None,
            Some(s) => Some(parse_scope(Some(s))?),
        };
        let session_id = req
            .session_id
            .as_deref()
            .map(|s| parse_id(s, "session_id"))
            .transpose()?;
        let kind = req.r#type.as_deref().map(parse_type);
        let q = ObservationQuery {
            project: req.project,
            scope,
            kind,
            session_id,
            topic_key: req.topic_key,
            include_deleted: req.include_deleted,
            limit: req.limit,
        };
        let observations: Vec<Observation> = self.observations.list(q)?;
        Ok(observations.into_iter().map(ObservationDto::from).collect())
    }

    pub fn soft_delete_observation(&self, id: SeeleId) -> Result<()> {
        self.observations.soft_delete(id)?;
        Ok(())
    }

    pub fn restore_observation(&self, id: SeeleId) -> Result<()> {
        self.observations.restore(id)?;
        Ok(())
    }

    // -------- Sessions --------

    pub fn start_session(&self, req: SessionStartRequest) -> Result<SessionDto> {
        let session = self.sessions.start(SessionInput {
            project: req.project,
            directory: req.directory,
        })?;
        Ok(SessionDto::from(session))
    }

    pub fn end_session(&self, id: SeeleId, req: SessionEndRequest) -> Result<()> {
        self.sessions.end(id, req.summary)?;
        Ok(())
    }

    pub fn abort_session(&self, id: SeeleId) -> Result<()> {
        self.sessions.abort(id)?;
        Ok(())
    }

    pub fn get_session(&self, id: SeeleId) -> Result<Option<SessionDto>> {
        Ok(self.sessions.get(id)?.map(SessionDto::from))
    }

    pub fn list_sessions(&self, q: SessionListQuery) -> Result<Vec<SessionDto>> {
        let status = q.status.as_deref().map(parse_session_status).transpose()?;
        let filter = SessionFilter {
            project: q.project,
            status,
            limit: q.limit,
        };
        let sessions = self.sessions.list(filter)?;
        Ok(sessions.into_iter().map(SessionDto::from).collect())
    }

    // -------- Links --------

    pub fn create_link(&self, req: LinkCreateRequest) -> Result<LinkDto> {
        let from_id = parse_id(&req.from_id, "from_id")?;
        let to_id = parse_id(&req.to_id, "to_id")?;
        if req.link_type.trim().is_empty() {
            return Err(ApiError::BadRequest("link_type must not be empty".into()));
        }
        let metadata = parse_metadata(req.metadata);
        let link = self.links.create(LinkInput {
            from_id,
            to_id,
            link_type: req.link_type,
            metadata,
        })?;
        Ok(LinkDto::from(link))
    }

    pub fn list_links_for_observation(&self, id: SeeleId) -> Result<Vec<LinkDto>> {
        // Return links where the observation is on either side.
        let from = self.links.list(LinkQuery {
            from_id: Some(id),
            ..Default::default()
        })?;
        let to = self.links.list(LinkQuery {
            to_id: Some(id),
            ..Default::default()
        })?;
        // Merge by id to dedup self-links.
        let mut by_id = std::collections::BTreeMap::new();
        for l in from.into_iter().chain(to) {
            by_id.insert(l.id.to_string(), l);
        }
        Ok(by_id.into_values().map(LinkDto::from).collect())
    }

    pub fn delete_link(&self, id: SeeleId) -> Result<()> {
        self.links.delete(id)?;
        Ok(())
    }
}

/// Anti-empty-query gate shared by HTTP `/search` and MCP `seele_search`.
/// Returns `Err(ApiError::BadRequest)` if the query text is empty AND no
/// filter is present. Mitigates the list-all-DB exfiltration vector flagged
/// by Cloven (2026-05-10).
pub fn enforce_search_query_or_filter(req: &SearchRequest) -> Result<()> {
    if req.query.trim().is_empty()
        && req.project.is_none()
        && req.scope.is_none()
        && req.r#type.is_none()
    {
        return Err(ApiError::BadRequest(
            "empty query requires at least one filter (project, scope, or type)".into(),
        ));
    }
    Ok(())
}

/// Bridge an `Arc<dyn Embedder>` into a `Box<dyn Embedder>` so the same
/// embedder instance is shared by both the SearchEngine (which currently
/// requires owned `Box<dyn Embedder>`) and direct service callers.
struct ArcEmbedder(Arc<dyn Embedder>);

impl Embedder for ArcEmbedder {
    fn embed(&self, text: &str) -> seele_embedder::Result<Vec<f32>> {
        self.0.embed(text)
    }
    fn embed_batch(&self, texts: &[&str]) -> seele_embedder::Result<Vec<Vec<f32>>> {
        self.0.embed_batch(texts)
    }
    fn dim(&self) -> usize {
        self.0.dim()
    }
    fn model_id(&self) -> &str {
        self.0.model_id()
    }
    fn expected_sha256(&self) -> Option<&str> {
        self.0.expected_sha256()
    }
}
