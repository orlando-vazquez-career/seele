## 14. Agent Setup Wizard — `seele-setup`

`seele-setup` is the crate behind the `seele setup` subcommand: a one-shot wizard that wires SEELE's MCP stdio server (`seele mcp`) into a coding agent's MCP configuration file. Its job is narrow and deliberately offline — it never spawns the MCP server, never talks to the network, and never touches SQLite. It only locates the per-agent config path, computes the JSON entry SEELE needs, merges it idempotently into whatever already exists, optionally backs up the old file, and writes the result atomically.

### 14.1 Position in the crate graph

`seele-setup` is a near-leaf crate. Its only internal dependency is `seele-core` (declared in `Cargo.toml`), and even that is essentially nominal — nothing in `lib.rs` or `agents.rs` imports a `seele_core::` symbol; the dependency exists for workspace coherence (version/edition inheritance) rather than for code reuse. Its sole consumer inside SEELE is the `seele-cli` binary, specifically `crates/seele-cli/src/commands/setup.rs`, which is dispatched from `crates/seele-cli/src/app.rs` (`Command::Setup(args) => commands::setup::run(args, &out).await`, app.rs:108). The wizard is not invoked by `seele-mcp`, `seele-http`, the TUI, or any service-layer code; it is pure filesystem-config plumbing reachable only through the CLI.

External crates and why (`Cargo.toml`):

| Crate | Pin | Used for |
|---|---|---|
| `serde` | workspace | `#[derive(Serialize)]` on `InstallReport` / `Outcome` so `--json` can emit reports. |
| `serde_json` | workspace | Parse and re-serialize the agent config (`Value`, `Map`, `json!`, `to_string_pretty`). |
| `thiserror` | workspace | The `SetupError` enum. |
| `tracing` | workspace | Declared but unused in the current source (no `tracing::` call sites in this crate). |
| `chrono` | workspace | Timestamps for backup filenames (`timestamp_millis`) and tmp filenames (`timestamp_nanos_opt`). |
| `dirs` | `"5"` | `dirs::home_dir()` to resolve the canonical home directory per OS. |
| `tempfile` | workspace (dev) | E2E tests create a throwaway home so no real config is touched. |

Note the `Cargo.toml` `description` field over-promises ("Claude Code, Cursor, VS Code, OpenCode, Gemini CLI, Codex, Windsurf, Antigravity") relative to what is actually wired; the authoritative agent list lives in `AgentKind::all()`.

### 14.2 File-by-file map

- **`src/lib.rs`** — public API surface and the shared filesystem primitives. Declares the `SetupError` enum, the `Result<T>` alias, the `InstallReport` / `Outcome` / `InstallOptions` data types, the top-level `install()` entry point, the listing helpers (`all_agent_names`, `implemented_agent_names`), and the two crate-internal write primitives `write_atomic` and `backup_file` (plus the private helper `sidecar_tmp_path`). The module doc comment is also the canonical spec of which agents are implemented vs. skeleton.
- **`src/agents.rs`** — the per-agent dispatch and the JSON-merge installer. Defines `AgentKind` (the 8-variant enum), its `as_str` / `from_name` / `all` / `is_implemented` methods, the public `install_agent` dispatcher, the shared `install_mcp_json` merge routine, and the JSON-loading helper `load_json_object` plus a `type_name` formatter. Contains two unit tests covering `AgentKind` name round-tripping.
- **`tests/install_e2e.rs`** — 12 end-to-end tests that drive `seele_setup::install()` against a `tempfile::TempDir` used as `home_override`. They assert outcomes (Created/Added/Unchanged/Updated/DryRun), the exact config paths, the non-clobbering merge, idempotency, backup presence/absence, dry-run leaving disk untouched, skeleton `NotImplemented`, unknown-agent error, invalid-JSON and non-object-root errors, and the `all_agent_names` listing.
- **`docs/AGENT-SETUP.md`** — user-facing documentation: the implemented/skeleton table, per-agent invocation examples, the manual config snippet (including the `--tool-prefix mnema` ENGRAM-compat variant), `--dry-run` usage, and a `tools/list` stress-test recipe.

### 14.3 Public API surface

