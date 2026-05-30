//! Migration test for `embeddings_meta` (ADR-14, V002).
//!
//! Verifies the table is created by the migration and is queryable. The
//! backfill only affects observations present at upgrade time, so on a
//! fresh DB the table is empty — that is the invariant asserted here.

use seele_storage::init_db;
use tempfile::TempDir;

#[test]
fn embeddings_meta_exists_and_is_empty_on_fresh_db() {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let conn = pool.get().unwrap();

    // Table exists (V002 applied) and starts empty on a clean DB.
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM embeddings_meta", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0, "V002 must create an initially empty embeddings_meta");

    // Re-opening the same DB re-runs migrations idempotently.
    drop(conn);
    drop(pool);
    let pool2 = init_db(td.path().join("seele.db")).unwrap();
    let conn2 = pool2.get().unwrap();
    let n2: i64 = conn2
        .query_row("SELECT COUNT(*) FROM embeddings_meta", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n2, 0);
}
