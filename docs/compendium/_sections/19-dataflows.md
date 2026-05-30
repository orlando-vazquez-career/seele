## 19. End-to-End Data Flows

This section traces the five canonical SEELE data flows hop by hop, naming the exact function and file at every step. Where storage rows cross into the vec0 INTEGER rowid space, the `SeeleId::as_i64()` bridge is called out explicitly. The recurring shape is: a **transport** (CLI / MCP / HTTP) deserializes a request into a DTO, calls **one method on `SeeleService`** (`crates/seele-http/src/service.rs`), which calls into the **stores** (`crates/seele-storage/`), the **embedder** (`crates/seele-embedder/`), or the **search engine** (`crates/seele-search/`). The service layer is shared by both HTTP and MCP; the CLI builds the same `SeeleService` and calls the same methods directly (no network hop).

The unifying entry point for every DB-touching command is `build_service()` in `crates/seele-cli/src/app.rs:128`, which calls `init_db()` (`crates/seele-storage/src/lib.rs:34`) → `init_pool()` → `migrations::run_pending()`, then `pick_embedder()` (ONNX by default, `FakeEmbedder` if `--fake-embedder` / `SEELE_FAKE_EMBEDDER` is set or ONNX init fails), and finally `SeeleService::new(pool, embedder)`.

### 19.1 The SeeleId → vec0 rowid bridge (used by every flow)

vec0 (`observations_vec`) and FTS5 (`observations_fts`) are virtual tables keyed by an INTEGER `rowid`, but the canonical PK is the ULID string `observations.id`. The authoritative bridge is `SeeleId::as_i64()` (`crates/seele-core/src/id.rs:35`):

```rust
// crates/seele-core/src/id.rs:35
pub fn as_i64(&self) -> i64 {
    let bytes = self.0.to_bytes();
    let mut int_bytes = [0u8; 8];
    int_bytes[1..8].copy_from_slice(&bytes[9..16]); // last 7 ULID bytes, top byte zeroed → non-negative
    i64::from_be_bytes(int_bytes)
}
```

This value is stored verbatim into the `observations.int_id` column at INSERT time (`save_in_tx`, `save_raw_in_tx`). The schema comment at `V001__initial_schema.sql:7-13` is emphatic that the SQL "virtual generated column" idea for `int_id` was abandoned because `CAST` of a base32 ULID yields 0; `int_id` is a **real** `INTEGER NOT NULL UNIQUE` column populated from Rust. The FTS triggers (`observations_ai/ad/au`, `V001:102-117`) mirror `new.int_id`/`old.int_id` into `observations_fts.rowid`, and `set_embedding` writes `observations_vec.rowid = int_id`. Searches then JOIN back via `JOIN observations o ON o.int_id = vec.rowid` / `= fts.rowid`. So the chain is: `SeeleId::as_i64()` → `observations.int_id` → FTS/vec `rowid`. The shared-context caveat holds: `as_i64()` is the authoritative mapping; `int_id` is the column that materializes it for JOINs.

---

### 19.2 (A) SAVE flow — write path

The SAVE flow has three transport entries that all converge on `SeeleService::save_observation` (`service.rs:73`).

| Entry | File:line | Builds |
|-------|-----------|--------|
| CLI `seele save` | `crates/seele-cli/src/commands/save.rs:48` (built), `:59` (dispatched) | `SaveRequest` (sets `tool_name = Some("seele-cli")`, `session_id = None`) |
| MCP `seele_save` | `crates/seele-mcp/src/tool_impls/memories.rs:20` | `serde_json::from_value::<SaveRequest>` |
| HTTP `POST /memories` (and legacy `POST /save`) | `crates/seele-http/src/handlers.rs:23` | `Json<SaveRequest>` |

Step by step:

