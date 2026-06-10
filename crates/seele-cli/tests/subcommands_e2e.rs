//! Bloque D.2 — E2E tests for the new clap subcommands. Each test
//! spawns the real `seele` binary against a fresh tempdir DB.

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

fn save_one(db: &str, title: &str, project: &str) -> String {
    let (stdout, _, status) = run(&[
        "--db",
        db,
        "--json",
        "save",
        title,
        "content",
        "--project",
        project,
    ]);
    assert!(status.success(), "save failed: {stdout}");
    let v: Value = serde_json::from_str(&stdout).expect("save json");
    assert_eq!(v["ok"], true, "envelope ok flag: {stdout}");
    v["data"]["id"].as_str().expect("save id").to_string()
}

// -------- save / list / show --------

#[test]
fn save_then_list_round_trip() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();

    save_one(db, "first", "p");
    save_one(db, "second", "p");

    let (stdout, _, status) = run(&["--db", db, "--json", "list", "--project", "p"]);
    assert!(status.success());
    let v: Value = serde_json::from_str(&stdout).unwrap();
    let titles: Vec<&str> = v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"first"));
    assert!(titles.contains(&"second"));
}

#[test]
fn save_then_show_returns_full_observation() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    let id = save_one(db, "alpha", "p");

    let (stdout, _, status) = run(&["--db", db, "--json", "show", &id]);
    assert!(status.success(), "show failed: {stdout}");
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["id"], id);
    assert_eq!(v["data"]["title"], "alpha");
    assert_eq!(v["data"]["content"], "content");
}

// -------- search --------

#[test]
fn search_with_query_returns_hits() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    save_one(db, "deploy notes", "p");

    let (stdout, _, status) = run(&["--db", db, "--json", "search", "deploy"]);
    assert!(status.success(), "search failed: {stdout}");
    let v: Value = serde_json::from_str(&stdout).unwrap();
    let hits = v["data"]["hits"].as_array().unwrap();
    assert!(!hits.is_empty(), "expected at least one hit");
}

#[test]
fn search_empty_query_no_filter_fails() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    let (_, _, status) = run(&["--db", db, "search"]);
    assert!(!status.success(), "expected empty-query gate to fail");
}

// -------- delete / restore --------

#[test]
fn delete_then_list_excludes_default() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    let id = save_one(db, "doomed", "p");

    let (_, _, status) = run(&["--db", db, "delete", &id]);
    assert!(status.success(), "delete failed");

    let (stdout, _, _) = run(&["--db", db, "--json", "list", "--project", "p"]);
    let v: Value = serde_json::from_str(&stdout).unwrap();
    let ids: Vec<&str> = v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["id"].as_str().unwrap())
        .collect();
    assert!(!ids.contains(&id.as_str()));
}

#[test]
fn restore_undoes_soft_delete() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    let id = save_one(db, "phoenix", "p");
    run(&["--db", db, "delete", &id]);
    let (_, _, status) = run(&["--db", db, "restore", &id]);
    assert!(status.success());

    let (stdout, _, _) = run(&["--db", db, "--json", "show", &id]);
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert!(v["data"]["deleted_at"].is_null());
}

// -------- link --------

#[test]
fn link_creates_typed_link_between_two_observations() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    let a = save_one(db, "a", "p");
    let b = save_one(db, "b", "p");
    let (stdout, _, status) = run(&["--db", db, "--json", "link", &a, &b, "derives_from"]);
    assert!(status.success());
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["from_id"], a);
    assert_eq!(v["data"]["to_id"], b);
    assert_eq!(v["data"]["link_type"], "derives_from");
}

// -------- stats / doctor / projects --------

#[test]
fn stats_reflects_observations_count() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    save_one(db, "one", "p");
    save_one(db, "two", "q");

    let (stdout, _, status) = run(&["--db", db, "--json", "stats"]);
    assert!(status.success(), "stats failed: {stdout}");
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["observations"]["active"], 2);
    assert_eq!(v["data"]["observations"]["projects"], 2);
}

