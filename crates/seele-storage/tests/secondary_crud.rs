//! CRUD smoke for links, relations, chunks.

use seele_core::id::SeeleId;
use seele_core::memory::{ObservationType, Scope};
use seele_core::metadata::Metadata;
use seele_core::relation::{JudgmentStatus, RelationKind};
use seele_storage::{
    init_db, ChunkStore, JudgmentInput, LinkInput, LinkQuery, LinkStore, ObservationStore,
    RelationInput, RelationQuery, RelationStore, SaveInput,
};
use serde_json::json;
use tempfile::TempDir;

fn make_two_observations() -> (TempDir, ObservationStore, SeeleId, SeeleId) {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let store = ObservationStore::new(pool);
    let a = store
        .save(SaveInput {
            session_id: None,
            kind: ObservationType::Decision,
            title: "A".into(),
            content: "decision A".into(),
            tool_name: None,
            project: Some("p".into()),
            scope: Scope::Project,
            topic_key: None,
            metadata: Metadata::new(),
        })
        .unwrap()
        .id();
    let b = store
        .save(SaveInput {
            session_id: None,
            kind: ObservationType::Decision,
            title: "B".into(),
            content: "decision B".into(),
            tool_name: None,
            project: Some("p".into()),
            scope: Scope::Project,
            topic_key: None,
            metadata: Metadata::new(),
        })
        .unwrap()
        .id();
    (td, store, a, b)
}

#[test]
fn links_create_list_delete_round_trip() {
    let (td, _store, a, b) = make_two_observations();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let links = LinkStore::new(pool);

    let link = links
        .create(LinkInput {
            from_id: a,
            to_id: b,
            link_type: "supersedes".into(),
            metadata: Metadata::from_value(json!({"note": "test"})),
        })
        .unwrap();
    assert_eq!(link.from_id, a);
    assert_eq!(link.to_id, b);

    let from_a = links
        .list(LinkQuery {
            from_id: Some(a),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(from_a.len(), 1);
    assert_eq!(from_a[0].link_type, "supersedes");

    links.delete(link.id).unwrap();
    let after = links
        .list(LinkQuery {
            from_id: Some(a),
            ..Default::default()
        })
        .unwrap();
    assert!(after.is_empty());
}

#[test]
fn links_unique_constraint_blocks_duplicate_edge() {
    let (td, _store, a, b) = make_two_observations();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let links = LinkStore::new(pool);
    links
        .create(LinkInput {
            from_id: a,
            to_id: b,
            link_type: "related_to".into(),
            metadata: Metadata::new(),
        })
        .unwrap();
    let err = links
        .create(LinkInput {
            from_id: a,
            to_id: b,
            link_type: "related_to".into(),
            metadata: Metadata::new(),
        })
        .unwrap_err();
    assert!(format!("{err}").contains("UNIQUE") || format!("{err}").contains("unique"));
}

#[test]
fn relations_create_judge_lifecycle() {
    let (td, _store, a, b) = make_two_observations();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let rels = RelationStore::new(pool);

    let r = rels
        .create(RelationInput {
            sync_id: "sync_test_1".into(),
            source_id: a,
            target_id: b,
            relation: RelationKind::Supersedes,
            reason: Some("newer wins".into()),
            evidence: None,
            confidence: Some(0.9),
            marked_by_actor: Some("mnema".into()),
            marked_by_kind: Some("agent".into()),
            marked_by_model: Some("opus-4-7".into()),
            session_id: None,
        })
        .unwrap();
    assert_eq!(r.judgment_status, JudgmentStatus::Pending);

    rels.judge(
        r.id,
        JudgmentInput {
            status: JudgmentStatus::Judged,
            reason: Some("verified".into()),
            evidence: Some("PR #42".into()),
            confidence: Some(0.95),
        },
    )
    .unwrap();

    let updated = rels.get(r.id).unwrap().unwrap();
    assert_eq!(updated.judgment_status, JudgmentStatus::Judged);
    assert_eq!(updated.evidence.as_deref(), Some("PR #42"));
    assert!(updated.confidence.unwrap() > 0.9);

    let judged = rels
        .list(RelationQuery {
            status: Some(JudgmentStatus::Judged),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(judged.len(), 1);
}

#[test]
fn chunks_idempotent_mark_imported() {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let chunks = ChunkStore::new(pool);

    assert!(!chunks.was_imported("origin/main", "abc123").unwrap());
    let first = chunks.mark_imported("origin/main", "abc123").unwrap();
    assert!(first, "first mark must report newly recorded");
    let second = chunks.mark_imported("origin/main", "abc123").unwrap();
    assert!(!second, "second mark must report already present");
    assert!(chunks.was_imported("origin/main", "abc123").unwrap());

    let listed = chunks.list_for_target("origin/main").unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].chunk_id, "abc123");
}
