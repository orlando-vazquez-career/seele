## 5. Storage Layer II — Stores, Dedup, Privacy, Links, Relations & Chunks

This section documents the behavioral layer that sits on top of the schema, pool, and vec0 loading covered in Section 4. Where Section 4 explains *how the database exists*, this section explains *what the code does with it*: the CRUD stores (`ObservationStore`, `PromptStore`, `SessionStore`, `RelationStore`, `LinkStore`, `ChunkStore`), the dedup and privacy machinery (`hash.rs`, `privacy.rs`), the transaction helpers that let `seele-sync` and `seele-engram-import` drive multiple stores atomically, the vec0 embedding write path keyed by `SeeleId::as_i64()`, and the soft-delete / restore lifecycle. All of these live in the `seele-storage` crate, which depends on `seele-core` (domain types) plus the external crates `rusqlite`, `chrono`, `serde`/`serde_json`, `sha2`, `hex`, `regex`, `once_cell`, `r2d2`, `refinery`, and `thiserror`.

### 5.1 Crate position, module map, and public surface

`seele-storage` is the second layer from the bottom of the crate graph: it depends on `seele-core` and is depended on by `seele-search`, `seele-sync`, `seele-engram-import`, `seele-http`, `seele-mcp` (transitively), `seele-tui`, and `seele-cli`. Every higher-level service reaches the database exclusively through the store types defined here; no caller issues raw SQL except inside tests.

`crates/seele-storage/src/lib.rs` declares the module tree and the curated re-export surface. The modules are declared one per line (`lib.rs:3-15`):

```rust
// crates/seele-storage/src/lib.rs:3
pub mod chunks;       pub mod error;   pub mod hash;     pub mod links;
pub mod migrations;   pub mod observations; pub mod pool; pub mod privacy;
pub mod prompts;      pub mod relations; pub mod sessions;
pub mod vec0_install; pub mod vec0_loader;
```

The flat `pub use` block (`lib.rs:17-28`) hoists the store types and their input/query DTOs (`ChunkStore`, `SyncChunk`; `LinkInput`, `LinkQuery`, `LinkStore`; `ObservationPatch`, `ObservationQuery`, `ObservationStore`, `RawSaveInput`, `RawSaveOutcome`, `SaveInput`, `SaveOutcome`; `PromptInput`, `PromptQuery`, `PromptStore`, `UserPrompt`; `JudgmentInput`, `RelationInput`, `RelationQuery`, `RelationStore`; `SessionFilter`, `SessionInput`, `SessionStore`), plus `Result`, `StorageError`, and the pool primitives (`init_pool`, `Pool`, `PoolConfig`). The crate also exposes one free function that ties pool + migrations together:

```rust
// crates/seele-storage/src/lib.rs:34
pub fn init_db(path: impl AsRef<Path>) -> Result<Pool> {
    let pool = init_pool(PoolConfig::with_path(path))?;
    let mut conn = pool.get()?;
    migrations::run_pending(&mut conn)?;
    Ok(pool)
}
```

Every store is a thin `#[derive(Clone)]` wrapper around a single `pool: Pool` field and is constructed with `new(pool: Pool)`. They share no state beyond the pool, so cloning a store is cheap (it clones an `r2d2` handle), and the same `Pool` is handed to several stores at once by the service layer.

#### Error model

`crates/seele-storage/src/error.rs` defines `StorageError` (a `thiserror` enum) and `pub type Result<T> = std::result::Result<T, StorageError>`. The variants are: `Sqlite(#[from] rusqlite::Error)`, `Pool(#[from] r2d2::Error)`, `Migration(#[from] refinery::Error)`, `Io(#[from] std::io::Error)`, the three vec0 variants (`Vec0NotSupportedTarget { os, arch }`, `Vec0CacheUnresolvable`, `Vec0IntegrityMismatch { path, expected, actual }`), and three domain-shaped variants used heavily by the stores: `NotFound(String)`, `InvalidInput(String)`, and `Conflict(String)`. The store code maps `rusqlite::Error::QueryReturnedNoRows` to either `Ok(None)` (for `get`) or to `NotFound` (for mutating ops where the row was expected), turns serde failures into `InvalidInput`, and reserves `Conflict` for the int_id-collision retry exhaustion path.

### 5.2 Privacy stripping (`privacy.rs`)

Privacy stripping is the first transform applied on the write path. `crates/seele-storage/src/privacy.rs` compiles a single lazy global regex and exposes one function:

```rust
// crates/seele-storage/src/privacy.rs:11
static PRIVATE_TAG_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?si)<private>.*?</private>").expect("private tag regex must compile")
});

pub fn strip_private_tags(s: &str) -> String {
    PRIVATE_TAG_RE.replace_all(s, "").to_string()
}
```

