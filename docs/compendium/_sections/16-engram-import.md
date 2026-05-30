## 16. ENGRAM / MNEMA Import — `seele-engram-import`

`seele-engram-import` is the one-shot migration subsystem that ports an existing ENGRAM/MNEMA SQLite database into a SEELE database. It implements the first decision of ADR-13 ("ENGRAM compatibility"): give consumers that already run on ENGRAM (notably MNEMA, whose persistence layer is `~/.mnema/mnema.db`) a way to move to SEELE without discarding the observations they have accumulated. The second ADR-13 decision — the runtime `--tool-prefix mnema` / `--legacy-engram-paths` aliasing shim — is handled in the MCP and HTTP crates and is out of scope for this section. (ADR-13 itself lives at `genesis/plans/arquitectura/13-engram-compatibility.md` per the migration guide's "Related" links; it is not present in this checkout, so claims attributed to it below are flagged where they go beyond what the code or `docs/ENGRAM-MIGRATION.md` directly state.)

### 16.1 Place in the crate graph

The crate sits in the upper-middle of the dependency layering. Per `crates/seele-engram-import/Cargo.toml`, it depends only on two internal crates — `seele-core` (for `SeeleId`, `ObservationType`, `Scope`, `Metadata`, imported via three `use` lines) and `seele-storage` (for `LinkInput`, `LinkStore`, `ObservationStore`, `RawSaveInput`, `RawSaveOutcome`, `StorageError`) — plus external crates `serde`, `serde_json`, `thiserror`, `tracing`, `chrono`, `rusqlite`, `r2d2`, and `ulid` (pinned `version = "1.1"`, with the `serde` feature). It deliberately does *not* depend on `seele-embedder`, `seele-search`, or any transport crate. That isolation is an observable fact from the manifest; the rationale that it is meant to let future importers (JSON, CSV) drop in side-by-side is plausible but is not stated in the crate's own source (the module docs do not mention JSON/CSV importers) and the sprint-04 devlog that would record it is not present in this checkout, so treat that rationale as unverified. Its only caller is `seele-cli` (`crates/seele-cli/src/commands/import.rs`), which wires the `seele import from-engram` subcommand to it.

### 16.2 Public API surface

The public surface is intentionally small: one struct, one constructor, one method, one report struct, and one error enum.

| Item | Signature | Purpose |
| --- | --- | --- |
| `EngramImporter<'a>` | `struct` holding `observations: &'a ObservationStore`, `_links: &'a LinkStore` | The importer, borrowing both destination stores (struct decl `lib.rs:90`, after the `#[derive]`-free doc-commented definition). |
| `EngramImporter::new` | `fn new(observations: &'a ObservationStore, links: &'a LinkStore) -> Self` | Wire the importer to a destination. **Both stores must point at the same SEELE DB** (`lib.rs:98`). |
| `EngramImporter::import_from` | `fn import_from(&self, source: &Path, dry_run: bool) -> Result<ImportReport>` | Run the migration; `dry_run=true` opens and maps but writes nothing (`lib.rs:107`). |
| `ImportReport` | `struct` (Serialize) with eight counters + `dry_run` + `errors` | Result of both real and dry-run runs (`#[derive]` at `lib.rs:60`, `struct` at `lib.rs:61`). |
| `EngramImportError` | `enum` (thiserror) | Typed hard errors (`#[derive]` at `lib.rs:40`, `enum` at `lib.rs:41`). |
| `Result<T>` | `type Result<T> = std::result::Result<T, EngramImportError>` | Crate-local result alias (`lib.rs:56`). |

Note the `_links` field is underscore-prefixed: `LinkStore` is borrowed only so the constructor signature documents the contract that links and observations share a pool, but the actual link writes go through the associated function `LinkStore::create_in_tx(&tx, ...)`, driven on the importer's own transaction rather than through the borrowed store instance.

#### `ImportReport` fields

| Field | Type | Meaning |
| --- | --- | --- |
| `source_path` | `PathBuf` | The source DB path passed in. |
| `rows_seen` | `usize` | Total rows the source `memories` table exposed. |
| `rows_inserted` | `usize` | Rows newly inserted into the destination. |
| `rows_skipped_existing` | `usize` | Rows whose preserved `id` already existed (idempotent skip). |
| `rows_invalid` | `usize` | Rows that could not be mapped (bad metadata); listed in `errors`. |
| `links_created` | `usize` | `linked_to` references resolved and inserted as `links` rows. |
| `links_already_present` | `usize` | `linked_to` tuples already present, skipped to keep re-runs idempotent. |
| `links_dangling` | `usize` | `linked_to` references whose target was not found — dropped, not fatal. |
| `dry_run` | `bool` | `true` when produced by a dry-run call. |
| `errors` | `Vec<String>` | Soft errors (per-row mapping failures). Hard errors short-circuit to `Err`. |

The struct carries **no session counter** despite the operator-guide example output mentioning sessions (see 16.10 / 16.11 — that is documentation drift, since the importer never touches sessions: `map_one` always sets `session_id: None`).

#### `EngramImportError` variants

| Variant | Trigger |
| --- | --- |
| `OpenSource { path, detail }` | `Connection::open_with_flags` on the source failed (`lib.rs:270`). |
| `BadSchema { detail }` | No `memories` table, or a required column is missing (raised in `verify_schema`, `lib.rs:278-302`). |
| `Storage(StorageError)` | `#[from]` propagation from `save_raw_in_tx` / `create_in_tx`. |
| `Sqlite(rusqlite::Error)` | `#[from]` propagation from any raw SQL call. |
| `Pool(r2d2::Error)` | `#[from]`; failure to check out a connection from the destination pool. |
| `InvalidMetadata { id, detail }` | Declared (`lib.rs:52-53`) but **never constructed** — per-row metadata failure is surfaced as a *soft* error string in `report.errors`, not this hard variant. |

### 16.3 Source schema and the schema contract

The importer opens the source database **read-only** — `Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)` inside `open_source` (`lib.rs:269-276`) — so the migration physically cannot mutate the ENGRAM file. The migration guide's rollback story rests on this: "the import itself never touched it."

`verify_schema` (`lib.rs:278-302`) enforces two preconditions. First, `sqlite_master` must contain a table named `memories`; otherwise `BadSchema { detail: "table \`memories\` not found" }`. Second, `PRAGMA table_info(memories)` (via `pragma_columns`, `lib.rs:304-313`, which reads the column name from PRAGMA result index 1) must expose the four mandatory columns `id`, `body`, `metadata`, `created_at`; any missing one yields `BadSchema { detail: "\`memories\` is missing required column \`{required}\`" }`. The optional ENGRAM/MNEMA columns the module docs name as "picked up when present" — `title`, `updated_at`, `project`, `scope`, `tool_name`, `session_id`, `type`, `topic_key` — are ignored at the SQL level on this read path; everything SEELE needs beyond the four mandatory columns is read out of the `metadata` JSON blob. (The module docs do not mention any `mnema_*` virtual columns; that claim from an earlier draft was unsupported and has been dropped.) The actual read is a flat `SELECT id, body, metadata, created_at FROM memories` (`lib.rs:329`, inside `read_and_map_rows`).

### 16.4 Field mapping (ENGRAM → SEELE)

`map_one` (`lib.rs:365-414`) converts one source row into a `RawSaveInput`. The mapping is:

| Source | Destination | Notes |
| --- | --- | --- |
| `memories.id` | `RawSaveInput.id` | Preserved if a valid ULID, else minted (see 16.5). |
| `memories.body` | `RawSaveInput.content` | Verbatim. |
| `memories.created_at` | `created_at` **and** `updated_at` | Both set to the same value; the source carries no separate updated time on this path. |
| `metadata.kind` | `RawSaveInput.kind` | Parsed via `ObservationType::from_str_relaxed` (`seele-core` `memory.rs:76`; unknown ⇒ `Memory`); also re-stored in metadata as the canonical `as_str()` form. |
| `metadata.project` | `RawSaveInput.project` | |
| `metadata.scope` | `RawSaveInput.scope` | Parsed via `Scope::from_str_strict` (`seele-core` `memory.rs:111`); on failure defaults to `Scope::Project`. |
| `metadata.topic_key` | `RawSaveInput.topic_key` | |
| `metadata.tool_name` / `metadata.tool` | `RawSaveInput.tool_name` | Either key accepted; the original key (`tool_name` or `tool`) is preserved in metadata. |
| `metadata.linked_to[]` | `links` rows (pass 2) | See 16.6. |
| any other metadata key | preserved in `RawSaveInput.metadata` | Whole blob passes through (`domain`, `axiomatic`, `verdict_id`, `earn_score`, etc.). |

Note `RawSaveInput.session_id` is hard-coded to `None` (`lib.rs:396` region): no session is ever derived from the source on this path.

`created_at` parsing is defensive (`read_created_at`, `lib.rs:343-357`): it first tries `i64` epoch-milliseconds, then an RFC-3339 string, then epoch-ms-encoded-as-text, and finally falls back to `Utc::now()`. `ms_to_dt` (`lib.rs:359-363`) guards against out-of-range millisecond values by also falling back to `Utc::now()`.

The title is synthesized by `synthesize_title` (`lib.rs:529-545`) because ENGRAM's `memories` has no first-class title on this read path: it prefers `metadata.title` if non-blank, otherwise takes the first non-empty line of the body (trimmed), truncating to 77 chars + `"..."` when the first line exceeds 80 characters (counted in `chars`, not bytes), and returns `"(untitled)"` for an empty body.

`parse_metadata` (`lib.rs:445-516`) returns a `ParsedMetadata` struct (introduced specifically to avoid a 7-tuple return type that clippy's `type_complexity` lint rejected — doc comment + struct at `lib.rs:416-428`). It treats `None`, the empty/whitespace string, and the literal `"null"` as an empty (default) metadata. A non-JSON blob returns `Err("metadata not JSON: …")`; a JSON value that is not an object returns `Err("metadata not an object: …")`. Crucially these are returned as `String` errors, which `map_one` converts into a **soft** per-row failure (`MappedRow.mapped = None`, `map_error = Some(...)`), not a hard `Err`.

### 16.5 ULID preservation and the `engram_id` breadcrumb

`resolve_dest_id` (`lib.rs:522-527`) is the heart of id handling:

```rust
// crates/seele-engram-import/src/lib.rs:522
fn resolve_dest_id(source: &str) -> (SeeleId, bool) {
    if let Ok(id) = source.parse::<SeeleId>() {
        return (id, true);
    }
    (SeeleId::new(), false)
}
```

If the source id parses as a ULID it is preserved verbatim (`preserved = true`). Otherwise — e.g. an ENGRAM cuid-style id like `mem_abc123` — a fresh `SeeleId` is minted and `map_one` stashes the original under `metadata.engram_id` so it remains recoverable post-migration (`lib.rs:384-389`). The E2E tests `import_preserves_ulid_id_when_source_id_is_a_ulid` and `import_stashes_engram_id_in_metadata_when_source_is_not_a_ulid` (`tests/import_e2e.rs:85,107`) pin both branches.

Preserving the ULID is what makes re-runs idempotent: `save_raw_in_tx` issues `INSERT OR IGNORE INTO observations(...)` keyed on the `(id)` primary key (`crates/seele-storage/src/observations.rs:590-595`). A second import of the same source therefore inserts nothing for that row (rowcount 0) and returns `RawSaveOutcome::AlreadyExisted`, counted in `rows_skipped_existing`. Minted (non-ULID-source) ids are *not* idempotent across runs by themselves — each run mints a new ULID — but this is acceptable because real ENGRAM/MNEMA data uses ULIDs; the cuid path is a fallback.

### 16.6 `linked_to[]` → first-class `links` rows

`import_from` runs in two passes over the same in-memory `Vec<MappedRow>`, inside one transaction.

**Pass 1** (`lib.rs:140-166`): insert every mappable observation via `ObservationStore::save_raw_in_tx(&tx, input.clone())`. As each row lands (whether `Inserted` or `AlreadyExisted`), the importer records `engram_id (text) → dest SeeleId` in an `id_map: HashMap<String, SeeleId>`. Rows that failed to map (`mapped == None`) increment `rows_invalid` and push their `map_error` into `errors`.

**Pass 2** (`lib.rs:189-223`): for every source row that itself mapped, resolve each entry of its `linked_to` array to a destination `SeeleId`, then insert a `links` row of type `"related_to"`. The link type is the literal string `"related_to"` in the code (`lib.rs:203,211`); the migration guide attributes the `linked_to[]` decision to ADR-13, but ADR-13 is not in this checkout, so any framing of `related_to` as a deliberate choice "over `derives_from`" is not corroborated by available sources — what is verifiable is only that the code always uses `"related_to"`. Resolution order for each target `to_engram` (`lib.rs:194-200`):

1. The current import's `id_map` (this run's rows).
2. Failing that, try `to_engram.parse::<SeeleId>()` and probe whether that ULID already exists in the destination `observations` table via `observation_exists_in_tx`. This closes a Cloven 2026-05-11 [MEDIO] finding (cited in the source comment, `lib.rs:173-176`): link targets that live in a *previous* import pass should resolve, not be counted dangling. The test `import_resolves_linked_to_against_rows_from_a_previous_pass` (`tests/import_e2e.rs:160`) guards it.
3. Anything else → **dangling**: counted in `links_dangling`, dropped, not fatal. The code notes a deliberately deferred case (`lib.rs:177-180`): a cuid-style id whose new ULID lives behind `metadata.engram_id` would require a JSON scan to resolve and is "deferred to v0.2 if real migrations need it."

Link idempotency is handled explicitly because, as the comment notes (`lib.rs:182-185`), the `links` table has **no UNIQUE constraint** on `(from_id, to_id, link_type)`. Before each insert the importer probes via `link_exists_in_tx` (`lib.rs:253-267`); a hit increments `links_already_present` and skips the insert. This closes Cloven 2026-05-11 [MEDIO 2] and is pinned by `re_running_import_does_not_duplicate_links` (`tests/import_e2e.rs:226`).

```rust
// crates/seele-engram-import/src/lib.rs:201
match to_dest {
    Some(to_dest) => {
        if link_exists_in_tx(&tx, &from_dest, &to_dest, "related_to") {
            links_already_present += 1;
        } else {
            LinkStore::create_in_tx(&tx, LinkInput {
                from_id: from_dest, to_id: to_dest,
                link_type: "related_to".to_string(), metadata: Metadata::new(),
            })?;
            links_created += 1;
        }
    }
    None => { links_dangling += 1; }
}
```

After both passes, `tx.commit()` (`lib.rs:225`) makes the whole import atomic per source file. If a source row's own observation failed to map, the pass-2 loop `continue`s and skips that row's links entirely (`lib.rs:190-193`).

### 16.7 `save_raw_in_tx`: bypassing re-embedding while preserving ids

The import never goes through `ObservationStore::save()` (the normal write path with privacy stripping, topic-key upsert, and the 24h normalized-hash dedup window — `DEDUP_WINDOW_MS = 24 * 60 * 60 * 1000`, `observations.rs:22`). Instead it uses `ObservationStore::save_raw_in_tx` (public wrapper at `crates/seele-storage/src/observations.rs:146-151`, body at `:582-619`), a raw `INSERT OR IGNORE` that takes a caller-owned `id` and timestamps and writes the row verbatim. The store doc-comment states the precondition: "the source data has already been audited / trusted" (`observations.rs:42-43`).

This is the mechanism behind the default (no-`--re-embed`) behavior described in `docs/ENGRAM-MIGRATION.md`: because the raw insert touches only the `observations` table and never invokes the embedder, the source's existing vectors carry over as-is (the guide states this works for any embedder producing 384-dim L2-normalized output, "which ENGRAM's does"). Notably, the `int_id` column is set from `input.id.as_i64()` (`observations.rs:588`). Per the repo's ID convention, `SeeleId::as_i64()` (bytes `9..16` of the ULID — the last 7 bytes / low 56 bits) is the *authoritative* ULID→vec0-rowid bridge, written into the regular **stored** `int_id` column — so writing the authoritative value keeps a preserved ULID consistent with the vec0 mapping. (`int_id` is a stored value, not virtual; `CLAUDE.md`'s "approximate virtual column" note is stale.) (The raw insert also writes a `normalized_hash` of the content and `revision_count`/`duplicate_count` of 0, but does **not** run the dedup window or topic upsert.) `--re-embed` is the rare opt-in to recompute embeddings against SEELE's own model — but see the important caveat in 16.10: in v0.1 the flag is a no-op.

### 16.8 Dry-run

When `dry_run=true` (`lib.rs:113-126`), `import_from` returns *before* checking out any connection. It still opens the source, verifies schema, and maps every row, so it can report `rows_seen`, count `rows_invalid` (`mapped.is_none()`), surface the same `errors`, and report `links_dangling` as the *total* count of all `linked_to` entries across mapped rows (`mapped.iter().map(|r| r.linked_to.len()).sum()`, `lib.rs:122`). Important subtlety: in dry-run, `links_dangling` over-reports — it counts every prospective link as dangling rather than running pass-2 resolution, and `links_created`/`links_already_present`/`rows_inserted`/`rows_skipped_existing` are all forced to 0. `dry_run_writes_nothing_but_returns_counts` (`tests/import_e2e.rs:351`) verifies the destination stays empty.

### 16.9 Error handling and atomicity model

Two error tiers coexist. **Hard errors** (cannot open source, bad schema, SQL/pool/storage failures, a `save_raw_in_tx` or `create_in_tx` returning `Err`) short-circuit via `?` and propagate as `EngramImportError`; because everything happens inside one uncommitted transaction, a hard error mid-loop drops the transaction and rolls back all writes — the destination is left untouched. **Soft errors** (per-row invalid metadata) do not abort: they are counted in `rows_invalid`, collected in `errors`, and the transaction still commits the good rows. The test `import_atomicity_failure_rolls_back_partial_writes` (`tests/import_e2e.rs:288`) documents that true mid-tx crash atomicity is impractical to test, so it instead verifies the soft-error counter advances without panic while the good row still lands. There is no async, locking, or concurrency in this crate beyond the single `r2d2` pool checkout (`self.observations.pool().get()?`, `lib.rs:128`) and one `rusqlite::Transaction`.

### 16.10 CLI integration

`crates/seele-cli/src/commands/import.rs` is the sole caller. `ImportCmd::FromEngram(FromEngramArgs)` exposes `path: PathBuf`, `--re-embed` (field `re_embed`), and `--dry-run` (field `dry_run`). `run_from_engram` (`import.rs:48-100`) resolves the destination DB (`--db` or `default_db_path`), creates parent dirs, calls `init_db` (which runs migrations, so an absent destination is initialized), builds the two stores from one shared pool, and calls `importer.import_from(&args.path, args.dry_run)`. It then emits the report through `output::emit_split` (human text or `--json`). The human text prints `rows seen / rows inserted / rows skipped (exist) / rows invalid / links created / links dangling` and the `errors` list — note it does **not** print `links_already_present` or any session count. The CLI E2E tests `import_from_engram_against_synthetic_source_inserts_rows` and `import_from_engram_rejects_db_without_memories_table` (`crates/seele-cli/tests/subcommands_e2e.rs:286,333`) assert the JSON counters (including idempotent re-run) and that a non-ENGRAM DB exits non-zero with "memories" in stderr.

Two notable gotchas live in the CLI layer, not the crate. First, `--re-embed` is recognized but a **no-op**, surfaced via `tracing::warn!` (`import.rs:64-69`); the doc-comment on the `re_embed` field (`import.rs:27-29`) calls it recognized "but a no-op until Sprint-05 lands the real ONNX backend" — at odds with `docs/ENGRAM-MIGRATION.md`, which documents `--re-embed` as functional (and references SEELE's `all-MiniLM-L6-v2` model). This is a documentation/implementation drift worth flagging to the improvement reviewer. (Note the source comment's "v0.1 ships FakeEmbedder / ONNX lands in Sprint-05" framing itself sits uneasily next to `CLAUDE.md`, which states ONNX is the default in v0.1 — a separate drift in the comment.) Second, the importer itself ignores `--re-embed` entirely; the flag is read only in the CLI and never reaches `import_from`, so even when the ONNX backend exists, re-embedding would need additional wiring.

### 16.11 File-by-file map

- **`crates/seele-engram-import/src/lib.rs`** — the entire crate. Module docs (`:1-24`) state the contract: read-only source, `memories`→`observations` mapping, `save_raw_in_tx` atomic per file, `INSERT OR IGNORE` idempotency, ULID preservation with `engram_id` fallback, and `linked_to` registered during the main pass and resolved once all observations are in. Contains the public API (`EngramImporter`, `import_from`, `ImportReport`, `EngramImportError`, `Result`), the internal `MappedRow`/`ParsedMetadata` structs, the source helpers (`open_source`, `verify_schema`, `pragma_columns`, `read_and_map_rows`, `read_created_at`, `ms_to_dt`), the mappers (`map_one`, `parse_metadata`, `resolve_dest_id`, `synthesize_title`), the tx probes (`observation_exists_in_tx`, `link_exists_in_tx`), and **11 unit tests** (`#[cfg(test)] mod tests`) covering title synthesis, id resolution, and metadata parsing.
- **`crates/seele-engram-import/Cargo.toml`** — declares the crate (description "ENGRAM-to-SEELE migration tool (ADR-13)."), its two internal path deps (`seele-core`, `seele-storage`), the external deps (`serde`, `serde_json`, `thiserror`, `tracing`, `chrono`, `rusqlite`, `r2d2`, `ulid`), and `tempfile` as the sole dev-dependency for the E2E harness.
- **`crates/seele-engram-import/tests/import_e2e.rs`** — **13 integration tests** that synthesize a real ENGRAM-shaped SQLite DB and run the importer against a fresh SEELE destination, guarding the ADR-13 contract end to end: basic insert, ULID preservation, `engram_id` breadcrumb, `linked_to`→links, cross-pass resolution, dangling counting, link de-duplication, ULID idempotency, soft-error handling, both schema rejections (no `memories` table and missing required column), dry-run, and metadata field extraction (`project`/`scope`/`topic_key`).
- **`crates/seele-cli/tests/subcommands_e2e.rs`** — the two CLI-level E2E tests for this subcommand (`import_from_engram_against_synthetic_source_inserts_rows` at `:286`, `import_from_engram_rejects_db_without_memories_table` at `:333`).
- **`docs/ENGRAM-MIGRATION.md`** — the operator guide: back up the source, dry-run first, real run, the (documented-as-functional) `--re-embed`, verification with `list`/`stats`/`doctor`, the `--tool-prefix mnema` and `--legacy-engram-paths` runtime shims, caveats (dangling ids dropped, cross-session `linked_to` chains not reconstructed, topic-key collisions not merged because the raw path is used), and rollback (delete the destination; the source was never touched). Caution: the guide's example dry-run output and a caveat both speak of "sessions" being imported, but the importer code never reads or writes sessions (`session_id` is always `None`) — that is drift in the guide, not behavior of this crate.

### 16.12 Edge cases, invariants, and limitations

- **Read-only source is an invariant** — the source is opened `SQLITE_OPEN_READ_ONLY`; the migration is strictly one-way. The "no SEELE→ENGRAM / no continuous sync" framing is attributed to ADR-13's "Out of scope" section, which is not present in this checkout; the verifiable fact is the read-only open flag in `open_source`.
- **`InvalidMetadata` is dead** — declared (`lib.rs:52-53`) but never constructed; per-row metadata failures travel as soft strings. A reviewer may want to remove it or wire it in.
- **Sessions are not imported** — `map_one` hard-codes `session_id: None`; nothing in the crate reads a session from the source. The migration guide's "Sessions are imported" caveat contradicts this and should be corrected.
- **Dry-run `links_dangling` over-counts** — it reports total `linked_to` entries, not the true dangling subset, since pass-2 resolution is skipped (`lib.rs:122`).
- **Non-ULID source ids are not idempotent across runs** — each run mints a fresh ULID, so re-importing cuid-keyed rows would duplicate them; only ULID-keyed rows are protected by `INSERT OR IGNORE`.
- **No `links` UNIQUE constraint** — link idempotency depends entirely on the `link_exists_in_tx` probe; if a concurrent writer inserted between probe and insert, a duplicate could still appear (no such concurrency exists in the single-tx flow).
- **Deferred resolution case** — cuid `linked_to` targets whose new ULID is behind `metadata.engram_id` are dropped as dangling (`lib.rs:177-180`); the source comment defers a JSON-scan resolution to v0.2.
- **`--re-embed` doc/impl drift** — documented as functional in `ENGRAM-MIGRATION.md` but a `tracing::warn!` no-op in the CLI and entirely unread by `import_from`.
