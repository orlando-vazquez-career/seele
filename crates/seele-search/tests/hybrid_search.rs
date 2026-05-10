//! End-to-end hybrid search: storage + FakeEmbedder + RRF combiner.

use seele_core::memory::{ObservationType, Scope};
use seele_core::metadata::Metadata;
use seele_embedder::{Embedder, FakeEmbedder};
use seele_search::{SearchEngine, SearchQuery};
use seele_storage::{init_db, ObservationStore, SaveInput};
use serde_json::json;
use tempfile::TempDir;

fn fresh_engine() -> (TempDir, ObservationStore, SearchEngine) {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let store = ObservationStore::new(pool.clone());
    let embedder: Box<dyn Embedder> = Box::new(FakeEmbedder);
    let engine = SearchEngine::new(pool, embedder);
    (td, store, engine)
}

fn save_with_embedding(
    store: &ObservationStore,
    embedder: &dyn Embedder,
    title: &str,
    content: &str,
    project: &str,
    metadata: Metadata,
) -> seele_core::id::SeeleId {
    let outcome = store
        .save(SaveInput {
            session_id: None,
            kind: ObservationType::Decision,
            title: title.into(),
            content: content.into(),
            tool_name: None,
            project: Some(project.into()),
            scope: Scope::Project,
            topic_key: None,
            metadata,
        })
        .unwrap();
    let id = outcome.id();
    let embedding = embedder.embed(content).unwrap();
    store.set_embedding(id, &embedding).unwrap();
    id
}

#[test]
fn search_returns_results_when_fts_matches() {
    let (_td, store, engine) = fresh_engine();
    let embedder = FakeEmbedder;
    let target = save_with_embedding(
        &store,
        &embedder,
        "wal mode",
        "Postgres uses WAL for crash recovery",
        "p",
        Metadata::new(),
    );
    let _other = save_with_embedding(
        &store,
        &embedder,
        "tropical",
        "Bora Bora has crystal water",
        "p",
        Metadata::new(),
    );

    let hits = engine
        .search(SearchQuery {
            text: "postgres".into(),
            project: Some("p".into()),
            ..Default::default()
        })
        .unwrap();
    assert!(!hits.is_empty(), "expected at least one hit");
    assert_eq!(hits[0].observation.id, target);
    assert!(hits[0].fts_rank.is_some());
}

#[test]
fn search_returns_results_when_only_vec_matches() {
    // FakeEmbedder gives same content → same vector. So if we query the
    // exact content, the vec method will find it even if FTS5 doesn't (e.g.
    // because the FTS5 query terms are unusual punctuation).
    let (_td, store, engine) = fresh_engine();
    let embedder = FakeEmbedder;
    let target = save_with_embedding(
        &store,
        &embedder,
        "encoded",
        "@@@@##**",
        "p",
        Metadata::new(),
    );
    let hits = engine
        .search(SearchQuery {
            text: "@@@@##**".into(),
            project: Some("p".into()),
            ..Default::default()
        })
        .unwrap();
    assert!(!hits.is_empty());
    assert_eq!(hits[0].observation.id, target);
    assert!(hits[0].vec_rank.is_some());
}

#[test]
fn project_filter_excludes_other_projects() {
    let (_td, store, engine) = fresh_engine();
    let embedder = FakeEmbedder;
    let _a = save_with_embedding(
        &store,
        &embedder,
        "in a",
        "shared keyword postgres",
        "proj-a",
        Metadata::new(),
    );
    let _b = save_with_embedding(
        &store,
        &embedder,
        "in b",
        "shared keyword postgres",
        "proj-b",
        Metadata::new(),
    );

    let only_a = engine
        .search(SearchQuery {
            text: "postgres".into(),
            project: Some("proj-a".into()),
            ..Default::default()
        })
        .unwrap();
    for h in &only_a {
        assert_eq!(h.observation.project.as_deref(), Some("proj-a"));
    }
    assert!(!only_a.is_empty());
}

#[test]
fn purist_metadata_is_excluded_by_default_and_included_on_opt_in() {
    let (_td, store, engine) = fresh_engine();
    let embedder = FakeEmbedder;
    let purist = save_with_embedding(
        &store,
        &embedder,
        "purist memo",
        "first principles dictate that we minimize state",
        "p",
        Metadata::from_value(json!({"context_mode": "purist"})),
    );
    let contextual = save_with_embedding(
        &store,
        &embedder,
        "contextual memo",
        "context informed by prior counsels suggests caching layer",
        "p",
        Metadata::from_value(json!({"context_mode": "contextual"})),
    );

    let default_hits = engine
        .search(SearchQuery {
            text: "first principles caching".into(),
            project: Some("p".into()),
            ..Default::default()
        })
        .unwrap();
    let returned: Vec<_> = default_hits.iter().map(|h| h.observation.id).collect();
    assert!(
        !returned.contains(&purist),
        "purist must be excluded by default"
    );
    assert!(returned.contains(&contextual));

    let with_purist = engine
        .search(SearchQuery {
            text: "first principles caching".into(),
            project: Some("p".into()),
            include_purist: true,
            ..Default::default()
        })
        .unwrap();
    let returned: Vec<_> = with_purist.iter().map(|h| h.observation.id).collect();
    assert!(returned.contains(&purist));
}

#[test]
fn deleted_observations_excluded_from_search() {
    let (_td, store, engine) = fresh_engine();
    let embedder = FakeEmbedder;
    let alive = save_with_embedding(
        &store,
        &embedder,
        "alive",
        "postgres connection pooling decisions",
        "p",
        Metadata::new(),
    );
    let dead = save_with_embedding(
        &store,
        &embedder,
        "dead",
        "postgres connection pooling decisions",
        "p",
        Metadata::new(),
    );
    store.soft_delete(dead).unwrap();

    let hits = engine
        .search(SearchQuery {
            text: "postgres pooling".into(),
            project: Some("p".into()),
            ..Default::default()
        })
        .unwrap();
    let returned: Vec<_> = hits.iter().map(|h| h.observation.id).collect();
    assert!(returned.contains(&alive));
    assert!(!returned.contains(&dead));
}

#[test]
fn empty_query_returns_invalid_input_error() {
    let (_td, _store, engine) = fresh_engine();
    let err = engine
        .search(SearchQuery {
            text: "   ".into(),
            ..Default::default()
        })
        .unwrap_err();
    assert!(format!("{err}").contains("query text is empty"));
}

#[test]
fn limit_is_respected_after_rrf() {
    let (_td, store, engine) = fresh_engine();
    let embedder = FakeEmbedder;
    for i in 0..15 {
        save_with_embedding(
            &store,
            &embedder,
            &format!("decision {i}"),
            &format!("postgres tuning #{i} we considered options"),
            "p",
            Metadata::new(),
        );
    }
    let hits = engine
        .search(SearchQuery {
            text: "postgres tuning".into(),
            project: Some("p".into()),
            limit: Some(5),
            ..Default::default()
        })
        .unwrap();
    assert!(hits.len() <= 5);
    assert!(!hits.is_empty());
}