The flags matter: `(?s)` makes `.` match newlines (blocks may span lines), `(?i)` makes the tag names case-insensitive (`<PRIVATE>` is stripped too), and the `.*?` is **lazy** — it stops at the first `</private>`. Documented consequences, all backed by unit tests in the same file (`privacy.rs:22-75`):

- **Unclosed tags are left intact** (`<private>oops never closed` is returned verbatim; test `unclosed_tag_left_intact`, `privacy.rs:49`). The module comment states this is deliberate: "Malformed blocks (no closing tag) are left untouched on purpose — silent … stripping would surprise the caller" (`privacy.rs:4-6`).
- **Nested tags**: `<private>outer<private>inner</private>tail</private>` strips only up to the first close, leaving `tail</private>` (test `nested_inner_tag_treated_as_text_until_first_close`, `privacy.rs:68`). This is a real gotcha — nested private blocks do not fully strip.
- **Multiple blocks** all strip (test `multiple_blocks_all_stripped`, `privacy.rs:62`); the text between/around them is preserved with the surrounding whitespace untouched (so `Public <private>secret</private> rest.` becomes `Public  rest.` with a double space — test `strips_single_line_block`, `privacy.rs:27`).

Per the repo conventions (CLAUDE.md "Privacy" section), stripping is applied to **both `title` and `content`** in `ObservationStore::save()`, and CLAUDE.md also lists `PromptStore::save()` as a strip site. **Note on the actual code:** the strip for prompts is *not* implemented inside `PromptStore::save` (`prompts.rs:44` writes `input.content` verbatim) — so in the current source the prompt strip is the caller/service-layer's responsibility, not the store's; treat the CLAUDE.md line as an intent/contract that the `prompts.rs` store does not itself enforce. For observations, stripping happens **before** `normalized_hash` and **before** the row is written, so the FTS5 index (populated by the V001 triggers off the stored `content`/`title`) never sees private text. The property test `strip_private_tags_is_idempotent` (`tests/properties.rs:69`) confirms applying twice equals once and that no tag literals survive.

### 5.3 Normalized hashing for dedup (`hash.rs`)

`crates/seele-storage/src/hash.rs` computes the dedup fingerprint over *content only* (not title):

```rust
// crates/seele-storage/src/hash.rs:17
pub fn normalized_hash(content: &str) -> String {
    let normalized = normalize(content);
    let digest = Sha256::digest(normalized.as_bytes());
    hex::encode(digest)
}

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
```

The exact algorithm, in order: (1) lowercase the whole string (Unicode-aware via `str::to_lowercase`), (2) `split_whitespace()` which both collapses any run of whitespace and drops leading/trailing whitespace, (3) re-join tokens with a single ASCII space, (4) SHA-256 the UTF-8 bytes, (5) lowercase hex-encode (always 64 chars). The module comment frames the intent: "The transform is intentionally aggressive: small reformat-only edits to the same content should hash identically and be merged as duplicates" (`hash.rs:15`). Note that `to_lowercase()` is full Unicode case-folding, while the property test `normalized_hash_is_ascii_case_insensitive` (`tests/properties.rs:54`) only asserts ASCII behavior — non-ASCII case behavior follows whatever Rust's `to_lowercase` does and is untested. The hash is a *component* of the dedup key, not the whole key; the time window, project, scope, type, and title comparisons live in the upsert layer (see §5.4), exactly as the module comment notes: "the time/scope check belongs to the upsert layer" (`hash.rs:5`).

### 5.4 `ObservationStore` — the core write path (`observations.rs`)

`ObservationStore` is the heart of SEELE's storage. Its module doc summarizes the `save()` pipeline: privacy strip → normalized hash → topic_key upsert → 24h normalized-hash dedup → new-row insert, with FTS5 kept in sync automatically by the V001 triggers (`observations_ai/ad/au`).

#### DTOs and outcome types

| Type | Role | Notable fields |
|------|------|----------------|
| `SaveInput` | Normal save request | `session_id: Option<SeeleId>`, `kind: ObservationType`, `title`, `content`, `tool_name: Option<String>`, `project: Option<String>`, `scope: Scope`, `topic_key: Option<String>`, `metadata: Metadata` |
| `RawSaveInput` | Migration/bulk insert; caller owns `id` + timestamps | same shape as `SaveInput` plus `id: SeeleId`, `created_at: DateTime<Utc>`, `updated_at: DateTime<Utc>`; no dedup/strip applied (no separate `last_seen_at` field — `save_raw_in_tx` reuses `updated_at` for `last_seen_at`) |
| `SaveOutcome` | Result of `save` | `Created(SeeleId)` / `UpsertedTopic { id, revision_count: u32 }` / `DuplicateMerged { id, duplicate_count: u32 }`; `.id()` extracts the id from any variant |
| `RawSaveOutcome` | Result of `save_raw_in_tx` | `Inserted` / `AlreadyExisted` (a `Copy` enum) |
| `ObservationQuery` | `list` filter (all `Option` except the flag) | `project`, `scope`, `kind`, `session_id`, `topic_key`, `include_deleted: bool`, `limit: Option<u32>` |
| `ObservationPatch` | `update` patch | all fields `Option`; `tool_name`/`topic_key` are `Option<Option<String>>` to distinguish "leave alone" from "set to NULL" |

