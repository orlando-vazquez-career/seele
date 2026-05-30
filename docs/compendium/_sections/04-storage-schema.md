## 4. Storage Layer I — Schema, Migrations, Connection Pool & vec0 Loading

This section documents the SQLite persistence foundation of SEELE: the `seele-storage` crate's database schema and refinery migration set, the r2d2 connection pool and its canonical PRAGMAs, the runtime mechanism that locates and loads the vendored `sqlite-vec` (`vec0`) loadable extension, the DB open/init flow in `lib.rs`, and the `StorageError` enum. The *stores* layered on top of this foundation (observations, sessions, links, relations, chunks, prompts, dedup, privacy) are covered in Section 5; here we cover only schema, migrations, pool, and extension loading. Where store code is load-bearing for understanding the schema (notably the `int_id` bridge and embedding writes), it is referenced but not re-documented.

### 4.1 Crate placement and responsibilities

`seele-storage` is a leaf-adjacent crate: it depends only on `seele-core` (for `SeeleId`, `ObservationType`, `Scope`, `Metadata`) and on external crates. Everything above it — `seele-search`, `seele-http`, `seele-mcp`, `seele-tui`, `seele-sync`, `seele-engram-import`, `seele-cli` — reaches the database through this crate's `Pool` and store types. The crate's stated purpose in `Cargo.toml:8` is "SQLite + FTS5 + sqlite-vec storage layer for SEELE."

External dependencies (`crates/seele-storage/Cargo.toml`) all use `.workspace = true`; the concrete pins live in the root workspace manifest (`Cargo.toml:44-85`). Their roles:

| Crate | Workspace pin | Role |
| --- | --- | --- |
| `rusqlite` | `0.32`, features `bundled` + `load_extension` | SQLite driver; `bundled` statically compiles SQLite (no system dep), `load_extension` enables loading `vec0` |
| `r2d2` | `0.8` | Generic connection-pool abstraction |
| `r2d2_sqlite` | `0.25` | `SqliteConnectionManager` adapter for r2d2 |
| `refinery` | `0.8`, feature `rusqlite` | Compile-time-embedded SQL migrations |
| `sha2` + `hex` | `0.10` / `0.4` | SHA-256 integrity check + filename derivation for the cached `vec0` binary; also content-dedup hashing (Section 5) |
| `dirs` | `5` | Resolves the user cache directory for the `vec0` install path |
| `serde` / `serde_json` | `1` / `1` | Metadata JSON (de)serialization |
| `thiserror` | `2` | Typed `StorageError` |
| `tracing` | `0.1` | Structured logging |
| `chrono`, `ulid`, `regex`, `once_cell` | `0.4` / `1.1` / `1` / `1.20` | Timestamps, ID handling, privacy regex, lazy statics (used in the stores) |

Dev-dependencies are `tempfile` (throwaway DB dirs in tests) and `proptest` (property tests in `tests/properties.rs`).

### 4.2 File-by-file map

- **`lib.rs`** — Crate root. Declares every module (`chunks`, `error`, `hash`, `links`, `migrations`, `observations`, `pool`, `privacy`, `prompts`, `relations`, `sessions`, `vec0_install`, `vec0_loader`), re-exports the public store types and `Result`/`StorageError`/`Pool`/`PoolConfig`/`init_pool`, and defines the top-level `init_db()` open-and-migrate entry point.
- **`migrations/mod.rs`** — Wraps `refinery::embed_migrations!("./src/migrations")` so the SQL files are compiled into the binary, and exposes `run_pending(conn)`.
- **`migrations/V001__initial_schema.sql`** — The single migration (so far): all tables, indexes, FTS5 virtual tables + triggers, and vec0 virtual table. This is the entire schema.
- **`pool.rs`** — `PoolConfig`, `init_pool()`, the per-connection init closure that sets PRAGMAs and loads `vec0`, and the `load_vec0()` helper using `LoadExtensionGuard`.
- **`vec0_loader.rs`** — Build-time embedding of the per-target `vec0` binary via `include_bytes!`; exposes `vec0_bytes()` and `vec0_extension_suffix()`.
- **`vec0_install.rs`** — Runtime resolution of a usable `vec0` file on disk: env override, cache-dir write with atomic rename, SHA-256 integrity verification. Exposes `ensure_vec0_extension()`.
- **`error.rs`** — `StorageError` enum and the crate's `Result<T>` alias.
- **`vendor/sqlite-vec/`** — Five vendored loadable binaries (one per supported target), upstream checksums, both upstream license texts, and a README documenting provenance and the bump procedure.

