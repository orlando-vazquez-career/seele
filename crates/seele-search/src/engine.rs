//! Hybrid search engine: FTS5 + vec0 combined via Reciprocal Rank Fusion.

use std::collections::HashMap;

use rusqlite::Row;
use seele_core::id::SeeleId;
use seele_core::memory::{Observation, Scope};
use seele_embedder::Embedder;
use seele_storage::Pool;

use crate::error::{Result, SearchError};
use crate::rrf::{self, RrfHit, DEFAULT_K};

const DEFAULT_PER_METHOD_LIMIT: u32 = 50;
const DEFAULT_FINAL_LIMIT: u32 = 10;

#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    pub text: String,
    pub project: Option<String>,
    pub scope: Option<Scope>,
    pub kind: Option<String>,
    /// How many candidates to pull from FTS and from vec0 before RRF.
    /// Default: 50 each.
    pub per_method_limit: Option<u32>,
    /// How many results to return after RRF.
    /// Default: 10.
    pub limit: Option<u32>,
    /// Include `metadata.context_mode = 'purist'` rows? Default false to
    /// match Counsel pattern's "purists do not see prior counsels" rule.
    pub include_purist: bool,
    /// Multiplier applied per unit of `metadata.score` (virtual column
    /// `meta_score`). The post-RRF score becomes
    /// `rrf_score * (1.0 + score_boost_multiplier * meta_score.unwrap_or(1.0))`
    /// (ADR-03 §"Capa 5"). Default `0.0` disables boost — keeps Sprint-01
    /// behavior.
    pub score_boost_multiplier: f64,
    /// Drop vec hits whose cosine distance exceeds this threshold. Useful
    /// when vec0 returns top-K regardless of similarity. `None` keeps all.
    pub max_vec_distance: Option<f64>,
    /// Attach `memory_relations` annotations to each hit. Costs one extra
    /// query against `memory_relations`. Default `false` to preserve the
    /// sub-300ms target on large queries; opt in per call.
    pub include_annotations: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationKind {
    /// `self` supersedes `other_id`.
    Supersedes,
    /// `other_id` supersedes `self`.
    SupersededBy,
    /// Peer-level conflict, judgment still `pending`.
    ConflictsWith,
    /// `self` was on the losing side of a judged conflict.
    ContestedBy,
}

#[derive(Debug, Clone)]
pub struct RelationAnnotation {
    pub kind: AnnotationKind,
    pub other_id: SeeleId,
    pub other_title: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub observation: Observation,
    pub score: f64,
    pub fts_rank: Option<usize>,
    pub vec_rank: Option<usize>,
    /// Rank in the bag-of-words FTS rescue path (Q3). `Some` means the
    /// loose OR-of-tokens query found this hit; the strict phrase path
    /// may have missed it entirely (the paraphrase failure mode).
    pub fts_loose_rank: Option<usize>,
    pub annotations: Vec<RelationAnnotation>,
}

/// Auditable snapshot of one hybrid-search execution (Q8, GRAIL
/// QueryTracer lesson, deterministic flavor). Carries counts, effective
/// parameters and vec distances — never candidate contents (privacy).
/// `version` guards downstream parsers against shape changes.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchTrace {
    pub version: u32,
    pub query_text: String,
    /// What the strict path actually sent to FTS5 MATCH — makes the
    /// phrase-quoting visible (`"entire query as one phrase"`).
    pub fts_match: String,
    /// OR-of-tokens form, `None` when the query has < 2 tokens (the loose
    /// path is skipped as identical to the strict one).
    pub fts_loose_match: Option<String>,
    pub fts_candidates: usize,
    pub fts_loose_candidates: Option<usize>,
    pub vec_candidates: usize,
    /// Distances of the vec candidates, in rank order. Previously
    /// SELECTed and discarded (drift vs ADR-03) — now surfaced.
    pub vec_distances: Vec<f64>,
    pub rrf_k: usize,
    pub per_method_limit: u32,
    pub final_limit: usize,
    pub score_boost_multiplier: f64,
    pub max_vec_distance: Option<f64>,
    pub embedder_model_id: String,
    pub embedder_dim: usize,
}

