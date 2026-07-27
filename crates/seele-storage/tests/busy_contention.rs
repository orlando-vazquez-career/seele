//! Write contention (T-01): two writers on one DB file must never surface
//! "database is locked" to the caller. `PRAGMA busy_timeout` absorbs short
//! lock waits; the single retry in the write path covers what the busy
//! handler cannot retry (a stale WAL snapshot, SQLITE_BUSY_SNAPSHOT).

use std::sync::Barrier;
use std::time::Duration;

use rusqlite::TransactionBehavior;
use seele_core::memory::{ObservationType, Scope};
use seele_core::metadata::Metadata;
use seele_storage::{init_db, ObservationStore, SaveInput};
use serde_json::json;
use tempfile::TempDir;

fn input(tag: &str) -> SaveInput {
    SaveInput {
        session_id: None,
        kind: ObservationType::Decision,
        title: format!("busy {tag}"),
        content: format!("content {tag}"),
        tool_name: None,
        project: Some("contention".into()),
        scope: Scope::Project,
        topic_key: None,
        metadata: Metadata::from_value(json!({})),
    }
}

/// A second writer (raw connection, no busy_timeout — the "other process")
/// holds the write lock for 300 ms while the store saves. Without the fix
/// the save fails immediately with "database is locked".
#[test]
fn busy_held_write_lock_does_not_fail_save() {
    let td = TempDir::new().unwrap();
    let db_path = td.path().join("seele.db");
    let store = ObservationStore::new(init_db(&db_path).unwrap());

    let mut holder = rusqlite::Connection::open(&db_path).unwrap();
    let tx = holder
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    // A real write so the tx cannot be elided by SQLite.
    tx.execute_batch("PRAGMA user_version = 1").unwrap();

    std::thread::scope(|s| {
        s.spawn(|| {
            store
                .save(input("held-lock"))
                .expect("save must wait out a briefly-held write lock");
        });
        std::thread::sleep(Duration::from_millis(300));
        tx.commit().unwrap();
    });
}

/// Two pools (two "processes") × 50 saves each, started on a barrier so the
/// first writes collide. Without busy_timeout the losing side gets
/// "database is locked"; with it every save lands.
#[test]
fn busy_concurrent_saves_from_two_pools() {
    let td = TempDir::new().unwrap();
    let db_path = td.path().join("seele.db");
    let store_a = ObservationStore::new(init_db(&db_path).unwrap());
    let store_b = ObservationStore::new(init_db(&db_path).unwrap());
    let barrier = Barrier::new(2);

    std::thread::scope(|s| {
        for (worker, store) in [&store_a, &store_b].into_iter().enumerate() {
            let barrier = &barrier;
            s.spawn(move || {
                barrier.wait();
                for i in 0..50 {
                    store
                        .save(input(&format!("w{worker}-{i}")))
                        .expect("concurrent saves must not see database is locked");
                }
            });
        }
    });

    assert_eq!(
        store_a.count_active().unwrap(),
        100,
        "every save from both writers must land exactly once"
    );
}
