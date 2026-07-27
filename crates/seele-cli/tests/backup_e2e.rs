//! T-11 — E2E tests for `seele backup <destino>` (VACUUM INTO) and for
//! the FTS health check that `seele doctor` gained in the same task
//! (`--fix` runs FTS5 'optimize'). Each test spawns the real `seele`
//! binary against a fresh tempdir DB; DB poking goes through
//! `seele_storage::init_db` connections (vec0 loaded) or bare rusqlite
//! when only plain tables / FTS5 are read.

use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

const SEELE_BIN: &str = env!("CARGO_BIN_EXE_seele");

fn run(args: &[&str]) -> (String, String, std::process::ExitStatus) {
    // Force FakeEmbedder so tests never try to download the ONNX model
    // (~90 MB on first run) and stay deterministic in CI.
    let out = Command::new(SEELE_BIN)
        .env("SEELE_FAKE_EMBEDDER", "1")
        .args(args)
        .output()
        .expect("spawn");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status,
    )
}

fn run_json(args: &[&str]) -> Value {
    let (stdout, stderr, status) = run(args);
    assert!(status.success(), "command failed: {args:?}\n{stderr}");
    let v: Value = serde_json::from_str(&stdout).expect("json envelope");
    assert_eq!(v["ok"], true, "envelope ok flag: {stdout}");
    v
}

fn save_one(db: &str, title: &str, project: &str) -> String {
    let v = run_json(&[
        "--db",
        db,
        "--json",
        "save",
        title,
        &format!("content of {title}"),
        "--project",
        project,
    ]);
    v["data"]["id"].as_str().expect("save id").to_string()
}

/// Run `f` against the test DB with the vec0 extension loaded.
fn with_db<R>(db: &str, f: impl FnOnce(&rusqlite::Connection) -> R) -> R {
    let pool = seele_storage::init_db(db).expect("init db");
    let conn = pool.get().expect("checkout");
    f(&conn)
}

#[test]
fn backup_writes_consistent_copy_and_observations_are_counted() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    let dest = td.path().join("backup.db");

    save_one(db, "alpha", "p");
    save_one(db, "beta", "p");
    save_one(db, "gamma", "p");

    let v = run_json(&["--db", db, "--json", "backup", dest.to_str().unwrap()]);
    assert_eq!(v["data"]["destination"], dest.to_str().unwrap());
    assert!(
        v["data"]["bytes"].as_u64().unwrap() > 0,
        "copy is non-empty"
    );

    // Open the backup with a bare connection (FTS5 is compiled into the
    // bundled SQLite; only vec0 needs the loader) and count rows: the
    // copy must carry observations and their FTS index entries.
    let conn = rusqlite::Connection::open(&dest).expect("open backup");
    let observations: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM observations WHERE deleted_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(observations, 3, "backup carries all observations");
    let fts_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM observations_fts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fts_rows, 3, "backup carries the FTS index content");
    // Point-in-time consistency: a MATCH query resolves against the copy.
    let hits: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM observations_fts WHERE observations_fts MATCH 'beta'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(hits, 1, "FTS queries work on the backup");
}

#[test]
fn backup_refuses_to_overwrite_an_existing_destination() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    let dest = td.path().join("backup.db");

    save_one(db, "one", "p");
    let dest_arg = dest.to_str().unwrap();
    run_json(&["--db", db, "--json", "backup", dest_arg]);

    // Second run against the same destination must fail loudly instead
    // of clobbering a previous backup.
    let (stdout, stderr, status) = run(&["--db", db, "--json", "backup", dest_arg]);
    assert!(!status.success(), "overwrite must fail: {stdout}");
    let v: Value = serde_json::from_str(&stdout).expect("json error envelope");
    assert_eq!(v["ok"], false);
    assert!(
        v["error"].as_str().unwrap().contains("already exists"),
        "error names the cause: {stdout} {stderr}"
    );
}

#[test]
fn backup_succeeds_while_a_wal_reader_holds_a_snapshot() {
    // `seele serve`/`seele mcp` hold long-lived connections on the same
    // WAL database. Under WAL a reader never blocks VACUUM INTO (it is a
    // plain read transaction), so the backup must succeed and stay
    // consistent even with a live reader pinning an old snapshot.
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    let dest = td.path().join("backup.db");

    save_one(db, "pinned-one", "p");
    save_one(db, "pinned-two", "p");

    let pool = seele_storage::init_db(db).expect("init db");
    let reader = pool.get().expect("checkout");
    let snapshot = reader.unchecked_transaction().expect("read txn");
    let pinned: i64 = snapshot
        .query_row("SELECT COUNT(*) FROM observations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(pinned, 2);

    let v = run_json(&["--db", db, "--json", "backup", dest.to_str().unwrap()]);
    assert!(v["data"]["bytes"].as_u64().unwrap() > 0);

    let conn = rusqlite::Connection::open(&dest).expect("open backup");
    let copied: i64 = conn
        .query_row("SELECT COUNT(*) FROM observations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(copied, pinned, "backup matches the pinned snapshot");
    drop(snapshot);
}

#[test]
fn doctor_reports_fts_health_and_fix_merges_segments() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();

    // Fresh DB: empty index, no remediation recommended.
    let clean = run_json(&["--db", db, "--json", "doctor"]);
    assert_eq!(clean["data"]["fts"]["docs"], 0);
    assert_eq!(clean["data"]["fts"]["segments"], 0);
    assert_eq!(clean["data"]["fts"]["optimize_recommended"], false);
    assert_eq!(clean["data"]["fts"]["optimize_applied"], false);

    // Fragment the index deterministically: with automerge disabled,
    // every committed insert leaves one on-disk segment. Ten single-row
    // commits push the segment count past the recommendation threshold.
    // Real observation rows are needed: `observations_fts` is an
    // external-content table, so its row count joins back to
    // `observations` — phantom index entries would read as 0 docs.
    with_db(db, |c| {
        c.execute(
            "INSERT INTO observations_fts(observations_fts, rank) VALUES('automerge', 0)",
            [],
        )
        .unwrap();
        for i in 1..=10i64 {
            c.execute(
                "INSERT INTO observations (id, int_id, type, title, content, \
                 last_seen_at, created_at, updated_at) \
                 VALUES (?1, ?2, 'fact', 'frag title', 'frag content', 0, 0, 0)",
                rusqlite::params![format!("frag-{i:02}"), i],
            )
            .unwrap();
        }
    });

    let dirty = run_json(&["--db", db, "--json", "doctor"]);
    assert_eq!(dirty["data"]["fts"]["docs"], 10);
    assert_eq!(dirty["data"]["fts"]["segments"], 10);
    assert_eq!(dirty["data"]["fts"]["optimize_recommended"], true);
    assert_eq!(
        dirty["data"]["fts"]["optimize_applied"], false,
        "plain doctor never rewrites the index"
    );

    // --fix is the opt-in remediation: it runs FTS5 'optimize' and
    // reports the segment count before and after.
    let fixed = run_json(&["--db", db, "--json", "doctor", "--fix"]);
    assert_eq!(fixed["data"]["fts"]["optimize_applied"], true);
    assert_eq!(fixed["data"]["fts"]["segments"], 10, "pre-fix count kept");
    assert_eq!(fixed["data"]["fts"]["segments_after"], 1);

    // After the fix the index is back to a single segment.
    let after = run_json(&["--db", db, "--json", "doctor"]);
    assert_eq!(after["data"]["fts"]["segments"], 1);
    assert_eq!(after["data"]["fts"]["optimize_recommended"], false);
}
