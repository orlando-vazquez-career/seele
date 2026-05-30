## 8. MCP Server — `seele-mcp`

`seele-mcp` is SEELE's Model Context Protocol transport: a JSON-RPC 2.0 server spoken over stdio so that MCP host clients (Claude Code, Cursor, Windsurf, OpenCode) can read and write the local memory store as a tool surface. It is one of the three transports over the same service/storage core; unlike the CLI (which links the storage stores directly) the MCP server reuses the HTTP crate's service layer. In the crate graph it depends on `seele-core`, `seele-storage`, `seele-search`, `seele-embedder`, and crucially `seele-http` (for `SeeleService`, `ApiError`, and the `dto::*` request/response types). The binary `seele` mounts it via the `seele mcp [--tool-prefix <p>] [--db <path>]` subcommand. The crate is small and synchronous at its core: only the I/O loop is async (tokio); every tool handler is a plain blocking `fn`.

The crate-level doc comment states the design intent (`crates/seele-mcp/src/lib.rs:1`):

```rust
// crates/seele-mcp/src/lib.rs:1
//! SEELE MCP server — stdio transport for Claude Code, Cursor, OpenCode, etc.
//! The crate exposes a JSON-RPC 2.0 server over a generic
//! `AsyncRead + AsyncWrite` pair (stdio by default). 19 canonical SEELE
//! tools are registered; consumers can rename them at the boundary via
//! `McpServerConfig.tool_prefix` (ADR-13: `mnema` is a first-class
//! ENGRAM-compat alias set).
```

The public API surface is re-exported at the crate root (`lib.rs:14`): `McpServer`, `McpServerConfig` (from `server`), and `all_tools`, `build_index`, `Tool`, `ToolError` (from `tools`). The `jsonrpc` module is public but not re-exported.

### 8.1 File-by-file map

| File | Responsibility |
|------|----------------|
| `src/lib.rs` | Module declarations and public re-exports. |
| `src/jsonrpc.rs` | JSON-RPC 2.0 wire types (`Request`, `Response`, `ErrorObject`) and the error-code constants. |
| `src/server.rs` | `McpServer`, `McpServerConfig`, the stdio/IO read loop, line dispatch, `initialize`/`tools/list`/`tools/call` handling, and the `CallToolResult` envelope wrapping. |
| `src/tools.rs` | `Tool`/`ToolDescriptor`/`ToolHandler` types, `ToolError` enum + conversions, `all_tools()` registry of all 19 tools, `build_index()` + `translate_name()` (ADR-13 prefix mapping), and all `schema_*()` input-schema builders. |
| `src/tool_impls/mod.rs` | Declares the four handler modules: `memories`, `meta`, `relations`, `sessions`. |
| `src/tool_impls/memories.rs` | Handlers: `save`, `search`, `show`, `list`, `update_metadata`, `soft_delete`, `restore`, `link`. |
| `src/tool_impls/sessions.rs` | Handlers: `start`, `end`, `summary`, `capture_passive` + the `extract_key_learnings` / `truncate_title` helpers. |
| `src/tool_impls/relations.rs` | Handlers: `judge`, `compare`. |
| `src/tool_impls/meta.rs` | Handlers: `stats`, `projects`, `doctor`, `version`, `suggest_topic_key`. |
| `tests/stdio_e2e.rs` | 12 E2E tests over a `tokio::io::duplex` transport (protocol + tools/call). |
| `tests/call_tool_result_envelope.rs` | 2 narrow tests asserting the `CallToolResult` wire shape (added by the 2026-05-20 patch). |

### 8.2 `jsonrpc.rs` — wire framing and error codes

MCP rides a subset of JSON-RPC 2.0. The module's doc note (`jsonrpc.rs:1`) summarizes it: request side carries only `id`, `method`, `params`; response side carries `result` xor `error`; notifications (no `id`) get logged and produce no response.

`Request` (`jsonrpc.rs:11`) is `Deserialize`-only:

