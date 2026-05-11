//! E2E tests for the 5-case detection algorithm using real tempdirs +
//! real `git` subprocesses where the case requires git.
//!
//! Tests that need git skip themselves if `git` is not available on the
//! PATH (rare on dev machines, but possible on minimal CI containers).

use std::fs;
use std::path::Path;
use std::process::Command;

use seele_project::{detect, DetectSource};
use tempfile::TempDir;

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn init_git_repo(dir: &Path) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(dir)
        .arg("init")
        .arg("--quiet")
        .status()
        .unwrap()
        .success();
    assert!(ok, "git init failed in {dir:?}");
}

fn set_remote(dir: &Path, url: &str) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["remote", "add", "origin", url])
        .status()
        .unwrap()
        .success();
    assert!(ok, "git remote add failed in {dir:?}");
}

// -------- Case 1: .seele/config.json --------

#[test]
fn case1_config_override_wins_over_everything() {
    let td = TempDir::new().unwrap();
    let path = td.path();
    fs::create_dir_all(path.join(".seele")).unwrap();
    fs::write(
        path.join(".seele/config.json"),
        r#"{"project": "configured-name"}"#,
    )
    .unwrap();
    // Add a git repo with a remote — case 1 should still win.
    if git_available() {
        init_git_repo(path);
        set_remote(path, "https://github.com/x/should-lose.git");
    }
    let r = detect(path).unwrap();
    assert_eq!(r.name, "configured-name");
    assert_eq!(r.source, DetectSource::Config);
}

#[test]
fn case1_empty_project_falls_through() {
    let td = TempDir::new().unwrap();
    let path = td.path();
    fs::create_dir_all(path.join(".seele")).unwrap();
    fs::write(path.join(".seele/config.json"), r#"{"project": "   "}"#).unwrap();
    let r = detect(path).unwrap();
    // Falls through to basename of the tempdir.
    assert_ne!(r.source, DetectSource::Config);
}

#[test]
fn case1_invalid_json_errors_loudly() {
    let td = TempDir::new().unwrap();
    let path = td.path();
    fs::create_dir_all(path.join(".seele")).unwrap();
    fs::write(path.join(".seele/config.json"), "not valid json").unwrap();
    let err = detect(path).unwrap_err();
    assert!(err.to_string().contains("invalid config.json"));
}

// -------- Case 2: git remote --------

#[test]
fn case2_git_remote_takes_basename() {
    if !git_available() {
        eprintln!("skip: git not available");
        return;
    }
    let td = TempDir::new().unwrap();
    init_git_repo(td.path());
    set_remote(td.path(), "git@github.com:org/coolrepo.git");
    let r = detect(td.path()).unwrap();
    assert_eq!(r.name, "coolrepo");
    assert_eq!(r.source, DetectSource::GitRemote);
}

// -------- Case 3: git root --------

#[test]
fn case3_git_root_no_remote_uses_toplevel_basename() {
    if !git_available() {
        eprintln!("skip: git not available");
        return;
    }
    let td = TempDir::new().unwrap();
    // We want a specific basename so we know what to expect. Create a
    // child dir, init git there.
    let repo = td.path().join("specific-name");
    fs::create_dir_all(&repo).unwrap();
    init_git_repo(&repo);
    // No remote set — case 2 should miss.
    let r = detect(&repo).unwrap();
    assert_eq!(r.name, "specific-name");
    assert_eq!(r.source, DetectSource::GitRoot);
}

// -------- Case 4: git child scan --------

#[test]
fn case4_child_scan_finds_first_subdir_with_dot_git() {
    if !git_available() {
        eprintln!("skip: git not available");
        return;
    }
    let td = TempDir::new().unwrap();
    let parent = td.path();
    // Create two child dirs; init git in the second one only.
    fs::create_dir_all(parent.join("first")).unwrap();
    let target = parent.join("my-child-repo");
    fs::create_dir_all(&target).unwrap();
    init_git_repo(&target);
    let r = detect(parent).unwrap();
    assert_eq!(r.name, "my-child-repo");
    assert_eq!(r.source, DetectSource::GitChild);
}

#[test]
fn case4_skips_noise_dirs() {
    if !git_available() {
        eprintln!("skip: git not available");
        return;
    }
    let td = TempDir::new().unwrap();
    let parent = td.path();
    // Plant a git repo inside `node_modules/` — must be skipped.
    let noise = parent.join("node_modules").join("foo");
    fs::create_dir_all(&noise).unwrap();
    init_git_repo(&noise);
    // Plant a legitimate sibling.
    let legit = parent.join("legit");
    fs::create_dir_all(&legit).unwrap();
    init_git_repo(&legit);
    let r = detect(parent).unwrap();
    assert_eq!(r.name, "legit");
    assert_eq!(r.source, DetectSource::GitChild);
}

// -------- Case 5: dir basename --------

#[test]
fn case5_fallback_to_dir_basename_no_git_anywhere() {
    let td = TempDir::new().unwrap();
    let path = td.path().join("plain-folder");
    fs::create_dir_all(&path).unwrap();
    let r = detect(&path).unwrap();
    assert_eq!(r.name, "plain-folder");
    assert_eq!(r.source, DetectSource::DirBasename);
}
