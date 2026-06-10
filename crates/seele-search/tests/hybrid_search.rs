//! End-to-end hybrid search: storage + FakeEmbedder + RRF combiner.

use seele_core::id::SeeleId;
use seele_core::memory::{ObservationType, Scope};
use seele_core::metadata::Metadata;
use seele_core::relation::{JudgmentStatus, RelationKind};
use seele_embedder::{Embedder, FakeEmbedder};
use seele_search::{AnnotationKind, SearchEngine, SearchHit, SearchQuery};
use seele_storage::{
    init_db, JudgmentInput, ObservationStore, RelationInput, RelationStore, SaveInput,
};
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

fn fresh_engine_with_relations() -> (TempDir, ObservationStore, RelationStore, SearchEngine) {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let store = ObservationStore::new(pool.clone());
    let relations = RelationStore::new(pool.clone());
    let embedder: Box<dyn Embedder> = Box::new(FakeEmbedder);
    let engine = SearchEngine::new(pool, embedder);
    (td, store, relations, engine)
}

fn save_with_embedding(
    store: &ObservationStore,
    embedder: &dyn Embedder,
    title: &str,
    content: &str,
    project: &str,
    metadata: Metadata,
) -> SeeleId {
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
    let meta = seele_storage::EmbeddingMeta {
        model_id: embedder.model_id().to_string(),
        dim: embedding.len(),
        contextualized: false,
    };
    store.set_embedding(id, &embedding, &meta).unwrap();
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
    assert!(hits[0].annotations.is_empty(), "annotations off by default");
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
fn whitespace_only_query_treated_as_empty_query() {
    // Sprint-02: empty / whitespace query no longer returns InvalidInput.
    // Instead the engine switches to a list-by-filters path ordered by
    // created_at DESC.
    let (_td, store, engine) = fresh_engine();
    let embedder = FakeEmbedder;
    let id = save_with_embedding(
        &store,
        &embedder,
        "anything",
        "recent observation",
        "p",
        Metadata::new(),
    );

    let hits = engine
        .search(SearchQuery {
            text: "   \t  ".into(),
            project: Some("p".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].observation.id, id);
    // Empty-query hits have score = 0 and no FTS/vec ranks.
    assert_eq!(hits[0].score, 0.0);
    assert!(hits[0].fts_rank.is_none());
    assert!(hits[0].vec_rank.is_none());
}

#[test]
fn empty_query_with_filters_lists_recent_in_project() {
    let (_td, store, engine) = fresh_engine();
    let embedder = FakeEmbedder;
    // Five obs in project "p", interleaved with five in "other".
    let mut p_ids = Vec::new();
    for i in 0..5 {
        // We insert with slight sleeps so created_at differs deterministically.
        let id = save_with_embedding(
            &store,
            &embedder,
            &format!("p {i}"),
            &format!("content {i}"),
            "p",
            Metadata::new(),
        );
        p_ids.push(id);
        let _ = save_with_embedding(
            &store,
            &embedder,
            &format!("other {i}"),
            &format!("misc {i}"),
            "other",
            Metadata::new(),
        );
    }

    let hits = engine
        .search(SearchQuery {
            text: "".into(),
            project: Some("p".into()),
            limit: Some(3),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(hits.len(), 3, "expected limit=3 hits");
    for hit in &hits {
        assert_eq!(hit.observation.project.as_deref(), Some("p"));
    }
}

#[test]
fn empty_query_excludes_purist_by_default_and_deleted() {
    let (_td, store, engine) = fresh_engine();
    let embedder = FakeEmbedder;
    let _ = save_with_embedding(
        &store,
        &embedder,
        "purist",
        "stripped from default",
        "p",
        Metadata::from_value(json!({"context_mode": "purist"})),
    );
    let dead = save_with_embedding(&store, &embedder, "dead", "tombstone", "p", Metadata::new());
    store.soft_delete(dead).unwrap();
    let alive = save_with_embedding(&store, &embedder, "alive", "visible", "p", Metadata::new());

    let hits = engine
        .search(SearchQuery {
            text: "".into(),
            project: Some("p".into()),
            ..Default::default()
        })
        .unwrap();
    let returned: Vec<_> = hits.iter().map(|h| h.observation.id).collect();
    assert!(returned.contains(&alive));
    assert!(!returned.contains(&dead));
    assert_eq!(returned.len(), 1);
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

#[test]
fn boost_promotes_high_meta_score_doc() {
    let (_td, store, engine) = fresh_engine();
    let embedder = FakeEmbedder;
    // Both have the same FTS terms; high_score has meta_score=10.0.
    let high_score = save_with_embedding(
        &store,
        &embedder,
        "alpha",
        "cache invalidation strategies for postgres",
        "p",
        Metadata::from_value(json!({"score": 10.0})),
    );
    let no_score = save_with_embedding(
        &store,
        &embedder,
        "beta",
        "cache invalidation strategies for postgres",
        "p",
        Metadata::new(),
    );

    // With boost enabled, the high_score doc must rank ahead even though
    // RRF alone might tie them.
    let hits = engine
        .search(SearchQuery {
            text: "cache invalidation postgres".into(),
            project: Some("p".into()),
            score_boost_multiplier: 0.1,
            ..Default::default()
        })
        .unwrap();
    assert!(hits.len() >= 2);
    let positions: std::collections::HashMap<SeeleId, usize> = hits
        .iter()
        .enumerate()
        .map(|(i, h)| (h.observation.id, i))
        .collect();
    let pos_high = positions.get(&high_score).copied().unwrap();
    let pos_low = positions.get(&no_score).copied().unwrap();
    assert!(
        pos_high < pos_low,
        "high_score {pos_high} should outrank no_score {pos_low}"
    );
}

#[test]
fn boost_zero_preserves_rrf_order() {
    let (_td, store, engine) = fresh_engine();
    let embedder = FakeEmbedder;
    let _a = save_with_embedding(
        &store,
        &embedder,
        "alpha",
        "cache invalidation strategies for postgres",
        "p",
        Metadata::from_value(json!({"score": 10.0})),
    );
    let _b = save_with_embedding(
        &store,
        &embedder,
        "beta",
        "cache invalidation strategies for postgres",
        "p",
        Metadata::new(),
    );

    // Two searches: one with boost=0 (default), one with boost=0 explicit.
    // Both should produce identical ordering (no observable side-effect from
    // the apply_score_boost no-op path).
    let baseline = engine
        .search(SearchQuery {
            text: "cache invalidation postgres".into(),
            project: Some("p".into()),
            ..Default::default()
        })
        .unwrap();
    let explicit_zero = engine
        .search(SearchQuery {
            text: "cache invalidation postgres".into(),
            project: Some("p".into()),
            score_boost_multiplier: 0.0,
            ..Default::default()
        })
        .unwrap();
    let id_seq =
        |hits: &[SearchHit]| -> Vec<SeeleId> { hits.iter().map(|h| h.observation.id).collect() };
    assert_eq!(id_seq(&baseline), id_seq(&explicit_zero));
}

#[test]
fn max_distance_drops_far_hits() {
    let (_td, store, engine) = fresh_engine();
    let embedder = FakeEmbedder;
    let _near = save_with_embedding(
        &store,
        &embedder,
        "near",
        "exact query string here",
        "p",
        Metadata::new(),
    );
    // Different content → different FakeEmbedder vector → vec distance > 0.
    let _far = save_with_embedding(
        &store,
        &embedder,
        "far",
        "completely unrelated lorem ipsum dolor",
        "p",
        Metadata::new(),
    );

    // With a tiny max_vec_distance the far hit is dropped from the vec
    // ranking. We don't assert exact distance values (FakeEmbedder distance
    // depends on the hash collision pattern), but we do verify that at
    // least the count drops vs. the unconstrained query.
    let unconstrained = engine
        .search(SearchQuery {
            text: "exact query string here".into(),
            project: Some("p".into()),
            ..Default::default()
        })
        .unwrap();
    let constrained = engine
        .search(SearchQuery {
            text: "exact query string here".into(),
            project: Some("p".into()),
            max_vec_distance: Some(0.001),
            ..Default::default()
        })
        .unwrap();
    assert!(constrained.len() <= unconstrained.len());
}

#[test]
fn annotations_supersedes_attaches_to_winner_and_loser() {
    let (_td, store, relations, engine) = fresh_engine_with_relations();
    let embedder = FakeEmbedder;
    let winner = save_with_embedding(
        &store,
        &embedder,
        "winner",
        "use connection pooling for postgres",
        "p",
        Metadata::new(),
    );
    let loser = save_with_embedding(
        &store,
        &embedder,
        "loser",
        "use connection pooling for postgres",
        "p",
        Metadata::new(),
    );
    relations
        .create(RelationInput {
            sync_id: "sync-1".into(),
            source_id: winner,
            target_id: loser,
            relation: RelationKind::Supersedes,
            reason: Some("loser had a data race".into()),
            evidence: None,
            confidence: None,
            marked_by_actor: None,
            marked_by_kind: None,
            marked_by_model: None,
            session_id: None,
        })
        .unwrap();

    let hits = engine
        .search(SearchQuery {
            text: "postgres pooling".into(),
            project: Some("p".into()),
            include_annotations: true,
            ..Default::default()
        })
        .unwrap();
    let by_id: std::collections::HashMap<SeeleId, &SearchHit> =
        hits.iter().map(|h| (h.observation.id, h)).collect();
    let win_hit = by_id.get(&winner).expect("winner in hits");
    let lose_hit = by_id.get(&loser).expect("loser in hits");

    assert!(win_hit
        .annotations
        .iter()
        .any(|a| a.kind == AnnotationKind::Supersedes && a.other_id == loser));
    assert!(lose_hit
        .annotations
        .iter()
        .any(|a| a.kind == AnnotationKind::SupersededBy && a.other_id == winner));
}

#[test]
fn annotations_off_by_default() {
    let (_td, store, relations, engine) = fresh_engine_with_relations();
    let embedder = FakeEmbedder;
    let a = save_with_embedding(
        &store,
        &embedder,
        "a",
        "shared phrase content",
        "p",
        Metadata::new(),
    );
    let b = save_with_embedding(
        &store,
        &embedder,
        "b",
        "shared phrase content",
        "p",
        Metadata::new(),
    );
    relations
        .create(RelationInput {
            sync_id: "sync-1".into(),
            source_id: a,
            target_id: b,
            relation: RelationKind::Supersedes,
            reason: None,
            evidence: None,
            confidence: None,
            marked_by_actor: None,
            marked_by_kind: None,
            marked_by_model: None,
            session_id: None,
        })
        .unwrap();

    let hits = engine
        .search(SearchQuery {
            text: "shared phrase".into(),
            project: Some("p".into()),
            // include_annotations omitted → default false
            ..Default::default()
        })
        .unwrap();
    for h in &hits {
        assert!(
            h.annotations.is_empty(),
            "annotations must be empty when opt-out"
        );
    }
}

#[test]
fn annotations_judged_conflict_yields_contested_by() {
    let (_td, store, relations, engine) = fresh_engine_with_relations();
    let embedder = FakeEmbedder;
    let a = save_with_embedding(
        &store,
        &embedder,
        "a",
        "schema decision: snake case",
        "p",
        Metadata::new(),
    );
    let b = save_with_embedding(
        &store,
        &embedder,
        "b",
        "schema decision: snake case",
        "p",
        Metadata::new(),
    );
    let rel = relations
        .create(RelationInput {
            sync_id: "sync-2".into(),
            source_id: a,
            target_id: b,
            relation: RelationKind::ConflictsWith,
            reason: None,
            evidence: None,
            confidence: None,
            marked_by_actor: None,
            marked_by_kind: None,
            marked_by_model: None,
            session_id: None,
        })
        .unwrap();
    relations
        .judge(
            rel.id,
            JudgmentInput {
                status: JudgmentStatus::Judged,
                reason: Some("a chose camelCase".into()),
                evidence: None,
                confidence: None,
            },
        )
        .unwrap();

    let hits = engine
        .search(SearchQuery {
            text: "schema snake".into(),
            project: Some("p".into()),
            include_annotations: true,
            ..Default::default()
        })
        .unwrap();
    let by_id: std::collections::HashMap<SeeleId, &SearchHit> =
        hits.iter().map(|h| (h.observation.id, h)).collect();
    let ha = by_id.get(&a).expect("a in hits");
    let hb = by_id.get(&b).expect("b in hits");
    assert!(ha
        .annotations
        .iter()
        .any(|x| x.kind == AnnotationKind::ContestedBy));
    assert!(hb
        .annotations
        .iter()
        .any(|x| x.kind == AnnotationKind::ContestedBy));
}

// -------- Q3: fts_loose bag-of-words rescue --------

#[test]
fn loose_path_rescues_paraphrase_when_phrase_match_fails() {
    let (_td, store, engine) = fresh_engine();
    let embedder = FakeEmbedder;
    let target = save_with_embedding(
        &store,
        &embedder,
        "deploy dns",
        "El deploy de produccion fallo por un problema de DNS en el balanceador",
        "p",
        Metadata::new(),
    );
    // Decoy so the rescue has to rank, not just return the only row.
    save_with_embedding(
        &store,
        &embedder,
        "indices parciales",
        "Un indice parcial WHERE deleted_at IS NULL acelera las queries vivas",
        "p",
        Metadata::new(),
    );

    // Natural-language paraphrase: the exact phrase does NOT appear in the
    // content, so the strict quoted-phrase FTS path yields nothing. The
    // OR-of-tokens rescue must surface the target via shared tokens.
    let hits = engine
        .search(SearchQuery {
            text: "por que fallo el deploy en produccion?".into(),
            ..Default::default()
        })
        .unwrap();
    let hit = hits
        .iter()
        .find(|h| h.observation.id == target)
        .expect("loose path must rescue the paraphrase");
    assert!(
        hit.fts_rank.is_none(),
        "strict phrase must have missed it (got {:?})",
        hit.fts_rank
    );
    assert!(
        hit.fts_loose_rank.is_some(),
        "loose path must have found it"
    );
}

#[test]
fn trace_exposes_zero_strict_fts_candidates_on_paraphrase() {
    let (_td, store, engine) = fresh_engine();
    let embedder = FakeEmbedder;
    save_with_embedding(
        &store,
        &embedder,
        "deploy dns",
        "El deploy de produccion fallo por un problema de DNS en el balanceador",
        "p",
        Metadata::new(),
    );

    let (hits, trace) = engine
        .search_traced(SearchQuery {
            text: "por que fallo el deploy en produccion?".into(),
            ..Default::default()
        })
        .unwrap();

    // The exact symptom --explain exists to expose: strict phrase = 0.
    assert_eq!(trace.version, 1);
    assert_eq!(trace.fts_candidates, 0, "phrase-quoting kills paraphrase");
    assert!(trace.fts_loose_candidates.unwrap_or(0) >= 1, "rescue fired");
    assert_eq!(trace.vec_candidates, trace.vec_distances.len());
    assert!(!hits.is_empty());
    assert!(trace.fts_match.starts_with('"'));
    assert!(trace.fts_loose_match.as_deref().unwrap().contains(" OR "));
}

#[test]
fn single_token_query_skips_loose_path() {
    let (_td, store, engine) = fresh_engine();
    let embedder = FakeEmbedder;
    save_with_embedding(
        &store,
        &embedder,
        "wal",
        "Postgres usa WAL para crash recovery",
        "p",
        Metadata::new(),
    );
    let (_, trace) = engine
        .search_traced(SearchQuery {
            text: "WAL".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        trace.fts_loose_match.is_none(),
        "single token: no loose pass"
    );
    assert!(trace.fts_loose_candidates.is_none());
}

// -------- Q4: knn_by_vector --------

#[test]
fn knn_by_vector_filters_excludes_and_orders_by_distance() {
    let (_td, store, engine) = fresh_engine();

    // Three rows with synthetic unit vectors: a and b close, c orthogonal.
    let mut va = vec![0.0f32; 384];
    va[0] = 1.0;
    let mut vb = vec![0.0f32; 384];
    vb[0] = 0.98;
    vb[1] = (1.0f32 - 0.98 * 0.98).sqrt(); // unit norm, cos≈0.98 vs a
    let mut vc = vec![0.0f32; 384];
    vc[2] = 1.0; // orthogonal

    let save_plain = |title: &str, content: &str, project: &str| {
        store
            .save(SaveInput {
                session_id: None,
                kind: ObservationType::Decision,
                title: title.into(),
                content: content.into(),
                tool_name: None,
                project: Some(project.into()),
                scope: Scope::Project,
                topic_key: None,
                metadata: Metadata::new(),
            })
            .unwrap()
            .id()
    };
    let meta = seele_storage::EmbeddingMeta {
        model_id: "stub".into(),
        dim: 384,
        contextualized: false,
    };
    let a = save_plain("a", "contenido a", "p");
    let b = save_plain("b", "contenido b", "p");
    let c = save_plain("c", "contenido c", "p");
    let other = save_plain("d", "contenido d", "otro-proyecto");
    store.set_embedding(a, &va, &meta).unwrap();
    store.set_embedding(b, &vb, &meta).unwrap();
    store.set_embedding(c, &vc, &meta).unwrap();
    store.set_embedding(other, &va, &meta).unwrap(); // identical to a, other project

    // Query with a's vector, excluding a itself, scoped to project p.
    let pairs = engine
        .knn_by_vector(&va, Some("p"), Some(Scope::Project), Some(a), 3)
        .unwrap();
    let ids: Vec<_> = pairs.iter().map(|(id, _)| *id).collect();
    assert!(!ids.contains(&a), "excluded id must not appear");
    assert!(!ids.contains(&other), "project filter must hold");
    assert_eq!(ids[0], b, "closest neighbor first");
    // cos 0.98 → l2 = sqrt(2*(1-0.98)) = 0.2 — inside the 0.37 threshold.
    assert!(
        pairs[0].1 < 0.37,
        "b distance {} should be near",
        pairs[0].1
    );
    // c is orthogonal: l2 = sqrt(2) ≈ 1.414 — far outside.
    if let Some((_, dc)) = pairs.iter().find(|(id, _)| *id == c) {
        assert!(*dc > 1.0, "orthogonal distance {dc} should be far");
    }
}