### 4.3 The migration system

Migrations are driven by **refinery** with compile-time embedding. `migrations/mod.rs` is intentionally tiny:

```rust
// crates/seele-storage/src/migrations/mod.rs:7-17
use refinery::embed_migrations;

embed_migrations!("./src/migrations");

use crate::error::Result;

/// Run all pending migrations against `conn`.
pub fn run_pending(conn: &mut rusqlite::Connection) -> Result<()> {
    migrations::runner().run(conn)?;
    Ok(())
}
```

`embed_migrations!` scans `./src/migrations` **at compile time** and bakes each `V{N}__{description}.sql` file into the binary, so no SQL is read from disk at runtime — a deliberate property that lets `cargo install seele` produce a self-contained executable. Migration files follow refinery's versioned convention `V{N}__{snake_description}.sql`; the module doc-comment (`migrations/mod.rs:1-5`) instructs that new migrations go in `crates/seele-storage/src/migrations/` as `V{N}__{description}.sql` and are picked up automatically at compile time. Refinery tracks applied versions in its own bookkeeping table (`refinery_schema_history`) inside the same SQLite DB, so `run_pending` is idempotent: re-running it applies only versions not yet recorded. As of workspace version v0.2.0 there is exactly one migration, `V001__initial_schema.sql`, whose own header banner reads "SEELE schema v0.1.0 — initial migration"; the whole schema lives in that file, meaning there have been **no in-place schema evolutions** — the schema was frozen at genesis.

Failures surface as `StorageError::Migration` (a `#[from] refinery::Error`).

### 4.4 Schema reference — tables

`V001__initial_schema.sql` references ADR-02 (`schema-sqlite`), ADR-10 (MNEMA↔SEELE mapping / `context_mode`), and ADR-11 (vendored sqlite-vec). All base tables are declared `WITHOUT ROWID` and use a **TEXT ULID primary key** (`id`). The full table set:

| Table | Purpose | Key columns / constraints |
| --- | --- | --- |
| `sessions` | Agent work sessions | `id` PK; `project` NOT NULL; nullable `directory` and `summary`; `started_at` NOT NULL / `ended_at` nullable, epoch ms; `status` CHECK in (`active`,`ended`,`aborted`) default `active` |
| `observations` | Core memory entity | `id` TEXT PK + `int_id` INTEGER NOT NULL UNIQUE; `session_id` FK→sessions ON DELETE SET NULL; `type`, `title`, `content` NOT NULL; `scope` CHECK in (`project`,`personal`); dedup/soft-delete/timestamp columns; `metadata` JSON default `'{}'` |
| `observations_fts` | FTS5 over observations | external-content table, `content='observations'`, `content_rowid='int_id'` |
| `observations_vec` | vec0 KNN index | `embedding FLOAT[384]` |
| `user_prompts` | Raw user prompts | `id` PK; `session_id` FK→sessions ON DELETE SET NULL; `content` NOT NULL; `project`; `created_at` |
| `prompts_fts` | FTS5 over user_prompts | external-content, `content='user_prompts'` (no `content_rowid`, no sync triggers — see §4.6 gotcha) |
| `links` | General graph edges | `id` PK; `from_id`/`to_id` NOT NULL FK→observations ON DELETE CASCADE; `link_type` NOT NULL; `metadata` JSON default `'{}'`; `created_at`; UNIQUE(`from_id`,`to_id`,`link_type`) |
| `memory_relations` | Judgment-lifecycle relations (ENGRAM-inherited) | `id` PK + `sync_id` NOT NULL UNIQUE; `source_id`/`target_id` NOT NULL FK→observations CASCADE; `relation` NOT NULL; `judgment_status` NOT NULL CHECK in (`pending`,`judged`,`orphaned`,`ignored`) default `pending`; `reason`/`evidence`/`confidence` + `marked_by_actor`/`marked_by_kind`/`marked_by_model` metadata; `session_id` FK SET NULL |
| `sync_chunks` | Git-sync import dedup | composite PK (`target_key`,`chunk_id`); `imported_at` NOT NULL |

#### The `observations` table in detail

