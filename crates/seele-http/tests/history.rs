//! T-12 — Per-DB op-log (`<db>.history.jsonl`, GRAIL `_history.jsonl`
//! port): every observation mutation through `SeeleService`
//! (save/update/delete/restore) appends one JSON line
//! `{"op","ts","id","project"}`. Append-only, best-effort: a log write
//! failure warns but never fails the mutation.

use std::sync::Arc;

use seele_embedder::FakeEmbedder;
use seele_http::dto::SaveRequest;
use seele_http::SeeleService;
use seele_storage::init_db;
use tempfile::TempDir;

fn service_with_history(td: &TempDir) -> (SeeleService, std::path::PathBuf) {
    let db = td.path().join("seele.db");
    let pool = init_db(&db).unwrap();
    let history = SeeleService::history_path_for_db(&db);
    let svc = SeeleService::new(pool, Arc::new(FakeEmbedder)).with_history_path(&history);
    (svc, history)
}

fn save(svc: &SeeleService, title: &str, project: &str) -> String {
    svc.save_observation(SaveRequest {
        title: title.to_string(),
        content: format!("contenido de {title}"),
        r#type: "memory".to_string(),
        project: Some(project.to_string()),
        scope: None,
        topic_key: None,
        session_id: None,
        tool_name: None,
        metadata: serde_json::Value::Null,
    })
    .unwrap()
    .id
}

fn read_lines(history: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(history)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).expect("each history line is valid JSON"))
        .collect()
}

#[test]
fn history_save_delete_restore_append_three_parseable_lines() {
    let td = TempDir::new().unwrap();
    let (svc, history) = service_with_history(&td);

    let id = save(&svc, "nota uno", "alpha");
    let ulid: seele_core::id::SeeleId = id.parse().unwrap();
    svc.soft_delete_observation(ulid).unwrap();
    svc.restore_observation(ulid).unwrap();

    let lines = read_lines(&history);
    assert_eq!(lines.len(), 3, "save + delete + restore = 3 lines");
    let ops: Vec<&str> = lines.iter().map(|l| l["op"].as_str().unwrap()).collect();
    assert_eq!(ops, ["save", "delete", "restore"]);
    for line in &lines {
        assert_eq!(line["id"].as_str().unwrap(), id);
        assert_eq!(line["project"].as_str().unwrap(), "alpha");
        let ts = line["ts"].as_str().unwrap();
        assert!(
            chrono::DateTime::parse_from_rfc3339(ts).is_ok(),
            "ts is ISO-8601/RFC3339: {ts}"
        );
    }
}

#[test]
fn history_update_op_logged_on_metadata_merge() {
    let td = TempDir::new().unwrap();
    let (svc, history) = service_with_history(&td);

    let id = save(&svc, "nota dos", "beta");
    let ulid: seele_core::id::SeeleId = id.parse().unwrap();
    svc.merge_observation_metadata(ulid, serde_json::json!({"score": 0.9}))
        .unwrap();

    let lines = read_lines(&history);
    assert_eq!(lines.len(), 2, "save + update = 2 lines");
    assert_eq!(lines[1]["op"].as_str().unwrap(), "update");
    assert_eq!(lines[1]["id"].as_str().unwrap(), id);
    assert_eq!(lines[1]["project"].as_str().unwrap(), "beta");
}

#[test]
fn history_is_append_only_across_service_instances() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("seele.db");
    let pool = init_db(&db).unwrap();
    let history = SeeleService::history_path_for_db(&db);

    let one = SeeleService::new(pool.clone(), Arc::new(FakeEmbedder)).with_history_path(&history);
    save(&one, "primera", "p");
    drop(one);

    // A second process lifetime over the same DB appends, never truncates.
    let two = SeeleService::new(pool, Arc::new(FakeEmbedder)).with_history_path(&history);
    save(&two, "segunda", "p");

    let lines = read_lines(&history);
    assert_eq!(lines.len(), 2, "append across service instances");
    assert!(lines.iter().all(|l| l["op"].as_str().unwrap() == "save"));
}

#[test]
fn history_write_failure_never_fails_the_mutation() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("seele.db");
    let pool = init_db(&db).unwrap();
    // Parent dir does not exist: every append fails, saves still land.
    let broken = td.path().join("no/such/dir/seele.db.history.jsonl");
    let svc = SeeleService::new(pool, Arc::new(FakeEmbedder)).with_history_path(&broken);

    let id = save(&svc, "resiliente", "p");
    assert!(!id.is_empty());
    assert!(!broken.exists());
}

#[test]
fn history_path_derives_from_db_path() {
    let db = std::path::Path::new("/data/dir/seele.db");
    assert_eq!(
        SeeleService::history_path_for_db(db),
        std::path::Path::new("/data/dir/seele.db.history.jsonl")
    );
}

#[test]
fn history_disabled_by_default() {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let svc = SeeleService::new(pool, Arc::new(FakeEmbedder));
    save(&svc, "sin log", "p");
    assert!(
        !SeeleService::history_path_for_db(&td.path().join("seele.db")).exists(),
        "no history file unless opted in"
    );
}
