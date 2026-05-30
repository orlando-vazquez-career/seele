## 13. Multi-Machine Sync — `seele-sync`

`seele-sync` is SEELE's mechanism for moving observations between machines without a server: an export writes a single self-describing, content-addressed, gzipped JSON file ("chunk") that can be committed to a git repository, and an import folds that chunk into another machine's local SQLite database. The design goal is *git-friendly, conflict-free, idempotent* sync: re-running an import is a no-op, two machines that hold the same observation collapse to one row, and the whole import is atomic so a crash cannot leave a half-populated database.

### 13.1 Place in the crate graph

`seele-sync` is a near-leaf crate. Per the workspace layering it depends only on `seele-core` (for the `Observation` domain type, `SeeleId`/ULID, `Scope`, `ObservationType`, `Metadata`) and `seele-storage` (for `ObservationStore`, `ChunkStore`, the raw-save path via `RawSaveInput`/`RawSaveOutcome`, and `ObservationQuery`). It has no dependency on `seele-search`, `seele-embedder`, or any transport crate. Notably the sync feature is **CLI-only**: the only caller in the whole workspace is `crates/seele-cli/src/commands/sync.rs` (`seele sync export|import`); there is no HTTP endpoint and no MCP tool for sync. Confirmed by grepping `crates/` for `export_to_dir`/`import_from_file` — exactly three files reference them: the CLI command, the crate's own `src/lib.rs`, and `tests/roundtrip.rs` (the property suite `tests/properties.rs` only exercises `compute_chunk_id`, so it does not appear in that grep). Embeddings are **not** transported: a chunk carries only the observation rows, so the destination must re-embed (or rely on FTS) after import. The crate is `Cargo.toml`-declared as `description = "Git-friendly compressed-chunk sync between machines."`.

External crates pulled in (`crates/seele-sync/Cargo.toml`): `flate2` (gzip encode/decode), `sha2` + `hex` (SHA-256 chunk id), `serde` + `serde_json` (payload (de)serialization), `chrono` (timestamps), `thiserror` (the error enum), `rusqlite` + `r2d2` (only to convert their error types into `SyncError`), and `tracing`. All are pulled in via `*.workspace = true`. Dev-deps: `tempfile` and `proptest`.

### 13.2 Public API surface

| Item | Signature (abridged) | Purpose |
|------|----------------------|---------|
| `ChunkPayload` | `struct { format_version: u32, seele_version: String, exported_at: DateTime<Utc>, project: Option<String>, observations: Vec<Observation> }` | The on-disk payload, serialized to JSON then gzipped. |
| `ChunkPayload::CURRENT_FORMAT_VERSION` | `const u32 = 1` | The format version this build writes and the max it will read. |
| `ExportFilter` | `struct { project: Option<String> }` (derives `Default`) | Selects which observations to export. v0.1 filters by project only. |
| `ExportReport` | `struct { chunk_id, path, observation_count, bytes_on_disk }` | Returned by `export_to_dir`. |
| `ImportReport` | `struct { chunk_id, path, outcome, observation_count_saved, observation_count_already_present, observation_count_skipped_chunk_level }` | Returned by `import_from_file`. |
| `ImportOutcome` | `enum { Imported, AlreadyImported }` (serde `snake_case`) | Whether the chunk was newly applied or skipped via the ledger. |
| `SyncError` | `enum { Io, Json, Storage, InvalidChunk(String) }` (`thiserror`) | Crate error type. `pub type Result<T> = std::result::Result<T, SyncError>`. |
| `build_chunk` | `fn(&ObservationStore, ExportFilter) -> Result<ChunkPayload>` | Query observations into a payload (no I/O). |
| `compute_chunk_id` | `fn(&ChunkPayload) -> Result<String>` | The canonical SHA-256 content hash (64 hex chars). |
| `export_to_dir` | `fn(&ObservationStore, &Path, ExportFilter) -> Result<ExportReport>` | Build + hash + gzip + write `<chunk_id>.json.gz`. |
| `read_chunk_file` | `fn(&Path) -> Result<(String, ChunkPayload)>` | Decode a chunk file, validate version + filename integrity. |
| `import_from_file` | `fn(&ObservationStore, &ChunkStore, target_key: &str, &Path) -> Result<ImportReport>` | Idempotent, atomic import into the local DB. |