Two module constants govern dedup and id generation:

```rust
// crates/seele-storage/src/observations.rs:19
const DEDUP_WINDOW_MS: i64 = 24 * 60 * 60 * 1000;   // 24h, inherited from ENGRAM
const ID_COLLISION_RETRIES: usize = 5;              // int_id tail collisions (bytes 9..16 of ULID)
```

(The `DEDUP_WINDOW_MS` declaration is at `observations.rs:22` with its doc comment beginning at line 19; `ID_COLLISION_RETRIES` at line 24.)

#### `save()` step by step

`save()` checks out a connection, opens a transaction, calls the free function `save_in_tx`, commits, and returns the outcome (`observations.rs:119`). All the logic is in `save_in_tx` (`observations.rs:439`). The control flow, in order:

**Step 0 — Normalize inputs.** Strip private tags from both title and content, hash the *stripped content*, snapshot `now_ms = Utc::now().timestamp_millis()`, and serialize `metadata` to JSON (a serde failure becomes `StorageError::InvalidInput`).

```rust
// crates/seele-storage/src/observations.rs:440
let stripped_title = strip_private_tags(&input.title);
let stripped_content = strip_private_tags(&input.content);
let hash = normalized_hash(&stripped_content);
```

**Step 1 — Topic-key upsert.** If `input.topic_key` is `Some`, query for an existing *active* row matching `WHERE project IS ?1 AND scope = ?2 AND topic_key = ?3 AND deleted_at IS NULL LIMIT 1` (`observations.rs:451-454`). The `project IS ?1` operator is null-safe, so two rows with `project = NULL` and the same topic_key collide correctly. If a row is found, it is **updated in place** — title, content, hash, `revision_count = old + 1`, `last_seen_at`/`updated_at = now`, metadata, type, and tool_name are all overwritten (`observations.rs:466-470`) — and the function returns `UpsertedTopic { id, revision_count: new_rev }`. This is the mechanism behind the documented invariant "same `(project, scope, topic_key)` active → update + revision_count++" (CLAUDE.md). The integration test `topic_key_upsert_increments_revision` (`observations_crud.rs:68`) confirms the second save returns `revision_count == 1` and the row reflects the latest title/content. The test `topic_key_isolated_per_scope_and_project` (`observations_crud.rs:120`) confirms the key is scoped per `(project, scope)`: the same `topic_key` under different projects, or under `Project` vs `Personal` scope, yields distinct rows. The schema backs this path with the partial index `idx_obs_topic_upsert ON observations(project, scope, topic_key) WHERE topic_key IS NOT NULL AND deleted_at IS NULL` (V001 schema).

**Step 2 — Normalized-hash dedup window.** If no topic_key matched (or none was supplied), look for a recent duplicate. The query is the full ENGRAM dedup key: `normalized_hash = ?` AND null-safe project equality AND `scope = ?` AND `type = ?` AND `title = ?` (the *stripped* title) AND `last_seen_at >= now - DEDUP_WINDOW_MS` AND `deleted_at IS NULL`.

```rust
// crates/seele-storage/src/observations.rs:494
let dedup_threshold = now_ms - DEDUP_WINDOW_MS;
// SELECT id, duplicate_count FROM observations
//  WHERE normalized_hash = ?1
//    AND ((project IS NULL AND ?2 IS NULL) OR project = ?2)
//    AND scope = ?3 AND type = ?4 AND title = ?5
//    AND last_seen_at >= ?6 AND deleted_at IS NULL LIMIT 1
```

On a hit the existing row's `duplicate_count` is incremented and `last_seen_at` is bumped to `now` (`observations.rs:523-527`), which slides the dedup window forward, and the function returns `DuplicateMerged { id, duplicate_count }`. The test `dedup_hash_window_increments_duplicate_count` (`observations_crud.rs:95`) shows that `"same content"` and `"  SAME content  "` collapse (because the normalized hash ignores case and whitespace) and report `duplicate_count == 1`. The test `different_title_with_same_content_is_new_row` (`observations_crud.rs:111`) shows that title is part of the key — same body, different title → new row. Two gotchas to flag for the improvement section: title is matched by **exact stored (stripped) string**, not by normalized hash, so case/whitespace differences in the title defeat dedup. As for indexing, **this path is index-assisted**: the schema declares the composite partial index `idx_obs_dedup ON observations(normalized_hash, project, scope, type, title) WHERE deleted_at IS NULL` (V001 schema, lines 72-73), which covers five of the six predicates in column order; `last_seen_at` is the only predicate not in the index, so it is applied as a residual filter after the index seek. (This corrects an earlier draft claim that "no composite index" exists — it does.)