```sql
-- crates/seele-storage/src/migrations/V001__initial_schema.sql:33-53
CREATE TABLE observations (
    id                TEXT PRIMARY KEY,     -- ULID
    int_id            INTEGER NOT NULL UNIQUE,  -- ULID random tail mapped to i64
    session_id        TEXT REFERENCES sessions(id) ON DELETE SET NULL,
    type              TEXT NOT NULL,
    title             TEXT NOT NULL,
    content           TEXT NOT NULL,
    tool_name         TEXT,
    project           TEXT,
    scope             TEXT NOT NULL DEFAULT 'project'
                      CHECK (scope IN ('project', 'personal')),
    topic_key         TEXT,
    normalized_hash   TEXT,
    revision_count    INTEGER NOT NULL DEFAULT 0,
    duplicate_count   INTEGER NOT NULL DEFAULT 0,
    last_seen_at      INTEGER NOT NULL,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    deleted_at        INTEGER,
    metadata          TEXT NOT NULL DEFAULT '{}'  -- JSON
) WITHOUT ROWID;
```

Field semantics relevant to this layer:

- **`int_id`** — the authoritative INTEGER rowid surrogate. Because the base tables are `WITHOUT ROWID`, FTS5 and vec0 (which require an integer rowid) cannot reference SQLite's implicit rowid. `int_id` is computed in Rust from `SeeleId::as_i64()` and stored at INSERT time (see §4.6). `UNIQUE` enforces a 1:1 ULID↔int_id mapping per row.
- **`normalized_hash`** — content-dedup key; see §4.8 and Section 5.
- **`revision_count` / `duplicate_count`** — topic-upsert revision counter and within-window duplicate counter (defaults 0).
- **`last_seen_at`, `created_at`, `updated_at`, `deleted_at`** — epoch-ms timestamps; `deleted_at` NULL means *alive* (soft-delete model, §4.8).
- **`metadata`** — JSON blob, default `'{}'`.

##### Virtual generated columns over `metadata`

Five `VIRTUAL` generated columns are added by `ALTER TABLE` so JSON metadata fields become indexable without denormalizing:

```sql
-- crates/seele-storage/src/migrations/V001__initial_schema.sql:56-65
ALTER TABLE observations ADD COLUMN meta_kind TEXT
    GENERATED ALWAYS AS (json_extract(metadata, '$.kind')) VIRTUAL;
ALTER TABLE observations ADD COLUMN meta_domain TEXT
    GENERATED ALWAYS AS (json_extract(metadata, '$.domain')) VIRTUAL;
ALTER TABLE observations ADD COLUMN meta_axiomatic INTEGER
    GENERATED ALWAYS AS (json_extract(metadata, '$.axiomatic')) VIRTUAL;
ALTER TABLE observations ADD COLUMN meta_score REAL
    GENERATED ALWAYS AS (json_extract(metadata, '$.score')) VIRTUAL;
ALTER TABLE observations ADD COLUMN meta_context_mode TEXT
    GENERATED ALWAYS AS (json_extract(metadata, '$.context_mode')) VIRTUAL;
```

These are `VIRTUAL` (computed on read, not stored), so they cost nothing in row size but can back partial indexes. `meta_context_mode` is the ADR-10 MNEMA-bridge field; `meta_axiomatic` marks always-true facts; `meta_kind`/`meta_domain` are taxonomy keys; `meta_score` is a relevance/quality score. These five are the *only* `VIRTUAL` generated columns in the schema (relevant to the `int_id` caveat in §4.6).

### 4.5 Indexes (including partial indexes)

SEELE uses **partial indexes** keyed on `WHERE deleted_at IS NULL` extensively, so soft-deleted rows never bloat the indexes that serve live queries. Full index inventory:

| Index | Table | Columns | Partial predicate |
| --- | --- | --- | --- |
| `idx_sessions_project` | sessions | project | — |
| `idx_sessions_started_at` | sessions | started_at | — |
| `idx_sessions_status` | sessions | status | — |
| `idx_obs_session` | observations | session_id | `deleted_at IS NULL` |
| `idx_obs_project` | observations | project | `deleted_at IS NULL` |
| `idx_obs_topic_upsert` | observations | (project, scope, topic_key) | `topic_key IS NOT NULL AND deleted_at IS NULL` |
| `idx_obs_dedup` | observations | (normalized_hash, project, scope, type, title) | `deleted_at IS NULL` |
| `idx_obs_meta_kind` | observations | meta_kind | `meta_kind IS NOT NULL AND deleted_at IS NULL` |
| `idx_obs_meta_domain` | observations | meta_domain | `meta_domain IS NOT NULL AND deleted_at IS NULL` |
| `idx_obs_meta_kind_domain` | observations | (meta_kind, meta_domain) | `deleted_at IS NULL` |
| `idx_obs_meta_axiomatic` | observations | meta_axiomatic | `meta_axiomatic = 1` |
| `idx_obs_meta_score` | observations | meta_score | `deleted_at IS NULL` |
| `idx_obs_meta_context_mode` | observations | meta_context_mode | `meta_context_mode IS NOT NULL AND deleted_at IS NULL` |
| `idx_obs_created_at` | observations | created_at | `deleted_at IS NULL` |
| `idx_obs_deleted_at` | observations | deleted_at | `deleted_at IS NOT NULL` |
| `idx_prompts_session` | user_prompts | session_id | — |
| `idx_prompts_project` | user_prompts | project | — |
| `idx_prompts_created_at` | user_prompts | created_at | — |
| `idx_links_from` | links | from_id | — |
| `idx_links_to` | links | to_id | — |
| `idx_links_type` | links | link_type | — |
| `idx_relations_source` | memory_relations | source_id | — |
| `idx_relations_target` | memory_relations | target_id | — |
| `idx_relations_status` | memory_relations | judgment_status | — |
| `idx_relations_relation` | memory_relations | relation | — |

Two are noteworthy. `idx_obs_dedup` directly backs the content-dedup lookup (matching `normalized_hash` within a `(project, scope, type, title)` partition). `idx_obs_deleted_at` is the *complement* partial index — it indexes only soft-deleted rows (`deleted_at IS NOT NULL`), so listing/restoring trash is fast without touching the live working set. `idx_obs_meta_axiomatic` is the most narrowly scoped: it indexes only rows where `meta_axiomatic = 1`.

### 4.6 The `int_id` bridge and FTS5 wiring

The single most important schema decision is how a base table keyed by a TEXT ULID drives FTS5 and vec0, both of which require an integer rowid. The migration header records the history (an earlier ADR proposal was broken):

```
-- crates/seele-storage/src/migrations/V001__initial_schema.sql:7-13
-- NOTE on int_id:
--   The arquitectura plan originally proposed a virtual generated column
--   computing int_id from SUBSTR(id, ...) as INTEGER. ULIDs are base32
--   strings, so CAST(letter AS INTEGER) yields 0 — the formula was broken.
--   We instead store int_id as a regular INTEGER column populated at
--   INSERT time from Rust (SeeleId::as_i64). FTS5 and vec0 both reference
--   it for stable rowid mapping.
```

The authoritative mapping is `SeeleId::as_i64()` in `seele-core`, which takes **bytes 9..16 (the last 7 bytes / 56 bits) of the 16-byte ULID** and packs them into an i64 with the top byte zeroed to keep the value non-negative:

```rust
// crates/seele-core/src/id.rs:35-41
pub fn as_i64(&self) -> i64 {
    let bytes = self.0.to_bytes();
    let mut int_bytes = [0u8; 8];
    // Top byte zeroed to keep value positive (sign bit clear).
    int_bytes[1..8].copy_from_slice(&bytes[9..16]);
    i64::from_be_bytes(int_bytes)
}
```

This is a lossy projection — it uses 56 bits of the ULID's random tail, so collisions are theoretically possible (birthday bound ≈ 2^28 ≈ 268M IDs for 50% probability; the `id.rs:28-34` doc-comment frames the same risk as "~1 in 10^4 for 10^6 IDs"). The `observations` insert path defends against this with bounded retry (`ID_COLLISION_RETRIES = 5`, `observations.rs:24`): in the new-row insert loop (`observations.rs:537-580`), on an `Err(rusqlite::Error::SqliteFailure(..))` whose `code` is `ConstraintViolation` and whose message contains `observations.int_id`, it `continue`s — regenerating a fresh `SeeleId` and recomputing `int_id` — and returns `StorageError::Conflict("exhausted 5 retries on int_id collision")` only after exhausting all five attempts. Any other error returns immediately.