`SyncError` carries `#[from]` conversions for `std::io::Error` (`Io`), `serde_json::Error` (`Json`), and `seele_storage::StorageError` (`Storage`); additional manual `From<rusqlite::Error>` and `From<r2d2::Error>` impls funnel those into `SyncError::Storage` (by routing through `StorageError::from`) so the import's connection/transaction plumbing can use `?` directly (`crates/seele-sync/src/lib.rs:49-59`). Note that `write_chunk_file` is a private `fn` (not part of the public surface); the public export entry point is `export_to_dir`.

### 13.3 The chunk format and its versioning

`ChunkPayload` is the wire format. `format_version` is an explicit, forward-checked integer (`CURRENT_FORMAT_VERSION = 1` for v0.1); `seele_version` (set in `build_chunk` from `env!("CARGO_PKG_VERSION")`) and `exported_at` (set to `Utc::now()`) are *informational only*. The split matters: only `format_version`, `project`, and the observations participate in the content hash, while `seele_version` and `exported_at` do not — this is what makes the chunk id stable across runs and across SEELE versions.

A chunk file is named `<chunk_id>.json.gz` and is the gzip of `serde_json::to_vec(&payload)`. The format is *git-friendly* for several converging reasons: (1) it is a single regular file per export, so a `git add` + commit + push is the entire transport; (2) the filename is content-addressed, so a logically-identical export from a second machine produces the *same filename*, and git deduplicates / shows no diff churn; (3) two diverging exports never overwrite each other — distinct content means distinct chunk id means distinct file, so there is no merge conflict at the git layer; and (4) gzip keeps the committed artifact small. The format being binary (gzipped) means git stores it as a blob rather than diffing line-by-line, which is the intended trade-off.

### 13.4 The content-addressed chunk id

`compute_chunk_id` defines the equivalence relation "same observation set" (`crates/seele-sync/src/lib.rs:162-174`):

```rust
// crates/seele-sync/src/lib.rs:162
pub fn compute_chunk_id(payload: &ChunkPayload) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(payload.format_version.to_le_bytes());
    if let Some(p) = &payload.project {
        hasher.update(p.as_bytes());
    }
    hasher.update([0u8]);                       // null terminator after project (unconditional)
    let mut obs = payload.observations.clone();
    obs.sort_by_key(|o| o.id.to_string());      // canonical ordering by ULID
    let bytes = serde_json::to_vec(&obs)?;
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}
```

Key properties, each pinned by a property test in `crates/seele-sync/tests/properties.rs`:

- **Order-independent** — observations are sorted by stringified ULID before hashing, so in-memory ordering does not change the id (`chunk_id_ignores_observation_order`).
- **Ignores informational fields** — `exported_at` and `seele_version` are not hashed (`chunk_id_ignores_exported_at_and_version`).
- **Content-sensitive** — any change to an observation flips the id (`chunk_id_changes_when_observation_content_changes`).
- **Filter-sensitive** — the `project` filter is mixed in (with a single unconditional `0u8` terminator written after the optional project bytes), so `None` vs `Some("p")` yield different ids even over the same rows (`chunk_id_changes_when_project_filter_changes`). The terminator is unconditional, so it disambiguates `Some("p")` (`p\0`) from `None` (`\0`); it does **not** distinguish `Some("")` from `None` (both reduce to the lone `\0`).

Because the full `Observation` is serialized into the hash, the chunk id is sensitive to mutable fields like `revision_count`, `last_seen_at`, and `metadata` — two machines must hold *byte-identical* observation rows (not just the same id) to produce an identical chunk. That is a deliberate "same snapshot" semantics, not "same id" semantics. (Note this is a property of the *export hash* only; on *import* the raw-save path does not carry those mutable fields — see 13.7.)

### 13.5 Export flow

`export_to_dir` (`lib.rs:181-199`) is the top-level export: it calls `build_chunk`, computes the id, `create_dir_all`s the target directory, joins `<chunk_id>.json.gz`, captures `observation_count` from `payload.observations.len()`, writes via `write_chunk_file`, then `stat`s the file (`std::fs::metadata(&path)?.len()`) for `bytes_on_disk`. `build_chunk` (`lib.rs:136-150`) constructs an `ObservationQuery { project: filter.project.clone(), include_deleted: false, ..Default::default() }` and calls `ObservationStore::list` — so soft-deleted rows (`deleted_at` set) are excluded from export. `write_chunk_file` (`lib.rs:201-208`, private) gzips with `Compression::default()` over a `BufWriter<File>`, and explicitly `encoder.finish()?.flush()?` to guarantee the stream is fully drained before the report's `metadata().len()` reads the size. Writing is idempotent at the filesystem layer: a same-id file is simply overwritten (via `File::create`) with content equal by construction.