**`pub fn install(agent_name: &str, opts: &InstallOptions) -> Result<InstallReport>`** (lib.rs:126) — the single top-level entry point. It resolves `agent_name` via `AgentKind::from_name`; an unrecognized name yields `SetupError::UnknownAgent`, then delegates to `install_agent(kind, opts)`. A declared-but-skeleton agent will pass name resolution but fail inside `install_agent` with `SetupError::NotImplemented`.

**`pub fn all_agent_names() -> Vec<&'static str>`** (lib.rs:134) — every recognized name, implemented or skeleton (drives `--list`).

**`pub fn implemented_agent_names() -> Vec<&'static str>`** (lib.rs:140) — only the names with a real installer; filters via `AgentKind::is_implemented()`. This is what `--all` iterates over so skeletons are never reported as errors.

**`pub fn install_agent(kind: AgentKind, opts: &InstallOptions) -> Result<InstallReport>`** (agents.rs:74) — re-exported from `agents`; the dispatcher mapping `AgentKind` to a concrete config path and installer.

**`pub enum AgentKind`** (agents.rs:22) — the canonical identifier enum, re-exported at crate root.

Crate-internal (`pub(crate)`) primitives: `write_atomic` (lib.rs:168) and `backup_file` (lib.rs:195).

#### `InstallOptions` (lib.rs:83)

| Field | Type | Meaning |
|---|---|---|
| `dry_run` | `bool` | When `true`, compute the would-be result but write nothing. |
| `backup` | `bool` | When `true` (default), copy the existing config to `<path>.<ext>.bak.<unix_ms>` before writing. |
| `home_override` | `Option<PathBuf>` | Test seam: overrides `dirs::home_dir()`. `None` uses the real home. |
| `seele_binary` | `Option<PathBuf>` | The binary path written into the config's `command`. `None` falls back to the literal `"seele"` (assumes on PATH). |

`Default` (lib.rs:96) sets `dry_run: false, backup: true, home_override: None, seele_binary: None` — note the default is **backup-on**. Two helpers: `home()` (lib.rs:108) returns the override or `dirs::home_dir().ok_or(SetupError::NoHome)`; `seele_binary_str()` (lib.rs:115) lossily stringifies the override path or returns `"seele"`.

#### `InstallReport` (lib.rs:53) and `Outcome` (lib.rs:65)

`InstallReport` is `Serialize` so `--json` emits it directly. Fields: `agent: String`, `config_path: PathBuf`, `outcome: Outcome`, `preview: Option<String>` (the would-be JSON, populated only on dry-run), `backup_path: Option<PathBuf>` (set only when a backup was actually written).

`Outcome` (`#[serde(rename_all = "snake_case")]`) is the five-way classification of what happened:

| Variant | Meaning |
|---|---|
| `Created` | Config file did not exist; SEELE added as sole/initial entry. |
| `Added` | File existed without a `seele` entry; entry added, other content preserved. |
| `Unchanged` | File already had an identical `seele` entry; nothing written (idempotent). |
| `Updated` | File had a `seele` entry that differed; replaced in place. |
| `DryRun` | `dry_run = true`; nothing written, `preview` populated. |

#### `SetupError` (lib.rs:34)

| Variant | Trigger |
|---|---|
| `UnknownAgent(String)` | `agent_name` not in `AgentKind::all()`. |
| `NotImplemented(String)` | A skeleton agent was requested; message reads `agent {0} not implemented in v0.1 (planned for v0.2)`. |
| `NoHome` | `dirs::home_dir()` returned `None` and no override given. |
| `Io(std::io::Error)` | Any filesystem error (`#[from]`). |
| `InvalidExistingConfig { path, detail }` | Existing config is not valid JSON, root is not an object, or the top-level key is not an object. |
| `Json(serde_json::Error)` | Serialization failure (`#[from]`); practically unreachable since we build the JSON ourselves. |

### 14.4 `AgentKind`: the dispatch table