1. **Transport deserialization.** The CLI fills `SaveRequest` (`dto.rs:17`) from clap args; `r#type` defaults to `"memory"` via `default_type`. MCP/HTTP deserialize JSON directly. All three then call `svc.save_observation(req)`.
2. **Field parsing (`service.rs:74-93`).** `session_id` is parsed via `parse_id`; `scope` via `parse_scope` (defaults to `Scope::Project`); `kind` via `parse_type` (relaxed — any string is accepted, unknown maps to a custom type); `metadata` via `parse_metadata`. These assemble a `SaveInput` (`observations.rs:26`).
3. **`ObservationStore::save` (`observations.rs:119`)** checks a connection out of the r2d2 pool, opens a `transaction()`, and calls the free function `save_in_tx` (`observations.rs:439`). Everything below 4–7 runs inside that one transaction; `tx.commit()` at `observations.rs:124` is the atomic boundary for the row + its FTS trigger writes.
4. **Privacy strip + normalized hash (`observations.rs:440-443`).** `strip_private_tags` (`privacy.rs:18`) removes every `(?si)<private>.*?</private>` block from **both** `title` and `content`; unclosed tags are left intact on purpose. `normalized_hash` (`hash.rs:17`) lowercases, collapses whitespace runs, trims, and SHA-256-hexes the *stripped* content. Order matters: stripping happens before hashing and before the row is written (so FTS never indexes private text).
5. **Topic-key upsert (`observations.rs:447-491`).** If `topic_key` is `Some`, the tx queries for an active row with the same `(project, scope, topic_key)` (`idx_obs_topic_upsert`). On a hit it `UPDATE`s title/content/hash/metadata/type/tool_name, sets `revision_count = rev + 1` and `last_seen_at = updated_at = now`, and returns `SaveOutcome::UpsertedTopic { id, revision_count }`. The UPDATE fires the `observations_au` trigger which re-syncs FTS (delete old rowid + insert new). No new ULID is minted; `int_id` is unchanged.
6. **24h normalized-hash dedup (`observations.rs:493-535`).** If no topic upsert happened, the tx looks for an active row matching `normalized_hash` AND `project` (NULL-safe) AND `scope` AND `type` AND `title` AND `last_seen_at >= now - DEDUP_WINDOW_MS` (`DEDUP_WINDOW_MS = 24*60*60*1000`, `observations.rs:22`). On a hit it bumps `duplicate_count` and `last_seen_at`, returning `SaveOutcome::DuplicateMerged { id, duplicate_count }`. This is ENGRAM-inherited and uses `idx_obs_dedup`.
7. **Insert new row (`observations.rs:537-579`).** Otherwise a fresh `SeeleId::new()` is generated, `int_id = id.as_i64()` (← the bridge), and the row is `INSERT`ed with `revision_count = duplicate_count = 0` and `created_at = updated_at = last_seen_at = now`. The `observations_ai` trigger (`V001:102`) immediately copies `title, content, tool_name, type, project` into `observations_fts` keyed by `new.int_id`. If the INSERT hits a `UNIQUE` violation specifically on `observations.int_id` (an `as_i64()` tail collision), the loop regenerates the ULID up to `ID_COLLISION_RETRIES = 5` (`observations.rs:24,565-572`); exhausting retries yields `StorageError::Conflict`. Returns `SaveOutcome::Created(id)`.
8. **Post-save embedding (best effort) (`service.rs:96-107`).** Back in the service, *after* `save()` commits, it calls `self.embedder.embed(&req.content)`. For ONNX the `Embedder::embed` entry is `onnx.rs:197`, delegating to `run_inference` (`onnx.rs:115`) which tokenizes, runs the session under a `Mutex<Session>` (lock released before pooling, `onnx.rs:142-167`), mean-pools with attention-mask weighting, and L2-normalizes to a 384-dim vector (`DEFAULT_DIM = 384`, `onnx.rs:31`); for `FakeEmbedder` (`embed` at `fake.rs:45` → `hash_to_vector` at `fake.rs:23`) it SHA-256-hashes the text per axis-block into a deterministic L2-normalized 384-dim vector. The vector is written by `ObservationStore::set_embedding` (`observations.rs:318`), which looks up `int_id` for the (active) row, packs the `f32`s little-endian, and runs `INSERT OR REPLACE INTO observations_vec(rowid, embedding) VALUES (int_id, bytes)` (`observations.rs:333-336`) — the **second** place the `int_id`/`as_i64()` bridge is materialized in the write path. Crucially this is **best effort**: an embedder or write failure only logs `tracing::warn!` and does **not** turn into a 5xx; the observation row persists and the vec branch simply skips it until a reindex. This is a documented gotcha (`service.rs:69-72`): the row and the embedding are *not* in the same transaction. (Note: the service always embeds `req.content` — the *raw* request content, not the privacy-stripped content that `save_in_tx` indexed into FTS; for inputs without `<private>` blocks these are identical.)
9. **Response mapping (`service.rs:109-131`).** The `SaveOutcome` is mapped to a `SaveResponse { id, outcome: "created"|"upserted_topic"|"duplicate_merged", revision_count?, duplicate_count? }` (`dto.rs:45`). HTTP returns `Json<SaveResponse>` (200); MCP wraps it (see 19.4); the CLI prints `saved <id> (<outcome>)`.

