//! Sessions CRUD: start/end/abort/get/list lifecycle.

use seele_core::session::SessionStatus;
use seele_storage::{init_db, SessionFilter, SessionInput, SessionStore};
use tempfile::TempDir;

fn fresh() -> (TempDir, SessionStore) {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let store = SessionStore::new(pool);
    (td, store)
}

#[test]
fn start_creates_active_session() {
    let (_td, store) = fresh();
    let s = store
        .start(SessionInput {
            project: "rangi-moana".into(),
            directory: Some("/repo/x".into()),
        })
        .unwrap();
    assert_eq!(s.status, SessionStatus::Active);
    assert_eq!(s.project, "rangi-moana");
    assert!(s.ended_at.is_none());
    assert!(s.summary.is_none());
}

#[test]
fn end_closes_with_summary() {
    let (_td, store) = fresh();
    let s = store
        .start(SessionInput {
            project: "p".into(),
            directory: None,
        })
        .unwrap();
    store.end(s.id, Some("done".into())).unwrap();

    let got = store.get(s.id).unwrap().expect("must exist");
    assert_eq!(got.status, SessionStatus::Ended);
    assert_eq!(got.summary.as_deref(), Some("done"));
    assert!(got.ended_at.is_some());
}

#[test]
fn end_idempotency_rejects_already_ended() {
    let (_td, store) = fresh();
    let s = store
        .start(SessionInput {
            project: "p".into(),
            directory: None,
        })
        .unwrap();
    store.end(s.id, None).unwrap();
    let err = store.end(s.id, None).unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[test]
fn abort_marks_aborted() {
    let (_td, store) = fresh();
    let s = store
        .start(SessionInput {
            project: "p".into(),
            directory: None,
        })
        .unwrap();
    store.abort(s.id).unwrap();
    let got = store.get(s.id).unwrap().unwrap();
    assert_eq!(got.status, SessionStatus::Aborted);
}

#[test]
fn get_unknown_returns_none() {
    let (_td, store) = fresh();
    let unknown = seele_core::id::SeeleId::new();
    assert!(store.get(unknown).unwrap().is_none());
}

#[test]
fn list_filters_by_project_and_status() {
    let (_td, store) = fresh();
    let _ = store
        .start(SessionInput {
            project: "a".into(),
            directory: None,
        })
        .unwrap();
    let s2 = store
        .start(SessionInput {
            project: "a".into(),
            directory: None,
        })
        .unwrap();
    let _ = store
        .start(SessionInput {
            project: "b".into(),
            directory: None,
        })
        .unwrap();
    store.end(s2.id, None).unwrap();

    let actives_a = store
        .list(SessionFilter {
            project: Some("a".into()),
            status: Some(SessionStatus::Active),
            limit: None,
        })
        .unwrap();
    assert_eq!(actives_a.len(), 1);
    assert_eq!(actives_a[0].project, "a");
    assert_eq!(actives_a[0].status, SessionStatus::Active);

    let all_a = store
        .list(SessionFilter {
            project: Some("a".into()),
            status: None,
            limit: None,
        })
        .unwrap();
    assert_eq!(all_a.len(), 2);
}

#[test]
fn list_respects_limit_and_orders_desc() {
    let (_td, store) = fresh();
    for _ in 0..5 {
        let _ = store
            .start(SessionInput {
                project: "p".into(),
                directory: None,
            })
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let three = store
        .list(SessionFilter {
            project: Some("p".into()),
            status: None,
            limit: Some(3),
        })
        .unwrap();
    assert_eq!(three.len(), 3);
    assert!(three[0].started_at >= three[1].started_at);
    assert!(three[1].started_at >= three[2].started_at);
}
