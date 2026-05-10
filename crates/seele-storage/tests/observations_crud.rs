//! Observations CRUD + upsert + dedup integration.

use seele_core::memory::{ObservationType, Scope};
use seele_core::metadata::Metadata;
use seele_storage::{
    init_db, ObservationPatch, ObservationQuery, ObservationStore, SaveInput, SaveOutcome,
};
use serde_json::json;
use tempfile::TempDir;

fn fresh() -> (TempDir, ObservationStore) {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let store = ObservationStore::new(pool);
    (td, store)
}

fn meta(value: serde_json::Value) -> Metadata {
    Metadata::from_value(value)
}

fn input(title: &str, content: &str) -> SaveInput {
    SaveInput {
        session_id: None,
        kind: ObservationType::Decision,
        title: title.into(),
        content: content.into(),
        tool_name: None,
        project: Some("dev-zen".into()),
        scope: Scope::Project,
        topic_key: None,
        metadata: meta(json!({})),
    }
}

#[test]
fn save_creates_new_observation() {
    let (_td, store) = fresh();
    let outcome = store
        .save(input("postgres tuning", "WAL is better than DELETE"))
        .unwrap();
    let SaveOutcome::Created(id) = outcome else {
        panic!("expected Created, got {outcome:?}");
    };
    let got = store.get(id).unwrap().expect("must exist");
    assert_eq!(got.title, "postgres tuning");
    assert_eq!(got.scope, Scope::Project);
    assert_eq!(got.revision_count, 0);
    assert_eq!(got.duplicate_count, 0);
}

#[test]
fn save_strips_private_blocks_in_title_and_content() {
    let (_td, store) = fresh();
    let outcome = store
        .save(SaveInput {
            title: "title <private>secret-name</private> here".into(),
            content: "before <private>password=hunter2</private> after".into(),
            ..input("ignored", "ignored")
        })
        .unwrap();
    let got = store.get(outcome.id()).unwrap().unwrap();
    assert!(!got.title.contains("secret"));
    assert!(!got.content.contains("hunter2"));
}

#[test]
fn topic_key_upsert_increments_revision() {
    let (_td, store) = fresh();
    let first = store
        .save(SaveInput {
            topic_key: Some("axiomatica/postgres".into()),
            ..input("v1", "rev 1")
        })
        .unwrap();
    let second = store
        .save(SaveInput {
            topic_key: Some("axiomatica/postgres".into()),
            ..input("v2", "rev 2")
        })
        .unwrap();
    assert_eq!(first.id(), second.id(), "topic_key should map to same row");
    let SaveOutcome::UpsertedTopic { revision_count, .. } = second else {
        panic!("expected UpsertedTopic, got {second:?}");
    };
    assert_eq!(revision_count, 1);

    let got = store.get(first.id()).unwrap().unwrap();
    assert_eq!(got.title, "v2");
    assert_eq!(got.content, "rev 2");
    assert_eq!(got.revision_count, 1);
}

#[test]
fn dedup_hash_window_increments_duplicate_count() {
    let (_td, store) = fresh();
    let first = store.save(input("title", "same content")).unwrap();
    // Same title + content + project + scope + type = same dedup key.
    let second = store.save(input("title", "  SAME content  ")).unwrap();
    assert_eq!(first.id(), second.id());
    let SaveOutcome::DuplicateMerged {
        duplicate_count, ..
    } = second
    else {
        panic!("expected DuplicateMerged, got {second:?}");
    };
    assert_eq!(duplicate_count, 1);
}

#[test]
fn different_title_with_same_content_is_new_row() {
    let (_td, store) = fresh();
    let a = store.save(input("title-A", "shared body")).unwrap();
    let b = store.save(input("title-B", "shared body")).unwrap();
    assert_ne!(a.id(), b.id());
    assert!(matches!(b, SaveOutcome::Created(_)));
}

#[test]
fn topic_key_isolated_per_scope_and_project() {
    let (_td, store) = fresh();
    let proj_a = store
        .save(SaveInput {
            project: Some("a".into()),
            topic_key: Some("k".into()),
            ..input("t", "ca")
        })
        .unwrap();
    let proj_b = store
        .save(SaveInput {
            project: Some("b".into()),
            topic_key: Some("k".into()),
            ..input("t", "cb")
        })
        .unwrap();
    assert_ne!(proj_a.id(), proj_b.id());

    let personal = store
        .save(SaveInput {
            project: Some("a".into()),
            scope: Scope::Personal,
            topic_key: Some("k".into()),
            ..input("t", "personal")
        })
        .unwrap();
    assert_ne!(proj_a.id(), personal.id());
}

#[test]
fn list_filters_by_project_and_excludes_deleted_by_default() {
    let (_td, store) = fresh();
    let a = store.save(input("t1", "c1")).unwrap();
    let _b = store.save(input("t2", "c2")).unwrap();
    store.soft_delete(a.id()).unwrap();

    let active = store
        .list(ObservationQuery {
            project: Some("dev-zen".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].title, "t2");

    let with_deleted = store
        .list(ObservationQuery {
            project: Some("dev-zen".into()),
            include_deleted: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(with_deleted.len(), 2);
}

#[test]
fn soft_delete_then_restore_round_trip() {
    let (_td, store) = fresh();
    let outcome = store.save(input("t", "c")).unwrap();
    let id = outcome.id();
    store.soft_delete(id).unwrap();
    assert!(store.get(id).unwrap().unwrap().deleted_at.is_some());
    store.restore(id).unwrap();
    assert!(store.get(id).unwrap().unwrap().deleted_at.is_none());
}

#[test]
fn hard_delete_removes_row() {
    let (_td, store) = fresh();
    let outcome = store.save(input("t", "c")).unwrap();
    let id = outcome.id();
    store.hard_delete(id).unwrap();
    assert!(store.get(id).unwrap().is_none());
}

#[test]
fn update_recomputes_normalized_hash() {
    let (_td, store) = fresh();
    let outcome = store.save(input("t", "original")).unwrap();
    let id = outcome.id();
    let before = store.get(id).unwrap().unwrap().normalized_hash.clone();
    store
        .update(
            id,
            ObservationPatch {
                content: Some("updated body".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let after = store.get(id).unwrap().unwrap().normalized_hash.clone();
    assert_ne!(before, after);
}

#[test]
fn metadata_persists_through_save_get_roundtrip() {
    let (_td, store) = fresh();
    let m = meta(json!({"kind": "decision", "domain": "infra", "context_mode": "purist"}));
    let outcome = store
        .save(SaveInput {
            metadata: m.clone(),
            ..input("t", "c")
        })
        .unwrap();
    let got = store.get(outcome.id()).unwrap().unwrap();
    assert_eq!(got.metadata, m);
}