> Edge case: because dedup keys on the post-strip `title`+`content`, a reformat-only edit (whitespace/case) within 24h merges as a duplicate (`hash.rs` is intentionally aggressive). Topic upsert takes precedence over dedup — if `topic_key` is set, the dedup branch is never reached.

---

### 19.3 (B) SEARCH flow — hybrid FTS + vec0 + RRF

Entries converge on `SeeleService::search_observations` (`service.rs:136`).

| Entry | File:line | Empty-query gate |
|-------|-----------|------------------|
| CLI `seele search` | `crates/seele-cli/src/commands/search.rs:47` | calls `enforce_search_query_or_filter` before dispatch |
| MCP `seele_search` | `crates/seele-mcp/src/tool_impls/memories.rs:28` | calls `enforce_search_query_or_filter` |
| HTTP `POST /search` | `crates/seele-http/src/handlers.rs:35` | calls `enforce_search_query_or_filter` |

1. **Anti-exfiltration gate (`service.rs:442`).** `enforce_search_query_or_filter` rejects an empty `query` when *no* filter (`project`/`scope`/`type`) is present, returning `ApiError::BadRequest`. This is applied at the transport layer by all three callers (the service method itself does not re-check — see its doc at `service.rs:134`). It mitigates the "list-all-DB" vector flagged by Cloven (2026-05-10).
2. **Build `SearchQuery` (`service.rs:141-152`).** `scope` is parsed; the request maps to `seele_search::SearchQuery` (`engine.rs:17`) carrying `per_method_limit`, `limit`, `include_purist`, `score_boost_multiplier`, `max_vec_distance`, `include_annotations`.
3. **`SearchEngine::search` (`engine.rs:96`).** If `query.text` is blank it short-circuits to `list_by_filters` (`engine.rs:143`) — a plain `SELECT ... ORDER BY created_at DESC LIMIT` with no embedder call (score `0.0`, no ranks). Otherwise it computes `per_method = per_method_limit.unwrap_or(50)` and `final_limit = limit.unwrap_or(10)`.
4. **FTS branch `fts_query` (`engine.rs:191`).** Runs `SELECT o.id FROM observations_fts fts JOIN observations o ON o.int_id = fts.rowid WHERE observations_fts MATCH ?1 AND o.deleted_at IS NULL ... ORDER BY rank LIMIT ?`. The user text is wrapped by `escape_fts` (`engine.rs:501`) into a quoted phrase with doubled internal quotes. Filters (`project`/`scope`/`type`/non-purist) are appended. Returns a `Vec<SeeleId>` in rank order. The JOIN here is the read-side use of the `int_id` bridge.
5. **Vec branch `vec_query` (`engine.rs:232`).** Calls `self.embedder.embed(&query.text)` and checks the returned length equals `embedder.dim()` (else `SearchError::DimensionMismatch`). The vector is packed little-endian and the query is `SELECT o.id, vec.distance FROM observations_vec vec JOIN observations o ON o.int_id = vec.rowid WHERE vec.embedding MATCH ?1 AND vec.k = ?2 AND o.deleted_at IS NULL ... ORDER BY vec.distance`. `vec.k = per_method` is the KNN bound; optional `max_vec_distance` adds `AND vec.distance <= ?`. Again JOINs through `int_id`. Returns `Vec<SeeleId>` ordered by ascending distance.
6. **RRF fusion `rrf::combine` (`engine.rs:107` → `rrf.rs:31`).** Both ranked lists are passed as `[("fts", fts_rank), ("vec", vec_rank)]` with `k = DEFAULT_K = 60` (`rrf.rs:15`). Each doc accrues `sum 1/(k + rank)` (rank starts at 1), and `per_source` records `(source, rank)` per appearance. A doc appearing in both lists outranks one in a single list. Results sort by descending score with stable ties (first-seen first).
7. **Optional score boost (`engine.rs:113` → `apply_score_boost`, `engine.rs:290`).** If `score_boost_multiplier != 0`, each score is multiplied by `1 + multiplier * meta_score` (read from the `meta_score` virtual column, null treated as 1.0 per ADR-03), then re-sorted. Default `0.0` is a no-op. The list is truncated to `final_limit` (`engine.rs:114`).
8. **Hydration (`engine.rs:116` → `hydrate`, `engine.rs:431`).** The surviving `SeeleId`s are batch-fetched (`SELECT ... WHERE id IN (...)`) and mapped into `SearchHit { observation, score, fts_rank, vec_rank, annotations: [] }`, with `unpack_per_source` (`engine.rs:458`) splitting the RRF `per_source` back into `fts_rank`/`vec_rank`.
9. **Optional annotations (`engine.rs:135` → `attach_annotations`/`fetch_annotations`, `engine.rs:343/354`).** When `include_annotations`, one extra query against `memory_relations` (joined to `observations` for the other endpoint's title) classifies each hit as `Supersedes`/`SupersededBy`/`ConflictsWith`/`ContestedBy` via `annotation_for_source`/`annotation_for_target` (`engine.rs:474/486`). Default off to keep the sub-300ms target.
10. **DTO + response (`service.rs:154-156`).** Hits become `SearchHitDto` (`dto.rs:84`, exposing `id`, `title`, `content`, `score`, `fts_rank`, `vec_rank`, `annotations`, etc.) inside `SearchResponse { hits, count }`. HTTP returns `Json`; MCP stringifies (19.4); the CLI prints `<id> [<type>] <score> <title>` per hit.

> Gotcha: if the embedder failed at SAVE time (step A.8), the row has an FTS entry but no vec0 row, so it can only surface through the FTS branch. The vec branch silently omits it. With `FakeEmbedder`, vec hits are hash-deterministic rather than semantic — the CLI warns about degraded quality on ONNX-init fallback (`app.rs:151`).

---

### 19.4 (C) MCP `tools/call` lifecycle — stdin frame to stdout frame

Driven by `seele mcp` (`crates/seele-cli/src/commands/mcp.rs:14`), which builds the `SeeleService` and calls `McpServer::new(svc, McpServerConfig { tool_prefix })`. `McpServer::new` (`server.rs:31`) calls `build_index(prefix)` to materialize the tool name → `Tool` map.

1. **Read loop (`server.rs:46` `run_io`).** `run_stdio` wires real `tokio::io::stdin/stdout`; `run_io` wraps the reader in `BufReader::lines()` and reads one line per iteration. Blank lines are skipped. The transport is line-delimited JSON-RPC 2.0 (not Content-Length framed).
2. **Parse + route (`handle_line`, `server.rs:68`).** The line is `serde_json::from_str::<Request>` (`jsonrpc.rs:11`). A parse failure returns `Response::err(Null, PARSE_ERROR=-32700)`. If `req.id` is `None` it is a **notification** → returns `None` and the loop writes nothing. Otherwise it matches `req.method`:
   - `"initialize"` → returns `serverInfo {name:"seele", version}`, `protocolVersion:"2024-11-05"`, `capabilities.tools`.
   - `"tools/list"` → emits every registered `ToolDescriptor { name, description, inputSchema }`, sorted by name.
   - `"tools/call"` → `dispatch_tool_call`.
   - anything else → `METHOD_NOT_FOUND=-32601`.
3. **Dispatch (`dispatch_tool_call`, `server.rs:113`).** Reads `params.name` (missing → `INVALID_PARAMS=-32602`) and `params.arguments` (defaulting to `{}`). Looks the tool up in the index (unknown → `INVALID_PARAMS "unknown tool: <name>"`).
4. **Handler invocation (`server.rs:137`).** `(tool.handler)(&self.service, arguments)` runs the `fn(&SeeleService, Value) -> Result<Value, ToolError>` shim. For `seele_save`/`seele_search` these are the same `memories::save`/`memories::search` shown in 19.2/19.3 — i.e. the MCP path **reuses the HTTP service layer**, deserializing `arguments` into the very same `SaveRequest`/`SearchRequest` DTOs (`tool_impls/memories.rs:21,27`). `seele_search` re-applies `enforce_search_query_or_filter` before calling the service.
5. **Result framing (`server.rs:138-154`).** On `Ok(v)`, the handler's arbitrary JSON is stringified and wrapped in the MCP `CallToolResult` shape `{ "content": [{ "type": "text", "text": <stringified-json> }], "isError": false }`. (`structuredContent` from the 2025-06-18 spec is deferred — see the comment at `server.rs:139-144`.) On `Err(e)`, `ToolError::to_jsonrpc` (`tools.rs:31`) maps `BadParams → INVALID_PARAMS(-32602)`, `NotFound`/`Conflict → TOOL_ERROR(1001)`, `Internal → INTERNAL_ERROR(-32603)`. `ApiError` from the service is converted into `ToolError` via the `From` impl at `tools.rs:41`.
6. **Write frame (`server.rs:56-61`).** The `Response` is serialized and a single line + `\n` is written to stdout and flushed.

> Tool naming (ADR-13): `build_index` (`tools.rs:205`) renames tools when a prefix is given. `--tool-prefix mnema` produces `mnema_save`, `mnema_recall` (the special `search → recall` rename in `RENAMES`, `tools.rs:223`), etc. `None` or `"seele"` keeps canonical `seele_*`. The handler set is identical regardless of prefix.

---

### 19.5 (D) SYNC export → import round-trip across machines

Sync (`crates/seele-sync/src/lib.rs`) moves observations as git-friendly gzip chunks. Both halves go through the CLI `seele sync` subcommand (`crates/seele-cli/src/commands/sync.rs:35`).

**Export (machine A):**

1. CLI `sync export <dir> [--project P]` builds the service and calls `seele_sync::export_to_dir(&svc.observations, &dir, ExportFilter { project })` (`sync.rs:47`).
2. `export_to_dir` (`lib.rs:181`) → `build_chunk` (`lib.rs:136`) runs `ObservationStore::list` with `include_deleted: false` (soft-deleted rows never travel) and assembles a `ChunkPayload { format_version: 1, seele_version, exported_at, project, observations }` (`lib.rs:65`).
3. `compute_chunk_id` (`lib.rs:162`) SHA-256-hashes `format_version` (LE) + `project` bytes + a `0u8` separator + the observations serialized as JSON **after sorting by `id`**. `exported_at` and `seele_version` are deliberately excluded, so two exports of the same set produce the **same** `chunk_id` (content-addressed, deterministic).
4. `write_chunk_file` (`lib.rs:201`) gzips the full payload JSON (`GzEncoder`, default compression) to `<dir>/<chunk_id>.json.gz`. The report carries `chunk_id`, path, `observation_count`, `bytes_on_disk`. The file can be committed to a git repo and pushed.

**Import (machine B):**

1. CLI `sync import <path> --target-key <key>` calls `seele_sync::import_from_file(&svc.observations, &svc.chunks, &target_key, &path)` (`sync.rs:62`). `target_key` identifies the local node (hostname/UUID).
2. `read_chunk_file` (`lib.rs:216`) gunzips and deserializes the payload. It rejects `format_version > CURRENT_FORMAT_VERSION (1)` and, if the filename stem is a 64-hex string, verifies it equals the **recomputed** `chunk_id` (integrity check catching silent corruption, `lib.rs:235-242`).
3. **Idempotency pre-check (`lib.rs:291`).** `ChunkStore::was_imported(target_key, chunk_id)` (`chunks.rs:58`) queries `sync_chunks`. If present, returns `ImportReport { outcome: AlreadyImported, ... observation_count_skipped_chunk_level = N }` without opening a transaction.
4. **Atomic import (`lib.rs:305-335`).** A single connection is checked out and `conn.transaction()` opened. For each observation, a `RawSaveInput` is built and saved via `ObservationStore::save_raw_in_tx` (`observations.rs:582`) — the **raw** path: it bypasses privacy strip, topic-key upsert, and the dedup window, and uses `INSERT OR IGNORE` keyed on `id`. This **preserves source ULIDs**, so the same observation arriving from two machines collapses on the shared `id` (returns `RawSaveOutcome::AlreadyExisted`) rather than duplicating. `int_id` is recomputed as `input.id.as_i64()` (`observations.rs:588`) — the bridge again — and the `observations_ai` trigger indexes the new row into FTS. Note: embeddings are **not** part of the chunk and are not written during import; vec0 rows for imported observations only appear after a reindex/re-embed.
5. **`session_id` is dropped (`lib.rs:317`).** The raw input forces `session_id: None` because sessions don't travel in v0.1 chunks; keeping a foreign id would trip the `observations.session_id` FK (`REFERENCES sessions(id)`) and abort the whole tx (closes Cloven 2026-05-11 [MEDIO 1]).
6. **Ledger write + commit (`lib.rs:334-335`).** `ChunkStore::mark_imported_in_tx(&tx, target_key, chunk_id)` (`chunks.rs:44`) records the chunk in `sync_chunks` *inside the same tx*, then `tx.commit()`. A crash mid-loop rolls back both the saves and the ledger mark, so a re-run reprocesses cleanly (closes Cloven 2026-05-11 [CRITICO]).
7. The report distinguishes `observation_count_saved` (freshly inserted), `observation_count_already_present` (per-row `INSERT OR IGNORE` skips), and `observation_count_skipped_chunk_level` (whole-chunk skip). The CLI prints all three.

> Round-trip invariant: A→export→git→B→import preserves ULIDs and content; re-importing the same file under the same `target_key` is a no-op (ledger); importing the same observation from a different chunk/target collapses on `INSERT OR IGNORE`. The cost is that `metadata` carries the only breadcrumb of the originating session.

---

### 19.6 (E) HTTP request lifecycle — auth, handler, service, storage, response

Server is launched by `seele serve` (`crates/seele-cli/src/commands/serve.rs:49`), which builds the service, resolves optional chat config, and calls `Server::run` (`crates/seele-http/src/server.rs:190`).

1. **Router assembly (`server.rs:99` `router`).** `AppState { service: Arc<SeeleService>, chat }` is built. Protected routes (`/memories`, `/search`, `/sessions`, `/links`, `/relations`, `/conflicts`, `/stats`, `/embedder`, `/chat`, ...) are registered on one sub-router; legacy ENGRAM aliases (`POST /save`, `GET /show/{id}`) are added only if `legacy_engram_paths` (ADR-13, `server.rs:154`). Public routes `/health`, `/version`, and the OpenAPI/Swagger routes (`openapi::routes()`) are merged **outside** the auth layer. Layers applied outermost-to-innermost: `TraceLayer`, `CorsLayer`, `CompressionLayer`.
2. **Bind + announce (`Server::run`, `server.rs:190-198`).** `TcpListener::bind` (`server.rs:192`) then `eprintln!("seele http listening on http://{addr}")` (`server.rs:194`, using the resolved `local_addr()` so `--port 0` callers can discover the OS-chosen port), then `axum::serve` (`server.rs:196`). The default port is `7777` and default bind `127.0.0.1` (`serve.rs:11,14`).
3. **Auth middleware (`server.rs:165` → `auth.rs:37`).** When `auth_bearer = Some(token)`, the protected sub-router is wrapped with `require_bearer`. `check_bearer` (`auth.rs:17`) reads `Authorization`, strips the mandatory `Bearer ` prefix, and compares the token (case-sensitive); a missing/mismatched token short-circuits to `401` before any handler runs. Public routes never see this layer.
4. **Extraction + handler (`handlers.rs`).** axum extracts `State<Arc<SeeleService>>` (via the `FromRef` impl, `server.rs:33`), plus `Path`/`Query`/`Json` as the route requires. Example: `save_memory` (`handlers.rs:23`) takes `Json<SaveRequest>`; `get_memory` (`handlers.rs:40`) takes `Path<String>` and calls `parse_id`. The handler is a thin shim that calls exactly one `SeeleService` method.
5. **Service + storage.** The service method runs the domain logic (for `/memories` and `/search`, the identical flows of 19.2/19.3, including the post-save embedding and the FTS/vec0 writes via the `int_id` bridge). Stores use the r2d2 pool (`init_pool`, `pool.rs:30-48`, `max_size = 8` per `PoolConfig::with_path`); each physical connection runs the canonical PRAGMAs (`journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`, `temp_store=MEMORY`) and loads the vec0 extension (entry point `sqlite3_vec_init`) once at creation via the `SqliteConnectionManager::with_init` closure — failure to load vec0 is fatal.
6. **Response / error mapping (`error.rs:38`).** A handler returning `Ok` serializes to `Json<T>` (200) or `StatusCode::NO_CONTENT` (204) for deletes/judgments. A handler returning `Err(ApiError)` is converted by `IntoResponse` into a JSON `ErrorBody { code, message }` with the canonical status: `BadRequest→400`, `Unauthorized→401`, `NotFound→404`, `Conflict→409`, `Internal→500`. The `From` impls at `error.rs:58-96` collapse `StorageError`/`SearchError`/`EmbedderError`/`SeeleError` into `ApiError` (e.g. `StorageError::NotFound → ApiError::NotFound`, embedder errors → `Internal`).

> The `/chat` handler (`handlers.rs:254`) is the one stateful exception: it builds a `ChatProvider` — `AnthropicProvider` when `provider == "anthropic"` (case-insensitive), otherwise `OpenAICompatibleProvider` driving a `/v1/chat/completions` endpoint (`handlers.rs:304-315`) — and registers a `seele_search` tool whose handler closure calls `svc.search_observations` (`handlers.rs:343`) — i.e. the LLM's tool-use loops back through the same SEARCH flow, with `limit = args.limit.unwrap_or(5).min(20)` (`handlers.rs:331`, default 5, hard cap 20) and `score_boost_multiplier = 1.0` (`handlers.rs:339`), returning 280-char content snippets (`handlers.rs:355`). Per-request `api_key`/`provider`/`model`/`endpoint` override the CLI config and are used once, never persisted (`handlers.rs:215`).

---

### 19.7 Cross-flow summary

- **Single service core.** CLI, MCP, and HTTP all funnel into `SeeleService` methods; the only differences are (de)serialization and the error envelope (`ApiError` JSON vs `ToolError`/JSON-RPC vs anyhow/stdout). The empty-query gate is enforced identically by all three search entries.
- **Two materializations of the `as_i64()` bridge on write** (`observations.int_id` at INSERT, `observations_vec.rowid` at `set_embedding`) and **two reads** (FTS and vec JOINs `ON o.int_id = <rowid>`).
- **Atomicity boundaries differ:** SAVE commits the row + FTS trigger writes in one tx but writes the embedding *after* commit (best-effort, can desync); SYNC import commits all raw saves + the ledger mark in one tx (strict). The known limitation is the row/embedding split in SAVE — flagged in the service doc and worth a reindex command in a future section.
