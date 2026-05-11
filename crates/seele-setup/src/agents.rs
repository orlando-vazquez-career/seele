//! Per-agent installer implementations.
//!
//! Each implemented agent provides:
//! - `config_path(opts)`: where the config lives.
//! - `install(opts)`: idempotent JSON merge against that path.
//!
//! Skeleton agents (planned for v0.2) return `SetupError::NotImplemented`
//! from `install_agent`. The dispatch table here is the single source of
//! truth for which names `seele setup --agent <name>` accepts.

use std::path::PathBuf;

use serde_json::{json, Map, Value};

use crate::{
    backup_file, write_atomic, InstallOptions, InstallReport, Outcome, Result, SetupError,
};

/// Canonical agent identifiers. `as_str()` is the value `--agent <name>`
/// accepts on the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentKind {
    ClaudeCode,
    Cursor,
    Windsurf,
    // Skeleton — declared so `--agent <name>` gives a useful error
    // instead of "unknown agent".
    OpenCode,
    Aider,
    Cody,
    Continue,
    Zed,
}

impl AgentKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Cursor => "cursor",
            Self::Windsurf => "windsurf",
            Self::OpenCode => "opencode",
            Self::Aider => "aider",
            Self::Cody => "cody",
            Self::Continue => "continue",
            Self::Zed => "zed",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::all().iter().copied().find(|k| k.as_str() == name)
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::ClaudeCode,
            Self::Cursor,
            Self::Windsurf,
            Self::OpenCode,
            Self::Aider,
            Self::Cody,
            Self::Continue,
            Self::Zed,
        ]
    }

    /// `true` when this agent has a full installer in v0.1. `false` for
    /// skeletons declared so `--agent <name>` gives a useful error
    /// instead of "unknown agent". Drives `setup --all` filtering.
    pub fn is_implemented(&self) -> bool {
        matches!(self, Self::ClaudeCode | Self::Cursor | Self::Windsurf)
    }
}

pub fn install_agent(kind: AgentKind, opts: &InstallOptions) -> Result<InstallReport> {
    match kind {
        AgentKind::ClaudeCode => install_mcp_json(
            kind,
            opts.home()?.join(".claude.json"),
            opts,
            /*top_level_key=*/ "mcpServers",
        ),
        AgentKind::Cursor => install_mcp_json(
            kind,
            opts.home()?.join(".cursor").join("mcp.json"),
            opts,
            "mcpServers",
        ),
        AgentKind::Windsurf => install_mcp_json(
            kind,
            opts.home()?
                .join(".codeium")
                .join("windsurf")
                .join("mcp_config.json"),
            opts,
            "mcpServers",
        ),
        skeleton => Err(SetupError::NotImplemented(skeleton.as_str().to_string())),
    }
}

/// Shared implementation for agents that use a JSON file with a top-level
/// object whose key (typically `"mcpServers"`) maps server name → spec
/// `{command, args}`. Covers Claude Code, Cursor, Windsurf — the three
/// dominant MCP consumers as of v0.1.
fn install_mcp_json(
    kind: AgentKind,
    config_path: PathBuf,
    opts: &InstallOptions,
    top_level_key: &str,
) -> Result<InstallReport> {
    let seele_entry = json!({
        "command": opts.seele_binary_str(),
        "args": ["mcp"],
    });

    // 1. Load the existing file (or start with an empty object).
    let (mut root, existed) = load_json_object(&config_path)?;

    // 2. Decide outcome by comparing what's there to what we'd write.
    let servers = root
        .entry(top_level_key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let servers = servers
        .as_object_mut()
        .ok_or_else(|| SetupError::InvalidExistingConfig {
            path: config_path.clone(),
            detail: format!("`{top_level_key}` is not an object"),
        })?;

    let outcome = match servers.get("seele") {
        Some(existing) if existing == &seele_entry => Outcome::Unchanged,
        Some(_) => Outcome::Updated,
        None if existed => Outcome::Added,
        None => Outcome::Created,
    };

    servers.insert("seele".to_string(), seele_entry);

    let new_content = serde_json::to_string_pretty(&Value::Object(root))?;

    if opts.dry_run {
        return Ok(InstallReport {
            agent: kind.as_str().to_string(),
            config_path,
            outcome: Outcome::DryRun,
            preview: Some(new_content),
            backup_path: None,
        });
    }

    if outcome == Outcome::Unchanged {
        return Ok(InstallReport {
            agent: kind.as_str().to_string(),
            config_path,
            outcome,
            preview: None,
            backup_path: None,
        });
    }

    let backup_path = if opts.backup {
        backup_file(&config_path)?
    } else {
        None
    };
    write_atomic(&config_path, &new_content)?;

    Ok(InstallReport {
        agent: kind.as_str().to_string(),
        config_path,
        outcome,
        preview: None,
        backup_path,
    })
}

/// Load a JSON object from `path`. Returns `(map, existed)`:
/// - `existed = false` and an empty map if the file doesn't exist.
/// - An error if the file exists but doesn't parse, or if it parses to
///   a non-object (we never want to clobber arbitrary JSON).
fn load_json_object(path: &PathBuf) -> Result<(Map<String, Value>, bool)> {
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Map::new(), false));
        }
        Err(e) => return Err(e.into()),
    };
    if raw.trim().is_empty() {
        return Ok((Map::new(), true));
    }
    let v: Value = serde_json::from_str(&raw).map_err(|e| SetupError::InvalidExistingConfig {
        path: path.clone(),
        detail: format!("not valid JSON: {e}"),
    })?;
    match v {
        Value::Object(map) => Ok((map, true)),
        other => Err(SetupError::InvalidExistingConfig {
            path: path.clone(),
            detail: format!("root is not an object (got {})", type_name(&other)),
        }),
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_kind_round_trips() {
        for k in AgentKind::all() {
            assert_eq!(AgentKind::from_name(k.as_str()), Some(*k));
        }
    }

    #[test]
    fn unknown_agent_name_returns_none() {
        assert_eq!(AgentKind::from_name("nonexistent"), None);
    }
}