### 13.6 Import flow and the Cloven CRITICO-1 fix

`import_from_file` (`lib.rs:281-345`) is the heart of the crate and embodies several review findings from Cloven (the external review agent). The ordered control flow:

1. **Decode + validate**: `read_chunk_file` (`lib.rs:216-244`) opens the file, `GzDecoder`s it over a `BufReader`, `read_to_end` + `serde_json::from_slice` to deserialize the `ChunkPayload`, rejects any `format_version` greater than `CURRENT_FORMAT_VERSION` with `SyncError::InvalidChunk`, recomputes the id, and — *only* when the filename stem (the part before `.json.gz`) is exactly 64 ASCII hex chars — verifies it matches the recomputed id, raising `chunk_id mismatch` otherwise. This integrity check catches silent corruption / tampering but tolerates arbitrarily-named files. It returns `(recomputed_id, payload)`, so the recomputed id (not the filename) is what flows downstream. Tests `read_chunk_file_rejects_unknown_future_format_version` (version `999`) and `read_chunk_file_detects_filename_chunk_id_mismatch` (file renamed to 64 zeros) cover both guards.
2. **Cheap ledger pre-check**: `chunks.was_imported(target_key, &chunk_id)?` — if the `(target_key, chunk_id)` pair is already in the `sync_chunks` ledger, return early with `outcome = AlreadyImported`, `observation_count_skipped_chunk_level = payload.observations.len()`, and zero saves / zero already-present. This avoids even opening a transaction for an already-applied chunk.
3. **Atomic apply**: otherwise, grab one pooled connection (`observations.pool().get()?`) and open a single `Transaction`. Loop over every observation, building a `RawSaveInput` and calling `ObservationStore::save_raw_in_tx`, then `ChunkStore::mark_imported_in_tx`, then `tx.commit()`.

The single-transaction wrapping is **the Cloven CRITICO-1 fix** (Sprint-04 mid-review, 2026-05-10, commit `d152842`). Before it, the import ran each save on its own connection/transaction, so a crash mid-loop left the destination partially populated *and* with no ledger mark — re-running would then double-apply. The doc comment states the contract explicitly:

```text
// crates/seele-sync/src/lib.rs:250
// **Atomic per chunk**: every observation save + the `sync_chunks`
// ledger write run inside a single SQLite transaction. A crash
// mid-loop rolls the entire chunk back ...
```

