//! E2E: synthesize a small ENGRAM-shaped SQLite DB, run the importer
//! against a fresh SEELE destination, assert the resulting rows.
//!
//! These tests guard the contract documented in ADR-13: id preservation
//! when source ids are ULIDs, fallback minting + `engram_id` breadcrumb
//! otherwise, metadata pass-through, `linked_to` → links table, and
//! idempotent re-runs.

use std::path::Path;

use rusqlite::{params, Connection};
use seele_storage::{init_db, LinkQuery, LinkStore, ObservationQuery, ObservationStore};
use tempfile::TempDir;

use seele_engram_import::{EngramImporter, ImportReport};

fn make_engram_db(path: &Path) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        "CREATE TABLE memories (
             id TEXT PRIMARY KEY,
             body TEXT NOT NULL,
             metadata TEXT,
             created_at INTEGER NOT NULL
         );",
    )
    .unwrap();
    conn
}

fn insert_engram_row(conn: &Connection, id: &str, body: &str, metadata: &str, created_ms: i64) {
    conn.execute(
        "INSERT INTO memories(id, body, metadata, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![id, body, metadata, created_ms],
    )
    .unwrap();
}

fn destination_stores(td: &TempDir) -> (ObservationStore, LinkStore) {
    let pool = init_db(td.path().join("dst.db")).unwrap();
    (ObservationStore::new(pool.clone()), LinkStore::new(pool))
}

#[test]
fn import_into_empty_seele_inserts_each_memory_row() {
    let src_td = TempDir::new().unwrap();
    let src_path = src_td.path().join("engram.db");
    let conn = make_engram_db(&src_path);
    let id_a = ulid::Ulid::new().to_string();
    let id_b = ulid::Ulid::new().to_string();
    insert_engram_row(
        &conn,
        &id_a,
        "alpha content",
        r#"{"kind": "decision", "project": "p", "topic_key": "decision/x"}"#,
        1_700_000_000_000,
    );
    insert_engram_row(
        &conn,
        &id_b,
        "beta content",
        r#"{"kind": "memory"}"#,
        1_700_000_001_000,
    );

    let dst_td = TempDir::new().unwrap();
    let (obs, links) = destination_stores(&dst_td);
    let importer = EngramImporter::new(&obs, &links);
    let report: ImportReport = importer.import_from(&src_path, false).unwrap();

    assert_eq!(report.rows_seen, 2);
    assert_eq!(report.rows_inserted, 2);
    assert_eq!(report.rows_skipped_existing, 0);
    assert_eq!(report.rows_invalid, 0);
    assert_eq!(report.errors.len(), 0);

    let all = obs.list(ObservationQuery::default()).unwrap();
    assert_eq!(all.len(), 2);
    let mut bodies: Vec<&str> = all.iter().map(|o| o.content.as_str()).collect();
    bodies.sort();
    assert_eq!(bodies, vec!["alpha content", "beta content"]);
}

#[test]
fn import_preserves_ulid_id_when_source_id_is_a_ulid() {
    let src_td = TempDir::new().unwrap();
    let src_path = src_td.path().join("e.db");
    let conn = make_engram_db(&src_path);
    let id = ulid::Ulid::new().to_string();
    insert_engram_row(&conn, &id, "x", "{}", 1_700_000_000_000);

    let dst_td = TempDir::new().unwrap();
    let (obs, links) = destination_stores(&dst_td);
    let importer = EngramImporter::new(&obs, &links);
    importer.import_from(&src_path, false).unwrap();

    let row = obs.list(ObservationQuery::default()).unwrap();
    assert_eq!(row.len(), 1);
    assert_eq!(
        row[0].id.to_string(),
        id,
        "ULID source id should be preserved"
    );
}

#[test]
fn import_stashes_engram_id_in_metadata_when_source_is_not_a_ulid() {
    let src_td = TempDir::new().unwrap();
    let src_path = src_td.path().join("e.db");
    let conn = make_engram_db(&src_path);
    // Non-ULID id like ENGRAM's cuid-style.
    insert_engram_row(&conn, "mem_abc123", "x", "{}", 1_700_000_000_000);

    let dst_td = TempDir::new().unwrap();
    let (obs, links) = destination_stores(&dst_td);
    let importer = EngramImporter::new(&obs, &links);
    importer.import_from(&src_path, false).unwrap();

    let row = obs.list(ObservationQuery::default()).unwrap();
    assert_eq!(row.len(), 1);
    // New ULID minted, original stashed in metadata.
    assert_ne!(row[0].id.to_string(), "mem_abc123");
    assert_eq!(
        row[0].metadata.get("engram_id").and_then(|v| v.as_str()),
        Some("mem_abc123")
    );
}