pub struct SearchEngine {
    pool: Pool,
    embedder: Box<dyn Embedder>,
    rrf_k: usize,
}

impl SearchEngine {
    pub fn new(pool: Pool, embedder: Box<dyn Embedder>) -> Self {
        Self {
            pool,
            embedder,
            rrf_k: DEFAULT_K,
        }
    }

    pub fn with_rrf_k(mut self, k: usize) -> Self {
        self.rrf_k = k;
        self
    }

    pub fn search(&self, query: SearchQuery) -> Result<Vec<SearchHit>> {
        Ok(self.search_traced(query)?.0)
    }

    /// Like [`search`](Self::search), but also returns the execution trace
    /// (`seele search --explain`). Same work either way — the trace is
    /// assembled from values the pipeline already computes.
    pub fn search_traced(&self, query: SearchQuery) -> Result<(Vec<SearchHit>, SearchTrace)> {
        let per_method = query.per_method_limit.unwrap_or(DEFAULT_PER_METHOD_LIMIT);
        let final_limit = query.limit.unwrap_or(DEFAULT_FINAL_LIMIT) as usize;

        if query.text.trim().is_empty() {
            let hits = self.list_by_filters(&query)?;
            let trace = SearchTrace {
                version: 1,
                query_text: query.text.clone(),
                fts_match: String::new(),
                fts_loose_match: None,
                fts_candidates: 0,
                fts_loose_candidates: None,
                vec_candidates: 0,
                vec_distances: Vec::new(),
                rrf_k: self.rrf_k,
                per_method_limit: per_method,
                final_limit,
                score_boost_multiplier: query.score_boost_multiplier,
                max_vec_distance: query.max_vec_distance,
                embedder_model_id: self.embedder.model_id().to_string(),
                embedder_dim: self.embedder.dim(),
            };
            return Ok((hits, trace));
        }

        let fts_match = escape_fts(&query.text);
        let fts_loose_match = loose_fts_match(&query.text);

        let fts_rank = self.fts_query_with(&fts_match, &query, per_method)?;
        let vec_pairs = self.vec_query(&query, per_method)?;
        let vec_rank: Vec<SeeleId> = vec_pairs.iter().map(|(id, _)| *id).collect();
        let fts_loose_rank = match &fts_loose_match {
            Some(m) => Some(self.fts_query_with(m, &query, per_method)?),
            None => None,
        };

        // Third RRF path (Q3): the strict phrase query misses natural-
        // language paraphrases entirely (0 candidates); the OR-of-tokens
        // rescue feeds RRF a textual signal in exactly those cases.
        // `rrf::combine` is generic over N sources, so the extra path is
        // one more entry, not a redesign.
        let mut sources: Vec<(&'static str, Vec<SeeleId>)> =
            vec![("fts", fts_rank.clone()), ("vec", vec_rank.clone())];
        if let Some(loose) = &fts_loose_rank {
            sources.push(("fts_loose", loose.clone()));
        }
        let combined = rrf::combine(&sources, self.rrf_k);

        // Apply meta_score boost if enabled, then re-sort and truncate.
        let boosted = self.apply_score_boost(combined, query.score_boost_multiplier)?;
        let top: Vec<RrfHit<SeeleId>> = boosted.into_iter().take(final_limit).collect();

        let observations = self.hydrate(&top.iter().map(|h| h.id).collect::<Vec<_>>())?;
        let by_id: HashMap<SeeleId, Observation> =
            observations.into_iter().map(|o| (o.id, o)).collect();

        let mut hits: Vec<SearchHit> = top
            .into_iter()
            .filter_map(|hit| {
                let obs = by_id.get(&hit.id).cloned()?;
                let (fts_r, vec_r, loose_r) = unpack_per_source(&hit);
                Some(SearchHit {
                    observation: obs,
                    score: hit.score,
                    fts_rank: fts_r,
                    vec_rank: vec_r,
                    fts_loose_rank: loose_r,
                    annotations: Vec::new(),
                })
            })
            .collect();

        if query.include_annotations && !hits.is_empty() {
            self.attach_annotations(&mut hits)?;
        }

        let trace = SearchTrace {
            version: 1,
            query_text: query.text.clone(),
            fts_match,
            fts_candidates: fts_rank.len(),
            fts_loose_candidates: fts_loose_rank.as_ref().map(Vec::len),
            fts_loose_match,
            vec_candidates: vec_pairs.len(),
            vec_distances: vec_pairs.iter().map(|(_, d)| *d).collect(),
            rrf_k: self.rrf_k,
            per_method_limit: per_method,
            final_limit,
            score_boost_multiplier: query.score_boost_multiplier,
            max_vec_distance: query.max_vec_distance,
            embedder_model_id: self.embedder.model_id().to_string(),
            embedder_dim: self.embedder.dim(),
        };
        Ok((hits, trace))
    }

    /// Empty-query path: list observations matching the filters, ordered by
    /// `created_at DESC`. Skips FTS + vec entirely (no embedder call).
    fn list_by_filters(&self, query: &SearchQuery) -> Result<Vec<SearchHit>> {
        let limit = query.limit.unwrap_or(DEFAULT_FINAL_LIMIT) as i64;
        let mut sql = String::from(
            "SELECT id, int_id, session_id, type, title, content, tool_name, project, \
                    scope, topic_key, normalized_hash, revision_count, duplicate_count, \
                    last_seen_at, created_at, updated_at, deleted_at, metadata \
             FROM observations WHERE deleted_at IS NULL",
        );
        let mut bound: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(p) = &query.project {
            sql.push_str(" AND project = ?");
            bound.push(Box::new(p.clone()));
        }
        if let Some(s) = query.scope {
            sql.push_str(" AND scope = ?");
            bound.push(Box::new(s.as_str().to_string()));
        }
        if let Some(k) = &query.kind {
            sql.push_str(" AND type = ?");
            bound.push(Box::new(k.clone()));
        }
        if !query.include_purist {
            sql.push_str(" AND (meta_context_mode IS NULL OR meta_context_mode != 'purist')");
        }
        sql.push_str(" ORDER BY created_at DESC LIMIT ?");
        bound.push(Box::new(limit));

        let conn = self.pool.get().map_err(seele_storage::StorageError::Pool)?;
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(rusqlite::params_from_iter(bound.iter().map(|b| b.as_ref())))?;
        let mut hits = Vec::new();
        while let Some(row) = rows.next()? {
            let observation = parse_observation(row)?;
            hits.push(SearchHit {
                observation,
                score: 0.0,
                fts_rank: None,
                vec_rank: None,
                fts_loose_rank: None,
                annotations: Vec::new(),
            });
        }

        if query.include_annotations && !hits.is_empty() {
            self.attach_annotations(&mut hits)?;
        }
        Ok(hits)
    }

    /// One FTS5 pass with an explicit MATCH expression. Shared by the
    /// strict phrase path (`escape_fts`) and the loose bag-of-words path
    /// (`loose_fts_match`); both apply identical row filters.
    fn fts_query_with(
        &self,
        match_expr: &str,
        query: &SearchQuery,
        limit: u32,
    ) -> Result<Vec<SeeleId>> {
        let mut sql = String::from(
            "SELECT o.id FROM observations_fts fts \
             JOIN observations o ON o.int_id = fts.rowid \
             WHERE observations_fts MATCH ?1 \
                AND o.deleted_at IS NULL",
        );
        let mut bound: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(match_expr.to_string())];
        if let Some(p) = &query.project {
            sql.push_str(" AND o.project = ?");
            bound.push(Box::new(p.clone()));
        }
        if let Some(s) = query.scope {
            sql.push_str(" AND o.scope = ?");
            bound.push(Box::new(s.as_str().to_string()));
        }
        if let Some(k) = &query.kind {
            sql.push_str(" AND o.type = ?");
            bound.push(Box::new(k.clone()));
        }
        if !query.include_purist {
            sql.push_str(" AND (o.meta_context_mode IS NULL OR o.meta_context_mode != 'purist')");
        }
        sql.push_str(" ORDER BY rank LIMIT ?");
        bound.push(Box::new(limit as i64));

        let conn = self.pool.get().map_err(seele_storage::StorageError::Pool)?;
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(rusqlite::params_from_iter(bound.iter().map(|b| b.as_ref())))?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let id_str: String = row.get(0)?;
            out.push(
                id_str
                    .parse::<SeeleId>()
                    .map_err(|e| SearchError::InvalidInput(format!("bad ULID '{id_str}': {e}")))?,
            );
        }
        Ok(out)
    }

    /// KNN over vec0. Returns `(id, distance)` pairs in rank order — the
    /// distance feeds the trace (Q8) and future consumers (near-dup
    /// detection); it was previously SELECTed and discarded.
    fn vec_query(&self, query: &SearchQuery, limit: u32) -> Result<Vec<(SeeleId, f64)>> {
        let embedding = self.embedder.embed(&query.text)?;
        if embedding.len() != self.embedder.dim() {
            return Err(SearchError::DimensionMismatch {
                query: embedding.len(),
                db: self.embedder.dim(),
            });
        }
        let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        let mut sql = String::from(
            "SELECT o.id, vec.distance FROM observations_vec vec \
             JOIN observations o ON o.int_id = vec.rowid \
             WHERE vec.embedding MATCH ?1 \
                AND vec.k = ?2 \
                AND o.deleted_at IS NULL",
        );
        let mut bound: Vec<Box<dyn rusqlite::ToSql>> =
            vec![Box::new(bytes), Box::new(limit as i64)];
        if let Some(p) = &query.project {
            sql.push_str(" AND o.project = ?");
            bound.push(Box::new(p.clone()));
        }
        if let Some(s) = query.scope {
            sql.push_str(" AND o.scope = ?");
            bound.push(Box::new(s.as_str().to_string()));
        }
        if let Some(k) = &query.kind {
            sql.push_str(" AND o.type = ?");
            bound.push(Box::new(k.clone()));
        }
        if !query.include_purist {
            sql.push_str(" AND (o.meta_context_mode IS NULL OR o.meta_context_mode != 'purist')");
        }
        if let Some(maxd) = query.max_vec_distance {
            sql.push_str(" AND vec.distance <= ?");
            bound.push(Box::new(maxd));
        }
        sql.push_str(" ORDER BY vec.distance");

        let conn = self.pool.get().map_err(seele_storage::StorageError::Pool)?;
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(rusqlite::params_from_iter(bound.iter().map(|b| b.as_ref())))?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let id_str: String = row.get(0)?;
            let distance: f64 = row.get(1)?;
            let id = id_str
                .parse::<SeeleId>()
                .map_err(|e| SearchError::InvalidInput(format!("bad ULID '{id_str}': {e}")))?;
            out.push((id, distance));
        }
        Ok(out)
    }

    /// Multiply each RRF score by `(1 + multiplier * meta_score)` and re-sort.
    /// No-op when multiplier is 0.0 (default). `meta_score` is read from the
    /// virtual column on `observations`; null is treated as 1.0 per ADR-03.
    fn apply_score_boost(
        &self,
        hits: Vec<RrfHit<SeeleId>>,
        multiplier: f64,
    ) -> Result<Vec<RrfHit<SeeleId>>> {
        if multiplier == 0.0 || hits.is_empty() {
            return Ok(hits);
        }
        let ids: Vec<SeeleId> = hits.iter().map(|h| h.id).collect();
        let scores = self.fetch_meta_scores(&ids)?;
        let mut boosted: Vec<RrfHit<SeeleId>> = hits
            .into_iter()
            .map(|mut hit| {
                let ms = scores.get(&hit.id).copied().unwrap_or(1.0);
                hit.score *= 1.0 + multiplier * ms;
                hit
            })
            .collect();
        boosted.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(boosted)
    }

    fn fetch_meta_scores(&self, ids: &[SeeleId]) -> Result<HashMap<SeeleId, f64>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("SELECT id, meta_score FROM observations WHERE id IN ({placeholders})");
        let bound: Vec<Box<dyn rusqlite::ToSql>> =
            ids.iter().map(|id| Box::new(id.to_string()) as _).collect();
        let conn = self.pool.get().map_err(seele_storage::StorageError::Pool)?;
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(rusqlite::params_from_iter(bound.iter().map(|b| b.as_ref())))?;
        let mut out = HashMap::new();
        while let Some(row) = rows.next()? {
            let id_str: String = row.get(0)?;
            let score: Option<f64> = row.get(1)?;
            if let Some(s) = score {
                let parsed: SeeleId = id_str
                    .parse()
                    .map_err(|e| SearchError::InvalidInput(format!("bad ULID '{id_str}': {e}")))?;
                out.insert(parsed, s);
            }
        }
        Ok(out)
    }

    fn attach_annotations(&self, hits: &mut [SearchHit]) -> Result<()> {
        let ids: Vec<SeeleId> = hits.iter().map(|h| h.observation.id).collect();
        let annotations = self.fetch_annotations(&ids)?;
        for hit in hits {
            if let Some(list) = annotations.get(&hit.observation.id) {
                hit.annotations = list.clone();
            }
        }
        Ok(())
    }

    fn fetch_annotations(
        &self,
        ids: &[SeeleId],
    ) -> Result<HashMap<SeeleId, Vec<RelationAnnotation>>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let id_in = format!("({placeholders})");
        // Fetch relations where either side is in the hit set, plus the
        // titles of the *other* end so the caller can render
        // "supersedes: <title>" without an extra round-trip.
        let sql = format!(
            "SELECT r.source_id, r.target_id, r.relation, r.judgment_status, r.reason, \
                    src.title, tgt.title \
             FROM memory_relations r \
             JOIN observations src ON src.id = r.source_id \
             JOIN observations tgt ON tgt.id = r.target_id \
             WHERE r.source_id IN {id_in} OR r.target_id IN {id_in}"
        );
        // Bind ids twice — once for source_id IN, once for target_id IN.
        let mut bound: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(ids.len() * 2);
        for id in ids {
            bound.push(Box::new(id.to_string()));
        }
        for id in ids {
            bound.push(Box::new(id.to_string()));
        }

        let conn = self.pool.get().map_err(seele_storage::StorageError::Pool)?;
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(rusqlite::params_from_iter(bound.iter().map(|b| b.as_ref())))?;
        let hit_set: std::collections::HashSet<SeeleId> = ids.iter().copied().collect();
        let mut out: HashMap<SeeleId, Vec<RelationAnnotation>> = HashMap::new();
        while let Some(row) = rows.next()? {
            let source_str: String = row.get(0)?;
            let target_str: String = row.get(1)?;
            let relation: String = row.get(2)?;
            let status: String = row.get(3)?;
            let reason: Option<String> = row.get(4)?;
            let source_title: String = row.get(5)?;
            let target_title: String = row.get(6)?;

            let source_id: SeeleId = source_str.parse().map_err(|e| {
                SearchError::InvalidInput(format!("bad source ULID '{source_str}': {e}"))
            })?;
            let target_id: SeeleId = target_str.parse().map_err(|e| {
                SearchError::InvalidInput(format!("bad target ULID '{target_str}': {e}"))
            })?;

            // Attach annotations to whichever endpoints are in the hit set.
            if hit_set.contains(&source_id) {
                if let Some(kind) = annotation_for_source(&relation, &status) {
                    out.entry(source_id).or_default().push(RelationAnnotation {
                        kind,
                        other_id: target_id,
                        other_title: Some(target_title.clone()),
                        reason: reason.clone(),
                    });
                }
            }
            if hit_set.contains(&target_id) {
                if let Some(kind) = annotation_for_target(&relation, &status) {
                    out.entry(target_id).or_default().push(RelationAnnotation {
                        kind,
                        other_id: source_id,
                        other_title: Some(source_title),
                        reason,
                    });
                }
            }
        }
        Ok(out)
    }

    fn hydrate(&self, ids: &[SeeleId]) -> Result<Vec<Observation>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id, int_id, session_id, type, title, content, tool_name, project, \
                    scope, topic_key, normalized_hash, revision_count, duplicate_count, \
                    last_seen_at, created_at, updated_at, deleted_at, metadata \
             FROM observations WHERE id IN ({placeholders})"
        );
        let bound: Vec<Box<dyn rusqlite::ToSql>> =
            ids.iter().map(|id| Box::new(id.to_string()) as _).collect();

        let conn = self.pool.get().map_err(seele_storage::StorageError::Pool)?;
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(rusqlite::params_from_iter(bound.iter().map(|b| b.as_ref())))?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(parse_observation(row)?);
        }
        Ok(out)
    }
}