#[test]
fn doctor_emits_fake_embedder_warning() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();

    let (stdout, _, status) = run(&["--db", db, "--json", "doctor"]);
    assert!(status.success());
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["status"], "ok");
    // v0.1 ships FakeEmbedder; doctor must surface the warning so
    // users don't silently lose vector search quality. (Cloven sight.)
    assert!(v["data"]["fake_embedder_warning"].is_string());
    let warning = v["data"]["fake_embedder_warning"].as_str().unwrap();
    assert!(warning.contains("FakeEmbedder is active"));

    // Q5: embedding-provenance section is always present. Fresh DB → no
    // combos, nothing vectorless, no mix warning.
    assert!(v["data"]["embedding_models"].as_array().unwrap().is_empty());
    assert_eq!(v["data"]["observations_active_without_vector"], 0);
    assert!(v["data"]["embedding_mix_warning"].is_null());
}

#[test]
fn doctor_reports_embedding_provenance_after_saves() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    save_one(db, "embedded", "p");

    let (stdout, _, status) = run(&["--db", db, "--json", "doctor"]);
    assert!(status.success());
    let v: Value = serde_json::from_str(&stdout).unwrap();
    let models = v["data"]["embedding_models"].as_array().unwrap();
    assert_eq!(models.len(), 1, "one combo after fake-embedded saves");
    assert_eq!(models[0]["model_id"], "seele/fake-embedder");
    assert_eq!(models[0]["dim"], 384);
    assert_eq!(models[0]["count"], 1);
    assert_eq!(v["data"]["observations_active_without_vector"], 0);
    // Stored model == active model (both fake) → no mix warning.
    assert!(v["data"]["embedding_mix_warning"].is_null());
}

#[test]
fn projects_lists_distinct_active_projects() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    save_one(db, "a", "alpha");
    save_one(db, "b", "beta");
    save_one(db, "c", "alpha");

    let (stdout, _, _) = run(&["--db", db, "--json", "projects"]);
    let v: Value = serde_json::from_str(&stdout).unwrap();
    let projects: Vec<&str> = v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap())
        .collect();
    assert_eq!(projects.len(), 2);
    assert!(projects.contains(&"alpha"));
    assert!(projects.contains(&"beta"));
}

// -------- sync --------

#[test]
fn sync_export_then_import_round_trip() {
    let src_td = TempDir::new().unwrap();
    let src_db = src_td.path().join("src.db");
    let src_db = src_db.to_str().unwrap();
    save_one(src_db, "shared-thought", "p");

    let chunks_td = TempDir::new().unwrap();
    let chunks_dir = chunks_td.path().to_str().unwrap();
    let (stdout, _, status) = run(&[
        "--db",
        src_db,
        "--json",
        "sync",
        "export",
        chunks_dir,
        "--project",
        "p",
    ]);
    assert!(status.success(), "export failed: {stdout}");
    let exp: Value = serde_json::from_str(&stdout).unwrap();
    let chunk_path = exp["data"]["path"].as_str().unwrap();

    let dst_td = TempDir::new().unwrap();
    let dst_db = dst_td.path().join("dst.db");
    let dst_db = dst_db.to_str().unwrap();
    let (stdout, _, status) = run(&[
        "--db",
        dst_db,
        "--json",
        "sync",
        "import",
        chunk_path,
        "--target-key",
        "node-test",
    ]);
    assert!(status.success(), "import failed: {stdout}");
    let imp: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(imp["data"]["outcome"], "imported");
    assert_eq!(imp["data"]["observation_count_saved"], 1);

    // The destination now contains the observation.
    let (stdout, _, _) = run(&["--db", dst_db, "--json", "list", "--project", "p"]);
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"].as_array().unwrap().len(), 1);
}

// -------- setup --------

