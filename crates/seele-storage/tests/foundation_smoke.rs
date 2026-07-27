//! End-to-end smoke for foundation: init_db creates a usable DB with the
//! v0.1.0 schema, vec0 loaded, and FTS5 ready.

use seele_storage::init_db;
use tempfile::TempDir;

const EXPECTED_TABLES: &[&str] = &[
    "sessions",
    "observations",
    "links",
    "memory_relations",
    "sync_chunks",
];

#[test]
fn init_db_applies_full_schema() {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).expect("init_db must succeed");
    let conn = pool.get().expect("checkout");

    for &t in EXPECTED_TABLES {
        let exists: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [t],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "table {t} must exist");
    }

    // FTS5 virtual tables (registered as 'table' in sqlite_master).
    let vt = "observations_fts";
    let exists: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name=?1",
            [vt],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(exists, 1, "fts {vt} must exist");

    // vec0 virtual table.
    let vec_exists: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='observations_vec'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(vec_exists, 1);

    // Triggers.
    for tr in ["observations_ai", "observations_ad", "observations_au"] {
        let exists: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='trigger' AND name=?1",
                [tr],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "trigger {tr} must exist");
    }
}

#[test]
fn init_db_drops_user_prompts_pipeline() {
    // ADR-16 (decision 2): `user_prompts` + `prompts_fts` are dropped by
    // V003 — the pipeline was write-dead in production and the FTS was
    // inert (no content_rowid, no sync triggers). Fresh DBs run
    // V001→V003, so the tables must be gone right after init.
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).expect("init_db must succeed");
    let conn = pool.get().expect("checkout");

    // Base table, FTS virtual table, and every FTS5 shadow table.
    let leftovers: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master \
             WHERE name='user_prompts' OR name LIKE 'prompts_fts%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        leftovers, 0,
        "user_prompts and prompts_fts (incl. shadows) must be dropped"
    );
}

#[test]
fn init_db_is_idempotent() {
    let td = TempDir::new().unwrap();
    let path = td.path().join("seele.db");
    let _ = init_db(&path).unwrap();
    // Re-running init_db on the same file is a no-op (refinery + WAL handle it).
    let _ = init_db(&path).unwrap();
}

#[test]
fn vec0_basic_operations_work() {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let conn = pool.get().unwrap();

    // INSERT into observations_vec with a 384-dim vector blob (1.5KB).
    let mut buf = Vec::with_capacity(384 * 4);
    for i in 0..384u32 {
        let f = (i as f32) / 384.0;
        buf.extend_from_slice(&f.to_le_bytes());
    }

    conn.execute(
        "INSERT INTO observations_vec(rowid, embedding) VALUES (?1, ?2)",
        rusqlite::params![1_i64, buf],
    )
    .unwrap();

    let count: i64 = conn
        .query_row("SELECT count(*) FROM observations_vec", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn fts5_basic_operations_work() {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let conn = pool.get().unwrap();

    // Insert observation; FTS row should be populated by trigger.
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO observations(id, int_id, type, title, content, last_seen_at, created_at, updated_at) \
         VALUES ('01HXZ0000000000000000ABCDE', 12345, 'decision', 'gp design', 'about Postgres tuning', ?1, ?1, ?1)",
        [now],
    )
    .unwrap();

    let hit: i64 = conn
        .query_row(
            "SELECT count(*) FROM observations_fts WHERE observations_fts MATCH 'postgres'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(hit, 1);
}
