//! Service layer used by both HTTP handlers and MCP tools.
//!
//! Application logic lives here. Transport-specific code (axum handlers,
//! JSON-RPC dispatch) only deserializes the request, calls a method on
//! `SeeleService`, and serializes the response. This keeps logic DRY across
//! the two transports — when an operation needs to change, it changes in
//! exactly one place.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use seele_core::id::SeeleId;
use seele_core::memory::Observation;
use seele_embedder::Embedder;
use seele_search::{SearchEngine, SearchQuery};
use seele_storage::{
    ChunkStore, LinkInput, LinkQuery, LinkStore, ObservationPatch, ObservationQuery,
    ObservationStore, Pool, RelationStore, SaveInput, SaveOutcome, SessionFilter, SessionInput,
    SessionStore,
};

use crate::dto::{
    parse_id, parse_judgment_status, parse_metadata, parse_relation_kind, parse_scope,
    parse_session_status, parse_type, CountBucket, EmbedderInfo, JudgeRequest, LinkCreateRequest,
    LinkDto, ListRequest, ObservationDto, ObservationStats, RelationCreateRequest, RelationDto,
    RelationListQuery, SaveRequest, SaveResponse, SearchHitDto, SearchRequest, SearchResponse,
    SessionDto, SessionEndRequest, SessionListQuery, SessionStartRequest, SessionStats,
    StatsResponse,
};
use crate::error::{ApiError, Result};
use seele_core::relation::{JudgmentStatus, RelationKind};
use seele_storage::{JudgmentInput, RelationInput, RelationQuery};

#[derive(Clone)]
pub struct SeeleService {
    pub observations: ObservationStore,
    pub sessions: SessionStore,
    pub links: LinkStore,
    pub relations: RelationStore,
    pub chunks: ChunkStore,
    pub search: Arc<SearchEngine>,
    pub embedder: Arc<dyn Embedder>,
    pub pool: Pool,
    /// Topic-key families (Q10), resolved ONCE at boot:
    /// env > project > user > builtin. Arc'd: the set is immutable for
    /// the process lifetime and the service is Clone.
    families: Arc<seele_core::families::FamilySet>,
    /// Append-only op-log path (T-12, GRAIL `_history.jsonl` port).
    /// `None` disables the log; the production bootstrap opts in via
    /// [`SeeleService::with_history_path`].
    history_path: Option<PathBuf>,
}

impl SeeleService {
    pub fn new(pool: Pool, embedder: Arc<dyn Embedder>) -> Self {
        let observations = ObservationStore::new(pool.clone());
        let sessions = SessionStore::new(pool.clone());
        let links = LinkStore::new(pool.clone());
        let relations = RelationStore::new(pool.clone());
        let chunks = ChunkStore::new(pool.clone());
        let search_embedder: Box<dyn Embedder> = Box::new(ArcEmbedder(embedder.clone()));
        let search = Arc::new(SearchEngine::new(pool.clone(), search_embedder));
        let (families, family_warnings) = seele_core::families::load_default();
        for w in family_warnings {
            tracing::warn!(warning = %w, "topic-families file ignored (falling through)");
        }
        Self {
            observations,
            sessions,
            links,
            relations,
            chunks,
            search,
            embedder,
            pool,
            families: Arc::new(families),
            history_path: None,
        }
    }

    /// The topic-key family set active for this process (Q10).
    pub fn topic_families(&self) -> &seele_core::families::FamilySet {
        &self.families
    }

    /// Canonical op-log path for a DB file (T-12): same directory as the
    /// DB, named `<db file name>.history.jsonl` — e.g. `seele.db` →
    /// `seele.db.history.jsonl`. One file per DB (not per project); each
    /// line carries its project slug, so per-project slices are a `grep`
    /// away and the log survives project renames.
    pub fn history_path_for_db(db_path: &Path) -> PathBuf {
        let mut name = db_path
            .file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_default();
        name.push(".history.jsonl");
        db_path.with_file_name(name)
    }