**Step 3 — Insert new row with collision retry.** If neither step matched, mint a fresh `SeeleId::new()`, derive `int_id = id.as_i64()` (bytes `9..16` of the ULID — the last 7 bytes / low 56 bits, matching the `ID_COLLISION_RETRIES` comment above), and `INSERT` the full row with `revision_count = 0`, `duplicate_count = 0`, and `created_at = updated_at = last_seen_at = now` (the SQL binds the single `?12` placeholder to `now_ms` for all three timestamp columns):

```rust
// crates/seele-storage/src/observations.rs:538
for _ in 0..ID_COLLISION_RETRIES {
    let id = SeeleId::new();
    let int_id = id.as_i64();
    let res = tx.execute("INSERT INTO observations(... int_id ...) VALUES (...)", params![...]);
    match res {
        Ok(_) => return Ok(SaveOutcome::Created(id)),
        Err(rusqlite::Error::SqliteFailure(e, msg))
            if matches!(e.code, rusqlite::ErrorCode::ConstraintViolation)
                && msg.as_deref().is_some_and(|m| m.contains("observations.int_id")) =>
            { continue; }   // collision on the random tail mapping; regenerate
        Err(other) => return Err(StorageError::from(other)),
    }
}
Err(StorageError::Conflict(format!("exhausted {ID_COLLISION_RETRIES} retries on int_id collision")))
```

The retry exists because `int_id` is declared `INTEGER NOT NULL UNIQUE` in the schema (V001 `observations.int_id`) and is only a 6-byte projection of the 16-byte ULID, so collisions are possible (astronomically unlikely but handled). Only an `int_id` UNIQUE violation is retried (the error message must contain `"observations.int_id"`); any other constraint or error propagates immediately. After `ID_COLLISION_RETRIES` (5) failures it returns `Conflict`. Note: an FTS5 row is *not* written by `save_in_tx` directly — the `observations_ai` AFTER INSERT trigger from V001 does it, inserting `INTO observations_fts(rowid, title, content, tool_name, type, project) VALUES (new.int_id, new.title, …)`. The `observations_fts` virtual table declares the indexed columns `title, content, tool_name, type, project` and uses `content='observations'` with `content_rowid='int_id'`, so `int_id` is the FTS rowid rather than an FTS column.

#### Embedding / vec0 write path

The vector index is **not** written inside `save`. Embedding generation happens at the service layer and is pushed in via a separate call. In `crates/seele-http/src/service.rs`, `save_observation` (`service.rs:73`) saves the observation (`service.rs:94`), then calls `self.embedder.embed(&req.content)` and, on success, `self.observations.set_embedding(outcome.id(), &v)` as a *best-effort* follow-up — failures are logged (`tracing::warn!`) and do **not** surface as a 5xx; the comment states "the row is persisted and a reindex pass can fix it" (`service.rs:96-97`). An embedder failure is likewise swallowed with a warning (`service.rs:104-106`).

`set_embedding` is where the `SeeleId` → vec0 rowid bridge is realized:

```rust
// crates/seele-storage/src/observations.rs:318
pub fn set_embedding(&self, id: SeeleId, embedding: &[f32]) -> Result<()> {
    let conn = self.pool.get()?;
    let int_id: i64 = conn.query_row(
        "SELECT int_id FROM observations WHERE id = ?1 AND deleted_at IS NULL",
        [id.to_string()], |r| r.get(0),
    ).map_err(/* NoRows -> NotFound */)?;
    let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
    conn.execute("INSERT OR REPLACE INTO observations_vec(rowid, embedding) VALUES (?1, ?2)",
                 params![int_id, bytes])?;
    Ok(())
}
```

Two things worth emphasizing. First, the rowid used for the vec0 row is the **stored `int_id` column**, looked up by the textual ULID (and only for active rows — the lookup filters `deleted_at IS NULL`, so calling `set_embedding` on a soft-deleted observation yields `NotFound`). The schema's `int_id` was itself populated from `SeeleId::as_i64()` at insert time, so they agree, but the authoritative mapping the system relies on is `as_i64()`; per the global invariant the SQL `int_id` column is "an approximation used only for JOIN queries". Second, the vector is serialized as a little-endian `f32` byte blob; the doc comment notes the caller owns normalization and that the length must equal the column dim (currently 384, matching the V001 `observations_vec` declaration `embedding FLOAT[384]`). The companion `delete_embedding` (`observations.rs:341`) deletes the vec0 row by sub-selecting `int_id` for the ULID and is a no-op if absent. `hard_delete` (`observations.rs:299`) deletes the observation row first and then *also* clears the vec0 row "so a future ID collision doesn't surface a stale embedding" (`observations.rs:305-306`) — that secondary delete is best-effort (`let _ = …`).