The atomicity is verified two ways: `import_atomicity_corrupt_chunk_leaves_db_untouched` overwrites the chunk file with `b"corrupt"` so gzip decode fails before the tx opens (DB and ledger both stay empty), and `import_rolls_back_when_save_in_tx_fails` asserts the *positive* invariant that on a healthy import the saves and the ledger row commit together (5 rows + exactly 1 ledger entry whose `chunk_id` matches the report). (The latter test's own comment notes it settles for the positive assertion because portably forcing a mid-tx commit failure is hard; the negative path is covered by the corrupt-chunk test.)

```rust
// crates/seele-sync/src/lib.rs:305 — one connection across the whole import
let mut conn = observations.pool().get()?;
let tx = conn.transaction()?;
// ... loop of save_raw_in_tx(&tx, input) ...
ChunkStore::mark_imported_in_tx(&tx, target_key, &chunk_id)?;
tx.commit()?;
```

### 13.7 Why the raw-save path (the other two Cloven findings)

Each observation is applied through `ObservationStore::save_raw_in_tx`. The public associated method is declared at `crates/seele-storage/src/observations.rs:146` and delegates to the free function `save_raw_in_tx` at `crates/seele-storage/src/observations.rs:582`, **not** the normal `save`/`save_in_tx` pipeline. The raw path emits `INSERT OR IGNORE INTO observations(...)` keyed on the ULID `id` (column list: `id, int_id, session_id, type, title, content, tool_name, project, scope, topic_key, normalized_hash, revision_count, duplicate_count, last_seen_at, created_at, updated_at, metadata`), bypassing privacy-strip, the topic-key upsert, and the dedup window. Notably it hardcodes `revision_count = 0` and `duplicate_count = 0`, recomputes `normalized_hash` from the content, and sets `last_seen_at = updated_at` — so the export-time values of those mutable fields are **not** carried across; only the fields present on `RawSaveInput` (`id`, `session_id`, `kind`, `title`, `content`, `tool_name`, `project`, `scope`, `topic_key`, `created_at`, `updated_at`, `metadata`) ride through. It returns `RawSaveOutcome::{Inserted, AlreadyExisted}`, which the import counts into `observation_count_saved` vs `observation_count_already_present`. This choice produces three guarantees:

- **Source ULIDs ride through unchanged.** The pre-fix import minted *new* ULIDs via the normal save pipeline, which broke cross-machine dedup. With raw save the id is preserved (test `import_preserves_source_observation_ids`).
- **Cross-machine dedup is automatic.** Two machines holding the same observation export it under the same ULID; imported into a common destination, the second one hits `INSERT OR IGNORE` on the shared `id` and is silently skipped — no duplicate row. This is the **Cloven CRITICO** dedup fix (test `re_import_under_different_target_keys_does_not_duplicate`: first import saves 2 / already-present 0, second under a different `target_key` is still `outcome = Imported` but saves 0 / already-present 2, and the destination row count stays 2).
- **No destination data loss on topic-key collision.** The legacy `save_in_tx` topic-key upsert would `UPDATE` a destination row that collides on `(project, scope, topic_key)`, silently overwriting its title/content/metadata with the chunk's. Raw save inserts under the source's own ULID and `INSERT OR IGNORE` leaves the colliding destination row untouched, so both coexist (test `import_does_not_overwrite_destination_topic_key_collision`: both `FROM_SOURCE` and `ALREADY_IN_DEST` survive, count = 2).

A separate **Cloven MEDIO-1** finding is handled inline: `session_id` is *dropped* on import (`session_id: None` in the `RawSaveInput`). Destinations enforce `PRAGMA foreign_keys = ON` and `observations.session_id REFERENCES sessions(id)`, so carrying an unknown session id would abort the whole transaction. Sessions are not part of the v0.1 chunk payload; the crate doc notes the session breadcrumb is also stamped into `metadata` by upstream consumers (MNEMA), making the strip safe, and flags that Sprint-05 may bump `format_version` to 2 to carry thread-level history. Test `import_strips_unknown_session_id_so_fk_constraint_does_not_fire` confirms the import succeeds and the destination row's `session_id` is `NULL` (the source-side test first creates a real session via `SessionStore::start` so the source FK is satisfied locally).

### 13.8 The per-target ledger (`sync_chunks`) — persistence side

Idempotency is anchored by `ChunkStore` in `crates/seele-storage/src/chunks.rs`, backed by table `sync_chunks` (`crates/seele-storage/src/migrations/V001__initial_schema.sql:184-189`). The schema as written (the `-- unix millis` annotation below is editorial; the actual SQL has no inline comments, but `imported_at` is in fact stored via `Utc::now().timestamp_millis()`):

```sql
CREATE TABLE sync_chunks (
    target_key  TEXT NOT NULL,
    chunk_id    TEXT NOT NULL,
    imported_at INTEGER NOT NULL,           -- (editorial) unix millis
    PRIMARY KEY (target_key, chunk_id)
) WITHOUT ROWID;
```

The composite primary key `(target_key, chunk_id)` is what makes re-import a no-op *per node*. `target_key` is a free-form node identifier supplied by the operator via `seele sync import --target-key <hostname|UUID>` (a required `#[arg(long)]` field on `ImportArgs` in `crates/seele-cli/src/commands/sync.rs:31-32`); there is no auto-derivation. `ChunkStore` exposes: `was_imported` (the cheap pre-check, a `SELECT count(*)` that returns `count == 1`), `mark_imported` (standalone `INSERT OR IGNORE`, returns `inserted == 1` i.e. whether newly recorded), `mark_imported_in_tx` (an *associated function* taking a `&rusqlite::Transaction` — the tx-scoped variant the import actually uses so the ledger row commits atomically with the saves), and `list_for_target` (used by tests and any tooling to read a node's import history, ordered `imported_at DESC`). Both insert paths use `INSERT OR IGNORE`, so even a concurrent double-import degrades gracefully to one ledger row. Note there are **two independent idempotency layers**: the chunk-level ledger (skip the whole chunk for a known `(target_key, chunk_id)`) and the row-level `INSERT OR IGNORE` on observation `id` (collapse individual rows even across different target keys). The `ImportReport` counters are deliberately split across these two abstraction levels — `observation_count_skipped_chunk_level` is non-zero only when the *ledger* short-circuits, while `observation_count_already_present` reflects per-row collisions during an actually-applied chunk; a Cloven NIT (`lib.rs:109-111`) documents exactly this distinction.

### 13.9 Edge cases, gotchas, invariants, and known limitations

- **The ~1 MB chunk-splitter is a v0.2 candidate, not shipped.** The crate doc states it plainly (`crates/seele-sync/src/lib.rs:19-20`): *"v0.1 ships a single chunk per export. Sprint-05 may add a size-bound splitter (~1 MB per chunk) if exports grow large enough to matter."* CLAUDE.md lists "sync chunk splitter (~1 MB cap)" among v0.2 features. Today a large export is one large file, loaded entirely into memory on both export and import.
- **Whole-payload-in-memory.** Both `serde_json::to_vec(payload)` (export) and `read_to_end` + `from_slice` (import) materialize the full observation set in RAM; there is no streaming. This is the practical motivation for the future splitter.
- **No deletes / tombstones travel.** Export skips `deleted_at` rows and import only ever inserts (`INSERT OR IGNORE`); a deletion on machine A is never propagated to machine B. Sync is additive-only.
- **Mutable bookkeeping fields are not carried.** `revision_count`, `duplicate_count`, `normalized_hash`, `last_seen_at`, and `deleted_at` are absent from `RawSaveInput`; on import they are reset/recomputed at the destination (`revision_count`/`duplicate_count` to 0, `last_seen_at` to the row's `updated_at`, `normalized_hash` from content). Only when the source row hits an existing `id` is it skipped wholesale (no merge).
- **Filename integrity check is conditional.** `read_chunk_file` only enforces the id-vs-filename match when the stem is exactly 64 ASCII hex chars; a deliberately mis-named file (e.g., `mychunk.json.gz`) bypasses that check but is still validated by version and successfully decoded — the recomputed id is used for the ledger regardless.
- **`format_version` is strictly forward-checked, not range-checked.** Any value `> 1` is rejected; there is no per-version migration path yet, which is acceptable while only v1 exists.
- **No concurrency model beyond SQLite.** The import takes a single pooled connection and one transaction; SQLite's own locking is the concurrency boundary. Two simultaneous imports of the same chunk on the same node either both succeed idempotently or one waits on the DB lock.

### 13.10 File-by-file map

- **`crates/seele-sync/src/lib.rs`** — the entire crate: error enum + conversions (`lib.rs:37-59`), `ChunkPayload`/reports/`ImportOutcome`, `ExportFilter`, the export functions (`build_chunk`, `compute_chunk_id`, `export_to_dir`, private `write_chunk_file`), and the import functions (`read_chunk_file`, `import_from_file`). The module doc comment (`lib.rs:1-20`) is the authoritative narrative of the design and explicitly names the Cloven CRITICO/MEDIO fixes. Inline `#[cfg(test)]` unit tests cover chunk-id determinism (`chunk_id_is_deterministic_for_same_observations`, `chunk_id_differs_when_observations_differ`).
- **`crates/seele-sync/tests/properties.rs`** — proptest suite (`cases: 32` each) pinning the four `compute_chunk_id` invariants: order-independence, ignoring `exported_at`/`seele_version`, content-sensitivity, and project-filter-sensitivity. It imports only `compute_chunk_id` and `ChunkPayload` — no store / no I/O.
- **`crates/seele-sync/tests/roundtrip.rs`** — end-to-end integration tests on real temp SQLite DBs (source + destination): gzip magic-byte check (`export_writes_a_gzipped_file_with_chunk_id_as_name`), clean import, ledger no-op re-import, atomicity on corrupt chunk, the positive atomicity assertion, project-filter exclusion, source-id preservation, cross-target dedup, topic-key non-overwrite, session-id stripping, future-version rejection, and filename mismatch.
- **`crates/seele-sync/Cargo.toml`** — declares deps (flate2, sha2/hex, serde/serde_json, chrono, thiserror, tracing, rusqlite/r2d2 for error conversion) and dev-deps (tempfile, proptest), all via `*.workspace = true`.
- **`crates/seele-storage/src/chunks.rs`** (persistence side) — `SyncChunk` row type + `ChunkStore` with `mark_imported`, `mark_imported_in_tx`, `was_imported`, `list_for_target` over the `sync_chunks` table.
- **`crates/seele-storage/src/observations.rs`** (raw-save side) — `RawSaveInput`/`RawSaveOutcome` (`:49-70`), the public `ObservationStore::save_raw_in_tx` (`:146`) delegating to the free `save_raw_in_tx` (`:582`) that runs the `INSERT OR IGNORE`.
- **`crates/seele-cli/src/commands/sync.rs`** (sole caller) — clap `SyncCmd { Export, Import }`, wires `--project` (on `ExportArgs`) / `--target-key` (on `ImportArgs`), and emits the report via `output::emit_split` (text or `--json`).
