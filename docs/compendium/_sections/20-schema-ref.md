## 20. Database Schema Reference (Data Dictionary)

This section is a pure reference appendix: a column-by-column data dictionary for every table, virtual table, index, and trigger in the SEELE SQLite database. Everything here is grounded in the single migration file `crates/seele-storage/src/migrations/V001__initial_schema.sql` (the only migration in the tree) plus the column usage confirmed in the store modules (`observations.rs`, `prompts.rs`, `sessions.rs`, `relations.rs`, `links.rs`, `chunks.rs`).

### 20.1 Migration / schema version

SEELE uses [`refinery`](https://crates.io/crates/refinery) for migrations. The migration set is embedded at compile time:

```rust
// crates/seele-storage/src/migrations/mod.rs:7,9
use refinery::embed_migrations;
embed_migrations!("./src/migrations");
```

`run_pending(conn)` (`migrations/mod.rs:14-17`) runs all pending migrations via `migrations::runner().run(conn)`; `init_db(path)` (`lib.rs:34-39`) opens the pool, then checks out a connection and calls `run_pending`. The migrations directory currently contains exactly **one** file, `V001__initial_schema.sql`, so the **current applied migration version is V1** (refinery records this in its own bookkeeping table `refinery_schema_history`, created and managed automatically by refinery — it is not declared in any SEELE SQL). The header of V001 labels the logical schema as `SEELE schema v0.1.0` (`V001:2`). There is **no** hand-rolled `schema_version` table in the live DB (the genesis plans proposed one, but the implementation delegates to refinery). When the MCP `seele_doctor` tool returns a `schema_version` field, that value is `env!("CARGO_PKG_VERSION")` (`crates/seele-mcp/src/tool_impls/meta.rs:30`), i.e. the binary/crate version — `0.2.0` per the workspace `Cargo.toml` (`seele-mcp` uses `version.workspace = true`) — **not** the SQL migration number. Do not conflate the two.

A note on a table referenced elsewhere: `sync_apply_deferred` appears in `CREDITS.md` and the genesis ENGRAM-audit docs (`genesis/plans/estrategia/02-reimplementacion-inspirada.md`, `05-engram-feature-audit.md` marks it as "v0.2", needed only with cloud sync) as part of the inherited 9-table design, but it is **not** created by V001 and **does not exist** in the SEELE database. V001 declares exactly **9 table objects**: 6 ordinary `WITHOUT ROWID` tables (`sessions`, `observations`, `user_prompts`, `links`, `memory_relations`, `sync_chunks`) and 3 virtual tables (`observations_fts`, `observations_vec`, `prompts_fts`). FTS5 and vec0 each silently create additional internal shadow tables beyond those 9 (see 20.4 / 20.5).

Connection-level invariants applied to every pooled connection in `init_pool`'s `with_init` callback (`pool.rs:33-41`): `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`, `temp_store=MEMORY` (executed as a single `execute_batch`, then the vec0 extension is loaded). Because `foreign_keys=ON` is set per connection, the FK constraints below are actually enforced. Loading the vec0 extension is fatal on failure — the pool refuses to build without it.

#### Conventions used in this dictionary
- All `*_at` timestamp columns are stored as `INTEGER` epoch **milliseconds** (Rust `chrono::Utc::now().timestamp_millis()`); they are decoded back via `Utc.timestamp_millis_opt(ms).single()`.
- All `id`/`*_id` columns hold **ULID text** (26-char Crockford base32), the textual form of `SeeleId`, except `int_id` (see below).
- "Nullable" reflects the `NOT NULL` constraints in DDL.

### 20.2 `sessions`

A coding session. `WITHOUT ROWID`, PK on the ULID `id`.

| Column | SQL type | Null | Default | Meaning |
|---|---|---|---|---|
| `id` | TEXT | no | — | PK. Session ULID. |
| `project` | TEXT | no | — | Project name the session belongs to. |
| `directory` | TEXT | yes | — | Working directory path of the session. |
| `started_at` | INTEGER | no | — | Epoch-ms session start. |
| `ended_at` | INTEGER | yes | — | Epoch-ms end; NULL while active. |
| `summary` | TEXT | yes | — | End-of-session summary text. |
| `status` | TEXT | no | `'active'` | Lifecycle. `CHECK (status IN ('active','ended','aborted'))`. |

- **Primary key:** `id`.
- **Indexes:** `idx_sessions_project(project)`, `idx_sessions_started_at(started_at)`, `idx_sessions_status(status)` — all non-partial.
- **State transitions (sessions.rs):** `start` inserts `status='active'` with only `(id, project, directory, started_at, status)` set (`sessions.rs:34-57`); `end` flips to `'ended'` setting `ended_at`+`summary` **only** `WHERE status='active'` (`sessions.rs:59-73`); `abort` flips to `'aborted'` setting `ended_at` only `WHERE status='active'` (`sessions.rs:75-89`). Both `end`/`abort` return `StorageError::NotFound` if zero rows updated (no active session by that id).

### 20.3 `observations` (core entity)

The central memory record. `WITHOUT ROWID`, PK on ULID `id`, with a separate `int_id` used as the stable rowid bridge to FTS5 and vec0.

| Column | SQL type | Null | Default | Meaning |
|---|---|---|---|---|
| `id` | TEXT | no | — | PK. Observation ULID. |
| `int_id` | INTEGER | no | — | `UNIQUE`. ULID **random tail** mapped to i64; the FTS5/vec0 rowid bridge. |
| `session_id` | TEXT | yes | — | FK → `sessions(id)` `ON DELETE SET NULL`. |
| `type` | TEXT | no | — | Observation kind (e.g. `code`, `decision`, `learning`). FTS-indexed. |
| `title` | TEXT | no | — | Short title. Privacy-stripped before insert. FTS-indexed. |
| `content` | TEXT | no | — | Body text. Privacy-stripped; hashed for dedup. FTS-indexed. |
| `tool_name` | TEXT | yes | — | Originating tool. FTS-indexed. |
| `project` | TEXT | yes | — | Project scoping key. |
| `scope` | TEXT | no | `'project'` | `CHECK (scope IN ('project','personal'))`. |
| `topic_key` | TEXT | yes | — | Upsert key (e.g. `architecture/db`). NULL = no upsert. |
| `normalized_hash` | TEXT | yes | — | sha256 hex of normalized `content`; **dedup hash column**. |
| `revision_count` | INTEGER | no | `0` | Incremented on each topic_key upsert. |
| `duplicate_count` | INTEGER | no | `0` | Incremented on each dedup-window merge. |
| `last_seen_at` | INTEGER | no | — | Epoch-ms; touched on upsert/dedup merge. |
| `created_at` | INTEGER | no | — | Epoch-ms creation. |
| `updated_at` | INTEGER | no | — | Epoch-ms last mutation. |
| `deleted_at` | INTEGER | yes | — | **Soft-delete** marker; NULL = active. |
| `metadata` | TEXT | no | `'{}'` | Arbitrary JSON blob. |

**Virtual generated columns** (added by `ALTER TABLE` after table creation, `GENERATED ALWAYS AS ... VIRTUAL`, projecting `json_extract` paths out of `metadata`; these are computed on read, never stored — `V001:55-65`):

| Column | SQL type | JSON path |
|---|---|---|
| `meta_kind` | TEXT | `$.kind` |
| `meta_domain` | TEXT | `$.domain` |
| `meta_axiomatic` | INTEGER | `$.axiomatic` |
| `meta_score` | REAL | `$.score` |
| `meta_context_mode` | TEXT | `$.context_mode` |

- **Primary key:** `id`. **Unique:** `int_id`.
- **Foreign keys:** `session_id → sessions(id) ON DELETE SET NULL`.

**Indexes (`V001:67-87`; most live-row indexes are partial on `deleted_at IS NULL`):**

| Index | Columns | Partial predicate |
|---|---|---|
| `idx_obs_session` | `(session_id)` | `WHERE deleted_at IS NULL` |
| `idx_obs_project` | `(project)` | `WHERE deleted_at IS NULL` |
| `idx_obs_topic_upsert` | `(project, scope, topic_key)` | `WHERE topic_key IS NOT NULL AND deleted_at IS NULL` |
| `idx_obs_dedup` | `(normalized_hash, project, scope, type, title)` | `WHERE deleted_at IS NULL` |
| `idx_obs_meta_kind` | `(meta_kind)` | `WHERE meta_kind IS NOT NULL AND deleted_at IS NULL` |
| `idx_obs_meta_domain` | `(meta_domain)` | `WHERE meta_domain IS NOT NULL AND deleted_at IS NULL` |
| `idx_obs_meta_kind_domain` | `(meta_kind, meta_domain)` | `WHERE deleted_at IS NULL` |
| `idx_obs_meta_axiomatic` | `(meta_axiomatic)` | `WHERE meta_axiomatic = 1` |
| `idx_obs_meta_score` | `(meta_score)` | `WHERE deleted_at IS NULL` |
| `idx_obs_meta_context_mode` | `(meta_context_mode)` | `WHERE meta_context_mode IS NOT NULL AND deleted_at IS NULL` |
| `idx_obs_created_at` | `(created_at)` | `WHERE deleted_at IS NULL` |
| `idx_obs_deleted_at` | `(deleted_at)` | `WHERE deleted_at IS NOT NULL` (inverted: indexes the trash) |

**Column-usage notes confirmed in `observations.rs`:**
- `idx_obs_dedup` mirrors the dedup query key `(normalized_hash, project, scope, type, title)` plus a `last_seen_at >= now-DEDUP_WINDOW_MS` window and `deleted_at IS NULL`, where `DEDUP_WINDOW_MS = 24*60*60*1000` (24h, `observations.rs:22`). The dedup query (`observations.rs:495-520`) matches `project` with explicit NULL handling (`(project IS NULL AND ?2 IS NULL) OR project = ?2`). A matching live row increments `duplicate_count` and updates `last_seen_at` instead of inserting (`SaveOutcome::DuplicateMerged`, `observations.rs:521-535`).
- `idx_obs_topic_upsert` backs the topic upsert (`observations.rs:447-491`): a live row with the same `(project, scope, topic_key)` is updated in place. The UPDATE overwrites `title`, `content`, `normalized_hash`, `metadata`, `type`, `tool_name`, bumps `revision_count` by 1, and touches `last_seen_at`/`updated_at` (`SaveOutcome::UpsertedTopic`). Note the topic-key project match uses `project IS ?1` (NULL-safe equality), not `=`.
- `normalized_hash` is sha256-hex over normalized content: lowercase → split on whitespace and rejoin with single spaces (which also trims) → sha256 → hex (`hash.rs:17-28`); always 64 lowercase hex chars when present.
- `deleted_at` drives soft delete/restore: `soft_delete` sets `deleted_at = now` (and `updated_at = now`) only `WHERE deleted_at IS NULL` (`observations.rs:267-281`); `restore` clears it (and sets `updated_at = now`) only `WHERE deleted_at IS NOT NULL` (`observations.rs:283-297`); `hard_delete` does a real `DELETE` and then best-effort purges the vec0 row (`observations.rs:299-313`). Both `soft_delete` and `restore` return `NotFound` if zero rows matched.
- **`int_id` is a regular stored column, not a virtual generated column.** The DDL comment (`V001:7-13`) is explicit: an earlier plan tried a virtual generated `int_id` via `SUBSTR(id,...) AS INTEGER`, which is broken because ULIDs are base32 strings (`CAST` yields 0). SEELE instead stores `int_id` as a regular `INTEGER NOT NULL UNIQUE` column populated at INSERT time from Rust `SeeleId::as_i64()` (`observations.rs:540` in `save_in_tx`, `observations.rs:588` in `save_raw_in_tx`). The canonical mapping is `id.as_i64()`, which takes **ULID bytes 9..16** (the 7-byte / 56-bit random tail) into a big-endian i64 with the top byte zeroed to stay non-negative (`crates/seele-core/src/id.rs:35-41`) — note this is the random tail, **not** the timestamp prefix, and **not** "the first 6 bytes" (as some older ADR-02 / `CLAUDE.md` text claims; the running code uses bytes 9..16). On `int_id` collision the insert retries up to `ID_COLLISION_RETRIES = 5` times with a fresh ULID (`observations.rs:24, 538-579`), detecting the collision by matching a `ConstraintViolation` whose message contains `"observations.int_id"`; exhausting retries yields `StorageError::Conflict`.

### 20.4 `observations_fts` (FTS5 shadow table)

External-content FTS5 index over `observations`.

```sql
-- V001:90-99
CREATE VIRTUAL TABLE observations_fts USING fts5(
    title, content, tool_name, type, project,
    content='observations',
    content_rowid='int_id',
    tokenize='porter unicode61 remove_diacritics 2'
);
```

- **Indexed columns:** `title`, `content`, `tool_name`, `type`, `project`.
- **External content:** `content='observations'`, `content_rowid='int_id'` — the FTS rows are keyed on `observations.int_id`, not the table's implicit rowid (there is none; the table is `WITHOUT ROWID`). FTS5 also materializes its own internal shadow tables (`observations_fts_data`, `_idx`, `_docsize`, `_config`) automatically; these are FTS5 implementation detail and are not declared in SEELE SQL.
- **Tokenizer:** Porter stemmer over `unicode61`, `remove_diacritics 2`.

**FTS sync triggers** (`V001:101-117`; keep the external-content index consistent using the standard FTS5 external-content idiom of inserting the special `'delete'` command row):

| Trigger | Fires | One-line body summary |
|---|---|---|
| `observations_ai` | `AFTER INSERT` | Inserts a new FTS row `(rowid=new.int_id, title, content, tool_name, type, project)` (`V001:102-105`). |
| `observations_ad` | `AFTER DELETE` | Emits the `'delete'` command row with `old.*` to retract the FTS entry for `old.int_id` (`V001:107-110`). |
| `observations_au` | `AFTER UPDATE` | First emits the `'delete'` row for `old.*`, then inserts the fresh row for `new.*` (delete-then-reinsert). |

```sql
-- V001:112-117 (observations_au)
CREATE TRIGGER observations_au AFTER UPDATE ON observations BEGIN
    INSERT INTO observations_fts(observations_fts, rowid, title, content, tool_name, type, project)
    VALUES('delete', old.int_id, old.title, old.content, old.tool_name, old.type, old.project);
    INSERT INTO observations_fts(rowid, title, content, tool_name, type, project)
    VALUES (new.int_id, new.title, new.content, new.tool_name, new.type, new.project);
END;
```

Gotcha: the triggers fire on **physical** row changes. A soft delete (`UPDATE ... SET deleted_at`) fires `observations_au`, which re-indexes the (still-present) row — so soft-deleted observations remain in the FTS index; callers must filter on `deleted_at IS NULL` after the FTS join. Only `hard_delete` (physical `DELETE`) removes the FTS entry via `observations_ad`.

### 20.5 `observations_vec` (vec0 virtual table)

KNN vector index from the vendored sqlite-vec (`vec0`) extension.

```sql
-- V001:119-122
CREATE VIRTUAL TABLE observations_vec USING vec0(
    embedding FLOAT[384]
);
```

- **Single column:** `embedding FLOAT[384]` — a 384-dim float32 vector (matching all-MiniLM-L6-v2 output).
- **Implicit `rowid`:** the vec0 INTEGER rowid is set to `observations.int_id` at write time. `set_embedding` resolves `int_id` for an **active** (non-soft-deleted) observation, packs the `&[f32]` little-endian via `f.to_le_bytes()`, and does `INSERT OR REPLACE INTO observations_vec(rowid, embedding)` (`observations.rs:318-338`). It returns `NotFound` if no active observation has that id. Caller is responsible for normalizing the vector and for the dimension matching the column.
- **No triggers** keep vec rows in sync — embedding writes/deletes are explicit (`set_embedding` at `observations.rs:318`, `delete_embedding` at `observations.rs:341-349`, and the vec cleanup inside `hard_delete` at `observations.rs:307-311`). Both the `hard_delete` cleanup and `delete_embedding` delete via `WHERE rowid IN (SELECT int_id FROM observations WHERE id = ?1)`. vec0 also creates internal shadow tables (`observations_vec_chunks`, `_rowids`, `_vector_chunks00`, etc.) automatically.

### 20.6 `user_prompts` + `prompts_fts`

Short records of user input. `WITHOUT ROWID`, PK on ULID `id`.

| Column | SQL type | Null | Default | Meaning |
|---|---|---|---|---|
| `id` | TEXT | no | — | PK. Prompt ULID. |
| `session_id` | TEXT | yes | — | FK → `sessions(id)` `ON DELETE SET NULL`. |
| `content` | TEXT | no | — | Prompt text. FTS-indexed (see asymmetry below). |
| `project` | TEXT | yes | — | Project scope. FTS-indexed. |
| `created_at` | INTEGER | no | — | Epoch-ms creation. |

- **Indexes:** `idx_prompts_session(session_id)`, `idx_prompts_project(project)`, `idx_prompts_created_at(created_at)` — all non-partial (`V001:133-135`).
- **No soft delete:** `PromptStore::delete` is a hard `DELETE` returning `NotFound` if zero rows removed (`prompts.rs:112-119`); there is no `deleted_at` on this table.
- **Privacy note:** the docstring at `prompts.rs:1` claims user prompts are "FTS-indexed", and `CLAUDE.md` states privacy stripping is applied in `PromptStore::save()`. **Neither is true in the current code.** `PromptStore::save` (`prompts.rs:44-66`) inserts `input.content` verbatim — it does **not** call `strip_private_tags`, and (per the asymmetry below) the insert is not mirrored into `prompts_fts`. Treat both as known gaps for the improvement pass.

**`prompts_fts`** FTS5 shadow:

```sql
-- V001:137-142
CREATE VIRTUAL TABLE prompts_fts USING fts5(
    content, project,
    content='user_prompts',
    tokenize='porter unicode61 remove_diacritics 2'
);
```

Indexed columns `content`, `project`; external content `content='user_prompts'`. **Notable asymmetry:** unlike `observations_fts`, `prompts_fts` declares **no `content_rowid`** and V001 defines **no `_ai`/`_ad`/`_au` triggers** for it. So `prompts_fts` is created but is **not auto-populated** by writes to `user_prompts` — it is effectively an unmaintained/empty index in the current schema (a known gap worth flagging for the improvement pass).

### 20.7 `links` (general-purpose graph)

Generic directed edge between observations. `WITHOUT ROWID`, PK on ULID `id`. Used by MNEMA's derivative graph and by the ENGRAM importer's `linked_to[]` mapping.

| Column | SQL type | Null | Default | Meaning |
|---|---|---|---|---|
| `id` | TEXT | no | — | PK. Link ULID. |
| `from_id` | TEXT | no | — | FK → `observations(id)` `ON DELETE CASCADE`. Edge source. |
| `to_id` | TEXT | no | — | FK → `observations(id)` `ON DELETE CASCADE`. Edge target. |
| `link_type` | TEXT | no | — | Edge label (free-form string). |
| `metadata` | TEXT | no | `'{}'` | JSON blob. |
| `created_at` | INTEGER | no | — | Epoch-ms creation. |

- **Primary key:** `id`. **Unique:** `UNIQUE(from_id, to_id, link_type)` — at most one edge of a given type between an ordered pair.
- **Foreign keys:** both `from_id` and `to_id` → `observations(id) ON DELETE CASCADE` (deleting an observation hard-deletes its edges; soft-delete leaves edges intact since the row still exists).
- **Indexes:** `idx_links_from(from_id)`, `idx_links_to(to_id)`, `idx_links_type(link_type)` (`V001:155-157`).
- `LinkStore` provides `create` (`links.rs:39`), `create_in_tx` for atomic import (`links.rs:69`), `list` (`links.rs:96`), `delete` (`links.rs:129`, returns `NotFound` if zero rows removed). No update path — edges are immutable once created.

### 20.8 `memory_relations` (judgment lifecycle)

ENGRAM-inherited table tracking proposed relations between observations and their judgment lifecycle. `WITHOUT ROWID`, PK on ULID `id`.

| Column | SQL type | Null | Default | Meaning |
|---|---|---|---|---|
| `id` | TEXT | no | — | PK. Relation ULID. |
| `sync_id` | TEXT | no | — | `UNIQUE`. Stable cross-machine sync identity. |
| `source_id` | TEXT | no | — | FK → `observations(id)` `ON DELETE CASCADE`. |
| `target_id` | TEXT | no | — | FK → `observations(id)` `ON DELETE CASCADE`. |
| `relation` | TEXT | no | — | Relation kind (`RelationKind`). |
| `judgment_status` | TEXT | no | `'pending'` | `CHECK (judgment_status IN ('pending','judged','orphaned','ignored'))`. |
| `reason` | TEXT | yes | — | Free-text rationale. |
| `evidence` | TEXT | yes | — | Supporting evidence. |
| `confidence` | REAL | yes | — | Confidence score (float). |
| `marked_by_actor` | TEXT | yes | — | Who/what marked it. |
| `marked_by_kind` | TEXT | yes | — | Actor kind. |
| `marked_by_model` | TEXT | yes | — | Model name if marked by an LLM. |
| `session_id` | TEXT | yes | — | FK → `sessions(id)` `ON DELETE SET NULL`. |
| `created_at` | INTEGER | no | — | Epoch-ms creation. |

- **Primary key:** `id`. **Unique:** `sync_id`.
- **Foreign keys:** `source_id`/`target_id → observations(id) ON DELETE CASCADE`; `session_id → sessions(id) ON DELETE SET NULL`.
- **Indexes:** `idx_relations_source(source_id)`, `idx_relations_target(target_id)`, `idx_relations_status(judgment_status)`, `idx_relations_relation(relation)` (`V001:178-181`).
- **Lifecycle (relations.rs):** `create` always inserts `judgment_status='pending'` (the literal is hard-coded in the INSERT, `relations.rs:53-95`); `judge` updates the status and `COALESCE`-merges `reason`/`evidence`/`confidence` — i.e. it only overwrites a column when the supplied value is non-NULL, otherwise keeps the existing value (`relations.rs:97-118`), returning `NotFound` if the id is absent.

### 20.9 `sync_chunks` (git-sync dedup ledger)

Idempotency ledger for the git-friendly sync importer. `WITHOUT ROWID`, **composite PK**.

| Column | SQL type | Null | Default | Meaning |
|---|---|---|---|---|
| `target_key` | TEXT | no | — | PK part 1. Sync target / source identity. |
| `chunk_id` | TEXT | no | — | PK part 2. SHA-256-derived chunk identity. |
| `imported_at` | INTEGER | no | — | Epoch-ms when the chunk was imported. |

- **Primary key:** `PRIMARY KEY (target_key, chunk_id)` (`V001:188`). No secondary indexes, no FKs.
- **Idempotency:** `mark_imported` / `mark_imported_in_tx` use `INSERT OR IGNORE`, returning `true` only when a new row was recorded (`inserted == 1`), `false` if the chunk was already applied (`chunks.rs:30-56`). `was_imported` probes existence via `SELECT count(*)` (`chunks.rs:58-66`). The `_in_tx` variant lets the importer commit the ledger write atomically with the observation saves of the same chunk. `ChunkStore` also exposes `list_for_target` (`chunks.rs:68-90`).

### 20.10 Cross-cutting invariants & gotchas

- **All ordinary tables are `WITHOUT ROWID`** and keyed on ULID text, so they have no implicit integer rowid; `observations.int_id` exists precisely because FTS5/vec0 need an INTEGER rowid to bridge to.
- **Timestamps are epoch-ms INTEGER**, decoded with `Utc.timestamp_millis_opt(ms).single().unwrap_or_else(Utc::now)` — a malformed/out-of-range timestamp silently becomes "now" rather than erroring (e.g. `observations.rs:691-695`, `sessions.rs:199-203`, and the same `ms_to_dt` idiom inline in `prompts.rs:144-147`, `relations.rs:229-232`, `links.rs:160-163`, `chunks.rs:83-87`).
- **Privacy stripping** (`<private>...</private>` removal via the lazy regex `(?si)<private>.*?</private>`, `privacy.rs:11-13`; unclosed tags are left intact on purpose) is applied to **both `title` and `content`** in the normal `save` path (`save_in_tx`, `observations.rs:440-441`), but in the `update` path **only `content` is stripped — the title is taken verbatim** (`observations.rs:224-227`). It is **not** applied at all in `save_raw_in_tx` (the import fast-path, which trusts pre-audited input — `observations.rs:582-619`). And, as noted in 20.6, `PromptStore::save` does **not** strip prompts despite `CLAUDE.md` saying it does.
- **Soft-delete vs FTS skew:** soft-deleted observations stay in `observations_fts` (triggers fire on the physical UPDATE); always re-filter `deleted_at IS NULL` after an FTS match.
- **`prompts_fts` is unmaintained** — declared with no `content_rowid` and no sync triggers, so it never gets populated by the current write path.
- **No `schema_version` table** in the live DB; version bookkeeping is refinery's `refinery_schema_history`, and the doctor's `schema_version` field is the crate version string (`0.2.0`), not the migration number.
- **CHECK constraints** enforce the closed enums: `sessions.status` ∈ {active, ended, aborted}; `observations.scope` ∈ {project, personal}; `memory_relations.judgment_status` ∈ {pending, judged, orphaned, ignored}. `observations.type` and `memory_relations.relation` are **not** constrained at the SQL level — validation lives in Rust on read (`ObservationType::from_str_relaxed` is lenient; `Scope::from_str_strict`, `RelationKind::from_str_strict`, `JudgmentStatus::from_str_strict`, and `SessionStatus::from_str_strict` reject unknown values during row parsing).
