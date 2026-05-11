//! 5-case project name detection (heredado de ENGRAM).
//!
//! Cuando un agente AI llama `seele save` sin pasar `project`, SEELE
//! infiere uno con este algoritmo. El orden importa: cada caso solo
//! corre si el anterior fallo. Esto evita pisar el `project` correcto
//! con un fallback cuando el caso mas especifico ya nos dio una
//! respuesta.
//!
//! 1. `.seele/config.json` con `{"project": "..."}` → override explicito.
//! 2. `git remote get-url origin` → parse → basename sin `.git`.
//! 3. `git rev-parse --show-toplevel` → basename.
//! 4. Git child scan: depth 1 desde cwd buscando un subdir con `.git/`,
//!    max 20 dirs visitados, timeout 200ms, skip noise dirs.
//! 5. Basename del cwd como fallback final.
//!
//! Skip dirs (caso 4): `node_modules`, `target`, `.git`, `vendor`,
//! `.venv`, `venv`, `__pycache__`, `dist`, `build`, `.next`, `.nuxt`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Project name inferido + el caso que lo produjo. El caso es util para
/// debugging (`seele doctor` lo expone) y para tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectName {
    pub name: String,
    pub source: DetectSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectSource {
    /// Caso 1: `.seele/config.json` override.
    Config,
    /// Caso 2: `git remote get-url origin`.
    GitRemote,
    /// Caso 3: `git rev-parse --show-toplevel`.
    GitRoot,
    /// Caso 4: child scan en subdirs depth 1.
    GitChild,
    /// Caso 5: fallback al basename del cwd.
    DirBasename,
}

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("cwd has no basename component")]
    NoBasename,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid config.json: {0}")]
    InvalidConfig(String),
}

pub type Result<T> = std::result::Result<T, ProjectError>;

/// Run the 5-case algorithm against `cwd`.
///
/// Always returns `Ok` because case 5 (basename) is reachable unless
/// `cwd` itself has no basename — a pathological case that surfaces as
/// `ProjectError::NoBasename`.
pub fn detect(cwd: &Path) -> Result<ProjectName> {
    // Case 1: explicit override.
    if let Some(name) = read_config_override(cwd)? {
        return Ok(ProjectName {
            name,
            source: DetectSource::Config,
        });
    }

    // Case 2: git remote (only inside a git repo).
    if let Some(name) = git_remote_name(cwd) {
        return Ok(ProjectName {
            name,
            source: DetectSource::GitRemote,
        });
    }

    // Case 3: git toplevel.
    if let Some(name) = git_root_basename(cwd) {
        return Ok(ProjectName {
            name,
            source: DetectSource::GitRoot,
        });
    }

    // Case 4: child scan.
    if let Some(name) = git_child_scan(cwd) {
        return Ok(ProjectName {
            name,
            source: DetectSource::GitChild,
        });
    }

    // Case 5: basename.
    let name = cwd
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .ok_or(ProjectError::NoBasename)?;
    Ok(ProjectName {
        name,
        source: DetectSource::DirBasename,
    })
}

// -------- Case 1 --------

#[derive(Debug, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    project: Option<String>,
}

fn read_config_override(cwd: &Path) -> Result<Option<String>> {
    let path = cwd.join(".seele").join("config.json");
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let cfg: ConfigFile = serde_json::from_str(&raw)
        .map_err(|e| ProjectError::InvalidConfig(format!("{path:?}: {e}")))?;
    Ok(cfg.project.filter(|s| !s.trim().is_empty()))
}

// -------- Case 2 --------

fn git_remote_name(cwd: &Path) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
    parse_remote_basename(&url)
}

/// Strip the trailing `.git`, then take the last path/`:` segment.
///
/// Handles:
/// - `git@github.com:org/repo.git` → `repo`
/// - `https://github.com/org/repo.git` → `repo`
/// - `ssh://git@host:22/group/repo` → `repo`
/// - `file:///tmp/repo` → `repo`
/// - `bare/path/repo` → `repo`
pub fn parse_remote_basename(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Strip query / fragment.
    let no_query = trimmed.split(['?', '#']).next().unwrap_or(trimmed);
    // Strip a trailing slash.
    let no_trailing_slash = no_query.trim_end_matches('/');
    // Strip a trailing `.git`.
    let no_git = no_trailing_slash
        .strip_suffix(".git")
        .unwrap_or(no_trailing_slash);
    // Take the segment after the last `/` or `:` — covers
    // `git@host:org/repo` (last `:` → `org/repo`, but inside that we
    // still want the part after `/`).
    let after_colon = no_git.rsplit(':').next().unwrap_or(no_git);
    let after_slash = after_colon.rsplit('/').next().unwrap_or(after_colon);
    let candidate = after_slash.trim();
    if candidate.is_empty() {
        None
    } else {
        Some(candidate.to_string())
    }
}

// -------- Case 3 --------

fn git_root_basename(cwd: &Path) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
    PathBuf::from(root)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

// -------- Case 4 --------

const SCAN_MAX_DIRS: usize = 20;
const SCAN_TIMEOUT: Duration = Duration::from_millis(200);

/// Dirs whose contents are skipped during child scan. These never hold
/// project roots; descending into them wastes the time budget.
const NOISE_DIRS: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    "vendor",
    ".venv",
    "venv",
    "__pycache__",
    "dist",
    "build",
    ".next",
    ".nuxt",
];

fn git_child_scan(cwd: &Path) -> Option<String> {
    let start = Instant::now();
    let mut visited = 0usize;

    let entries = match std::fs::read_dir(cwd) {
        Ok(it) => it,
        Err(_) => return None,
    };

    for entry in entries.flatten() {
        if start.elapsed() > SCAN_TIMEOUT {
            tracing::debug!("project: child scan hit timeout");
            return None;
        }
        if visited >= SCAN_MAX_DIRS {
            tracing::debug!("project: child scan hit max dirs");
            return None;
        }
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        if NOISE_DIRS.contains(&name.as_str()) {
            continue;
        }
        visited += 1;
        if path.join(".git").exists() {
            return Some(name);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_remote_basename_https_with_git_suffix() {
        assert_eq!(
            parse_remote_basename("https://github.com/org/myrepo.git"),
            Some("myrepo".to_string())
        );
    }

    #[test]
    fn parse_remote_basename_ssh_short() {
        assert_eq!(
            parse_remote_basename("git@github.com:org/myrepo.git"),
            Some("myrepo".to_string())
        );
    }

    #[test]
    fn parse_remote_basename_no_git_suffix() {
        assert_eq!(
            parse_remote_basename("https://gitlab.com/group/sub/repo"),
            Some("repo".to_string())
        );
    }

    #[test]
    fn parse_remote_basename_ssh_protocol_with_port() {
        assert_eq!(
            parse_remote_basename("ssh://git@host:22/group/repo.git"),
            Some("repo".to_string())
        );
    }

    #[test]
    fn parse_remote_basename_file_url() {
        assert_eq!(
            parse_remote_basename("file:///tmp/local-repo"),
            Some("local-repo".to_string())
        );
    }

    #[test]
    fn parse_remote_basename_trailing_slash() {
        assert_eq!(
            parse_remote_basename("https://github.com/org/repo/"),
            Some("repo".to_string())
        );
    }

    #[test]
    fn parse_remote_basename_empty_returns_none() {
        assert_eq!(parse_remote_basename(""), None);
        assert_eq!(parse_remote_basename("   "), None);
    }
}
