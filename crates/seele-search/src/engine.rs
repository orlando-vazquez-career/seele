//! Hybrid search engine: FTS5 + vec0 combined via Reciprocal Rank Fusion.

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
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub observation: Observation,
    pub score: f64,
    pub fts_rank: Option<usize>,
    pub vec_rank: Option<usize>,
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
        if query.text.trim().is_empty() {
            return Err(SearchError::InvalidInput("query text is empty".into()));
        }

        let per_method = query.per_method_limit.unwrap_or(DEFAULT_PER_METHOD_LIMIT);
        let final_limit = query.limit.unwrap_or(DEFAULT_FINAL_LIMIT) as usize;

        let fts_rank = self.fts_query(&query, per_method)?;
        let vec_rank = self.vec_query(&query, per_method)?;

        let combined = rrf::combine(
            &[("fts", fts_rank.clone()), ("vec", vec_rank.clone())],
            self.rrf_k,
        );

        let top: Vec<&RrfHit<SeeleId>> = combined.iter().take(final_limit).collect();
        let observations = self.hydrate(&top.iter().map(|h| h.id).collect::<Vec<_>>())?;
        let by_id: std::collections::HashMap<SeeleId, Observation> =
            observations.into_iter().map(|o| (o.id, o)).collect();

        let mut hits = Vec::with_capacity(top.len());
        for hit in top {
            let Some(obs) = by_id.get(&hit.id).cloned() else {
                continue; // race or filter mismatch
            };
            let mut fts_r = None;
            let mut vec_r = None;
            for (src, rank) in &hit.per_source {
                match *src {
                    "fts" => fts_r = Some(*rank),
                    "vec" => vec_r = Some(*rank),
                    _ => {}
                }
            }
            hits.push(SearchHit {
                observation: obs,
                score: hit.score,
                fts_rank: fts_r,
                vec_rank: vec_r,
            });
        }
        Ok(hits)
    }

    fn fts_query(&self, query: &SearchQuery, limit: u32) -> Result<Vec<SeeleId>> {
        let mut sql = String::from(
            "SELECT o.id FROM observations_fts fts \
             JOIN observations o ON o.int_id = fts.rowid \
             WHERE observations_fts MATCH ?1 \
                AND o.deleted_at IS NULL",
        );
        let mut bound: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(escape_fts(&query.text))];
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

    fn vec_query(&self, query: &SearchQuery, limit: u32) -> Result<Vec<SeeleId>> {
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
        sql.push_str(" ORDER BY vec.distance");

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

/// Conservative escape for FTS5 MATCH input. We wrap the user's query in
/// quotes so special characters don't break the parser. Caller can pass
/// FTS5 syntax directly by avoiding spaces — but in v0.1 we're OK with the
/// simpler quoted-phrase model for general text.
fn escape_fts(s: &str) -> String {
    // Escape internal double-quotes by doubling, then wrap in double-quotes.
    format!("\"{}\"", s.replace('"', "\"\""))
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
}