fn unpack_per_source(hit: &RrfHit<SeeleId>) -> (Option<usize>, Option<usize>, Option<usize>) {
    let mut fts = None;
    let mut vec = None;
    let mut loose = None;
    for (src, rank) in &hit.per_source {
        match *src {
            "fts" => fts = Some(*rank),
            "vec" => vec = Some(*rank),
            "fts_loose" => loose = Some(*rank),
            _ => {}
        }
    }
    (fts, vec, loose)
}

/// Map a relation row to an [`AnnotationKind`] from the **source** side's
/// point of view. `None` means the relation does not produce a useful
/// annotation for the source endpoint.
fn annotation_for_source(relation: &str, status: &str) -> Option<AnnotationKind> {
    match relation {
        "supersedes" => Some(AnnotationKind::Supersedes),
        "conflicts_with" => match status {
            "judged" => Some(AnnotationKind::ContestedBy),
            _ => Some(AnnotationKind::ConflictsWith),
        },
        _ => None,
    }
}

/// Same as [`annotation_for_source`] but from the **target** endpoint's POV.
fn annotation_for_target(relation: &str, status: &str) -> Option<AnnotationKind> {
    match relation {
        "supersedes" => Some(AnnotationKind::SupersededBy),
        "conflicts_with" => match status {
            "judged" => Some(AnnotationKind::ContestedBy),
            _ => Some(AnnotationKind::ConflictsWith),
        },
        _ => None,
    }
}

