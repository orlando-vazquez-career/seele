//! E2E tests for the setup wizard. Each test uses a tempdir as
//! `home_override` so no real home directory is ever touched.

use std::path::PathBuf;

use seele_setup::{install, InstallOptions, Outcome};
use serde_json::Value;
use tempfile::TempDir;

fn opts_for(home: &TempDir) -> InstallOptions {
    InstallOptions {
        dry_run: false,
        backup: true,
        home_override: Some(home.path().to_path_buf()),
        seele_binary: Some(PathBuf::from("/usr/local/bin/seele")),
    }
}

fn read_json(path: &std::path::Path) -> Value {
    let raw = std::fs::read_to_string(path).expect("read config");
    serde_json::from_str(&raw).expect("parse json")
}

// -------- claude-code --------

#[test]
fn claude_code_creates_new_config_if_absent() {
    let home = TempDir::new().unwrap();
    let report = install("claude-code", &opts_for(&home)).unwrap();

    assert_eq!(report.outcome, Outcome::Created);
    assert_eq!(report.agent, "claude-code");
    assert_eq!(report.config_path, home.path().join(".claude.json"));

    let v = read_json(&report.config_path);
    assert_eq!(v["mcpServers"]["seele"]["command"], "/usr/local/bin/seele");
    assert_eq!(v["mcpServers"]["seele"]["args"], serde_json::json!(["mcp"]));
}

#[test]
fn claude_code_adds_to_existing_config_without_clobbering() {
    let home = TempDir::new().unwrap();
    let path = home.path().join(".claude.json");
    // Existing config with another mcpServer + an unrelated top-level key.
    std::fs::write(
        &path,
        r#"{
          "mcpServers": {
            "other-tool": {"command": "other", "args": []}
          },
          "user_settings": {"theme": "dark"}
        }"#,
    )
    .unwrap();

    let report = install("claude-code", &opts_for(&home)).unwrap();
    assert_eq!(report.outcome, Outcome::Added);

    let v = read_json(&path);
    assert!(v["mcpServers"]["other-tool"]["command"] == "other");
    assert!(v["mcpServers"]["seele"]["command"] == "/usr/local/bin/seele");
    assert_eq!(v["user_settings"]["theme"], "dark");
}

#[test]
fn claude_code_re_running_is_idempotent_unchanged() {
    let home = TempDir::new().unwrap();
    let first = install("claude-code", &opts_for(&home)).unwrap();
    assert_eq!(first.outcome, Outcome::Created);
    let second = install("claude-code", &opts_for(&home)).unwrap();
    assert_eq!(second.outcome, Outcome::Unchanged);
    assert!(
        second.backup_path.is_none(),
        "unchanged should not produce a backup"
    );
}

#[test]
fn claude_code_updates_when_seele_entry_differs() {
    let home = TempDir::new().unwrap();
    // Pre-existing seele entry with a stale binary path.
    let path = home.path().join(".claude.json");
    std::fs::write(
        &path,
        r#"{"mcpServers":{"seele":{"command":"/old/seele","args":["mcp"]}}}"#,
    )
    .unwrap();

    let report = install("claude-code", &opts_for(&home)).unwrap();
    assert_eq!(report.outcome, Outcome::Updated);
    assert!(report.backup_path.is_some(), "update should back up");

    let v = read_json(&path);
    assert_eq!(v["mcpServers"]["seele"]["command"], "/usr/local/bin/seele");
}

#[test]
fn claude_code_dry_run_does_not_touch_disk() {
    let home = TempDir::new().unwrap();
    let mut opts = opts_for(&home);
    opts.dry_run = true;

    let report = install("claude-code", &opts).unwrap();
    assert_eq!(report.outcome, Outcome::DryRun);
    assert!(report.preview.is_some());
    assert!(!report.config_path.exists());
}

// -------- cursor --------

#[test]
fn cursor_writes_to_cursor_mcp_json() {
    let home = TempDir::new().unwrap();
    let report = install("cursor", &opts_for(&home)).unwrap();
    assert_eq!(
        report.config_path,
        home.path().join(".cursor").join("mcp.json")
    );
    assert!(report.config_path.exists());
    let v = read_json(&report.config_path);
    assert_eq!(v["mcpServers"]["seele"]["command"], "/usr/local/bin/seele");
}