#### Read, update, and lifecycle

`get(id)` runs `SQL_SELECT_BY_ID` (`observations.rs:627`) and maps `QueryReturnedNoRows` → `Ok(None)`. Note `get` does **not** filter `deleted_at`, so it returns soft-deleted rows (the test `soft_delete_then_restore_round_trip`, `observations_crud.rs:176`, relies on this to inspect `deleted_at`). `list(ObservationQuery)` builds SQL dynamically: it always starts from `SQL_SELECT_PREFIX` plus `WHERE 1=1`, appends `AND deleted_at IS NULL` unless `include_deleted` is set, then optional `project`/`scope`/`kind`/`session_id`/`topic_key` predicates, orders `created_at DESC`, and applies an optional `LIMIT`. Parameters are bound via boxed `dyn ToSql` and `params_from_iter`.

`update(id, ObservationPatch)` is transactional: it reads the current title/content (active rows only, via `WHERE id=?1 AND deleted_at IS NULL`), errors `NotFound` if missing, applies the patch (falling back to current values), **re-strips the resulting content** and **recomputes `normalized_hash`** (`observations.rs:226-227`), and always bumps `updated_at` and `last_seen_at`. Note the patch path strips and re-hashes *content* but does **not** re-strip the patched *title* before storing it. The test `update_recomputes_normalized_hash` (`observations_crud.rs:196`) confirms the hash changes after a content edit. The `Option<Option<String>>` shape of `tool_name`/`topic_key` is what allows explicitly nulling those columns.

Soft-delete / restore are mirror operations and both are guarded by status predicates so they are idempotency-safe:

```rust
// crates/seele-storage/src/observations.rs:267
// soft_delete: WHERE id = ?2 AND deleted_at IS NULL     -> 0 rows => NotFound
// restore:     WHERE id = ?2 AND deleted_at IS NOT NULL -> 0 rows => NotFound
```

`soft_delete` sets `deleted_at` and `updated_at` to now; `restore` clears `deleted_at` and bumps `updated_at`. Because they check the prior state in the WHERE clause, double-delete or restoring a live row both yield `NotFound` rather than silently succeeding. `hard_delete` physically removes the row (and its vec0 entry). The FTS5 `observations_ad` AFTER DELETE trigger only fires on `hard_delete`; **soft-deleted rows remain in the FTS index** — search must therefore re-check `deleted_at` itself (this is a cross-cutting fact relevant to Section 7).

#### Stats helpers

`ObservationStore` also exposes the aggregation queries consumed by `seele stats` / the doctor / HTTP stats: `count_active`, `count_deleted`, `count_by_type` (GROUP BY `type`, `(String,u64)` pairs ordered `n DESC, type ASC`), `count_by_scope` (ordered `n DESC, scope ASC`), `count_projects` (distinct non-null), and `list_projects` (distinct non-null, alphabetical `ASC`). All filter on `deleted_at IS NULL` except `count_deleted` (which selects `deleted_at IS NOT NULL`), and all clamp negative counts via `.max(0)`.

#### Transaction helpers and the raw insert path

Three associated functions exist specifically to let other crates compose multiple writes into one atomic boundary:

- `ObservationStore::save_in_tx(tx, input)` (`observations.rs:132`) — the same logic as `save` but on a caller-owned transaction; the caller does the commit/rollback. It delegates to the free function `save_in_tx` (`observations.rs:439`).
- `ObservationStore::pool()` (`observations.rs:138`) — borrows the underlying `&Pool` so a caller (notably `seele-sync`) can open one connection and drive several stores across it in a single transaction.
- `ObservationStore::save_raw_in_tx(tx, RawSaveInput)` (`observations.rs:146`) — wraps the free function `save_raw_in_tx` (`observations.rs:582`), an `INSERT OR IGNORE` that **bypasses privacy strip, topic-key upsert, and the dedup window** (it does still compute `normalized_hash` from the raw content for storage). The caller supplies the `id` and timestamps; idempotency comes from `INSERT OR IGNORE` on the `(id)` primary key. It returns `Inserted` (rowcount 1) or `AlreadyExisted` (rowcount 0).