| Field | Type | Notes |
|-------|------|-------|
| `jsonrpc` | `String` | `#[allow(dead_code)]` — accepted but never validated against `"2.0"`. |
| `id` | `Option<Value>` | `#[serde(default)]`; `None` means notification. The id is an opaque `Value` (number or string), echoed verbatim. |
| `method` | `String` | The RPC method name. |
| `params` | `Value` | `#[serde(default)]` → `Value::Null` when absent. |

`Response` (`jsonrpc.rs:23`) is `Serialize`-only with `jsonrpc: &'static str` pinned to `"2.0"`, an `id: Value`, and mutually exclusive `result`/`error` (both `#[serde(skip_serializing_if = "Option::is_none")]`). Two constructors enforce the xor: `Response::ok(id, result)` and `Response::err(id, error)`.

`ErrorObject` (`jsonrpc.rs:53`) carries `code: i32`, `message: String`, optional `data: Option<Value>` (skipped when `None`), with a `new(code, message)` constructor that leaves `data: None`.

The `codes` submodule (`jsonrpc.rs:72`) defines the standard codes plus one SEELE extension:

| Constant | Value | Meaning |
|----------|-------|---------|
| `PARSE_ERROR` | `-32700` | Line was not valid JSON. |
| `INVALID_REQUEST` | `-32600` | Declared but currently unused in dispatch. |
| `METHOD_NOT_FOUND` | `-32601` | Unknown JSON-RPC method. |
| `INVALID_PARAMS` | `-32602` | Missing/invalid params, unknown tool, or `ToolError::BadParams`. |
| `INTERNAL_ERROR` | `-32603` | `ToolError::Internal`. |
| `TOOL_ERROR` | `1001` | SEELE-specific: tool exists but handler returned a domain error (`NotFound`/`Conflict`). |

Note `TOOL_ERROR = 1001` is a positive, non-standard code (the reserved JSON-RPC server-error band is `-32099..-32000`); it is a deliberate SEELE convention to distinguish protocol failures from domain failures while still travelling through the JSON-RPC `error` channel.

### 8.3 `server.rs` — lifecycle, dispatch, and the CallToolResult patch

`McpServerConfig` (`server.rs:17`) is `#[derive(Clone, Default)]` and holds a single field, `tool_prefix: Option<String>`. `None` keeps canonical `seele_*` names; `Some("mnema")` triggers the ADR-13 alias set.

`McpServer` (`server.rs:25`) holds two fields: `service: SeeleService` (the reused HTTP service) and `tools: HashMap<String, Tool>` (the exposed-name → handler index). The constructor `McpServer::new(service, config)` (`server.rs:31`) builds the index eagerly via `build_index(config.tool_prefix.as_deref())` (`server.rs:32`), so prefix translation happens once at startup, not per request. The map is never mutated afterward.

**Lifecycle / transport.** `run_stdio(&self)` (`server.rs:38`) wires `tokio::io::stdin()`/`stdout()` into the generic `run_io`. `run_io<R, W>(&self, reader, mut writer)` (`server.rs:46`) is the heart of the loop. It is generic over `AsyncRead + Unpin` / `AsyncWrite + Unpin` precisely so tests can substitute `tokio::io::duplex` (the doc comment at `server.rs:44` says exactly this). The loop is line-delimited:

```rust
// crates/seele-mcp/src/server.rs:51
let mut lines = BufReader::new(reader).lines();
while let Some(line) = lines.next_line().await? {
    if line.trim().is_empty() { continue; }
    if let Some(resp) = self.handle_line(&line) {
        let s = serde_json::to_string(&resp)?;
        writer.write_all(s.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
    }
}
Ok(())
```

Each response is serialized, written, terminated with `\n`, and explicitly `flush()`ed (so a streaming client sees output immediately). Blank lines are skipped. The loop is single-threaded and strictly serial — there is no concurrency between requests, no per-request task spawn, and no batching. **Shutdown** is implicit: `next_line()` returns `None` at EOF on the reader (the host closing stdin), the `while` exits, and `run_io` returns `Ok(())`. There is no explicit MCP `shutdown` method handler and no graceful-drain step; the process simply ends when stdin closes.