`AgentKind` (agents.rs:22) has 8 `Copy` variants. The three implemented MCP consumers — `ClaudeCode`, `Cursor`, `Windsurf` — come first; the five skeletons — `OpenCode`, `Aider`, `Cody`, `Continue`, `Zed` — follow, declared (per the inline comment) "so `--agent <name>` gives a useful error instead of 'unknown agent'." The `as_str` mapping is the CLI-accepted spelling: `claude-code`, `cursor`, `windsurf`, `opencode`, `aider`, `cody`, `continue`, `zed`. `from_name` is the inverse (linear scan over `all()`), `all()` returns the canonical ordered slice, and `is_implemented()` (agents.rs:69) is the single predicate dividing real installers from skeletons:

```rust
// agents.rs:69
pub fn is_implemented(&self) -> bool {
    matches!(self, Self::ClaudeCode | Self::Cursor | Self::Windsurf)
}
```

This predicate is the *hide/filter* mechanism the ALTO Cloven finding (see §14.7) demanded. Skeletons are never *removed* from `all()` — they remain visible in `--list` tagged `[skeleton (v0.2)]` — but `--all` iterates only `implemented_agent_names()`, so a bulk install never emits five `NotImplemented` errors.

`install_agent` (agents.rs:74) maps each implemented kind to `install_mcp_json` with its config path and the top-level key `"mcpServers"`; the catch-all arm `skeleton => Err(SetupError::NotImplemented(...))` handles all five skeletons uniformly.

### 14.5 Per-agent config locations

All three implemented agents share the same JSON shape (a top-level `"mcpServers"` object mapping server name → `{command, args}`), so they share `install_mcp_json`; only the path differs. Paths are built relative to `opts.home()`:

| Agent | `--agent` value | Config path (relative to home) | Top-level key |
|---|---|---|---|
| Claude Code | `claude-code` | `.claude.json` | `mcpServers` |
| Cursor | `cursor` | `.cursor/mcp.json` | `mcpServers` |
| Windsurf | `windsurf` | `.codeium/windsurf/mcp_config.json` | `mcpServers` |

The entry SEELE injects under `mcpServers.seele` is constructed at agents.rs:111:

```rust
// agents.rs:111
let seele_entry = json!({
    "command": opts.seele_binary_str(),
    "args": ["mcp"],
});
```

So a fresh Claude Code config becomes `{"mcpServers":{"seele":{"command":"seele","args":["mcp"]}}}` (or with an absolute path when `--seele-binary` / `seele_binary` is set). The wizard never adds `--tool-prefix mnema`; that ENGRAM-compat variant is documented as a manual edit only (`docs/AGENT-SETUP.md`).

### 14.6 `install_mcp_json`: control flow

`install_mcp_json` (agents.rs:105) is the heart of the wizard. Step by step:

1. **Build the desired entry** (`seele_entry`, agents.rs:111).
2. **Load existing config** via `load_json_object(&config_path)` → `(root: Map, existed: bool)`. A missing file returns `(empty map, false)`; an empty/whitespace-only file returns `(empty map, true)`; a file that parses but is not a JSON object errors with `InvalidExistingConfig { detail: "root is not an object (got <type>)" }`; unparseable content errors with `detail: "not valid JSON: <e>"`. This is the explicit guard against clobbering arbitrary JSON.
3. **Get or create the `mcpServers` object** via `root.entry(key).or_insert_with(|| Value::Object(...))`, then `as_object_mut()`; if the key exists but is not an object, error `InvalidExistingConfig { detail: "`mcpServers` is not an object" }`.
4. **Classify the outcome** by comparing the *current* `seele` entry to `seele_entry` (agents.rs:130):

```rust
// agents.rs:130
let outcome = match servers.get("seele") {
    Some(existing) if existing == &seele_entry => Outcome::Unchanged,
    Some(_) => Outcome::Updated,
    None if existed => Outcome::Added,
    None => Outcome::Created,
};
```