// -------- windsurf --------

#[test]
fn windsurf_writes_to_codeium_windsurf_mcp_config_json() {
    let home = TempDir::new().unwrap();
    let report = install("windsurf", &opts_for(&home)).unwrap();
    let expected = home
        .path()
        .join(".codeium")
        .join("windsurf")
        .join("mcp_config.json");
    assert_eq!(report.config_path, expected);
    assert!(expected.exists());
}

// -------- kimi-code --------

#[test]
fn kimi_code_creates_new_config_if_absent() {
    let home = TempDir::new().unwrap();
    let report = install("kimi-code", &opts_for(&home)).unwrap();

    assert_eq!(report.outcome, Outcome::Created);
    assert_eq!(report.agent, "kimi-code");
    assert_eq!(
        report.config_path,
        home.path().join(".kimi-code").join("mcp.json")
    );

    let v = read_json(&report.config_path);
    assert_eq!(v["mcpServers"]["seele"]["command"], "/usr/local/bin/seele");
    assert_eq!(v["mcpServers"]["seele"]["args"], serde_json::json!(["mcp"]));
}

#[test]
fn kimi_code_re_running_is_idempotent_unchanged() {
    let home = TempDir::new().unwrap();
    let first = install("kimi-code", &opts_for(&home)).unwrap();
    assert_eq!(first.outcome, Outcome::Created);
    let second = install("kimi-code", &opts_for(&home)).unwrap();
    assert_eq!(second.outcome, Outcome::Unchanged);
    assert!(
        second.backup_path.is_none(),
        "unchanged should not produce a backup"
    );
}

#[test]
fn kimi_code_is_listed_as_implemented() {
    // `seele setup --list` tags agents from `implemented_agent_names()`
    // as "implemented"; the rest as "skeleton (v0.2)".
    let implemented = seele_setup::implemented_agent_names();
    assert!(
        implemented.contains(&"kimi-code"),
        "kimi-code missing from implemented list: {implemented:?}"
    );
    let all = seele_setup::all_agent_names();
    assert!(
        all.contains(&"kimi-code"),
        "kimi-code missing from known agents: {all:?}"
    );
}

// -------- skeleton agents --------

#[test]
fn skeleton_agents_return_not_implemented() {
    let home = TempDir::new().unwrap();
    for name in ["opencode", "aider", "cody", "continue", "zed"] {
        let err = install(name, &opts_for(&home)).unwrap_err();
        let s = err.to_string();
        assert!(
            s.contains("not implemented in v0.1"),
            "expected NotImplemented, got: {s}"
        );
    }
}

#[test]
fn unknown_agent_returns_unknown_agent_error() {
    let home = TempDir::new().unwrap();
    let err = install("notreal", &opts_for(&home)).unwrap_err();
    assert!(err.to_string().contains("unknown agent"));
}

// -------- error cases --------

#[test]
fn invalid_existing_config_errors_loudly_without_clobbering() {
    let home = TempDir::new().unwrap();
    let path = home.path().join(".claude.json");
    std::fs::write(&path, "not even json {{{").unwrap();
    let err = install("claude-code", &opts_for(&home)).unwrap_err();
    let s = err.to_string();
    assert!(
        s.contains("invalid existing config"),
        "expected InvalidExistingConfig, got: {s}"
    );
    // File is still untouched.
    let raw = std::fs::read_to_string(&path).unwrap();
    assert_eq!(raw, "not even json {{{");
}

#[test]
fn invalid_existing_config_non_object_root_errors() {
    let home = TempDir::new().unwrap();
    let path = home.path().join(".claude.json");
    std::fs::write(&path, "[1, 2, 3]").unwrap();
    let err = install("claude-code", &opts_for(&home)).unwrap_err();
    assert!(err.to_string().contains("root is not an object"));
}

// -------- listing --------

#[test]
fn all_agent_names_includes_implemented_and_skeleton() {
    let names = seele_setup::all_agent_names();
    for expected in [
        "claude-code",
        "cursor",
        "windsurf",
        "kimi-code",
        "opencode",
        "aider",
        "cody",
        "continue",
        "zed",
    ] {
        assert!(names.contains(&expected), "missing {expected} from list");
    }
}