`save_raw_in_tx` is the path used by both bulk importers, and the *reason* it exists rather than reusing `save_in_tx` is documented as a Cloven critical finding (the 2026-05-11 sync save-path finding). `seele-sync::import_from_file` (`crates/seele-sync/src/lib.rs:281`) opens one transaction via `observations.pool()`, calls `save_raw_in_tx` for every observation, then `ChunkStore::mark_imported_in_tx`, then commits once — so a crash mid-import rolls the entire chunk back and leaves the destination DB and ledger untouched. The doc comment explains the IDs-preserved rationale: source ULIDs ride through unchanged, so the same observation on two machines collapses on the shared id, and a re-import "does not overwrite a destination `topic_key` collision (which the legacy `save_in_tx` path would silently UPDATE, causing data loss)" (`sync/lib.rs:264-267`). The sync path also deliberately drops `session_id` (set to `None` at `sync/lib.rs:317`) to avoid tripping the `observations.session_id` foreign key (`REFERENCES sessions(id) ON DELETE SET NULL`) when the referenced session is unknown to the destination. `seele-engram-import` (`engram-import/src/lib.rs:151`) uses the same `save_raw_in_tx` plus `LinkStore::create_in_tx` to commit imported observations and their derived `links` atomically; before each link insert it probes with `link_exists_in_tx` (`engram-import/src/lib.rs:203`) to avoid hitting the `links` UNIQUE constraint and aborting the whole transaction.

#### Row parsing

`parse_observation` (`observations.rs:634`) maps an 18-column row into a `seele_core::memory::Observation`, converting epoch-ms integers to `DateTime<Utc>` via `ms_to_dt` (`observations.rs:691`, which falls back to `Utc::now()` if the timestamp is somehow out of range — a silent fallback worth noting). It parses the ULID text columns into `SeeleId` (errors → `InvalidInput`), uses `ObservationType::from_str_relaxed` (lenient) for the `type` column but `Scope::from_str_strict` (strict, errors on unknown) for `scope`, and `serde_json::from_str` for metadata. Note that `normalized_hash` is read as `Option<String>` (the column is nullable in the schema).

### 5.5 `PromptStore` (`prompts.rs`)