5. **Insert** the entry unconditionally (`servers.insert("seele", seele_entry)`) and pretty-print the whole root (`to_string_pretty`). The insert is harmless in the `Unchanged` case because the value is byte-identical.
6. **Dry-run short-circuit**: if `opts.dry_run`, return immediately with `outcome: Outcome::DryRun` (the classified `Created/Added/...` is discarded), `preview: Some(new_content)`, `backup_path: None`. Nothing is written.
7. **Unchanged short-circuit**: if `outcome == Unchanged`, return without writing and without backing up (`backup_path: None`) — this is the idempotency guarantee. Re-running is a no-op that never produces backup churn.
8. **Backup then write**: if `opts.backup`, call `backup_file(&config_path)` (which is a no-op returning `None` if the file doesn't exist, e.g. the `Created` path), then `write_atomic(&config_path, &new_content)`. Return the report with `backup_path` from the backup step.

The ordering — classify before insert, short-circuit Unchanged before backup, backup before write — is what makes the wizard simultaneously idempotent, non-clobbering, and recoverable.

### 14.7 Atomic write (Cloven CRITICO-2 fix)

`write_atomic` (lib.rs:168) is the closure of Cloven's Sprint-04 CRITICO-2 finding: the original implementation used a direct `std::fs::write`, which can leave `~/.claude.json` half-written if the process dies mid-write — and a reader (Claude Code) observing a truncated config is the real corruption bug. The fix is the classic write-to-sidecar-then-rename:

```rust
// lib.rs:168
pub(crate) fn write_atomic(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = sidecar_tmp_path(path);
    let _ = std::fs::remove_file(&tmp_path); // best-effort stale cleanup
    std::fs::write(&tmp_path, content)?;
    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e.into());
    }
    Ok(())
}
```

Key details: parent directories are created first (this is what materializes `~/.cursor/` or `~/.codeium/windsurf/` on a clean machine). The tmp path is a *sibling* of the destination — `sidecar_tmp_path` (lib.rs:184) appends `.seele-tmp-<pid>-<nanos>` to the full path string (e.g. `…/.claude.json.seele-tmp-1234-1700000000000000000`). Using a sibling (not the system temp dir) keeps the rename on the same volume, which is the precondition for atomicity: on POSIX `rename(2)` is atomic for same-volume renames; on Windows `std::fs::rename` issues `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`, atomic for same-volume files. The `<pid>-<nanos>` suffix avoids collisions between concurrent runs, and the best-effort `remove_file` clears any stale tmp from a prior crash so `std::fs::write` always starts fresh. On rename failure, the tmp is cleaned up before returning the error.

The doc comment (lib.rs:148–167) is candid about the **residual race**: if Claude Code is running, it may write `~/.claude.json` between SEELE's in-memory load and the rename, and the rename then clobbers Claude Code's change (last-write-wins). The atomic rename eliminates *partial-write corruption* but not this lost-update window. It is mitigated by the pre-write backup and by `seele setup` being a re-runnable one-shot. The documented v0.2 plan (lib.rs:166–167, echoed in `CLAUDE.md`) is to delegate to `claude mcp add` when the `claude` CLI is present, closing the gap.

### 14.8 Backup behavior

`backup_file` (lib.rs:195) returns `Ok(None)` when `path` does not exist (so a `Created` install never backs up), otherwise copies to a timestamped sibling and returns `Some(backup)`. The filename rule (lib.rs:200–206): take the existing extension; if empty, the backup extension is `bak.<unix_ms>`; otherwise it is `<ext>.bak.<unix_ms>`. Because `~/.claude.json`'s "extension" (per `Path::extension`) is `json`, the backup is `~/.claude.json.bak.<ms>` — matching what `docs/AGENT-SETUP.md` advertises. The timestamp is `chrono::Utc::now().timestamp_millis()`, so repeated runs within the same millisecond could in principle overwrite a backup, but in practice each run produces a distinct file. Backups are only made on the `Added`/`Updated` paths (the `Created` path has no file to copy; the `Unchanged` path short-circuits before backup; `dry_run` never reaches backup).

### 14.9 Idempotency and edge cases

Idempotency is exact and value-based: the `seele` entry is compared structurally (`existing == &seele_entry`) against the freshly built entry, so a second identical run yields `Unchanged` with no write and no backup (asserted by `claude_code_re_running_is_idempotent_unchanged`, which checks `second.backup_path.is_none()`). A stale entry (e.g. an old binary path) yields `Updated` *and* a backup (`claude_code_updates_when_seele_entry_differs`). Non-`seele` keys and unrelated top-level keys are preserved verbatim (`claude_code_adds_to_existing_config_without_clobbering` checks `other-tool` and `user_settings.theme` survive). Notable edge cases and gotchas:

- **Non-clobbering safety**: any non-object root or non-object `mcpServers` aborts with `InvalidExistingConfig` and leaves the file byte-for-byte intact (`invalid_existing_config_errors_loudly_without_clobbering` re-reads and asserts equality).
- **Empty file** is treated as `existed = true` with an empty map (`load_json_object` returns `(Map::new(), true)` for whitespace-only content, agents.rs:189). Because there is no `seele` key but `existed` is true, the match arm `None if existed => Outcome::Added` fires, so a touched-but-empty config reports `Added`, not `Created`. Emptiness counts as existence.
- **Outcome on dry-run is always `DryRun`**, discarding the underlying Created/Added/Updated/Unchanged classification — the preview JSON is the only signal of what *would* change.
- **`seele_binary` default `"seele"`** assumes the binary is on PATH; agents launching MCP servers without the user's PATH may fail to find it, which is why `--seele-binary /abs/path` exists and the docs recommend an absolute path.

### 14.10 CLI wiring (`seele setup`)

`crates/seele-cli/src/commands/setup.rs` translates flags into `InstallOptions` and renders reports. The `Args` struct exposes `--agent <name>`, `--all`, `--list`, `--dry-run`, `--no-backup`, and `--seele-binary <path>`. Flag handling:

- `--list` (handled first): prints all `all_agent_names()`, tagging each `[implemented]` or `[skeleton (v0.2)]` by membership in the `implemented_agent_names()` set; returns early. Honors `--json` via `output::emit_split`.
- Otherwise build `InstallOptions { dry_run: args.dry_run, backup: !args.no_backup, home_override: None, seele_binary: args.seele_binary }`. Note `home_override` is hard-coded `None` in the CLI — the override is purely a test seam. `--no-backup` inverts to `backup: false`.
- `--all`: iterates `implemented_agent_names()` only, collecting `Ok` reports and `Err` strings separately; prints per-agent `agent: Outcome → path` lines and an `errors:` block if any. This is the skeleton-aware path the ALTO Cloven finding produced.
- Single-agent: requires `--agent` (else `anyhow!("--agent <name> or --all required")`), runs `install`, prints one report line.

`--agent` and `--all` are documented as mutually exclusive (the `Args` doc comment), though the code checks `--all` first and would silently ignore a simultaneously-passed `--agent`. The CLI does not validate exclusivity beyond ordering.

### 14.11 Connection to the rest of SEELE

The data crossing the boundary is minimal and outbound only: the wizard writes a `command`/`args` pair that, when the agent later launches it, runs `seele mcp` — handing control to the `seele-mcp` crate's stdio JSON-RPC server (19 `seele_*` tools, §8). There is no runtime coupling; `seele-setup` produces a config file and exits, and (verified by grep) nothing in `seele-mcp` references `seele-setup` or `seele_setup`. Note that `docs/AGENT-SETUP.md` lists a `seele_setup_status` tool among its (also stale) "19 tools" enumeration, but **no such tool exists in source**: the authoritative tool table in `crates/seele-mcp/src/tools.rs` registers exactly 19 tools (`seele_save`, `seele_search`, `seele_show`, `seele_list`, `seele_update_metadata`, `seele_delete`, `seele_restore`, `seele_link`, `seele_stats`, `seele_session_start`, `seele_session_end`, `seele_session_summary`, `seele_capture_passive`, `seele_judge`, `seele_compare`, `seele_suggest_topic_key`, `seele_projects`, `seele_doctor`, `seele_version`) and `seele_setup_status` is not one of them. The count "19" is correct; the specific names in `AGENT-SETUP.md` are not, so do not rely on that doc for the tool inventory — see §8. The two unit tests in `agents.rs` (`agent_kind_round_trips`, `unknown_agent_name_returns_none`) plus the 12 E2E tests in `tests/install_e2e.rs` constitute the crate's verification, all running against `tempfile` homes so no developer's real `~/.claude.json` is ever mutated.