**`handle_line` dispatch.** `handle_line(&self, line: &str) -> Option<Response>` (`server.rs:68`) is the pure (non-async) parse+route function. It returns `Option` so notifications can yield no response:

1. Parse the line as a `Request`. On `serde_json` failure it returns `Some(Response::err(Value::Null, PARSE_ERROR, "parse error: …"))` — the id is `Null` because the request could not even be read (matches spec). This is the only branch that returns a response with a null id.
2. `let id = req.id.clone()?;` — the `?` on the `Option<Value>` short-circuits to `None` for notifications, so no response is emitted. This is the documented "no id → no response" behavior (verified by `notification_no_id_produces_no_response`).
3. Match on `req.method`:
   - `"initialize"` → the handshake (see below).
   - `"tools/list"` → enumerate tools.
   - `"tools/call"` → delegate to `dispatch_tool_call`.
   - anything else → `METHOD_NOT_FOUND` error.

**The initialize handshake** (`server.rs:81`) returns a fixed object:

```rust
// crates/seele-mcp/src/server.rs:83
serde_json::json!({
    "serverInfo": { "name": "seele", "version": env!("CARGO_PKG_VERSION") },
    "protocolVersion": "2024-11-05",
    "capabilities": { "tools": {} },
})
```

The advertised `protocolVersion` is pinned to `"2024-11-05"`. Capabilities advertise only `tools` (an empty object meaning "tools are supported"); no `resources`, `prompts`, `logging`, or `completions` capabilities are declared. The server does not negotiate or read the client's requested `protocolVersion` from `initialize` params — it always answers `2024-11-05`. The `notifications/initialized` notification a client sends after the handshake is handled by the generic notification path (no id → no response); there is no dedicated handler for it.

**`tools/list`** (`server.rs:89`) maps the `tools` HashMap into a `Vec<ToolDescriptor>`, **sorts by name** (`entries.sort_by(|a, b| a.name.cmp(&b.name))`) for deterministic ordering, and returns `{ "tools": [...] }`. Each descriptor serializes `name`, `description`, and `inputSchema` (the `input_schema` field is renamed to `inputSchema` via `#[serde(rename)]`).

**`tools/call`** (`server.rs:113`, `dispatch_tool_call`) is the most important path and the site of the 2026-05-20 patch:

1. Read `params.name` as a string; missing → `INVALID_PARAMS "params.name required"`.
2. Read `params.arguments`, defaulting to an empty object when absent (`unwrap_or(Value::Object(Default::default()))`).
3. Look up the tool by exposed name in `self.tools`; missing → `INVALID_PARAMS "unknown tool: {name}"` (note: unknown tool maps to `-32602`, not `METHOD_NOT_FOUND`, since the method `tools/call` exists; verified by `tools_call_unknown_tool_returns_invalid_params`).
4. Invoke the handler `(tool.handler)(&self.service, arguments)`.
5. On `Ok(v)`, wrap in the `CallToolResult` envelope; on `Err(e)`, return `Response::err(id, e.to_jsonrpc())`.

The success-path envelope is the patch's core change (`server.rs:138`):

```rust
// crates/seele-mcp/src/server.rs:145
let text = serde_json::to_string(&v)
    .unwrap_or_else(|_| "<unserializable tool output>".into());
Response::ok(
    id,
    serde_json::json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false,
    }),
)
```

**The 2026-05-20 patch (`docs/aegis/devlogs/2026-05-20-patch-mcp-call-tool-result.md`).** Before this patch (a bug present since Sprint-03, 2026-05-10, surviving v0.1.0 and v0.2.0), `dispatch_tool_call` returned the handler's raw JSON directly as the JSON-RPC `result`. MCP clients look for `result.content[0].text`; finding `undefined`, every tool call rendered as **"completed with no output"** even though the DB and handlers worked. The bug was caught by Orlando on 2026-05-20 during the CivicSys Syscoin hackathon when three calls (`seele_doctor`, `seele_version`, `seele_stats`) from Claude Code 4.7 produced no visible output; a `printf | seele mcp` wire probe confirmed `result` lacked `content[]`. The fix wraps every successful handler output as a single `text` content block holding the JSON-stringified value, with `isError: false`. The devlog records several deliberate decisions:

- **One `text` block, JSON-stringified**: idiomatic for structured-JSON tools pre-2025-06-18; the spec also allows `image`/`audio`/`embeddedResource` blocks but a single stringified `text` block is the ecosystem convention.
- **`unwrap_or_else` not `?`/`unwrap`**: although `Value` is always round-trippable to a string, a panic here would kill the entire MCP session. The visible fallback `"<unserializable tool output>"` is preferred over a process crash.
- **Errors stay on the JSON-RPC `error` channel** (unchanged). The spec also permits domain errors via `isError: true` in the envelope; migrating to that is a deferred follow-up.
- **`structuredContent` (MCP 2025-06-18) deferred**: it would require bumping the advertised `protocolVersion`, which older clients (some Cursor versions) might reject at handshake. Deferred to v0.3.

The patch shipped a new test file (`call_tool_result_envelope.rs`) verifying the envelope shape with `seele_doctor` and `seele_version`, and migrated five assertions in `stdio_e2e.rs` to a `tool_result()` helper that unwraps `r["result"]["content"][0]["text"]` and re-parses it. The crate's test count went to 20/20 green. The verification round also surfaced (but did not cause) pre-existing failures: OpenAPI drift for `/chat` + `/chat/info` and 10 drifted `seele-tui` insta snapshots.

### 8.4 `tools.rs` — the registry, ToolError, and the ADR-13 prefix mapping

`ToolHandler` (`tools.rs:60`) is a function-pointer type: `fn(&SeeleService, Value) -> Result<Value, ToolError>`. Using `fn` (not a boxed closure) keeps `Tool` `Clone` and the registry zero-allocation per entry. `Tool` (`tools.rs:62`) bundles `name: &'static str`, `description: &'static str`, `input_schema: Value`, and `handler: ToolHandler`. `ToolDescriptor<'a>` (`tools.rs:70`) is the serialized form sent on the wire, with `input_schema` renamed to `inputSchema`.

`ToolError` (`tools.rs:18`) is the crate's `thiserror` enum:

| Variant | `to_jsonrpc()` code | Source mapping |
|---------|---------------------|----------------|
| `BadParams(String)` | `INVALID_PARAMS` (-32602) | `ApiError::BadRequest`, `serde_json::Error`, local validation. |
| `NotFound(String)` | `TOOL_ERROR` (1001) | `ApiError::NotFound`. |
| `Conflict(String)` | `TOOL_ERROR` (1001) | `ApiError::Conflict`. |
| `Internal(String)` | `INTERNAL_ERROR` (-32603) | `ApiError::Internal`, `ApiError::Unauthorized`. |

Two `From` impls bridge the boundary. `From<seele_http::ApiError>` (`tools.rs:41`) maps the service-layer error enum: `BadRequest→BadParams`, `NotFound→NotFound`, `Conflict→Conflict`, `Internal→Internal`, and — notably — `Unauthorized→Internal("unauthorized")` (MCP has no auth concept, so the HTTP-only `Unauthorized` is collapsed to an internal error). `From<serde_json::Error>` (`tools.rs:54`) maps deserialization failures to `BadParams("json error: …")`, which is what lets handlers use `serde_json::from_value(params)?` and have a malformed payload surface as `-32602`.

`all_tools()` (`tools.rs:79`) is the static registry — a `Vec<Tool>` of exactly 19 entries, "never mutated at runtime." `build_index(prefix)` (`tools.rs:205`) converts it into the `HashMap<String, Tool>` used by the server, applying the prefix policy:

```rust
// crates/seele-mcp/src/tools.rs:208
let name = match prefix {
    None => tool.name.to_string(),
    Some("seele") => tool.name.to_string(),
    Some(p) => translate_name(tool.name, p),
};
```

So both `None` and the explicit `"seele"` prefix yield canonical names. **ADR-13 mapping** lives in `RENAMES` + `translate_name` (`tools.rs:223`):

```rust
// crates/seele-mcp/src/tools.rs:223
const RENAMES: &[(&str, &[(&str, &str)])] = &[("mnema", &[("search", "recall")])];
```

`translate_name(canonical, prefix)` strips the `seele_` prefix from the canonical name to get the suffix, looks for a `(prefix, [(from_suffix, to_suffix)])` row, applies any suffix rename, and formats `"{prefix}_{renamed}"`. For `prefix = "mnema"`, `seele_save → mnema_save`, but `seele_search → mnema_recall` because ENGRAM used "recall" rather than "search". Every other tool simply gets the `mnema_` prefix with its suffix unchanged. This is a table-driven design explicitly so that future consumers add a row instead of an `if` chain. The mapping is verified by `tools_list_with_mnema_prefix_aliases_recall` (expects both `mnema_save` and `mnema_recall`, and no `seele_*` names) and by `tools_call_under_mnema_prefix_routes_to_canonical_handler` (a `mnema_save` call routes to the canonical `memories::save` handler). The CLI flag that drives this is `--tool-prefix` (`crates/seele-cli/src/commands/mcp.rs:11`), passed straight into `McpServerConfig.tool_prefix`.

The remainder of `tools.rs` is the `schema_*()` helpers (`tools.rs:239`–415), each returning a `serde_json::Value` JSON Schema object. Shared helpers: `empty_object_schema()` (`{type:object, properties:{}, additionalProperties:false}`) and `schema_id_only(field)` (one required string ULID field). The rest are per-tool.

### 8.5 The 19 tools — schemas, handlers, effects

All handlers reach storage/search through `SeeleService` methods (e.g. `save_observation`, `search_observations`, `get_observation`, `create_link`, `judge_relation`, `stats`, `embedder_info`). They convert JSON params into the same `seele_http::dto` request structs the HTTP handlers use, call the service, and serialize the response DTO back to `Value`. Several handlers parse a ULID via a local `parse_id` (duplicated in `memories.rs`, `sessions.rs`, `relations.rs`) that fails with `BadParams("invalid id: …")`.

