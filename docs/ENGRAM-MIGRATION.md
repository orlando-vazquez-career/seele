# ENGRAM → SEELE migration

If you are an ENGRAM / MNEMA user moving to SEELE, this is the doc that
walks you through the migration. ADR-13 covers the design rationale;
this doc covers operations.

## Who should read this

- You have an existing `~/.mnema/mnema.db` (or another ENGRAM-shaped
  SQLite) and want to keep the observations you've already collected.
- You want your tools that call `mnema_*` to keep working under SEELE
  with no code change on their side (compat layer via `--tool-prefix
  mnema`).

If you're a fresh user with no prior ENGRAM data, skip this doc — go to
[`AGENT-SETUP.md`](AGENT-SETUP.md) instead.

## Pre-migration — back it up

The import is one-shot and idempotent on the SEELE side, but if you'd
rather not depend on that:

```bash
cp ~/.mnema/mnema.db ~/.mnema/mnema.db.bak.$(date +%F)
```

ENGRAM/MNEMA writes to that single file by default. Once it's backed
up, the rest of the steps are safe to retry.

## Step 1 — dry-run

Always run with `--dry-run` first. It prints what *would* happen
without writing to the destination DB:

```bash
seele import from-engram ~/.mnema/mnema.db --dry-run
```

Expected output (abridged):

```
dry-run: would insert 1432 observations, 18 sessions, 247 links.
unmapped linked_to[]: 3 ids (dangling, will be skipped).
re-embed: skipped (no --re-embed flag).
```

The `unmapped linked_to[]` count is observations whose `linked_to` array
references an id that does not appear anywhere in the source DB. These
links are silently skipped because there is no destination to point
them at; the source observation is still imported, only the dangling
edge is dropped.

## Step 2 — real run

Drop `--dry-run`:

```bash
seele import from-engram ~/.mnema/mnema.db
```

Same numbers, this time actually persisted. The import:

- Wraps everything in a single SQLite transaction (atomic per run).
- Preserves source ULIDs (`INSERT OR IGNORE` on `id`, so re-runs are
  idempotent — see ADR-13 §rationale).
- Maps ENGRAM `linked_to[]` arrays to first-class rows in the `links`
  table.
- Does **not** re-compute embeddings by default. Existing vectors are
  imported as-is so `seele search` works immediately.

### `--re-embed` (rare)

If you want SEELE to recompute embeddings against its own model:

```bash
seele import from-engram ~/.mnema/mnema.db --re-embed
```

This is slow (~5-20 ms per observation depending on length) and only
necessary if the source DB's embedder is incompatible with SEELE's
`all-MiniLM-L6-v2`. The default (no `--re-embed`) keeps the source
vectors and works for any embedder that produced 384-dim L2-normalized
output — which ENGRAM's does.

## Step 3 — verify

```bash
seele list --project <your-project> --limit 5
seele stats
seele doctor
```

You should see your old observations under the same project name they
had in ENGRAM. The `stats` count should match the dry-run's "would
insert" line.

If something looks off, the source DB is untouched — you can re-run
the import after fixing whatever was wrong. The destination already
has rows under the preserved ULIDs, so a second `import from-engram`
no-ops on those rows (counted as `observation_count_already_present`
in the report).

## Step 4 — keep your tools working

Tools that already call `mnema_save`, `mnema_recall`, etc. don't need
to be rewritten. Run SEELE's MCP server with the compat prefix:

```bash
seele mcp --tool-prefix mnema
```

That exposes the same 19 tools as `mnema_save`, `mnema_recall`,
`mnema_get`, … Same schemas, same semantics.

The `seele setup` wizard installs the default `seele_*` namespace. To
expose the `mnema_*` aliases instead, run the wizard normally and then
edit the resulting MCP config to append `"--tool-prefix", "mnema"` to
the `args` array — e.g. in `~/.claude.json`:

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

For HTTP consumers, the same idea is available via:

```bash
seele serve --legacy-engram-paths
```

That alias-mounts ENGRAM-shaped REST routes alongside the SEELE-canonical
ones.

## Caveats

- **`linked_to[]` dangling ids are dropped.** ENGRAM allows
  observations to reference ids that no longer exist (or never did).
  SEELE's `links` table requires both endpoints to exist; we drop
  dangling edges and count them in the dry-run report. The
  observation itself is still imported.
- **Sessions are imported but `linked_to` chains across sessions are
  not.** Within a session, observations link normally; across sessions
  the ENGRAM data does not carry enough context to reconstruct the
  edge cleanly. This rarely matters in practice (ENGRAM tools don't
  emit cross-session links).
- **Topic key collisions in the destination.** If you import twice
  with different `--db` paths and then sync them via `seele sync`,
  observations land via `save_raw_in_tx` (not the upsert path), so
  topic_key collisions are not silently merged. See ADR-13.

## Rollback

The migration writes to the destination only. To roll back, delete the
destination (e.g. `rm ~/.seele/seele.db`) and keep using ENGRAM. Your
backup from Step 0 is the safety net for the source side, but the
import itself never touched it.

## Related

- [ADR-13](../genesis/plans/arquitectura/13-engram-compatibility.md) — design
  rationale for the migration, including the `linked_to[]` decision.
- [`AGENT-SETUP.md`](AGENT-SETUP.md) — wiring SEELE into your agent
  after migration.
- [`INSTALLATION.md`](INSTALLATION.md) — how to install `seele` itself.