    /// Enable the op-log (T-12) at `path`. Builder-style so the single
    /// production call site stays one expression:
    /// `SeeleService::new(pool, emb).with_history_path(p)`.
    pub fn with_history_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.history_path = Some(path.into());
        self
    }

    /// Append one `{"op","ts","id","project"}` line to the op-log
    /// (T-12). Best-effort, like the post-save embedding write: a log
    /// failure warns but NEVER fails the mutation it describes — the DB
    /// is the source of truth, the log is for replay/debug/audit.
    fn log_op(&self, op: &'static str, id: SeeleId, project: Option<&str>) {
        let Some(path) = &self.history_path else {
            return;
        };
        let mut line = serde_json::json!({
            "op": op,
            "ts": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "id": id.to_string(),
            "project": project.unwrap_or(""),
        })
        .to_string();
        line.push('\n');
        let result = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
        if let Err(e) = result {
            tracing::warn!(error = %e, path = %path.display(), op, "history op-log append failed");
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
            project: req.project.clone(),
            scope,
            topic_key: req.topic_key,
            metadata,
        };
        let outcome = self.observations.save(input)?;
        // T-12: upserts and duplicate-merges count as "save" — one line
        // per call, logged only after the row is persisted.
        self.log_op("save", outcome.id(), req.project.as_deref());

        // Best-effort embedding write. Failure logs but does not surface
        // as a 5xx — the row is persisted and a reindex pass can fix it.
        // The same fresh vector feeds the near-duplicate scan (Q4): purely
        // informational, never changes the save outcome.
        let mut near_duplicates = Vec::new();
        match self.embedder.embed(&req.content) {
            Ok(v) => {
                let meta = seele_storage::EmbeddingMeta {
                    model_id: self.embedder.model_id().to_string(),
                    dim: v.len(),
                    contextualized: false,
                };
                if let Err(e) = self.observations.set_embedding(outcome.id(), &v, &meta) {
                    tracing::warn!(error=%e, id=%outcome.id(), "embedding write failed post-save");
                }
                match self.near_duplicates_for_vector(
                    &v,
                    req.project.as_deref(),
                    Some(scope),
                    Some(outcome.id()),
                ) {
                    Ok(nd) => near_duplicates = nd,
                    Err(e) => {
                        tracing::warn!(error=%e, id=%outcome.id(), "near-duplicate scan failed post-save");
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error=%e, id=%outcome.id(), "embedder failed post-save");
            }
        }

        let mut response = match outcome {
            SaveOutcome::Created(id) => SaveResponse {
                id: id.to_string(),
                outcome: "created",
                revision_count: None,
                duplicate_count: None,
                near_duplicates: Vec::new(),
            },
            SaveOutcome::UpsertedTopic { id, revision_count } => SaveResponse {
                id: id.to_string(),
                outcome: "upserted_topic",
                revision_count: Some(revision_count),
                duplicate_count: None,
                near_duplicates: Vec::new(),
            },
            SaveOutcome::DuplicateMerged {
                id,
                duplicate_count,
            } => SaveResponse {
                id: id.to_string(),
                outcome: "duplicate_merged",
                revision_count: None,
                duplicate_count: Some(duplicate_count),
                near_duplicates: Vec::new(),
            },
        };
        response.near_duplicates = near_duplicates;
        Ok(response)
    }

    /// KNN scan for near-duplicates of a fresh embedding (Q4). Distances
    /// are vec0's default **L2** over unit-norm vectors; the threshold
    /// 0.37 ≈ cosine 0.93 (`sqrt(2·(1−0.93))`) — the conservative end of
    /// GRAIL's alias-detection band, calibrated against false merges
    /// being worse than missed merges. Also consumed by
    /// `seele_suggest_topic_key` to propose the nearest neighbor's key.
    pub fn near_duplicates_for_vector(
        &self,
        embedding: &[f32],
        project: Option<&str>,
        scope: Option<seele_core::memory::Scope>,
        exclude: Option<SeeleId>,
    ) -> Result<Vec<crate::dto::NearDuplicateDto>> {
        const NEAR_DUP_K: u32 = 3;
        let pairs = self
            .search
            .knn_by_vector(embedding, project, scope, exclude, NEAR_DUP_K)?;
        let mut out = Vec::new();
        for (id, distance) in pairs {
            if distance > NEAR_DUP_MAX_L2 {
                continue;
            }
            if let Some(obs) = self.observations.get(id)? {
                out.push(crate::dto::NearDuplicateDto {
                    id: id.to_string(),
                    title: obs.title,
                    topic_key: obs.topic_key,
                    distance,
                });
            }
        }
        Ok(out)
    }

    /// Deterministic similarity scan for an existing observation (Q6,
    /// GRAIL `find_similar_entity` lesson): two signals in parallel —
    /// Jaro-Winkler on titles (same project + same type, the conservative
    /// posture against false merges) and cosine over **stored** embeddings
    /// (no re-embed). Candidates are deduped keeping each id's strongest
    /// signal, ranked by score, truncated to `top_k`. Zero LLM.
    pub fn find_similar(
        &self,
        id: SeeleId,
        top_k: usize,
    ) -> Result<Vec<crate::dto::SimilarCandidateDto>> {
        use seele_core::similarity::{cosine_from_unit_l2, SIGNAL_COS_MIN, SIGNAL_JW_MIN};

        let base = self
            .observations
            .get(id)?
            .ok_or_else(|| ApiError::NotFound(format!("observation {id}")))?;
        let mut best: std::collections::HashMap<SeeleId, (f64, &'static str)> =
            std::collections::HashMap::new();
        let mut keep_max = |bid: SeeleId, score: f64, signal: &'static str| {
            best.entry(bid)
                .and_modify(|e| {
                    if score > e.0 {
                        *e = (score, signal);
                    }
                })
                .or_insert((score, signal));
        };

        // Vector signal — reuse the vector already paid for at save time.
        if let Some(vector) = self.observations.get_embedding(id)? {
            let pairs = self.search.knn_by_vector(
                &vector,
                base.project.as_deref(),
                None,
                Some(id),
                (top_k.max(5) * 2) as u32,
            )?;
            for (nid, dist) in pairs {
                let cos = cosine_from_unit_l2(dist);
                if cos >= SIGNAL_COS_MIN {
                    keep_max(nid, cos, "vector");
                }
            }
        }

        // Title signal — JW within same project AND same type.
        let candidates = self.observations.list(seele_storage::ObservationQuery {
            project: base.project.clone(),
            kind: Some(base.kind.clone()),
            limit: Some(500),
            ..Default::default()
        })?;
        let base_title = base.title.to_lowercase();
        for obs in candidates {
            if obs.id == id {
                continue;
            }
            let other = obs.title.to_lowercase();
            if other == base_title {
                keep_max(obs.id, 1.0, "exact");
            } else {
                let jw = strsim::jaro_winkler(&base_title, &other);
                if jw >= SIGNAL_JW_MIN {
                    keep_max(obs.id, jw, "title");
                }
            }
        }

        let mut ranked: Vec<(SeeleId, f64, &'static str)> =
            best.into_iter().map(|(k, (s, sig))| (k, s, sig)).collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked.truncate(top_k);

        let mut out = Vec::with_capacity(ranked.len());
        for (cid, score, signal) in ranked {
            if let Some(obs) = self.observations.get(cid)? {
                out.push(crate::dto::SimilarCandidateDto {
                    id: cid.to_string(),
                    title: obs.title,
                    topic_key: obs.topic_key,
                    score,
                    signal,
                });
            }
        }
        Ok(out)
    }

    /// Strongest similarity between two specific observations (Q6 confirm
    /// path): max of title JW and embedding cosine. `None` when neither
    /// signal is computable (e.g. both rows lack vectors and titles are
    /// empty — practically never).
    pub fn similarity_between(&self, a: SeeleId, b: SeeleId) -> Result<Option<f64>> {
        use seele_core::similarity::cosine_from_unit_l2;
        let oa = self
            .observations
            .get(a)?
            .ok_or_else(|| ApiError::NotFound(format!("observation {a}")))?;
        let ob = self
            .observations
            .get(b)?
            .ok_or_else(|| ApiError::NotFound(format!("observation {b}")))?;
        let jw = strsim::jaro_winkler(&oa.title.to_lowercase(), &ob.title.to_lowercase());
        let mut score = jw;
        if let (Some(va), Some(vb)) = (
            self.observations.get_embedding(a)?,
            self.observations.get_embedding(b)?,
        ) {
            if va.len() == vb.len() {
                let l2 = va
                    .iter()
                    .zip(&vb)
                    .map(|(x, y)| ((*x - *y) as f64).powi(2))
                    .sum::<f64>()
                    .sqrt();
                score = score.max(cosine_from_unit_l2(l2));
            }
        }
        Ok(Some(score.clamp(0.0, 1.0)))
    }

    /// Run the search engine. Caller is responsible for the anti-empty-query
    /// gate at the transport layer (HTTP handler / MCP tool both apply it).
    pub fn search_observations(&self, req: SearchRequest) -> Result<SearchResponse> {
        let q = build_search_query(req)?;
        let hits = self.search.search(q)?;
        Ok(to_search_response(&hits))
    }

    /// `search_observations` + the engine's execution trace (Q8,
    /// `seele search --explain`). CLI-only surface for now: HTTP/MCP keep
    /// the plain variant until the trace shape settles (struct is
    /// versioned for that reason).
    pub fn search_observations_explain(
        &self,
        req: SearchRequest,
    ) -> Result<(SearchResponse, seele_search::SearchTrace)> {
        let q = build_search_query(req)?;
        let (hits, trace) = self.search.search_traced(q)?;
        Ok((to_search_response(&hits), trace))
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
        // T-12: resolve the project for the log line before mutating
        // (get() does not filter soft-deleted rows). Best-effort: a
        // read failure here must not block the delete.
        let project = self.history_project_of(id);
        self.observations.soft_delete(id)?;
        self.log_op("delete", id, project.as_deref());
        Ok(())
    }

    pub fn restore_observation(&self, id: SeeleId) -> Result<()> {
        let project = self.history_project_of(id);
        self.observations.restore(id)?;
        self.log_op("restore", id, project.as_deref());
        Ok(())
    }

    /// Project slug of `id` for an op-log line (T-12); `None` when the
    /// row is gone or unreadable — the log degrades, the mutation
    /// proceeds.
    fn history_project_of(&self, id: SeeleId) -> Option<String> {
        self.observations
            .get(id)
            .ok()
            .flatten()
            .and_then(|o| o.project)
    }

    /// Merge `patch_metadata` into the observation's existing metadata.
    /// New keys overwrite existing; nested objects are NOT recursively
    /// merged (caller passes a flat patch).
    pub fn merge_observation_metadata(
        &self,
        id: SeeleId,
        patch_metadata: serde_json::Value,
    ) -> Result<()> {
        let current = self
            .observations
            .get(id)?
            .ok_or_else(|| ApiError::NotFound(format!("observation {id}")))?;
        let mut merged = current.metadata.0.clone();
        if let Some(obj) = patch_metadata.as_object() {
            let target = merged
                .as_object_mut()
                .ok_or_else(|| ApiError::Internal("metadata is not an object".into()))?;
            for (k, v) in obj {
                target.insert(k.clone(), v.clone());
            }
        }
        self.observations.update(
            id,
            ObservationPatch {
                metadata: Some(seele_core::metadata::Metadata::from_value(merged)),
                ..Default::default()
            },
        )?;
        self.log_op("update", id, current.project.as_deref());
        Ok(())
    }

    /// Distinct project names across active observations.
    pub fn list_projects(&self) -> Result<Vec<String>> {
        Ok(self.observations.list_projects()?)
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

    // -------- Relations --------

    pub fn create_relation(&self, req: RelationCreateRequest) -> Result<RelationDto> {
        if req.sync_id.trim().is_empty() {
            return Err(ApiError::BadRequest("sync_id must not be empty".into()));
        }
        let source_id = parse_id(&req.source_id, "source_id")?;
        let target_id = parse_id(&req.target_id, "target_id")?;
        if source_id == target_id {
            return Err(ApiError::BadRequest(
                "source_id and target_id must differ".into(),
            ));
        }
        let relation: RelationKind = parse_relation_kind(&req.relation)?;
        let session_id = req
            .session_id
            .as_deref()
            .map(|s| parse_id(s, "session_id"))
            .transpose()?;
        let rel = self.relations.create(RelationInput {
            sync_id: req.sync_id,
            source_id,
            target_id,
            relation,
            reason: req.reason,
            evidence: req.evidence,
            confidence: req.confidence,
            marked_by_actor: req.marked_by_actor,
            marked_by_kind: req.marked_by_kind,
            marked_by_model: req.marked_by_model,
            session_id,
        })?;
        Ok(RelationDto::from(rel))
    }

    pub fn list_relations(&self, q: RelationListQuery) -> Result<Vec<RelationDto>> {
        let source_id = q
            .source_id
            .as_deref()
            .map(|s| parse_id(s, "source_id"))
            .transpose()?;
        let target_id = q
            .target_id
            .as_deref()
            .map(|s| parse_id(s, "target_id"))
            .transpose()?;
        let relation = q.relation.as_deref().map(parse_relation_kind).transpose()?;
        let status = q.status.as_deref().map(parse_judgment_status).transpose()?;
        let query = RelationQuery {
            source_id,
            target_id,
            relation,
            status,
            limit: q.limit,
        };
        let rels = self.relations.list(query)?;
        Ok(rels.into_iter().map(RelationDto::from).collect())
    }

    pub fn judge_relation(&self, id: SeeleId, req: JudgeRequest) -> Result<()> {
        let status: JudgmentStatus = parse_judgment_status(&req.status)?;
        self.relations.judge(
            id,
            JudgmentInput {
                status,
                reason: req.reason,
                evidence: req.evidence,
                confidence: req.confidence,
            },
        )?;
        Ok(())
    }

    /// List conflicts still awaiting judgment. Convenience over
    /// `list_relations` with `relation=conflicts_with&status=pending`.
    pub fn list_pending_conflicts(&self, limit: Option<u32>) -> Result<Vec<RelationDto>> {
        let rels = self.relations.list(RelationQuery {
            relation: Some(RelationKind::ConflictsWith),
            status: Some(JudgmentStatus::Pending),
            limit,
            ..Default::default()
        })?;
        Ok(rels.into_iter().map(RelationDto::from).collect())
    }

    // -------- Stats + embedder --------

    pub fn stats(&self) -> Result<StatsResponse> {
        let obs = ObservationStats {
            active: self.observations.count_active()?,
            deleted: self.observations.count_deleted()?,
            projects: self.observations.count_projects()?,
            by_type: self
                .observations
                .count_by_type()?
                .into_iter()
                .map(CountBucket::from)
                .collect(),
            by_scope: self
                .observations
                .count_by_scope()?
                .into_iter()
                .map(CountBucket::from)
                .collect(),
        };
        let ses = SessionStats {
            total: self.sessions.count_total()?,
            by_status: self
                .sessions
                .count_by_status()?
                .into_iter()
                .map(CountBucket::from)
                .collect(),
        };
        Ok(StatsResponse {
            observations: obs,
            sessions: ses,
        })
    }

    pub fn embedder_info(&self) -> EmbedderInfo {
        EmbedderInfo {
            model_id: self.embedder.model_id().to_string(),
            dim: self.embedder.dim(),
            expected_sha256: self.embedder.expected_sha256().map(str::to_string),
        }
    }

    /// Embedding provenance snapshot (ADR-14): which (model_id, dim) combos
    /// live in `embeddings_meta`, and how many active observations have no
    /// vector. Consumed by both doctor surfaces (CLI + MCP).
    pub fn embedding_provenance(&self) -> Result<seele_storage::EmbeddingProvenance> {
        Ok(self.observations.embedding_provenance()?)
    }

    /// Re-embed every active observation whose vector is missing or whose
    /// provenance names a different `(model_id, dim)` than the active
    /// embedder (T-08). This is the remediation half of doctor's
    /// `mix_warning`: that detects, this fixes.
    ///
    /// Unlike the save path's post-commit best-effort write, every vector
    /// lands via `ObservationStore::set_embedding`, which commits vector +
    /// provenance in ONE transaction — the hole this command exists to
    /// close. Idempotent: a second run finds no candidates. Rows the
    /// embedder fails on are skipped (counted, not fatal) so one bad text
    /// doesn't block the rest; a re-run picks them up again. With
    /// `dry_run` the pass only counts. `project` scopes it to one slug;
    /// `batch_size` rows go to the embedder per call.
    pub fn reembed_all(
        &self,
        project: Option<&str>,
        batch_size: usize,
        dry_run: bool,
    ) -> Result<ReembedReport> {
        let model_id = self.embedder.model_id().to_string();
        let dim = self.embedder.dim();
        let candidates = self
            .observations
            .reembed_candidates(&model_id, dim, project)?;
        let mut report = ReembedReport {
            model_id: model_id.clone(),
            dim,
            candidates: candidates.len() as u64,
            reembedded: 0,
            skipped: 0,
            dry_run,
        };
        if dry_run || candidates.is_empty() {
            return Ok(report);
        }
        let meta = seele_storage::EmbeddingMeta {
            model_id,
            dim,
            contextualized: false,
        };
        let batch_size = batch_size.max(1);
        for chunk in candidates.chunks(batch_size) {
            let texts: Vec<&str> = chunk.iter().map(|o| o.content.as_str()).collect();
            // One bad text must not skip the whole batch: when the batch
            // call fails (or violates the len contract) fall back to
            // per-row embedding for this chunk.
            let batch: Option<Vec<Vec<f32>>> = match self.embedder.embed_batch(&texts) {
                Ok(vs) if vs.len() == chunk.len() => Some(vs),
                Ok(_) => {
                    tracing::warn!("embed_batch returned wrong vector count; per-row fallback");
                    None
                }
                Err(e) => {
                    tracing::warn!(error = %e, "embed_batch failed; per-row fallback");
                    None
                }
            };
            for (i, obs) in chunk.iter().enumerate() {
                let vector = match &batch {
                    Some(vs) => Ok(vs[i].clone()),
                    None => self.embedder.embed(&obs.content),
                };
                let vector = match vector {
                    Ok(v) => v,
                    Err(e) => {
                        report.skipped += 1;
                        tracing::warn!(id = %obs.id, error = %e, "re-embed failed; row skipped");
                        continue;
                    }
                };
                match self.observations.set_embedding(obs.id, &vector, &meta) {
                    Ok(()) => report.reembedded += 1,
                    Err(e) => {
                        report.skipped += 1;
                        tracing::warn!(id = %obs.id, error = %e, "re-embed write failed; row skipped");
                    }
                }
            }
            tracing::info!(
                done = report.reembedded + report.skipped,
                total = report.candidates,
                "reembed progress"
            );
        }
        Ok(report)
    }
}

/// Outcome of a re-embed pass (T-08, `seele embedder reembed-all`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReembedReport {
    /// Active embedder the vectors were (re)computed with.
    pub model_id: String,
    /// Vector dimensionality of the active embedder.
    pub dim: usize,
    /// Active observations whose vector was missing or whose provenance
    /// didn't match the active embedder when the pass started.
    pub candidates: u64,
    /// Rows re-embedded and persisted (vector + provenance in one tx).
    pub reembedded: u64,
    /// Candidate rows the embedder (or the write) failed on — left
    /// untouched, so the next run picks them up again.
    pub skipped: u64,
    /// True when the pass only counted candidates (no writes).
    pub dry_run: bool,
}

/// Q4: max L2 distance for a stored vector to count as near-duplicate of
/// a fresh save. vec0's default metric is L2; on unit-normalized
/// embeddings `l2 = sqrt(2·(1−cos))`, so 0.37 ≈ cosine 0.93.
pub const NEAR_DUP_MAX_L2: f64 = 0.37;

/// Map a wire-level [`SearchRequest`] onto the engine's [`SearchQuery`].
/// Shared by the plain and `--explain` search paths.
fn build_search_query(req: SearchRequest) -> Result<SearchQuery> {
    let scope = match req.scope.as_deref() {
        None => None,
        Some(s) => Some(parse_scope(Some(s))?),
    };
    Ok(SearchQuery {
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
    })
}

fn to_search_response(hits: &[seele_search::SearchHit]) -> SearchResponse {
    let dtos: Vec<SearchHitDto> = hits.iter().map(SearchHitDto::from).collect();
    let count = dtos.len();
    SearchResponse { hits: dtos, count }
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