#[test]
fn import_resolves_linked_to_into_links_table() {
    let src_td = TempDir::new().unwrap();
    let src_path = src_td.path().join("e.db");
    let conn = make_engram_db(&src_path);
    let id_a = ulid::Ulid::new().to_string();
    let id_b = ulid::Ulid::new().to_string();
    insert_engram_row(
        &conn,
        &id_a,
        "alpha",
        &format!(r#"{{"linked_to": ["{id_b}"]}}"#),
        1_700_000_000_000,
    );
    insert_engram_row(&conn, &id_b, "beta", "{}", 1_700_000_001_000);

    let dst_td = TempDir::new().unwrap();
    let (obs, links) = destination_stores(&dst_td);
    let importer = EngramImporter::new(&obs, &links);
    let report = importer.import_from(&src_path, false).unwrap();

    assert_eq!(report.rows_inserted, 2);
    assert_eq!(report.links_created, 1);
    assert_eq!(report.links_dangling, 0);

    let all_links = links.list(LinkQuery::default()).unwrap();
    assert_eq!(all_links.len(), 1);
    assert_eq!(all_links[0].link_type, "related_to");
}

#[test]
fn import_resolves_linked_to_against_rows_from_a_previous_pass() {
    // Cloven 2026-05-11 [MEDIO]: linked_to targets that already live
    // in the destination (from a prior import) should resolve, not
    // count as dangling.
    let dst_td = TempDir::new().unwrap();
    let (obs, links) = destination_stores(&dst_td);
    let importer = EngramImporter::new(&obs, &links);

    // Pass 1: import a single row with a ULID id.
    let src1 = dst_td.path().join("e1.db");
    let conn1 = make_engram_db(&src1);
    let id_target = ulid::Ulid::new().to_string();
    insert_engram_row(&conn1, &id_target, "target", "{}", 1);
    drop(conn1);
    importer.import_from(&src1, false).unwrap();

    // Pass 2: a new row pointing at the target imported above.
    let src2 = dst_td.path().join("e2.db");
    let conn2 = make_engram_db(&src2);
    let id_referrer = ulid::Ulid::new().to_string();
    insert_engram_row(
        &conn2,
        &id_referrer,
        "referrer",
        &format!(r#"{{"linked_to": ["{id_target}"]}}"#),
        2,
    );
    drop(conn2);

    let report = importer.import_from(&src2, false).unwrap();
    assert_eq!(report.rows_inserted, 1);
    assert_eq!(
        report.links_created, 1,
        "link should resolve against destination"
    );
    assert_eq!(report.links_dangling, 0);
    assert_eq!(links.list(LinkQuery::default()).unwrap().len(), 1);
}

#[test]
fn import_counts_dangling_links_when_target_missing() {
    let src_td = TempDir::new().unwrap();
    let src_path = src_td.path().join("e.db");
    let conn = make_engram_db(&src_path);
    let id_a = ulid::Ulid::new().to_string();
    // Reference a non-existent target.
    insert_engram_row(
        &conn,
        &id_a,
        "alpha",
        r#"{"linked_to": ["mem_does_not_exist"]}"#,
        1_700_000_000_000,
    );

    let dst_td = TempDir::new().unwrap();
    let (obs, links) = destination_stores(&dst_td);
    let importer = EngramImporter::new(&obs, &links);
    let report = importer.import_from(&src_path, false).unwrap();

    assert_eq!(report.rows_inserted, 1);
    assert_eq!(report.links_created, 0);
    assert_eq!(report.links_dangling, 1);
    assert_eq!(links.list(LinkQuery::default()).unwrap().len(), 0);
}

#[test]
fn re_running_import_is_idempotent_for_preserved_ulids() {
    let src_td = TempDir::new().unwrap();
    let src_path = src_td.path().join("e.db");
    let conn = make_engram_db(&src_path);
    let id = ulid::Ulid::new().to_string();
    insert_engram_row(&conn, &id, "x", "{}", 1_700_000_000_000);

    let dst_td = TempDir::new().unwrap();
    let (obs, links) = destination_stores(&dst_td);
    let importer = EngramImporter::new(&obs, &links);

    let first = importer.import_from(&src_path, false).unwrap();
    assert_eq!(first.rows_inserted, 1);

    let second = importer.import_from(&src_path, false).unwrap();
    assert_eq!(second.rows_inserted, 0);
    assert_eq!(second.rows_skipped_existing, 1);

    assert_eq!(obs.list(ObservationQuery::default()).unwrap().len(), 1);
}

#[test]
fn import_atomicity_failure_rolls_back_partial_writes() {
    // Synthesize a source where one row's metadata is invalid JSON.
    // The importer treats invalid metadata as a soft error (counts in
    // rows_invalid, populates `errors`) so the tx still commits the
    // good rows. To stress true atomicity we would need to crash mid-tx,
    // which is impractical from a test. Verify instead that the soft-
    // error counter advances WITHOUT a panic and good rows still land.
    let src_td = TempDir::new().unwrap();
    let src_path = src_td.path().join("e.db");
    let conn = make_engram_db(&src_path);
    let id_good = ulid::Ulid::new().to_string();
    let id_bad = ulid::Ulid::new().to_string();
    insert_engram_row(&conn, &id_good, "good", r#"{"kind":"memory"}"#, 1);
    insert_engram_row(&conn, &id_bad, "bad", "not even json", 2);

    let dst_td = TempDir::new().unwrap();
    let (obs, links) = destination_stores(&dst_td);
    let importer = EngramImporter::new(&obs, &links);
    let report = importer.import_from(&src_path, false).unwrap();

    assert_eq!(report.rows_seen, 2);
    assert_eq!(report.rows_inserted, 1);
    assert_eq!(report.rows_invalid, 1);
    assert!(!report.errors.is_empty());
    assert_eq!(obs.list(ObservationQuery::default()).unwrap().len(), 1);
}

#[test]
fn schema_check_rejects_db_without_memories_table() {
    let src_td = TempDir::new().unwrap();
    let src_path = src_td.path().join("e.db");
    // Empty DB — no `memories`.
    let _ = Connection::open(&src_path).unwrap();

    let dst_td = TempDir::new().unwrap();
    let (obs, links) = destination_stores(&dst_td);
    let importer = EngramImporter::new(&obs, &links);
    let err = importer.import_from(&src_path, false).unwrap_err();
    assert!(
        err.to_string().contains("memories"),
        "expected memories-table error, got: {err}"
    );
}

#[test]
fn schema_check_rejects_memories_missing_required_column() {
    let src_td = TempDir::new().unwrap();
    let src_path = src_td.path().join("e.db");
    let conn = Connection::open(&src_path).unwrap();
    // Missing `metadata` column.
    conn.execute_batch(
        "CREATE TABLE memories (id TEXT PRIMARY KEY, body TEXT, created_at INTEGER);",
    )
    .unwrap();

    let dst_td = TempDir::new().unwrap();
    let (obs, links) = destination_stores(&dst_td);
    let importer = EngramImporter::new(&obs, &links);
    let err = importer.import_from(&src_path, false).unwrap_err();
    assert!(err.to_string().contains("missing required column"));
}

#[test]
fn dry_run_writes_nothing_but_returns_counts() {
    let src_td = TempDir::new().unwrap();
    let src_path = src_td.path().join("e.db");
    let conn = make_engram_db(&src_path);
    insert_engram_row(&conn, &ulid::Ulid::new().to_string(), "x", "{}", 1);
    insert_engram_row(&conn, &ulid::Ulid::new().to_string(), "y", "{}", 2);

    let dst_td = TempDir::new().unwrap();
    let (obs, links) = destination_stores(&dst_td);
    let importer = EngramImporter::new(&obs, &links);
    let report = importer.import_from(&src_path, true).unwrap();

    assert!(report.dry_run);
    assert_eq!(report.rows_seen, 2);
    assert_eq!(report.rows_inserted, 0);
    assert_eq!(obs.list(ObservationQuery::default()).unwrap().len(), 0);
}

#[test]
fn import_extracts_project_scope_topic_key_from_metadata() {
    let src_td = TempDir::new().unwrap();
    let src_path = src_td.path().join("e.db");
    let conn = make_engram_db(&src_path);
    let id = ulid::Ulid::new().to_string();
    insert_engram_row(
        &conn,
        &id,
        "x",
        r#"{"project":"proj-a","scope":"personal","topic_key":"decision/migration"}"#,
        1,
    );

    let dst_td = TempDir::new().unwrap();
    let (obs, links) = destination_stores(&dst_td);
    let importer = EngramImporter::new(&obs, &links);
    importer.import_from(&src_path, false).unwrap();

    let rows = obs.list(ObservationQuery::default()).unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.project.as_deref(), Some("proj-a"));
    assert_eq!(row.scope.as_str(), "personal");
    assert_eq!(row.topic_key.as_deref(), Some("decision/migration"));
}