| # | Tool | Required params | Optional params | Handler | Effect / returns |
|---|------|-----------------|-----------------|---------|------------------|
| 1 | `seele_save` | `title`, `content` | `type`(default memory), `project`, `scope`(project\|personal), `topic_key`, `session_id`, `tool_name`, `metadata` | `memories::save` | Save observation; computes embedding, strips `<private>`, topic-key upsert + dedup. Returns `SaveResponse` (`id`, `outcome`, ...). |
| 2 | `seele_search` | — | `query`, `project`, `scope`, `type`, `limit`, `include_purist`, `include_annotations`, `score_boost_multiplier`, `max_vec_distance` | `memories::search` | Calls `enforce_search_query_or_filter` then hybrid FTS+vec+RRF. Empty query w/o filter → `BadParams` (-32602). |
| 3 | `seele_show` | `id` | — | `memories::show` | Fetch one observation; absent → `NotFound("observation {id}")` (1001). |
| 4 | `seele_list` | — | `project`, `scope`, `type`, `topic_key`, `session_id`, `limit`, `include_deleted` | `memories::list` | List observations (`Vec<ObservationDto>`). |
| 5 | `seele_update_metadata` | `id`, `metadata_patch`(object) | — | `memories::update_metadata` | Merge patch into metadata JSON. Rejects non-object patch. Returns `{ok, id}`. |
| 6 | `seele_delete` | `id` | — | `memories::soft_delete` | Soft-delete (sets `deleted_at`). Returns `{ok, id}`. |
| 7 | `seele_restore` | `id` | — | `memories::restore` | Clears `deleted_at`. Returns `{ok, id}`. |
| 8 | `seele_link` | `from_id`, `to_id`, `link_type` | `metadata` | `memories::link` | Create a typed link. Returns `LinkDto`. |
| 9 | `seele_stats` | — | — | `meta::stats` | Aggregate stats (`StatsResponse`). |
| 10 | `seele_session_start` | `project` | `directory` | `sessions::start` | Start a session. Returns `SessionDto`. |
| 11 | `seele_session_end` | `id` | `summary` | `sessions::end` | End active session. Returns `{ok, id}`. |
| 12 | `seele_session_summary` | `session_id`, `title`, `summary` | `project` | `sessions::summary` | Saves a `type=memory` observation with `topic_key="session/<id>"` (upserts in-place on repeat) + `tool_name="seele_session_summary"`. Returns `SaveResponse`. |
| 13 | `seele_capture_passive` | `transcript` | `project`, `session_id` | `sessions::capture_passive` | Parses `## Key Learnings` block, saves each bullet as `type=learning`. Returns `{saved:[ids], count}`. |
| 14 | `seele_judge` | `relation_id`, `status`(pending\|judged\|orphaned\|ignored) | `reason`, `evidence`, `confidence` | `relations::judge` | Apply judgment to a `memory_relation`. Returns `{ok, id}`. |
| 15 | `seele_compare` | `sync_id`, `source_id`, `target_id` | `reason` | `relations::compare` | Create a `conflicts_with` relation (pending), `marked_by_actor="seele_compare"`, `marked_by_kind="tool"`. Returns `RelationDto`. |
| 16 | `seele_suggest_topic_key` | `title` | `content` | `meta::suggest_topic_key` | Keyword-scores against 7 hard-coded families; returns `{family, score, suggestion:"family/auto"}` or `{family:null, suggestion:null}`. Pure — ignores the service. |
| 17 | `seele_projects` | — | — | `meta::projects` | Distinct active projects → `{projects:[...]}`. |
| 18 | `seele_doctor` | — | — | `meta::doctor` | `{status:"ok", embedder:{model_id,dim,expected_sha256}, observations_active, sessions_total, schema_version}` (schema_version is `CARGO_PKG_VERSION`). |
| 19 | `seele_version` | — | — | `meta::version` | `{name:"seele", version:CARGO_PKG_VERSION}`. Pure — ignores the service. |

Three handlers carry non-trivial logic worth detailing:

**`sessions::capture_passive`** (`sessions.rs:68`) extracts learnings via `extract_key_learnings` (`sessions.rs:111`): it scans lines, toggles `in_section` when it hits a `## ` heading whose lowercased text equals `"key learnings"` or `"key learnings:"`, treats `- ` / `* ` prefixed lines as new bullets, and folds non-bullet continuation lines into the current bullet (joined by a space). It stops at the next `## ` heading or EOF. Empty transcript → `{saved:[], count:0}` early-return. Each learning becomes a `SaveRequest` with `r#type:"learning"`, `topic_key:None`, `tool_name:"seele_capture_passive"`, and a `truncate_title` (120-char cap with an ellipsis). Unit tests at `sessions.rs:175` pin the parsing behavior.

**`relations::compare`** (`relations.rs:46`) hard-codes `relation:"conflicts_with"` and the actor/kind provenance fields, leaving `evidence`/`confidence`/`session_id` as `None` — it is a thin "these two conflict, pending judgment" marker.

**`meta::suggest_topic_key`** (`meta.rs:44`) is the only handler with embedded domain heuristics: a `&[(&str, &[&str])]` of 7 ENGRAM-inherited families (architecture, bug, decision, pattern, config, discovery, learning), each scored by counting keyword substring hits in the lowercased `title + content`. The highest non-zero score wins; below the noise floor it returns nulls. It takes `&SeeleService` but never uses it (the test constructs a dummy in-memory service).

### 8.6 Boundary, concurrency, and gotchas