/// Conservative escape for FTS5 MATCH input. We wrap the user's query in
/// quotes so special characters don't break the parser. Caller can pass
/// FTS5 syntax directly by avoiding spaces — but in v0.1 we're OK with the
/// simpler quoted-phrase model for general text.
fn escape_fts(s: &str) -> String {
    // Escape internal double-quotes by doubling, then wrap in double-quotes.
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Bag-of-words MATCH expression (Q3): tokenize on non-alphanumerics,
/// quote each token, OR-join. `None` when fewer than 2 tokens survive —
/// the loose form would be identical to the strict phrase, so the extra
/// FTS pass is skipped. Each token is individually quoted, so no FTS5
/// metacharacter survives un-escaped (same conservative posture as
/// [`escape_fts`]).
fn loose_fts_match(s: &str) -> Option<String> {
    let tokens: Vec<String> = s
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\""))
        .collect();
    if tokens.len() < 2 {
        return None;
    }
    Some(tokens.join(" OR "))
}

fn parse_observation(row: &Row<'_>) -> Result<Observation> {
    use chrono::{TimeZone, Utc};
    use seele_core::memory::{ObservationType, Scope};
    use seele_core::metadata::Metadata;

    let id_str: String = row.get(0)?;
    let _int_id: i64 = row.get(1)?;
    let session_str: Option<String> = row.get(2)?;
    let type_str: String = row.get(3)?;
    let title: String = row.get(4)?;
    let content: String = row.get(5)?;
    let tool_name: Option<String> = row.get(6)?;
    let project: Option<String> = row.get(7)?;
    let scope_str: String = row.get(8)?;
    let topic_key: Option<String> = row.get(9)?;
    let normalized_hash: Option<String> = row.get(10)?;
    let revision_count: u32 = row.get(11)?;
    let duplicate_count: u32 = row.get(12)?;
    let last_seen_ms: i64 = row.get(13)?;
    let created_ms: i64 = row.get(14)?;
    let updated_ms: i64 = row.get(15)?;
    let deleted_ms: Option<i64> = row.get(16)?;
    let metadata_json: String = row.get(17)?;

    let id = id_str
        .parse::<SeeleId>()
        .map_err(|e| SearchError::InvalidInput(format!("bad ULID '{id_str}': {e}")))?;
    let session_id = session_str
        .map(|s| {
            s.parse::<SeeleId>()
                .map_err(|e| SearchError::InvalidInput(format!("bad session ULID '{s}': {e}")))
        })
        .transpose()?;
    let kind = ObservationType::from_str_relaxed(&type_str);
    let scope = Scope::from_str_strict(&scope_str)
        .ok_or_else(|| SearchError::InvalidInput(format!("unknown scope '{scope_str}'")))?;
    let metadata: Metadata = serde_json::from_str(&metadata_json).map_err(|e| {
        SearchError::InvalidInput(format!("metadata JSON '{metadata_json}' invalid: {e}"))
    })?;

    let to_dt = |ms: i64| {
        Utc.timestamp_millis_opt(ms)
            .single()
            .unwrap_or_else(Utc::now)
    };

    Ok(Observation {
        id,
        session_id,
        kind,
        title,
        content,
        tool_name,
        project,
        scope,
        topic_key,
        normalized_hash,
        revision_count,
        duplicate_count,
        last_seen_at: to_dt(last_seen_ms),
        created_at: to_dt(created_ms),
        updated_at: to_dt(updated_ms),
        deleted_at: deleted_ms.map(to_dt),
        metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_fts_wraps_in_quotes_and_escapes_internal_quotes() {
        assert_eq!(escape_fts("hello world"), "\"hello world\"");
        assert_eq!(escape_fts(r#"a "b" c"#), r#""a ""b"" c""#);
    }

    #[test]
    fn loose_fts_match_or_joins_quoted_tokens() {
        assert_eq!(
            loose_fts_match("como fallo el deploy").as_deref(),
            Some(r#""como" OR "fallo" OR "el" OR "deploy""#)
        );
        // Punctuation splits tokens; nothing survives un-quoted.
        assert_eq!(
            loose_fts_match("auth-flow (v2)?").as_deref(),
            Some(r#""auth" OR "flow" OR "v2""#)
        );
    }

    #[test]
    fn loose_fts_match_skips_single_token_and_empty() {
        assert_eq!(loose_fts_match("deploy"), None);
        assert_eq!(loose_fts_match("  ?!  "), None);
        assert_eq!(loose_fts_match("\"deploy\""), None);
    }

    #[test]
    fn annotation_for_source_maps_supersedes() {
        assert_eq!(
            annotation_for_source("supersedes", "pending"),
            Some(AnnotationKind::Supersedes)
        );
    }

    #[test]
    fn annotation_for_target_maps_supersedes_to_superseded_by() {
        assert_eq!(
            annotation_for_target("supersedes", "judged"),
            Some(AnnotationKind::SupersededBy)
        );
    }

    #[test]
    fn annotation_conflict_pending_is_peer_level() {
        assert_eq!(
            annotation_for_source("conflicts_with", "pending"),
            Some(AnnotationKind::ConflictsWith)
        );
        assert_eq!(
            annotation_for_target("conflicts_with", "pending"),
            Some(AnnotationKind::ConflictsWith)
        );
    }

    #[test]
    fn annotation_conflict_judged_marks_contested() {
        assert_eq!(
            annotation_for_source("conflicts_with", "judged"),
            Some(AnnotationKind::ContestedBy)
        );
        assert_eq!(
            annotation_for_target("conflicts_with", "judged"),
            Some(AnnotationKind::ContestedBy)
        );
    }

    #[test]
    fn annotation_unrelated_relation_is_none() {
        assert_eq!(annotation_for_source("related", "pending"), None);
        assert_eq!(annotation_for_target("compatible", "judged"), None);
    }
}