**Caveat for cross-section coherence.** The migration's `int_id` column is the authoritative INTEGER mapping used by FTS5/vec0. There is *no* SQL virtual generated `int_id` column in the shipped schema — `int_id INTEGER NOT NULL UNIQUE` is a plain stored column populated from Rust, and the broken `SUBSTR(...)` virtual-column proposal was abandoned (the only `VIRTUAL` generated columns that ship are the five `meta_*` columns). This **contradicts `CLAUDE.md`**, whose "IDs" section still describes a SQL virtual `int_id` column as "aproximada" and `as_i64()` as using the "primeros 6 bytes" — both are stale relative to source. The source of truth is `SeeleId::as_i64()` taking bytes 9..16 (the last 7 bytes / 56 bits), and `int_id` is an exact stored value, not a virtual or approximate one.

#### FTS5 external-content table + sync triggers

```sql
-- crates/seele-storage/src/migrations/V001__initial_schema.sql:90-99
CREATE VIRTUAL TABLE observations_fts USING fts5(
    title, content, tool_name, type, project,
    content='observations',
    content_rowid='int_id',
    tokenize='porter unicode61 remove_diacritics 2'
);
```

This is an **external-content** FTS5 table: it stores only the inverted index, not copies of the text, and uses `int_id` as `content_rowid`. The tokenizer chain is `porter unicode61 remove_diacritics 2` — Porter stemming over Unicode-aware tokenization with full diacritic folding. Because external-content tables are not auto-maintained, three triggers keep the index consistent:

| Trigger | Fires | Action |
| --- | --- | --- |
| `observations_ai` | AFTER INSERT | inserts the new row's searchable columns at `rowid = new.int_id` |
| `observations_ad` | AFTER DELETE | issues the FTS5 `'delete'` command for `old.int_id` (removes index entries) |
| `observations_au` | AFTER UPDATE | a `'delete'` of the old image followed by a re-insert of the new image |

```sql
-- crates/seele-storage/src/migrations/V001__initial_schema.sql:107-117
CREATE TRIGGER observations_ad AFTER DELETE ON observations BEGIN
    INSERT INTO observations_fts(observations_fts, rowid, title, content, tool_name, type, project)
    VALUES('delete', old.int_id, old.title, old.content, old.tool_name, old.type, old.project);
END;
CREATE TRIGGER observations_au AFTER UPDATE ON observations BEGIN
    INSERT INTO observations_fts(observations_fts, rowid, ...) VALUES('delete', old.int_id, ...);
    INSERT INTO observations_fts(rowid, ...) VALUES (new.int_id, ...);
END;
```

(The `observations_ai` trigger at `V001:102-105` is the simple insert: `INSERT INTO observations_fts(rowid, title, content, tool_name, type, project) VALUES (new.int_id, ...)`.)

**Gotcha — soft delete does not de-index FTS.** The triggers fire on physical `DELETE`/`UPDATE`. SEELE's normal "delete" is a soft delete (setting `deleted_at`), which is an `UPDATE`; the `_au` trigger re-inserts the row into FTS unchanged, so soft-deleted observations *remain in the FTS index*. Live-only filtering must therefore be enforced at query time (joining back to `observations` on `deleted_at IS NULL`) — the FTS index alone is not a "live rows" view. This is consistent with `idx_obs_deleted_at` existing to scan soft-deleted rows separately.