`PromptStore` persists short records of user input into the `user_prompts` table, which has its own FTS index (`prompts_fts`, over columns `content` and `project`) maintained by triggers. Its domain struct `UserPrompt { id: SeeleId, session_id: Option<SeeleId>, content: String, project: Option<String>, created_at: DateTime<Utc> }` is `Serialize + Deserialize` (it crosses the HTTP boundary). The API is small: `save(PromptInput) -> UserPrompt` (mints a `SeeleId`, stores `created_at` as epoch-ms, returns the full struct), `get(id) -> Option<UserPrompt>`, `list(PromptQuery)` (filter by `project`/`session_id`, order `created_at DESC`, optional `limit`), and `delete(id)` (hard delete; `NotFound` if the row didn't exist). **Important:** `PromptStore::save` (`prompts.rs:44`) does **not** strip private tags — it inserts `input.content` verbatim. CLAUDE.md lists `PromptStore::save()` as a privacy-strip site, but in the actual source the prompt strip is not present in the store, so any stripping for prompts must come from the caller. There is no dedup, no soft-delete, and no vec0 write for prompts. The test `prompts_save_get_list_delete_round_trip` (`secondary_crud.rs:50`) covers the full lifecycle.

### 5.6 `SessionStore` (`sessions.rs`)

`SessionStore` manages the `sessions` table whose `status` column is CHECK-constrained to `('active','ended','aborted')` (V001 schema). API:

| Method | Behavior |
|--------|----------|
| `start(SessionInput)` | INSERT with `status='active'`, `started_at=now`; returns a fully-built `Session` (`status: Active`, `ended_at: None`, `summary: None`) |
| `end(id, summary)` | `UPDATE ... status='ended', ended_at=?1, summary=?2 WHERE id=?3 AND status='active'`; 0 rows → `NotFound` |
| `abort(id)` | `UPDATE ... status='aborted', ended_at=?1 WHERE id=?2 AND status='active'`; 0 rows → `NotFound` (does not set `summary`) |
| `get(id)` | `Option<Session>` |
| `list(SessionFilter)` | filter `project`/`status`, order `started_at DESC`, optional `limit` |
| `count_total` / `count_by_status` | stats (`count_by_status` GROUP BY `status`, ordered `n DESC, status ASC`) |

`SessionInput` carries `project: String` (required) and `directory: Option<String>`. The `AND status='active'` guard on `end`/`abort` makes them state-machine-safe: ending an already-ended session returns `NotFound` rather than re-closing it. The test `end_idempotency_rejects_already_ended` (`sessions_crud.rs:47`) asserts exactly this. `row_to_session` (`sessions.rs:167`) uses an unusual nested-`Result` closure shape (`rusqlite::Result<rusqlite::Result<Session>>`) so that ULID/status parse failures map to `FromSqlConversionFailure` while still allowing `query_row`'s NoRows to be distinguished by the caller; `get` collapses with `.transpose()` and `list` collapses with `r??`. `SessionStatus::from_str_strict` rejects unknown status strings. Sessions are the FK target of `observations.session_id` (`ON DELETE SET NULL`), which is why `seele-sync` strips `session_id` on import.

### 5.7 `RelationStore` vs `LinkStore` — two distinct edge tables

SEELE has **two** separate graph-edge concepts, and they are *not* interchangeable.

`memory_relations` (via `RelationStore`, `relations.rs`) is the **judged**, provenance-rich relation table used by the `seele_judge` / `seele_compare` workflows. `RelationInput` carries a `sync_id: String` (a `NOT NULL UNIQUE` column for cross-machine identity), `source_id`/`target_id` (`SeeleId`), a typed `relation: RelationKind`, optional `reason`/`evidence`/`confidence: f64`, provenance triples `marked_by_actor`/`marked_by_kind`/`marked_by_model`, and an optional `session_id`. `create` always inserts with `judgment_status = 'pending'` (the literal `'pending'` is baked into the INSERT VALUES, `relations.rs:62`). The lifecycle method `judge(id, JudgmentInput)` sets the new status and `COALESCE`s reason/evidence/confidence so a `None` field leaves the existing value untouched:

```rust
// crates/seele-storage/src/relations.rs:100
"UPDATE memory_relations SET judgment_status = ?1,
    reason = COALESCE(?2, reason), evidence = COALESCE(?3, evidence),
    confidence = COALESCE(?4, confidence) WHERE id = ?5"
```

`judge` returns `NotFound` if 0 rows matched. `get`/`list` round-trip the full struct; `list` filters by `source_id`/`target_id`/`relation`/`status`, orders `created_at DESC`, optional `limit`. `RelationKind` and `JudgmentStatus` both parse with `from_str_strict` (unknown → `InvalidInput`); the schema CHECK-constrains `judgment_status` to `('pending', 'judged', 'orphaned', 'ignored')` (V001 schema). There is no `delete` and no soft-delete on relations. The test `relations_create_judge_lifecycle` (`secondary_crud.rs:139`) walks create (`Pending`) → judge (`Judged`, evidence "PR #42", confidence 0.95) → list-by-status.

`links` (via `LinkStore`, `links.rs`) is the **generic, unjudged** edge table the module doc describes as used by MNEMA for "the derivative graph (counsel→advisor→review→verdict→skill→decision)" (`links.rs:1-2`). `LinkInput` is minimal: `from_id`, `to_id`, `link_type: String` (free-form, not an enum), and `metadata`. `create` inserts a row with a fresh `SeeleId` and `created_at`; the schema enforces `UNIQUE(from_id, to_id, link_type)` (V001 schema, line 152), so creating the same edge twice errors — the test `links_unique_constraint_blocks_duplicate_edge` (`secondary_crud.rs:115`) asserts the error message contains "UNIQUE"/"unique". `create_in_tx` (`links.rs:69`) is the transactional variant used by the ENGRAM importer to commit `linked_to[]`-derived edges atomically with their observations. `list(LinkQuery)` filters by `from_id`/`to_id`/`link_type` (order `created_at DESC`, optional `limit`); `delete(id)` hard-deletes (`NotFound` if absent). Note there is no `get(id)` on `LinkStore`. (Caveat: the `seele-engram-import` source carries a stale comment claiming `links` has *no* UNIQUE constraint and that it probes manually for that reason; the V001 schema does declare the UNIQUE, and the manual probe in the importer is there to avoid aborting the surrounding transaction on a duplicate — not because the constraint is absent.)

The key distinction for the improvement reviewer: relations are typed + judged + carry a `sync_id` and provenance and have no uniqueness constraint on the `(source,target,relation)` edge (only `sync_id` is unique); links are free-typed, idempotent on `(from,to,type)`, carry only metadata, and have no judgment lifecycle. They write to different tables and serve different consumers.

### 5.8 `ChunkStore` — the sync import ledger (`chunks.rs`)

`ChunkStore` records which sync chunks have already been imported, keyed by composite PK `(target_key, chunk_id)` so re-imports are idempotent. `SyncChunk { target_key: String, chunk_id: String, imported_at: DateTime<Utc> }` is `Serialize + Deserialize`. The API: `mark_imported(target_key, chunk_id) -> bool` returns `true` if newly recorded, `false` if already present (it uses `INSERT OR IGNORE` and inspects the affected rowcount, returning `inserted == 1`); `mark_imported_in_tx(tx, ...)` is the transaction-scoped variant used inside `seele-sync::import_from_file` so the ledger entry commits or rolls back together with the chunk's observation saves; `was_imported(...) -> bool` is the cheap pre-check `seele-sync` runs before opening a transaction (`sync/lib.rs:291`, implemented as `SELECT count(*) … == 1`); and `list_for_target(target_key)` returns all recorded chunks for a target ordered `imported_at DESC`. The test `chunks_idempotent_mark_imported` (`secondary_crud.rs:187`) verifies the `true`/`false` semantics and that a second mark of the same `(target,chunk)` is a no-op. `target_key` is typically a git remote ref like `"origin/main"`; `chunk_id` is the chunk's SHA-256 (per CLAUDE.md, "Re-imports idempotent por SHA-256").

### 5.9 Concurrency, transactions, and failure modes

There is no async in this crate — every store method is synchronous and blocks on a pooled `rusqlite::Connection` checked out from the `r2d2` `Pool`. Higher layers (axum, MCP) wrap these calls in `tokio` blocking contexts. Concurrency control is delegated to SQLite/WAL plus the pool: each method that mutates either runs a single `conn.execute` (auto-commit) or explicitly opens `conn.transaction()` (`save`, `update`) for multi-statement atomicity. The "open one connection, drive many stores in one tx" pattern is only reachable through the `*_in_tx` associated functions plus `pool()`, and is exercised by `seele-sync` and `seele-engram-import`.

Failure-mode summary for the improvement section: (1) `int_id` collision is bounded-retried (5×) then surfaced as `Conflict`; (2) `QueryReturnedNoRows` is normalized to `Ok(None)` for reads and `NotFound` for guarded mutations; (3) bad ULID text, unknown enum strings (strict-parsed `scope`/`status`/`relation`), and unserializable metadata become `InvalidInput`; (4) the embedding write is intentionally best-effort and decoupled from the row write, so a row can exist without a vector (the system tolerates this and relies on a future reindex); (5) `ms_to_dt` silently substitutes `Utc::now()` for an out-of-range stored timestamp; (6) soft-deleted observations stay in the FTS5 index, so search must re-filter `deleted_at`. There are no `TODO`/`FIXME` markers in these store files. The two documented behavioral gotchas a reviewer should examine are: nested `<private>` tags only partially strip (`privacy.rs:68`), and title dedup is exact-string rather than normalized (so reformatting a title defeats the dedup window even when the content hash matches).

### 5.10 File-by-file map

- **`observations.rs`** — `ObservationStore` and the full write pipeline (`save`/`save_in_tx`), the raw migration insert (`save_raw_in_tx`), CRUD (`get`/`list`/`update`), lifecycle (`soft_delete`/`restore`/`hard_delete`), the vec0 embedding bridge (`set_embedding`/`delete_embedding`), stats helpers, and `parse_observation`. The single most important file in the layer.
- **`prompts.rs`** — `PromptStore` for `user_prompts`: save/get/list/delete, the `UserPrompt` serde struct; no dedup, no soft-delete, and no in-store private-tag strip.
- **`sessions.rs`** — `SessionStore` over `sessions` with the active→ended/aborted state machine guarded by `WHERE status='active'`, plus session stats (`count_total`/`count_by_status`).
- **`relations.rs`** — `RelationStore` for the judged `memory_relations` graph: create (always `pending`), `judge` with `COALESCE` merge, get/list, provenance + `sync_id`.
- **`links.rs`** — `LinkStore` for the generic `links` graph with `UNIQUE(from_id,to_id,link_type)`, including `create_in_tx` for atomic imports (create/list/delete; no `get`).
- **`chunks.rs`** — `ChunkStore` import ledger keyed by `(target_key, chunk_id)`, with `mark_imported`/`mark_imported_in_tx`/`was_imported`/`list_for_target`.
- **`hash.rs`** — `normalized_hash`: lowercase → collapse whitespace → SHA-256 → hex; the dedup fingerprint over content.
- **`privacy.rs`** — `strip_private_tags` via the lazy `(?si)<private>.*?</private>` regex; multiline, case-insensitive, lazy; leaves unclosed tags and nested-tail intact.
- **`lib.rs`** — module declarations, the curated re-export surface, and `init_db` (pool + `run_pending` migrations).
- **`error.rs`** — `StorageError` enum and crate `Result` alias.
- **`tests/`** — `observations_crud.rs` (save/upsert/dedup/privacy/lifecycle), `sessions_crud.rs` (state machine + filters), `secondary_crud.rs` (prompts/links/relations/chunks smoke + the UNIQUE-edge assertion), `foundation_smoke.rs` (schema/FTS/vec0 presence, idempotent `init_db`, raw vec0 + FTS round-trips), and `properties.rs` (proptest invariants for hash determinism/hex-length/whitespace/ASCII-case, privacy idempotence, and save→get / save→list round-trips — 32 cases for pure-function properties, 8 for the DB-touching ones).