**How handlers reach storage/search.** `seele-mcp` does not own a pool or stores; it holds a `SeeleService` (constructed by the CLI's `build_service`, `crates/seele-cli/src/commands/mcp.rs:15`). `SeeleService::new(pool, embedder)` (`crates/seele-http/src/service.rs:47`) assembles the `ObservationStore`, `SessionStore`, `LinkStore`, `RelationStore`, `PromptStore`, `ChunkStore`, and a `SearchEngine`. So the MCP server, HTTP server, and (indirectly) the CLI all funnel through the *same* service code — this is why the 2026-05-20 patch correctly notes the DB and handlers always worked; only the wire envelope was wrong. The anti-exfiltration gate `enforce_search_query_or_filter` (`service.rs:442`, flagged by Cloven 2026-05-10) is shared between HTTP `/search` and MCP `seele_search`; `memories::search` calls it explicitly before invoking the service.

**Concurrency / async / locking.** The only async surface is `run_io`'s read/write loop; everything inside `handle_line` and every handler is synchronous and blocking. Requests are processed strictly one at a time, in order, on a single task. There is no `Mutex`/`RwLock` in this crate — connection pooling and any locking live in `seele-storage`'s `Pool`. A slow handler (e.g. an embedding computation in `seele_save`) blocks the loop until it completes; there is no per-request timeout. Because handlers run on the tokio runtime thread without `spawn_blocking`, a long synchronous DB/embed call could stall the reactor, though in practice the MCP server is single-client and serial so this is acceptable.

**Error handling and failure modes.** Protocol-level failures (bad JSON, unknown method, missing `params.name`, unknown tool) become JSON-RPC `error` objects with the codes in §8.2. Domain failures bubble as `ToolError` → `to_jsonrpc()` → `error`. The success envelope's `serde_json::to_string` uses a defensive `unwrap_or_else` fallback to avoid panicking the whole session. There is no `isError: true` path yet — all domain errors travel as JSON-RPC `error` (the devlog lists "domain errors via `isError: true`" as a deferred follow-up, since clients render an in-chat message better than a bare error code).

**Edge cases / invariants / known limitations:**

- **`protocolVersion` is hard-coded `"2024-11-05"`** and never negotiated against the client's requested version. Bumping to `2025-06-18` (for `structuredContent`) is explicitly deferred to v0.3 (devlog §"Por qué NO migrar").
- **`jsonrpc` version is not validated** (`jsonrpc.rs:13` `#[allow(dead_code)]`) — a request claiming `"jsonrpc":"1.0"` is accepted.
- **`INVALID_REQUEST` (-32600) is defined but never emitted.**
- **Unknown tool yields `-32602` (INVALID_PARAMS), not `-32601`** — a subtle but intentional distinction (the `tools/call` method exists; the *tool* doesn't).
- **`tools/list` is sorted by name** for determinism; tests rely on the count being exactly 19.
- **No explicit `shutdown` method**; the server stops only on stdin EOF.
- **No batching**: JSON-RPC batch arrays are not supported — each line must be a single object (a batch array would fail to deserialize into `Request` and return a parse error).
- **`parse_id` is duplicated** across three `tool_impls` modules — a minor DRY wart, not a bug.
- The crate `Cargo.toml` lists `tracing` as a dependency, but the dispatch path emits no tracing spans/events itself (tracing surfaces in the underlying `SeeleService`, e.g. the best-effort embedding warning at `service.rs:101`).

**Tests.** `tests/stdio_e2e.rs` drives the full server over `tokio::io::duplex(64*1024)` and a `round_trip` helper; it covers `initialize`, the 19-tool list, the mnema alias list, method-not-found, parse-error-with-null-id, the notification no-response invariant, a save→search round trip, the empty-query tool error, unknown-tool, mnema routing, doctor, and capture_passive. `tests/call_tool_result_envelope.rs` adds the two envelope-shape guards from the patch. The devlog's own follow-up flags that all these tests use an in-memory duplex pipe, not a real OS pipe to a subprocess, so a true end-to-end test against a real MCP client remains a deferred ticket.