**Gotcha — `prompts_fts` has no triggers.** `prompts_fts` is declared as an external-content FTS5 table over `user_prompts` (`V001:137-142`, columns `content`, `project`) but the migration creates **no `_ai`/`_ad`/`_au` triggers for it**, and it omits `content_rowid` (so it would default to the base table's rowid, which does not exist on a `WITHOUT ROWID` table). As written, `prompts_fts` is effectively inert — nothing populates it. This is a latent defect in the schema worth flagging for the improvement pass (Section 23).

#### vec0 virtual table

```sql
-- crates/seele-storage/src/migrations/V001__initial_schema.sql:120-122
CREATE VIRTUAL TABLE observations_vec USING vec0(
    embedding FLOAT[384]
);
```

A single 384-dimension `FLOAT[384]` column matching the all-MiniLM-L6-v2 output. The `vec0` virtual table is keyed by an INTEGER `rowid` set equal to `int_id`. Embeddings are written by `ObservationStore::set_embedding` (`observations.rs:318-338`), which looks up the row's `int_id` (only for an active, `deleted_at IS NULL` row — otherwise `StorageError::NotFound`), packs the `&[f32]` into little-endian bytes, and runs `INSERT OR REPLACE INTO observations_vec(rowid, embedding)`. `hard_delete` clears the vec row too, explicitly to avoid a future `int_id` collision surfacing a stale embedding (`observations.rs:305-311`); a standalone `delete_embedding` (`observations.rs:340-349`) does the same on demand. KNN queries against this table are the vector half of hybrid search (Section 7).

### 4.7 Connection pool and PRAGMAs (`pool.rs`)

The pool type is a thin alias: `pub type Pool = r2d2::Pool<SqliteConnectionManager>` (`pool.rs:10`). `PoolConfig` carries the DB `path` and `max_size`; `PoolConfig::with_path` defaults `max_size` to **8** connections (`pool.rs:18-25`).

`init_pool` builds the manager with an init closure run on **every checked-out connection** (since each pooled SQLite connection is independent and PRAGMAs/loaded extensions are per-connection):

```rust
// crates/seele-storage/src/pool.rs:30-48
pub fn init_pool(cfg: PoolConfig) -> Result<Pool> {
    let vec0_path = vec0_install::ensure_vec0_extension()?;
    let manager = SqliteConnectionManager::file(&cfg.path).with_init(move |conn| {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             PRAGMA temp_store=MEMORY;",
        )?;
        load_vec0(conn, &vec0_path)
    });
    let pool = r2d2::Pool::builder().max_size(cfg.max_size).build(manager)
        .map_err(StorageError::Pool)?;
    Ok(pool)
}
```

PRAGMA rationale:

| PRAGMA | Value | Why |
| --- | --- | --- |
| `journal_mode` | `WAL` | Write-Ahead Logging lets readers proceed concurrently with a writer (multiple readers + one writer), critical because several transports (CLI/HTTP/MCP) and the pool's up-to-8 connections may touch the DB simultaneously. WAL is also faster for the small frequent writes SEELE does. (`journal_mode` is persistent per-DB, but is re-asserted on every connection harmlessly.) |
| `synchronous` | `NORMAL` | The recommended pairing with WAL: durable enough (no corruption on app crash) while avoiding an `fsync` on every commit, trading a small window of last-transaction loss on OS/power failure for throughput. |
| `foreign_keys` | `ON` | SQLite defaults FK enforcement *off* per-connection; SEELE relies on `ON DELETE CASCADE`/`SET NULL` (links, relations, sessions↔observations), so it must be explicitly enabled on every connection. |
| `temp_store` | `MEMORY` | Temp tables / sort spills go to RAM, speeding `ORDER BY` and FTS auxiliary work. |

Crucially, `ensure_vec0_extension()` is called **once** at pool construction (resolving/writing the binary to disk), but `load_vec0` runs **per connection** (inside the `with_init` closure) so every connection in the pool has the `vec0` functions and virtual-table module available. The init closure makes vec0 loading a hard requirement — a connection that fails to load the extension fails to enter the pool (doc comment `pool.rs:27-29`: "Failure to load the extension is fatal — SEELE depends on hybrid search"). A connection-level test, `pool_loads_vec0_and_pragmas` (`pool.rs:75-98`, cfg-gated to the five supported targets), asserts `vec_version()` responds and that `journal_mode`/`foreign_keys` are set.

`load_vec0` itself:

```rust
// crates/seele-storage/src/pool.rs:50-61
fn load_vec0(conn: &rusqlite::Connection, path: &Path) -> rusqlite::Result<()> {
    unsafe {
        let _guard = rusqlite::LoadExtensionGuard::new(conn)?;
        // Exported init function is `sqlite3_vec_init` (no `0` suffix).
        conn.load_extension(path, Some("sqlite3_vec_init"))?;
    }
    Ok(())
}
```

Two correctness points: (1) `LoadExtensionGuard` temporarily enables extension loading and disables it again when dropped (a security default, since arbitrary native code can be loaded — hence the `unsafe` block). (2) The entry-point symbol is passed **explicitly** as `Some("sqlite3_vec_init")`; the loadable *file* is named `vec0` but its exported init function has no `0` suffix, so passing `None` (which would derive the symbol from the filename → `sqlite3_vec0_init`) would fail to find the entry point.

### 4.8 WAL, dedup, and soft-delete invariants (summary)

- **WAL** is the journaling mode; combined with `synchronous=NORMAL` it provides reader/writer concurrency suitable for the multi-transport single-binary design, at the cost of WAL/SHM sidecar files (`-wal`, `-shm`) next to the DB.
- **Dedup** hinges on `normalized_hash` (computed in `hash.rs` / `privacy.rs`, Section 5), the partial index `idx_obs_dedup`, and the `DEDUP_WINDOW_MS = 24h` window (`observations.rs:22`, `24 * 60 * 60 * 1000`). Within the window, a re-save of the same normalized content under the same `(project, scope, type, title)` increments `duplicate_count` rather than inserting a new row.
- **Soft delete** is the `deleted_at` timestamp column. NULL = alive. Every live-query partial index carries `WHERE deleted_at IS NULL`; the complementary `idx_obs_deleted_at` serves trash listing/restore. As noted in §4.6, soft delete does not remove a row from FTS, so query-time filtering is mandatory.

### 4.9 Vendored `vec0` loading (build-time embed + runtime install)

SEELE ships the `sqlite-vec` loadable extension *inside the binary* rather than depending on a runtime download or a build script. This is split across two files.

**Build time — `vec0_loader.rs`.** Per host target, a `cfg`-gated `const VEC0_BYTES: Option<&[u8]>` is set with `include_bytes!` pointing at the matching vendored file. The five supported targets and their filenames:

| `cfg` target | Embedded file |
| --- | --- |
| linux + x86_64 | `vendor/sqlite-vec/linux-x86_64/vec0.so` |
| linux + aarch64 | `vendor/sqlite-vec/linux-aarch64/vec0.so` |
| macos + x86_64 | `vendor/sqlite-vec/macos-x86_64/vec0.dylib` |
| macos + aarch64 | `vendor/sqlite-vec/macos-aarch64/vec0.dylib` |
| windows + x86_64 | `vendor/sqlite-vec/windows-x86_64/vec0.dll` |

Any other target (`#[cfg(not(any(...)))]`) sets `VEC0_BYTES = None`. `vec0_bytes() -> Option<&'static [u8]>` returns those bytes; `vec0_extension_suffix() -> &'static str` returns `.so` / `.dylib` / `.dll` (empty string on unsupported OS) so the on-disk file gets the suffix the OS loader (`dlopen`/`LoadLibrary`) needs — SQLite itself infers nothing from the filename. Two tests enforce the contract: a soft size sanity check (`vec0_bytes_size_sanity`, `> 1024` bytes, ungated), and a hard `vec0_bytes_required_on_supported_targets` (cfg-gated to the five targets) that asserts `Some` and `> 100_000` bytes so a future bump that breaks all `include_bytes!` cannot pass silently (`vec0_loader.rs:89-104`). The five vendored binaries on disk total ~874 KB and range from ~128 KB (macos-x86_64) to ~289 KB (windows-x86_64), comfortably above both thresholds.

**Runtime — `vec0_install.rs`.** `ensure_vec0_extension() -> Result<PathBuf>` resolves a usable file on disk with this precedence:

1. **`$SEELE_VEC_PATH`** override (`VEC0_ENV_OVERRIDE`) — if set and pointing to an existing file (`p.is_file()`), use it verbatim (escape hatch for unsupported targets / debugging). If set but missing, return `StorageError::InvalidInput`.
2. **Vendored bytes** → write to `<user-cache>/seele/vec0-<sha><suffix>`. The SHA-256 prefix (first 16 hex chars, `SHA_PREFIX_LEN = 16`) of the embedded bytes is part of the filename, so a content change produces a new path and old cached copies are never silently reused. If the target file already exists, `verify_integrity` re-hashes it and returns it (or errors `Vec0IntegrityMismatch` on tamper); otherwise the bytes are written via `write_atomic`.
3. **No vendored bytes and no override** → `StorageError::Vec0NotSupportedTarget { os, arch }` (populated from `std::env::consts::OS` / `ARCH`).

The cache dir comes from `dirs::cache_dir()` joined with `seele`; if `cache_dir()` returns `None`, `StorageError::Vec0CacheUnresolvable`. `write_atomic` (`vec0_install.rs:72-100`) writes to a per-call unique temp name (`<name>.<pid>.<counter>.tmp`, with a process-static `AtomicU64` counter incremented `Relaxed`) then `fs::rename`s onto the target — atomic on POSIX. It explicitly handles the Windows case where rename fails if the target exists by treating "rename errored but target is now a file" as a benign post-race success and removing the temp, deferring to `verify_integrity` to catch genuine corruption. This makes concurrent installers (e.g. parallel test threads) safe.

**Vendoring strategy and provenance.** `vendor/sqlite-vec/README.md` documents that the binaries are upstream `sqlite-vec` **v0.1.9** (dual-licensed Apache-2.0 OR MIT, © 2024 Alex Garcia), redistributed with both license texts (`LICENSE-APACHE-upstream.txt`, `LICENSE-MIT-upstream.txt`) and the upstream `checksums.txt` (`CHECKSUMS-upstream.txt`) for manual re-verification. The README's rationale (mirroring ADR-11): a `build.rs` download would force network access at build time and break offline `cargo install`, and consolidated Rust bindings did not exist at genesis, so vendoring the loadable gives deterministic control of the exact binary in the final executable. The README states the accepted cost as ~880 KB across the five files (measured ~874 KB on disk). A six-step bump procedure is documented (bump README + CHANGELOG, download all `loadable-{target}.tar.gz`, extract each `vec0.{so,dylib,dll}`, replace `CHECKSUMS-upstream.txt`, run `cargo test -p seele-storage`, commit `vendor: bump sqlite-vec to vX.Y.Z`). CLAUDE.md reinforces that the vendored binaries must not be touched without updating this README.

### 4.10 Error model (`error.rs`)

`StorageError` is the crate's single error type; `pub type Result<T> = std::result::Result<T, StorageError>`. Variants:

| Variant | Source / trigger | Notes |
| --- | --- | --- |
| `Sqlite(rusqlite::Error)` | `#[from]` | Any SQLite-level failure |
| `Pool(r2d2::Error)` | `#[from]` (also mapped explicitly via `.map_err(StorageError::Pool)` in `init_pool`) | Pool build/checkout failure |
| `Migration(refinery::Error)` | `#[from]` | Migration apply failure |
| `Io(std::io::Error)` | `#[from]` | File I/O during vec0 install |
| `Vec0NotSupportedTarget { os, arch }` | `ensure_vec0_extension` | No vendored binary + no override (`os`/`arch` are `&'static str`) |
| `Vec0CacheUnresolvable` | `resolve_cache_dir` | `dirs::cache_dir()` returned `None` |
| `Vec0IntegrityMismatch { path, expected, actual }` | `verify_integrity` | Cached binary SHA prefix mismatch |
| `NotFound(String)` | stores | e.g. missing/inactive observation on `set_embedding`, soft-delete, hard-delete |
| `InvalidInput(String)` | stores / install | bad ULID, bad `$SEELE_VEC_PATH`, non-serializable metadata |
| `Conflict(String)` | observations insert | exhausted `int_id` collision retries |

Failure modes worth highlighting: pool init is **all-or-nothing** — if `ensure_vec0_extension` fails (unsupported target, unresolvable cache, integrity mismatch) the pool is never built, so the whole storage layer refuses to come up rather than silently degrading without vector search. There is no fallback to a non-vec0 mode at this layer (contrast the embedder, which per CLAUDE.md falls back transparently to a fake embedder with a warning when ONNX init fails).

### 4.11 Open/init flow and store wiring (`lib.rs`)

The single entry point combines all of the above:

```rust
// crates/seele-storage/src/lib.rs:34-39
pub fn init_db(path: impl AsRef<Path>) -> Result<Pool> {
    let pool = init_pool(PoolConfig::with_path(path))?;
    let mut conn = pool.get()?;
    migrations::run_pending(&mut conn)?;
    Ok(pool)
}
```

Order is significant: (1) `init_pool` resolves+caches the `vec0` binary and builds the pool, so the *first* connection checkout already has PRAGMAs set and `vec0` loaded — necessary because the migration creates a `vec0` virtual table (`CREATE VIRTUAL TABLE observations_vec USING vec0(...)`), which would fail if the extension were not loaded first. (2) A connection is checked out and `run_pending` applies any unapplied migrations (idempotent). (3) The `Pool` is returned. Every downstream store (`ObservationStore`, `SessionStore`, `LinkStore`, `RelationStore`, `PromptStore`, `ChunkStore` — the store types and their input/query types are re-exported from `lib.rs:17-25`) is constructed from a clone of this `Pool` and checks out connections as needed. The doc comment summarizes it: "Open a DB at `path`, applying canonical PRAGMAs, loading vec0, and running pending migrations. Idempotent." (`lib.rs:32-33`). This is the function every transport's bootstrap path ultimately calls to obtain a ready database.
