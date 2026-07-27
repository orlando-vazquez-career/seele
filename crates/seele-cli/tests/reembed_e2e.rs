//! T-08 — E2E tests for `seele embedder reembed-all` and for
//! `seele import from-engram --re-embed`. Each test spawns the real
//! `seele` binary against a fresh tempdir DB and sabotages/inspects
//! the DB through `seele_storage::init_db` connections, which load
//! the vec0 extension (a bare rusqlite connection cannot see the
//! `observations_vec` virtual table).

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

fn vec_count(db: &str) -> i64 {
    with_db(db, |c| {
        c.query_row("SELECT COUNT(*) FROM observations_vec", [], |r| r.get(0))
            .unwrap()
    })
}

#[test]
fn reembed_all_dry_run_apply_doctor_clean_and_idempotent() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();

    save_one(db, "one", "p");
    save_one(db, "two", "p");
    save_one(db, "three", "p");

    // Sabotage three different ways: vector gone, provenance gone,
    // provenance pointing at another model.
    with_db(db, |c| {
        c.execute(
            "DELETE FROM observations_vec WHERE rowid = \
             (SELECT int_id FROM observations WHERE title = 'one')",
            [],
        )
        .unwrap();
        c.execute(
            "DELETE FROM embeddings_meta WHERE observation_id = \
             (SELECT id FROM observations WHERE title = 'two')",
            [],
        )
        .unwrap();
        c.execute(
            "UPDATE embeddings_meta SET model_id = 'other/model' \
             WHERE observation_id = \
             (SELECT id FROM observations WHERE title = 'three')",
            [],
        )
        .unwrap();
    });
    assert_eq!(vec_count(db), 2, "one vector deleted");

    // --dry-run counts but writes nothing.
    let dry = run_json(&["--db", db, "--json", "embedder", "reembed-all", "--dry-run"]);
    assert_eq!(dry["data"]["candidates"], 3);
    assert_eq!(dry["data"]["reembedded"], 0);
    assert_eq!(dry["data"]["dry_run"], true);
    assert_eq!(vec_count(db), 2, "dry-run must not write");
    let foreign: i64 = with_db(db, |c| {
        c.query_row(
            "SELECT COUNT(*) FROM embeddings_meta WHERE model_id = 'other/model'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    });
    assert_eq!(foreign, 1, "dry-run leaves provenance untouched");

    // Apply: three candidates fixed, doctor goes clean.
    let applied = run_json(&["--db", db, "--json", "embedder", "reembed-all"]);
    assert_eq!(applied["data"]["candidates"], 3);
    assert_eq!(applied["data"]["reembedded"], 3);
    assert_eq!(applied["data"]["skipped"], 0);
    assert_eq!(vec_count(db), 3, "every active row has a vector");
    let non_fake: i64 = with_db(db, |c| {
        c.query_row(
            "SELECT COUNT(*) FROM embeddings_meta \
             WHERE model_id <> 'seele/fake-embedder'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    });
    assert_eq!(non_fake, 0, "all provenance rows name the active model");

    let doctor = run_json(&["--db", db, "--json", "doctor"]);
    assert_eq!(doctor["data"]["observations_active_without_vector"], 0);
    assert!(
        doctor["data"]["embedding_mix_warning"].is_null(),
        "mix warning cleared: {}",
        doctor["data"]["embedding_mix_warning"]
    );

    // Idempotent: second run finds no work.
    let second = run_json(&["--db", db, "--json", "embedder", "reembed-all"]);
    assert_eq!(second["data"]["candidates"], 0);
    assert_eq!(second["data"]["reembedded"], 0);
    assert_eq!(second["data"]["skipped"], 0);
}

#[test]
fn reembed_all_project_flag_scopes_the_pass() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();

    save_one(db, "px-one", "px");
    save_one(db, "py-one", "py");
    with_db(db, |c| {
        c.execute("DELETE FROM observations_vec", []).unwrap();
        c.execute("DELETE FROM embeddings_meta", []).unwrap();
    });
    assert_eq!(vec_count(db), 0);

    let scoped = run_json(&[
        "--db",
        db,
        "--json",
        "embedder",
        "reembed-all",
        "--project",
        "px",
    ]);
    assert_eq!(scoped["data"]["candidates"], 1);
    assert_eq!(scoped["data"]["reembedded"], 1);
    assert_eq!(vec_count(db), 1, "only the px row was re-embedded");

    // Human output smoke: the summary names the three counters.
    let (stdout, stderr, status) = run(&["--db", db, "embedder", "reembed-all"]);
    assert!(status.success(), "reembed failed: {stderr}");
    assert!(stdout.contains("candidates: 1"), "stdout: {stdout}");
    assert!(stdout.contains("re-embedded: 1"), "stdout: {stdout}");
    assert!(stdout.contains("skipped: 0"), "stdout: {stdout}");
    assert_eq!(vec_count(db), 2);
}

#[test]
fn import_from_engram_reembed_pass_embeds_imported_rows() {
    let td = TempDir::new().unwrap();
    let src = td.path().join("engram.db");
    let dst = td.path().join("seele.db");
    let dst = dst.to_str().unwrap();

    // Minimal ENGRAM fixture: the importer only requires
    // memories(id, body, metadata, created_at).
    let conn = rusqlite::Connection::open(&src).unwrap();
    conn.execute_batch(
        "CREATE TABLE memories (id TEXT PRIMARY KEY, body TEXT, metadata TEXT, created_at INTEGER);",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO memories VALUES ('01ARZ3NDEKTSV4RRFFQ69G5FAV', \
         'cuerpo engram uno', '{}', 1700000000000)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO memories VALUES ('01BX5ZZKBKACTAV9WEVGEMMVRZ', \
         'cuerpo engram dos', '{}', 1700000001000)",
        [],
    )
    .unwrap();
    drop(conn);
    let src_arg = src.to_str().unwrap();

    // --re-embed after a dry-run import is a no-op (nothing was written).
    let dry = run_json(&[
        "--db",
        dst,
        "--json",
        "import",
        "from-engram",
        src_arg,
        "--re-embed",
        "--dry-run",
    ]);
    assert_eq!(dry["data"]["rows_inserted"], 0);
    assert!(
        dry["data"].get("reembed").is_none(),
        "no re-embed pass on dry-run imports: {}",
        dry["data"]
    );

    // Real import: the two rows land without vectors, and --re-embed
    // fixes exactly them.
    let applied = run_json(&[
        "--db",
        dst,
        "--json",
        "import",
        "from-engram",
        src_arg,
        "--re-embed",
    ]);
    assert_eq!(applied["data"]["rows_inserted"], 2);
    assert_eq!(applied["data"]["reembed"]["candidates"], 2);
    assert_eq!(applied["data"]["reembed"]["reembedded"], 2);
    assert_eq!(applied["data"]["reembed"]["skipped"], 0);
    assert_eq!(vec_count(dst), 2, "imported rows got vectors");

    // Re-running the whole thing is idempotent on both axes: nothing
    // re-imported, no re-embed work left.
    let again = run_json(&[
        "--db",
        dst,
        "--json",
        "import",
        "from-engram",
        src_arg,
        "--re-embed",
    ]);
    assert_eq!(again["data"]["rows_inserted"], 0);
    assert_eq!(again["data"]["reembed"]["candidates"], 0);
}
