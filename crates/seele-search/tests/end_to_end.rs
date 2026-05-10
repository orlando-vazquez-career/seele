//! End-to-end search tests with a realistic fixture (~20 observations).
//!
//! Validates that the integration of storage + embedder + RRF + boost +
//! annotations behaves correctly with a populated DB. Specific behaviors of
//! single features live in `hybrid_search.rs`; this file is about
//! cross-feature interactions.

mod common;

use common::populated_20;
use seele_search::{AnnotationKind, SearchQuery};

#[test]
fn e2e_search_finds_specific_observation_in_dev_zen() {
    let p = populated_20();
    let hits =
        p.fx.engine
            .search(SearchQuery {
                text: "pgbouncer pooling".into(),
                project: Some("dev-zen".into()),
                ..Default::default()
            })
            .unwrap();
    // Both winner and loser have the same content; either may rank first
    // under FakeEmbedder. What matters is that at least one of them is
    // returned and no observation from another project leaks through.
    assert!(!hits.is_empty());
    for h in &hits {
        assert_eq!(h.observation.project.as_deref(), Some("dev-zen"));
    }
    let returned_ids: Vec<_> = hits.iter().map(|h| h.observation.id).collect();
    assert!(returned_ids.contains(&p.winner_id) || returned_ids.contains(&p.loser_id));
}

#[test]
fn e2e_purist_excluded_by_default_with_real_volume() {
    let p = populated_20();
    let hits =
        p.fx.engine
            .search(SearchQuery {
                text: "principles".into(),
                project: Some("mnema".into()),
                limit: Some(50),
                ..Default::default()
            })
            .unwrap();
    for h in &hits {
        let meta = h
            .observation
            .metadata
            .get("context_mode")
            .and_then(|v| v.as_str());
        assert_ne!(meta, Some("purist"), "purist must be excluded by default");
    }
}

#[test]
fn e2e_purist_included_on_opt_in_with_real_volume() {
    let p = populated_20();
    let hits =
        p.fx.engine
            .search(SearchQuery {
                text: "principles".into(),
                project: Some("mnema".into()),
                include_purist: true,
                limit: Some(50),
                ..Default::default()
            })
            .unwrap();
    let purist_count = hits
        .iter()
        .filter(|h| {
            h.observation
                .metadata
                .get("context_mode")
                .and_then(|v| v.as_str())
                == Some("purist")
        })
        .count();
    assert!(purist_count > 0, "opt-in must surface purist hits");
}

#[test]
fn e2e_soft_deleted_excluded_consistently() {
    let p = populated_20();
    // Search for the marker we wrote in soft-deleted content.
    let hits =
        p.fx.engine
            .search(SearchQuery {
                text: "cleanup tombstone".into(),
                project: Some("dev-zen".into()),
                include_purist: true,
                limit: Some(50),
                ..Default::default()
            })
            .unwrap();
    for h in &hits {
        assert!(
            h.observation.deleted_at.is_none(),
            "soft-deleted must never appear"
        );
    }
}

#[test]
fn e2e_annotations_in_real_dataset() {
    let p = populated_20();
    let hits =
        p.fx.engine
            .search(SearchQuery {
                text: "pgbouncer pooling".into(),
                project: Some("dev-zen".into()),
                include_annotations: true,
                limit: Some(10),
                ..Default::default()
            })
            .unwrap();
    let by_id: std::collections::HashMap<_, _> =
        hits.iter().map(|h| (h.observation.id, h)).collect();
    let winner = by_id.get(&p.winner_id).expect("winner in hits");
    let loser = by_id.get(&p.loser_id).expect("loser in hits");
    assert!(winner
        .annotations
        .iter()
        .any(|a| a.kind == AnnotationKind::Supersedes && a.other_id == p.loser_id));
    assert!(loser
        .annotations
        .iter()
        .any(|a| a.kind == AnnotationKind::SupersededBy && a.other_id == p.winner_id));
}

#[test]
fn e2e_annotations_contested_for_judged_conflict() {
    let p = populated_20();
    let hits =
        p.fx.engine
            .search(SearchQuery {
                text: "schema naming".into(),
                project: Some("dev-zen".into()),
                include_annotations: true,
                limit: Some(20),
                ..Default::default()
            })
            .unwrap();
    let by_id: std::collections::HashMap<_, _> =
        hits.iter().map(|h| (h.observation.id, h)).collect();
    let a = by_id.get(&p.schema_a_id).expect("schema_a in hits");
    let b = by_id.get(&p.schema_b_id).expect("schema_b in hits");
    assert!(a
        .annotations
        .iter()
        .any(|x| x.kind == AnnotationKind::ContestedBy));
    assert!(b
        .annotations
        .iter()
        .any(|x| x.kind == AnnotationKind::ContestedBy));
}

#[test]
fn e2e_empty_query_lists_recent_in_project() {
    let p = populated_20();
    let hits =
        p.fx.engine
            .search(SearchQuery {
                text: "".into(),
                project: Some("mnema".into()),
                limit: Some(5),
                ..Default::default()
            })
            .unwrap();
    assert_eq!(hits.len(), 5);
    for h in &hits {
        assert_eq!(h.observation.project.as_deref(), Some("mnema"));
    }
    // Empty-query hits have score=0 and no FTS/vec ranks.
    for h in &hits {
        assert_eq!(h.score, 0.0);
        assert!(h.fts_rank.is_none());
        assert!(h.vec_rank.is_none());
    }
}
