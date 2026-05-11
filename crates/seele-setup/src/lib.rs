//! Setup wizard para integraciones con agentes AI.
//!
//! Cada agente espera SEELE expuesto de forma distinta. Algunos leen un
//! `mcp.json` con definicion del comando, otros tienen su archivo de
//! rules markdown, otros configuran una entry en `settings.json`. Este
//! crate centraliza:
//!
//! - **Donde** vive el archivo de config (path canonico por OS).
//! - **Que** entry agregar (formato propio del agente).
//! - **Idempotencia**: re-correr no duplica entries.
//! - **Backup**: copia del archivo a `.bak.<timestamp>` antes de tocarlo.
//! - **Dry-run**: muestra el diff sin escribir.
//!
//! ## Agentes en v0.1
//!
//! Implementados (full install + dry-run + idempotente):
//! - `claude-code` — `~/.claude.json` `mcpServers` entry.
//! - `cursor` — `~/.cursor/mcp.json` `mcpServers` entry.
//! - `windsurf` — `~/.codeium/windsurf/mcp_config.json` entry.
//!
//! Skeleton (declarados, no implementados — `install("opencode", ...)` →
//! `SetupError::NotImplemented`. v0.2 los completa):
//! - `opencode`, `aider`, `cody`, `continue`, `zed`.

use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

mod agents;

pub use agents::{install_agent, AgentKind};

#[derive(Debug, Error)]
pub enum SetupError {
    #[error("unknown agent: {0}")]
    UnknownAgent(String),
    #[error("agent {0} not implemented in v0.1 (planned for v0.2)")]
    NotImplemented(String),
    #[error("could not resolve home directory")]
    NoHome,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid existing config at {path:?}: {detail}")]
    InvalidExistingConfig { path: PathBuf, detail: String },
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, SetupError>;

/// Result of installing into one agent's config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallReport {
    pub agent: String,
    pub config_path: PathBuf,
    pub outcome: Outcome,
    /// When `dry_run = true`, no file was modified; this string is the
    /// JSON that would have been written.
    pub preview: Option<String>,
    /// Path of the backup file, if a backup was made.
    pub backup_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Config file did not exist; SEELE was added as the sole entry.
    Created,
    /// Config file existed without a SEELE entry; SEELE was added.
    Added,
    /// Config file already had a SEELE entry; left unchanged (idempotent).
    Unchanged,
    /// Config file had a SEELE entry that differed from what we'd write;
    /// it was updated in place.
    Updated,
    /// `dry_run = true` — nothing written.
    DryRun,
}

/// Options that apply to every agent installer.
#[derive(Debug, Clone)]
pub struct InstallOptions {
    /// Don't touch files; just compute what would change.
    pub dry_run: bool,
    /// Copy the existing config to `<path>.bak.<unix_ms>` before writing.
    pub backup: bool,
    /// Override the home directory (used by tests). When `None`, uses
    /// `dirs::home_dir()`.
    pub home_override: Option<PathBuf>,
    /// Override the `seele` binary path that lands in agent configs.
    /// `None` uses the literal string `"seele"` (assumes it's on PATH).
    pub seele_binary: Option<PathBuf>,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            backup: true,
            home_override: None,
            seele_binary: None,
        }
    }
}

impl InstallOptions {
    pub fn home(&self) -> Result<PathBuf> {
        match &self.home_override {
            Some(p) => Ok(p.clone()),
            None => dirs::home_dir().ok_or(SetupError::NoHome),
        }
    }

    pub fn seele_binary_str(&self) -> String {
        self.seele_binary
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "seele".to_string())
    }
}

/// Top-level entry point. Resolves `agent_name` to a known agent, runs
/// its installer. Unknown names → `UnknownAgent`. Declared-but-skeleton
/// agents → `NotImplemented`.
pub fn install(agent_name: &str, opts: &InstallOptions) -> Result<InstallReport> {
    let kind = AgentKind::from_name(agent_name)
        .ok_or_else(|| SetupError::UnknownAgent(agent_name.to_string()))?;
    install_agent(kind, opts)
}

/// All agent names — implemented or skeleton — that `install` recognizes.
/// Useful for `seele setup --list` UX.
pub fn all_agent_names() -> Vec<&'static str> {
    AgentKind::all().iter().map(|k| k.as_str()).collect()
}

/// Agent names whose installer is wired in v0.1. Drives `seele setup --all`
/// so the iteration does not report skeletons as errors.
pub fn implemented_agent_names() -> Vec<&'static str> {
    AgentKind::all()
        .iter()
        .filter(|k| k.is_implemented())
        .map(|k| k.as_str())
        .collect()
}

/// Atomic write: create parent dirs if needed, write to a sibling
/// `.tmp` file, then rename over the destination. This guarantees a
/// reader observing the destination path sees either the old content
/// or the new content — never a half-written file.
///
/// On POSIX, `std::fs::rename` is atomic for same-volume renames. On
/// Windows, `std::fs::rename` issues `MoveFileExW` with
/// `MOVEFILE_REPLACE_EXISTING`, which is atomic for files on the same
/// volume.
///
/// **Residual race for Claude Code**: when `path` points at
/// `~/.claude.json` and Claude Code is running, Claude Code may write
/// to that file between our load-into-memory and our rename, in which
/// case our rename clobbers Claude Code's change. The atomic rename
/// rules out *partial-write* corruption (the real bug a reader of
/// `.claude.json` mid-write would otherwise hit), but cannot rule out
/// last-write-wins. Mitigated by `backup_file` running before the
/// write, and by the fact that `seele setup` is a one-shot operation
/// the user can re-run. v0.2 will delegate to `claude mcp add` when
/// the `claude` CLI is present, closing this gap.
pub(crate) fn write_atomic(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = sidecar_tmp_path(path);
    // Best-effort cleanup of any stale tmp from a prior crash, so the
    // subsequent `File::create` always starts fresh.
    let _ = std::fs::remove_file(&tmp_path);
    std::fs::write(&tmp_path, content)?;
    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e.into());
    }
    Ok(())
}

fn sidecar_tmp_path(path: &Path) -> PathBuf {
    let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let pid = std::process::id();
    let suffix = format!(".seele-tmp-{pid}-{ts}");
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(suffix);
    PathBuf::from(tmp)
}

/// Copy `path` to `<path>.bak.<unix_ms>` if it exists. Returns the
/// backup path written, or `None` if the file didn't exist.
pub(crate) fn backup_file(path: &Path) -> Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let ts = chrono::Utc::now().timestamp_millis();
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let new_ext = if ext.is_empty() {
        format!("bak.{ts}")
    } else {
        format!("{ext}.bak.{ts}")
    };
    let backup = path.with_extension(new_ext);
    std::fs::copy(path, &backup)?;
    Ok(Some(backup))
}
