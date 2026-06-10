//! `seele-eval` — memory-quality evaluation harness for SEELE.
//!
//! Runs labelled **suites** (a corpus to ingest + ground-truth queries)
//! against the live hybrid search pipeline (`seele-search`) and reports
//! recall@k / MRR per category. Exposed as `seele eval --suite <name>`
//! and as an `#[ignore]` test, so it never runs in CI by default (it needs
//! the real ONNX embedder + a model download). See ADR-14.
//!
//! Pipeline: contract types → [`load_suite`] → [`ingest`] → [`run_suite`].

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use seele_core::id::SeeleId;
use seele_core::memory::{ObservationType, Scope};
use seele_core::metadata::Metadata;
use seele_embedder::Embedder;
use seele_search::{SearchEngine, SearchQuery};
use seele_storage::{ObservationStore, RawSaveInput, StorageError};

/// Errors raised while loading, ingesting, or running a suite.
#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("storage: {0}")]
    Storage(#[from] StorageError),
    #[error("embedder: {0}")]
    Embedder(#[from] seele_embedder::EmbedderError),
    #[error("search: {0}")]
    Search(#[from] seele_search::SearchError),
}

pub type Result<T> = std::result::Result<T, EvalError>;

/// Input contract for a suite fixture (JSON). Per ADR-14:
/// `{ corpus: [{id, body, metadata}], queries: [{q, expected_ids[], category}] }`.
/// Unknown top-level keys (e.g. `_comment`) are ignored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suite {
    /// Observations to ingest into an ephemeral DB before querying.
    pub corpus: Vec<CorpusItem>,
    /// Ground-truth queries run against the ingested corpus.
    pub queries: Vec<EvalQuery>,
}

/// One corpus observation. `body` becomes the observation content; the
/// `metadata` object's string fields (`title`, `type`, `project`, `scope`,
/// `topic_key`) are mapped onto [`RawSaveInput`] during ingest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusItem {
    pub id: String,
    pub body: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// One ground-truth query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalQuery {
    /// The query text handed to hybrid search.
    pub q: String,
    /// Corpus ids that count as a correct hit if present in the top-k.
    pub expected_ids: Vec<String>,
    /// Category for per-category aggregation (e.g. `single-fact`,
    /// `paraphrase`, `multi-hop`, `knowledge-update`, `temporal`).
    pub category: String,
    /// Documentation of WHY this query tests what it tests (GRAIL-style).
    /// Ignored by the runner; for suite authors and reviewers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

/// Aggregated metrics for one suite run. Serialised to JSON to version a
/// baseline (T-B6).
#[derive(Debug, Clone, Serialize)]
pub struct SuiteReport {
    pub suite: String,
    /// Embedder model id the baseline was captured with.
    pub model_id: String,
    pub totals: Metrics,
    /// Stable ordering for diff-friendly baselines.
    pub by_category: BTreeMap<String, Metrics>,
}

/// Retrieval-quality metrics. `recall@k` is binary per query (hit if any
/// `expected_id` is in the top-k), averaged; `mrr` is mean reciprocal rank;
/// `ndcg@10` uses binary relevance (promised in ADR-14): DCG over the top-10
/// positions holding an expected id, normalized by the ideal DCG for that
/// query's expected-set size. Secondary metric until graded labels exist.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Metrics {
    pub queries: usize,
    pub recall_at_5: f64,
    pub recall_at_10: f64,
    pub mrr: f64,
    pub ndcg_at_10: f64,
}

/// Parse a suite fixture from a JSON file.
pub fn load_suite(path: impl AsRef<Path>) -> Result<Suite> {
    let raw = std::fs::read_to_string(path)?;
    let suite = serde_json::from_str(&raw)?;
    Ok(suite)
}

/// Names of the suites shipped inside the binary.
pub const BUILTIN_SUITES: &[&str] = &["coding-memory", "longmemeval-subset"];