#[test]
fn setup_list_includes_implemented_and_skeleton_agents() {
    let (stdout, _, status) = run(&["--json", "setup", "--list"]);
    assert!(status.success(), "setup --list failed: {stdout}");
    let v: Value = serde_json::from_str(&stdout).unwrap();
    let names: Vec<&str> = v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_str().unwrap())
        .collect();
    assert!(names.contains(&"claude-code"));
    assert!(names.contains(&"opencode"));
}

// -------- import --------

#[test]
fn import_from_engram_against_synthetic_source_inserts_rows() {
    use rusqlite::{params, Connection};
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db_str = db.to_str().unwrap();

    let src = td.path().join("engram.db");
    let conn = Connection::open(&src).unwrap();
    conn.execute_batch(
        "CREATE TABLE memories (
             id TEXT PRIMARY KEY,
             body TEXT NOT NULL,
             metadata TEXT,
             created_at INTEGER NOT NULL
         );",
    )
    .unwrap();
    let row_id = ulid::Ulid::new().to_string();
    conn.execute(
        "INSERT INTO memories(id, body, metadata, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            row_id,
            "hello from engram",
            r#"{"project":"p"}"#,
            1_700_000_000_000_i64
        ],
    )
    .unwrap();
    let src_str = src.to_str().unwrap();

    let (stdout, _stderr, status) =
        run(&["--db", db_str, "--json", "import", "from-engram", src_str]);
    assert!(status.success(), "import failed: {stdout}");
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["rows_seen"], 1);
    assert_eq!(v["data"]["rows_inserted"], 1);
    assert_eq!(v["data"]["rows_skipped_existing"], 0);

    // Re-import is idempotent.
    let (stdout, _, status) = run(&["--db", db_str, "--json", "import", "from-engram", src_str]);
    assert!(status.success());
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["rows_inserted"], 0);
    assert_eq!(v["data"]["rows_skipped_existing"], 1);
}

// -------- envelope contract (Q2, GRAIL-style Reply) --------

#[test]
fn json_error_lands_on_stdout_as_envelope_with_kind() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();

    let (stdout, _, status) = run(&["--db", db, "--json", "show", "not-a-ulid"]);
    assert!(!status.success(), "invalid id must exit non-zero");
    let v: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("error envelope must be JSON on stdout ({e}): {stdout}"));
    assert_eq!(v["ok"], false);
    assert!(
        v["error"].as_str().unwrap().contains("invalid id"),
        "error text: {}",
        v["error"]
    );
    assert!(v["kind"].is_string(), "kind must always be present");
}

#[test]
fn json_success_envelope_carries_ok_data_and_warnings() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();

    let (stdout, _, status) = run(&["--db", db, "--json", "save", "t", "c", "--project", "p"]);
    assert!(status.success());
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert!(v["data"].is_object(), "payload lives under data");
    assert!(v["warnings"].is_array(), "warnings always present");
    // Fake embedder was requested explicitly via env, not a fallback:
    // no degradation warning expected.
    assert!(v["warnings"].as_array().unwrap().is_empty());
}

#[test]
fn json_delete_and_restore_share_the_envelope_shape() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    let id = save_one(db, "enveloped", "p");

    let (stdout, _, status) = run(&["--db", db, "--json", "delete", &id]);
    assert!(status.success());
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["id"], id);
    assert_eq!(v["data"]["outcome"], "soft-deleted");

    let (stdout, _, status) = run(&["--db", db, "--json", "restore", &id]);
    assert!(status.success());
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["outcome"], "restored");
}

#[test]
fn import_from_engram_rejects_db_without_memories_table() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db_str = db.to_str().unwrap();
    let empty = td.path().join("empty.db");
    let _ = rusqlite::Connection::open(&empty).unwrap();
    let empty_str = empty.to_str().unwrap();

    let (_, stderr, status) = run(&["--db", db_str, "import", "from-engram", empty_str]);
    assert!(!status.success());
    assert!(
        stderr.contains("memories"),
        "expected memories-table error in stderr, got: {stderr}"
    );
}
