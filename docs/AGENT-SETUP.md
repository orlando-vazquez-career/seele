# Agent setup

SEELE ships an MCP stdio server (`seele mcp`) that exposes 19 tools
under the `seele_*` namespace (or `mnema_*` with `--tool-prefix mnema`
for ENGRAM/MNEMA compat — see [`ENGRAM-MIGRATION.md`](ENGRAM-MIGRATION.md)).

`seele setup` is a wizard that wires SEELE into your agent's MCP config
file with an atomic write + backup of the previous content. Three
agents are implemented in v0.1; five are skeleton (validate the agent
name but do not yet write a real config).

| Agent | Status |
|---|---|
| `claude-code` | ✅ implemented |
| `cursor` | ✅ implemented |
| `windsurf` | ✅ implemented |
| `opencode` | 🟡 skeleton (returns `NotImplemented`; lands v0.2) |
| `aider` | 🟡 skeleton |
| `cody` | 🟡 skeleton |
| `continue` | 🟡 skeleton |
| `zed` | 🟡 skeleton |

List the full set at runtime with:

```bash
seele setup --list
```

## Claude Code

```bash
seele setup --agent claude-code
```

The wizard:
1. Locates `~/.claude.json` (per Anthropic's spec).
2. Backs up the existing file to `~/.claude.json.bak.<timestamp>`
   unless `--no-backup` was passed.
3. Inserts or updates a `seele` entry under `mcpServers` pointing
   `command: <path-to-seele>`, `args: ["mcp"]`.
4. Validates the resulting JSON.

Verify by restarting Claude Code and listing tools (the slash command
`/mcp` shows configured servers). You should see `seele_save`,
`seele_recall`, … (19 in total).

## Cursor

```bash
seele setup --agent cursor
```

Writes to `~/.cursor/mcp.json`. Same backup + atomic write pattern.
Restart Cursor and check the MCP settings panel; the `seele` server
should report "connected" and list the 19 tools.

## Windsurf

```bash
seele setup --agent windsurf
```

Writes to `~/.codeium/windsurf/mcp_config.json`. Backup, atomic write,
same flow.

## Doing it manually

The wizard is convenience — the format is just MCP-standard JSON.
If your agent isn't supported yet, drop this snippet into its MCP
config:

```json
{
  "mcpServers": {
    "seele": {
      "command": "/absolute/path/to/seele",
      "args": ["mcp"]
    }
  }
}
```

For the ENGRAM compatibility namespace (drop-in for tools that already
call `mnema_*`):

```json
{
  "mcpServers": {
    "seele": {
      "command": "/absolute/path/to/seele",
      "args": ["mcp", "--tool-prefix", "mnema"]
    }
  }
}
```

## Dry-run + diff

Every wizard run accepts `--dry-run`:

```bash
seele setup --agent claude-code --dry-run
```

This prints the resulting JSON to stdout without touching the
filesystem — useful before committing a config change into a synced
dotfiles repo.

## Verifying tools/list

If the wizard ran cleanly but the agent does not see SEELE tools, the
underlying MCP transport may be misconfigured. You can stress-test
SEELE directly:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | seele mcp
```

Expected: one JSON line back with 19 tool descriptors. If that works
but the agent does not, the agent's MCP config is the issue, not SEELE.

## What the 19 tools do

The full list (alphabetical):

`seele_save`, `seele_recall`, `seele_get`, `seele_list`, `seele_delete`,
`seele_restore`, `seele_link`, `seele_stats`, `seele_doctor`,
`seele_projects`, `seele_relate`, `seele_judge`, `seele_relations`,
`seele_session_start`, `seele_session_end`, `seele_session_summary`,
`seele_export_chunk`, `seele_import_chunk`, `seele_setup_status`.

Detailed schemas live in
[`crates/seele-mcp/src/tool_impls/`](../crates/seele-mcp/src/tool_impls/).