/// Load a suite shipped inside the binary via `include_str!`, so
/// `seele eval --suite <name>` works from any install without fixture files
/// on disk. Returns `Ok(None)` for an unknown name.
pub fn builtin_suite(name: &str) -> Result<Option<Suite>> {
    let raw = match name {
        "coding-memory" => include_str!("../fixtures/coding-memory.json"),
        "longmemeval-subset" => include_str!("../fixtures/longmemeval-subset.json"),
        _ => return Ok(None),
    };
    Ok(Some(serde_json::from_str(raw)?))
}

/// Ingest a suite's corpus into `store`, embedding each `body` with
/// `embedder` and storing the vector so hybrid search can use it. Returns a
/// map from fixture id → assigned [`SeeleId`] so the runner can translate
/// search hits back to `expected_ids`.
///
/// Uses `save_raw` (not `save`): each corpus item becomes its **own row**,
/// verbatim, bypassing the topic-key upsert + dedup window. That matters —
/// otherwise two items sharing a `topic_key` (e.g. a superseding pair) would
/// collapse into one row and corrupt the ground truth.
///
/// `embedder` must produce vectors of the DB's vec0 width (384 for the
/// default model / `FakeEmbedder`); a mismatch fails at `set_embedding`.
pub fn ingest(
    suite: &Suite,
    store: &ObservationStore,
    embedder: &dyn Embedder,
) -> Result<HashMap<String, SeeleId>> {
    let now = Utc::now();
    let mut map = HashMap::with_capacity(suite.corpus.len());

    // Phase 1: save every row verbatim in one transaction.
    {
        let mut conn = store.pool().get().map_err(StorageError::from)?;
        let tx = conn.transaction().map_err(StorageError::from)?;
        for item in &suite.corpus {
            let id = SeeleId::new();
            let meta = &item.metadata;
            let input = RawSaveInput {
                id,
                session_id: None,
                kind: meta
                    .get("type")
                    .and_then(|v| v.as_str())
                    .map(ObservationType::from_str_relaxed)
                    .unwrap_or(ObservationType::Memory),
                title: meta
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                content: item.body.clone(),
                tool_name: None,
                project: meta
                    .get("project")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                scope: meta
                    .get("scope")
                    .and_then(|v| v.as_str())
                    .and_then(Scope::from_str_strict)
                    .unwrap_or_default(),
                topic_key: meta
                    .get("topic_key")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                created_at: now,
                updated_at: now,
                metadata: Metadata::new(),
            };
            ObservationStore::save_raw_in_tx(&tx, input)?;
            map.insert(item.id.clone(), id);
        }
        tx.commit().map_err(StorageError::from)?;
    }

    // Phase 2: embeddings use their own pooled connection, so they run after
    // the rows are committed.
    for item in &suite.corpus {
        let id = map[&item.id];
        let vector = embedder.embed(&item.body)?;
        let meta = seele_storage::EmbeddingMeta {
            model_id: embedder.model_id().to_string(),
            dim: vector.len(),
            contextualized: false,
        };
        store.set_embedding(id, &vector, &meta)?;
    }

    Ok(map)
}

