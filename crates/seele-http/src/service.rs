//! Service layer used by both HTTP handlers and MCP tools.
//!
//! Application logic lives here. Transport-specific code (axum handlers,
//! JSON-RPC dispatch) only deserializes the request, calls a method on
//! `SeeleService`, and serializes the response. This keeps logic DRY across
//! the two transports — when an operation needs to change, it changes in
//! exactly one place.

use std::sync::Arc;

use seele_embedder::Embedder;
use seele_search::SearchEngine;
use seele_storage::{
    ChunkStore, LinkStore, ObservationStore, Pool, PromptStore, RelationStore, SessionStore,
};

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
