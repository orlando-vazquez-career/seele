//! T-10 (ADR-16 D4) — `seele save` autodetects the project from the
//! cwd when `--project` is omitted. CLI-only wiring: serve/MCP keep
//! the explicit-project contract.

use std::path::Path;
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

const SEELE_BIN: &str = env!("CARGO_BIN_EXE_seele");

/// Spawn `seele` with a working directory (autodetect reads the cwd)
/// and the FakeEmbedder forced, like the other e2e suites.
fn run_in(cwd: &Path, args: &[&str]) -> (String, String, std::process::ExitStatus) {
    let out = Command::new(SEELE_BIN)
        .env("SEELE_FAKE_EMBEDDER", "1")
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("spawn");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status,
    )
}

/// `git init` + remote named `origin`. `git remote get-url origin`
/// (case 2 of `seele_project::detect`) does not need the remote to
/// exist, so any URL works.
fn init_repo_with_origin(dir: &Path, url: &str) {
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("spawn git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["init", "-q"]);
    git(&["remote", "add", "origin", url]);
}

/// Save from `cwd` (plus any extra save args) and return the stored
/// observation via `show`, together with the save stderr (where the
/// autodetect log line lands).
fn save_and_show(cwd: &Path, db: &str, extra_save_args: &[&str]) -> (Value, String) {
    let mut args = vec!["--db", db, "--json", "save", "t", "content"];
    args.extend_from_slice(extra_save_args);
    let (stdout, stderr, status) = run_in(cwd, &args);
    assert!(status.success(), "save failed: {stderr}");
    let v: Value = serde_json::from_str(&stdout).expect("save json");
    let id = v["data"]["id"].as_str().expect("save id").to_string();

    let (stdout, _, status) = run_in(cwd, &["--db", db, "--json", "show", &id]);
    assert!(status.success(), "show failed: {stdout}");
    let v: Value = serde_json::from_str(&stdout).unwrap();
    (v["data"].clone(), stderr)
}

#[test]
fn save_without_project_autodetects_from_git_remote() {
    let repo = TempDir::new().unwrap();
    init_repo_with_origin(repo.path(), "https://github.com/org/detected-proj.git");
    let db_td = TempDir::new().unwrap();
    let db = db_td.path().join("s.db");
    let db = db.to_str().unwrap();

    let (obs, stderr) = save_and_show(repo.path(), db, &[]);
    assert_eq!(
        obs["project"].as_str(),
        Some("detected-proj"),
        "project must come from the git remote: {obs}"
    );
    assert!(
        stderr.contains("detected-proj"),
        "autodetect must be logged so the user sees what happened: {stderr}"
    );
}

#[test]
fn save_with_explicit_project_flag_wins_over_autodetect() {
    let repo = TempDir::new().unwrap();
    init_repo_with_origin(repo.path(), "https://github.com/org/detected-proj.git");
    let db_td = TempDir::new().unwrap();
    let db = db_td.path().join("s.db");
    let db = db.to_str().unwrap();

    let (obs, stderr) = save_and_show(repo.path(), db, &["--project", "explicit-proj"]);
    assert_eq!(
        obs["project"].as_str(),
        Some("explicit-proj"),
        "the explicit flag must win over autodetect: {obs}"
    );
    assert!(
        !stderr.contains("autodetect"),
        "explicit flag must skip the autodetect log line: {stderr}"
    );
}