/// Ingest `suite`'s corpus into `store`, then run every query through hybrid
/// search and aggregate recall@5 / recall@10 / MRR, per category and overall.
///
/// `embedder` is used both to embed the corpus (ingest) and the queries
/// (search), so the baseline reflects one model end-to-end. `suite_name` and
/// `model_id` label the report. The search runs with default RRF and **no**
/// metadata-score boost (pure baseline).
pub fn run_suite(
    suite: &Suite,
    suite_name: &str,
    store: &ObservationStore,
    embedder: Box<dyn Embedder>,
    model_id: &str,
) -> Result<SuiteReport> {
    // Ingest borrows the embedder; the engine then takes ownership.
    let map = ingest(suite, store, embedder.as_ref())?;
    let rev: HashMap<SeeleId, &str> = map.iter().map(|(fid, id)| (*id, fid.as_str())).collect();

    let engine = SearchEngine::new(store.pool().clone(), embedder);

    let mut totals = Acc::default();
    let mut by_cat: BTreeMap<String, Acc> = BTreeMap::new();

    for query in &suite.queries {
        let hits = engine.search(SearchQuery {
            text: query.q.clone(),
            limit: Some(10),
            ..SearchQuery::default()
        })?;

        // Search returns SeeleIds; translate back to fixture ids to compare
        // against expected_ids.
        let ranked: Vec<&str> = hits
            .iter()
            .filter_map(|h| rev.get(&h.observation.id).copied())
            .collect();
        let expected: HashSet<&str> = query.expected_ids.iter().map(String::as_str).collect();

        let h5 = ranked
            .iter()
            .copied()
            .take(5)
            .any(|id| expected.contains(id)) as usize;
        let h10 = ranked
            .iter()
            .copied()
            .take(10)
            .any(|id| expected.contains(id)) as usize;
        let rr = ranked
            .iter()
            .copied()
            .position(|id| expected.contains(id))
            .map(|p| 1.0 / (p as f64 + 1.0))
            .unwrap_or(0.0);
        let ndcg = ndcg_at_10(&ranked, &expected);

        let cat_acc = by_cat.entry(query.category.clone()).or_default();
        for acc in [&mut totals, cat_acc] {
            acc.queries += 1;
            acc.r5 += h5;
            acc.r10 += h10;
            acc.rr_sum += rr;
            acc.ndcg_sum += ndcg;
        }
    }

    Ok(SuiteReport {
        suite: suite_name.to_string(),
        model_id: model_id.to_string(),
        totals: totals.finish(),
        by_category: by_cat.into_iter().map(|(k, v)| (k, v.finish())).collect(),
    })
}

/// Running accumulator for one bucket (overall or a single category).
#[derive(Default)]
struct Acc {
    queries: usize,
    r5: usize,
    r10: usize,
    rr_sum: f64,
    ndcg_sum: f64,
}

impl Acc {
    fn finish(&self) -> Metrics {
        let n = (self.queries.max(1)) as f64;
        Metrics {
            queries: self.queries,
            recall_at_5: self.r5 as f64 / n,
            recall_at_10: self.r10 as f64 / n,
            mrr: self.rr_sum / n,
            ndcg_at_10: self.ndcg_sum / n,
        }
    }
}

/// nDCG@10 with binary relevance. DCG sums `1/log2(pos+1)` over the first
/// 10 ranked positions that hold an expected id (positions 1-indexed);
/// IDCG is the DCG of the ideal ranking — all expected ids first, capped
/// at 10. Empty expected-set yields 0.0 (no query should ship one).
fn ndcg_at_10(ranked: &[&str], expected: &HashSet<&str>) -> f64 {
    let dcg: f64 = ranked
        .iter()
        .take(10)
        .enumerate()
        .filter(|(_, id)| expected.contains(*id))
        .map(|(i, _)| 1.0 / ((i as f64 + 2.0).log2()))
        .sum();
    let ideal_hits = expected.len().min(10);
    let idcg: f64 = (0..ideal_hits)
        .map(|i| 1.0 / ((i as f64 + 2.0).log2()))
        .sum();
    if idcg > 0.0 {
        dcg / idcg
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set<'a>(ids: &[&'a str]) -> HashSet<&'a str> {
        ids.iter().copied().collect()
    }

    #[test]
    fn ndcg_perfect_ranking_is_one() {
        let ranked = vec!["a", "b", "x", "y"];
        assert!((ndcg_at_10(&ranked, &set(&["a", "b"])) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn ndcg_single_expected_at_position_two() {
        let ranked = vec!["x", "a"];
        // DCG = 1/log2(3); IDCG = 1/log2(2) = 1.
        let want = 1.0 / 3f64.log2();
        assert!((ndcg_at_10(&ranked, &set(&["a"])) - want).abs() < 1e-12);
    }

    #[test]
    fn ndcg_miss_is_zero_and_beyond_ten_does_not_count() {
        assert_eq!(ndcg_at_10(&["x", "y"], &set(&["a"])), 0.0);
        let mut ranked = vec!["x"; 10];
        ranked.push("a"); // position 11 — outside the @10 window
        assert_eq!(ndcg_at_10(&ranked, &set(&["a"])), 0.0);
    }

    #[test]
    fn ndcg_empty_expected_is_zero_not_nan() {
        assert_eq!(ndcg_at_10(&["x"], &set(&[])), 0.0);
    }
}
