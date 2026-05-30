## 9. HTTP REST API — `seele-http`

`seele-http` is the axum-based REST transport for SEELE. It is also, structurally, the **home of the shared application service** (`SeeleService`) that every other transport reuses: the MCP server (`seele-mcp`), the TUI (`seele-tui`), and the CLI. The crate therefore plays a dual role — it is both a leaf-ish HTTP adapter and the application-layer core that sits between the storage/search/embedder crates and the outward-facing interfaces.

### 9.1 Purpose and place in the crate graph

Per the workspace layering, `seele-http` depends on `seele-core`, `seele-storage`, `seele-search`, `seele-embedder`, and `seele-chat` (see `crates/seele-http/Cargo.toml`, lines 11–15). It is depended upon by `seele-mcp` (which imports `SeeleService`, the DTOs, the `enforce_search_query_or_filter` gate, and `ApiError` — `crates/seele-mcp/Cargo.toml`, line 15), by `seele-tui` (which uses `SeeleService` and the DTOs **directly, in-process** — `crates/seele-tui/Cargo.toml`, line 14), and by `seele-cli` (the binary, whose `serve` subcommand builds and runs the `Server`). The crate doc spells out the three public surfaces (`crates/seele-http/src/lib.rs`, lines 1–21):

- `SeeleService` — application-layer service used by both HTTP handlers and MCP tools.
- `Server` / `ServerConfig` / `AppState` — the router builder and binder (re-exported from `server`; `ChatProviderConfig` is **not** re-exported at crate root and must be reached via `seele_http::server::ChatProviderConfig`).
- `ApiError` / `ErrorBody` / `Result` — the uniform JSON error envelope.

External crates and their roles: `axum` (HTTP framework + router + extractors), `tower`/`tower-http` (CORS, trace, gzip compression middleware), `utoipa` + `utoipa-swagger-ui` (OpenAPI doc + Swagger UI), `serde`/`serde_json` (DTO (de)serialization), `thiserror` (typed `ApiError`), `tracing` (structured logs), `chrono` (timestamp conversion in DTOs), `tokio` (async runtime + `TcpListener`), `anyhow` (only on `Server::run`'s return type). Dev-deps (`Cargo.toml`, lines 30–32) add `tempfile` (per-test temp DB dir), `reqwest` (in-process HTTP client for E2E tests) and `regex` (the anti-drift OpenAPI/router consistency test).

### 9.2 File-by-file map

- **`lib.rs`** — module declarations (`auth`, `dto`, `error`, `handlers`, `openapi`, `server`, `service`) and the re-export surface. Nothing else.
- **`server.rs`** — `AppState`, `ServerConfig`, `ChatProviderConfig`, the `Server` struct, `Server::router()` (router + middleware assembly), `Server::run()` (bind + serve), and the two public handlers `health` / `version`.
- **`service.rs`** — `SeeleService` (the application service holding all stores + search + embedder + pool), all of its business methods, the free function `enforce_search_query_or_filter`, and the private `ArcEmbedder` adapter.
- **`handlers.rs`** — one async axum handler per endpoint, plus the chat handler (`chat`, `chat_info`), the `ChatRequest`/`ChatResponse`/`ChatInfoResponse`/`ConflictsQuery` local types, and the `default_model_for`/`default_endpoint_for` helpers.
- **`dto.rs`** — every request/response DTO, their `From` conversions from core types, and the parsing helpers (`parse_id`, `parse_scope`, `parse_type`, `parse_metadata`, `parse_session_status`, `parse_relation_kind`, `parse_judgment_status`).
- **`auth.rs`** — `check_bearer` and the `require_bearer` middleware.
- **`error.rs`** — `ApiError` enum, `ErrorBody`, `IntoResponse` mapping, and `From` impls from each inner error tree.
- **`openapi.rs`** — `SchemaRegistry` (utoipa schema derive), `build_openapi`, hand-authored `build_paths`, the path-builder DSL helpers, and `routes()` (Swagger UI mount).
- **`tests/`** — six integration test files: `skeleton.rs` (health/version/404), `handlers_basicos.rs`, `handlers_c1_lifecycle.rs`, `handlers_c2_relations_stats.rs`, `handlers_d_auth_openapi.rs` (auth + OpenAPI + legacy paths), `openapi_consistency.rs` (router↔spec anti-drift).

### 9.3 `service.rs` — the shared application service

`SeeleService` (`service.rs`, lines 33–44) is `#[derive(Clone)]` and aggregates one store per domain plus the search engine, embedder, and pool:

```rust
// service.rs:33
#[derive(Clone)]
pub struct SeeleService {
    pub observations: ObservationStore,
    pub sessions: SessionStore,
    pub links: LinkStore,
    pub relations: RelationStore,
    pub prompts: PromptStore,
    pub chunks: ChunkStore,
    pub search: Arc<SearchEngine>,
    pub embedder: Arc<dyn Embedder>,
    pub pool: Pool,
}
```

`SeeleService::new(pool, embedder)` (lines 47–67) constructs each store over a clone of the `Pool` (cheap — the pool is itself a clone-shareable handle), wraps the search engine in `Arc`, and bridges the shared `Arc<dyn Embedder>` into the `Box<dyn Embedder>` that `SearchEngine::new` requires via the private `ArcEmbedder(Arc<dyn Embedder>)` adapter (struct at line 458, `impl Embedder` at lines 460–476). This adapter exists because `SearchEngine` currently demands an *owned* `Box<dyn Embedder>` while callers want to share one embedder instance; the adapter delegates `embed`, `embed_batch`, `dim`, `model_id`, and `expected_sha256` straight through to the inner `Arc`. The net effect is that one model instance serves both the post-save embedding write and the search-time query embedding. Note: `new` clones the supplied `embedder` once into the `ArcEmbedder` (line 54) and also stores the original `Arc<dyn Embedder>` on the struct (line 64), so the same instance is reachable both directly (for `save_observation`/`embedder_info`) and through the search engine.

The service is the **single point of truth for business logic** so that the HTTP handlers and the MCP tools stay DRY (`service.rs`, lines 1–7). Notable methods:

| Method | Behavior / notes |
|---|---|
| `save_observation(SaveRequest) -> Result<SaveResponse>` (lines 73–132) | Parses `session_id`/`scope`/`type`/`metadata` at the boundary, calls `observations.save(SaveInput)`, then **best-effort** embeds the content and writes the vector (lines 96–107). Embedding failure (or a failed embedding write) logs a `tracing::warn!` but does NOT 5xx — the row is persisted and a reindex can fix the vec branch later. The `SaveOutcome` enum maps to `outcome: "created" \| "upserted_topic" \| "duplicate_merged"` with the optional `revision_count`/`duplicate_count`. |
| `search_observations(SearchRequest) -> Result<SearchResponse>` (lines 136–157) | Builds a `SearchQuery` (mapping `type`→`kind`, parsing `scope`, `per_method_limit: None`, passing through `include_purist`, `score_boost_multiplier`, `max_vec_distance`, `include_annotations`), runs the engine, converts hits to `SearchHitDto`. **Does not** enforce the empty-query gate — that is the transport's job. |
| `get_observation` / `list_observations` / `soft_delete_observation` / `restore_observation` | Thin pass-throughs to `ObservationStore`. `list_observations` maps the query DTO to `ObservationQuery` including `include_deleted` and parses `scope`/`session_id`/`type` at the boundary. |
| `merge_observation_metadata(id, patch)` (lines 200–226) | Read-modify-write: fetches the row (404 via `ApiError::NotFound` if absent), shallow-merges the patch object into existing metadata (new keys overwrite; nested objects are NOT recursively merged), writes back via `ObservationPatch`. Errors `Internal("metadata is not an object")` if the stored metadata is non-object. (Note: this method has no dedicated HTTP route in `server.rs` — it is exposed via the MCP `seele_update_metadata` tool, not the REST router.) |
| `list_projects()` (lines 229–231) | Distinct active project names. (Likewise has no dedicated HTTP route; reached via the MCP `seele_projects` tool / CLI.) |
| `start_session` / `end_session` / `abort_session` / `get_session` / `list_sessions` | Session lifecycle over `SessionStore`. `list_sessions` parses the `status` filter strictly. |
| `create_link` / `list_links_for_observation` / `delete_link` | Links. `create_link` rejects empty `link_type` (after `trim`) with a 400 (lines 273–275). `list_links_for_observation` queries both `from_id` and `to_id` sides, then **dedups by link id** through a `BTreeMap<String, Link>` so a self-link is not returned twice (lines 286–302). |
| `create_relation` / `list_relations` / `judge_relation` / `list_pending_conflicts` | Relations. `create_relation` validates non-empty `sync_id` (after `trim`), parses both endpoint IDs, and rejects `source_id == target_id` with a 400 (lines 311–321). `list_pending_conflicts` is convenience sugar for `relation=conflicts_with & status=pending` (lines 384–392). |
| `stats()` (lines 396–427) | Aggregates `ObservationStats` (active/deleted/projects/by_type/by_scope) and `SessionStats` (total/by_status) into `StatsResponse`. |
| `embedder_info()` (lines 429–435) | Returns model id, dim, expected sha256 — infallible, returns `EmbedderInfo` directly (not `Result`). |

`enforce_search_query_or_filter(&SearchRequest)` (lines 442–453) is the **anti-exfiltration gate** shared by HTTP `/search` and MCP `seele_search`. It returns `Err(ApiError::BadRequest)` when the query text is empty (after `trim`) **and** none of `project`/`scope`/`type` is present, mitigating a list-all-DB vector. The doc comment attributes this to Cloven's 2026-05-10 review (lines 438–441). The HTTP handler calls it before delegating (`handlers.rs`, line 35); the MCP search tool calls it identically (`crates/seele-mcp/src/tool_impls/memories.rs`, line 28); the TUI also imports it (`crates/seele-tui/src/app.rs`, line 8).

### 9.4 `server.rs` — router, state, middleware, binding

**`AppState`** (lines 27–31) is the `#[derive(Clone)]` state every handler sees. It holds `service: Arc<SeeleService>` and `chat: Option<Arc<ChatProviderConfig>>`. Two `FromRef<AppState>` impls (lines 33–43) let handlers extract either `State<Arc<SeeleService>>` (the common case) or `State<Option<Arc<ChatProviderConfig>>>` (chat handlers) from the single composite state — the `chat` handler pulls both via two `State(...)` extractors.

**`ServerConfig`** (lines 45–57): `addr: SocketAddr`, `cors_origins: Vec<String>`, `auth_bearer: Option<String>`, `legacy_engram_paths: bool`, `chat: Option<ChatProviderConfig>`. The doc comment states the precise auth carve-out: when `auth_bearer` is set, *every route except `/health`, `/version`, `/docs/*`, `/openapi.json`* requires the bearer token (lines 49–50). `ServerConfig::loopback(port)` (lines 76–85) is a convenience constructor (127.0.0.1, no CORS, no auth, no legacy, no chat).

**`ChatProviderConfig`** (lines 61–73): `provider`, `api_key`, `model`, `endpoint: Option<String>`. The comment emphasizes the secret never leaves the box (lines 59–60) and that any provider label other than `"anthropic"` is treated as OpenAI-compatible using `endpoint` as the `/v1/chat/completions` URL (lines 63–66, 70–72).

**`Server::router()`** (lines 99–182) is the heart of the file and is `pub` so tests can mount it against an in-process listener. Control flow, in order:

1. Build `AppState` from the service (`Arc::new(self.service.clone())`) and the optional chat config mapped into `Arc` (lines 100–103).
2. Construct the CORS layer (lines 105–117): if `cors_origins` is empty, a bare `CorsLayer::new()` (effectively CORS-off for browsers); otherwise a **permissive** layer with `allow_origin(Any).allow_methods(Any).allow_headers(Any)`. The comment explains the explicit methods/headers are required so a browser JSON `POST` with `Content-Type` clears preflight — without them it fails with "failed to fetch" (lines 108–116). A per-origin allowlist is explicitly deferred ("Block D refines... today we open up permissively").
3. Build the **protected** sub-router (lines 122–152) — 22 operations across 16 canonical (non-legacy) paths. This is built first so the auth middleware wraps *only* these routes.
4. If `legacy_engram_paths` is set, append two ENGRAM aliases — `POST /save` (→ `save_memory`) and `GET /show/{id}` (→ `get_memory`) — to the protected router (lines 154–161). The comment (ADR-13) notes the new SEELE endpoints (sessions, relations, etc.) are deliberately NOT exposed under legacy paths because they don't exist in ENGRAM.
5. Attach state with `.with_state(state)` (line 163).
6. Conditionally wrap the protected router in the bearer middleware (lines 165–172): when `auth_bearer` is `Some(token)`, a `middleware::from_fn` closure clones the token per-request and calls `require_bearer(token, req, next)`; otherwise the protected router is unchanged.
7. Assemble the **outer** router (lines 174–182): public `/health` + `/version`, then `.merge(protected)`, then `.merge(openapi::routes())` (Swagger UI + `/openapi.json`), then the global layers in order: `TraceLayer::new_for_http()`, the CORS layer, `CompressionLayer::new()` (gzip). Because `/health`, `/version`, the docs, and the spec are merged *outside* the protected block, they are never wrapped by the auth middleware — the structural reason they stay public.

The route table inside the protected router uses axum 0.8 path syntax (`{id}` curly braces, not `:id`):

| Path | Methods → handlers |
|---|---|
| `/memories` | `POST save_memory`, `GET list_memories` |
| `/memories/{id}` | `GET get_memory`, `DELETE soft_delete_memory` |
| `/memories/{id}/restore` | `POST restore_memory` |
| `/memories/{id}/links` | `GET list_links_for_memory` |
| `/search` | `POST search_memories` |
| `/sessions` | `POST start_session`, `GET list_sessions` |
| `/sessions/{id}` | `GET get_session` |
| `/sessions/{id}/end` | `PUT end_session` |
| `/sessions/{id}/abort` | `PUT abort_session` |
| `/links` | `POST create_link` |
| `/links/{id}` | `DELETE delete_link` |
| `/relations` | `POST create_relation`, `GET list_relations` |
| `/relations/{id}/judge` | `PUT judge_relation` |
| `/conflicts` | `GET list_pending_conflicts` |
| `/stats` | `GET get_stats` |
| `/embedder` | `GET get_embedder_info` |
| `/chat` | `POST chat` |
| `/chat/info` | `GET chat_info` |
| `/save` (legacy) | `POST save_memory` |
| `/show/{id}` (legacy) | `GET get_memory` |

Counting operations: **22 protected operations across 18 protected paths** (16 paths × their methods sum to 22 operations including the two `/chat*` routes), plus 2 public (`/health`, `/version`), plus the docs/spec routes, plus 2 legacy aliases when enabled. (`/chat` and `/chat/info` are always *routed* but the `chat` handler errors with a 400 if no provider config is resolvable.)

**`Server::run()`** (lines 190–198) binds a `tokio::net::TcpListener` to `config.addr`, reads back `local_addr()`, and emits a **single grep-able line to stderr**: `seele http listening on http://<addr>` via `eprintln!` (line 194). The doc comment (lines 184–189) explains this exists so `--port 0` callers (tests, scripts) can discover the OS-chosen port without a listener-then-drop race. It then logs the same via `tracing::info!` (line 195) and calls `axum::serve(listener, app).await` (line 196). There is **no explicit graceful-shutdown wiring** (no `with_graceful_shutdown`) — the future runs until the process is killed; `run` returns `anyhow::Result<()>`.

The two public handlers are trivial: `health` returns `Json({"status":"ok"})` (lines 201–203); `version` returns `Json({"name":"seele","version": env!("CARGO_PKG_VERSION")})` (lines 205–210).

### 9.5 `auth.rs` — bearer token scheme

Two functions implement the opt-in scheme. `check_bearer(headers, expected_token)` (lines 17–32) looks up the `authorization` header (it calls `.get("authorization").or_else(|| .get("Authorization"))`; note that `http::HeaderMap` keys are already case-insensitive, so the second lookup is effectively redundant), reads it as a str, strips the **mandatory** `"Bearer "` prefix, trims surrounding whitespace from the remaining token via `str::trim`, and compares it for exact equality to the expected token. Any missing header, non-`to_str`-able header value, missing `Bearer ` prefix, or mismatch yields `Err(StatusCode::UNAUTHORIZED)` (401). The token comparison is a plain `==` on `&str` — **not constant-time**, a potential timing side-channel (no `subtle`/constant-time crate is used).

```rust
// auth.rs:23
let provided = header
    .strip_prefix("Bearer ")
    .map(str::trim)
    .ok_or(StatusCode::UNAUTHORIZED)?;
```

`require_bearer(expected_token: String, req, next) -> Result<Response, StatusCode>` (lines 37–44) is the middleware: it calls `check_bearer` on `req.headers()`, and on success runs `next.run(req).await`. On failure axum turns the bare `StatusCode::UNAUTHORIZED` into a 401 response with an empty body. Note: this 401 path does **not** go through the `ErrorBody` envelope — auth-rejected requests get a plain 401 with no JSON body, unlike domain errors. The module doc (lines 1–6) restates the carve-out, but only mentions `/health` and `/version` as public (the full four-route carve-out including `/docs/*` and `/openapi.json` is documented on `ServerConfig.auth_bearer` in `server.rs`).

Tests confirm the contract (`handlers_d_auth_openapi.rs`): auth disabled by default (200 without token), missing token → 401, wrong token → 401, valid token → 200, and `/health`, `/version`, `/openapi.json` remain public under auth. Legacy `/save` also respects auth (it lives in the protected block) — see `legacy_paths_respect_auth_bearer`. The CLI surfaces this as `--auth-bearer <token>` (`crates/seele-cli/src/commands/serve.rs`, line 21).

### 9.6 `dto.rs` — request/response shapes

DTOs are deliberately simpler than core types (`dto.rs`, lines 1–5): IDs are `String` (parsed/validated at the boundary), metadata is `serde_json::Value`, and timestamps are Unix-epoch **milliseconds as `i64`** (via `chrono`'s `timestamp_millis()`), avoiding chrono types on the wire. Every DTO derives `utoipa::ToSchema` for OpenAPI registration.

Key request DTOs:

| DTO | Notable fields / defaults |
|---|---|
| `SaveRequest` | `title`, `content` (required); `type` defaults to `"memory"` via `default_type()`; `project`, `scope` (`"project"`/`"personal"`, default project), `topic_key`, `session_id`, `tool_name`, `metadata` all optional. The doc comment enumerates the 12 canonical types (decision, architecture, bugfix, pattern, config, discovery, learning, memory, skill, advisor_output, review, verdict) or any custom string. |
| `SearchRequest` | `#[derive(Default)]`; `query` defaults to `""`; optional `project`/`scope`/`type`/`limit`/`max_vec_distance`; bools `include_purist`, `include_annotations`; `score_boost_multiplier: f64` (defaults to `0.0` — see gotcha below). |
| `ListRequest` | All-optional filters; `include_deleted: bool`. Used via `Query<ListRequest>`. |
| `SessionStartRequest` | `project` required, `directory` optional. |
| `SessionEndRequest` | `summary` optional. |
| `SessionListQuery` | `project`, `status`, `limit` — used via `Query`. |
| `LinkCreateRequest` | `from_id`, `to_id`, `link_type` required; `metadata` optional. |
| `RelationCreateRequest` | `sync_id`, `source_id`, `target_id`, `relation` required; plus optional `reason`/`evidence`/`confidence`/`marked_by_actor`/`marked_by_kind`/`marked_by_model`/`session_id`. |
| `RelationListQuery` | `source_id`/`target_id`/`relation`/`status`/`limit` — `Query`. |
| `JudgeRequest` | `status` required; `reason`/`evidence`/`confidence` optional. |

Key response DTOs: `SaveResponse` (`id`, `outcome: &'static str`, optional `revision_count`/`duplicate_count`), `SearchResponse` (`hits: Vec<SearchHitDto>`, `count: usize`), `SearchHitDto` (id/title/content/project/scope/type/score, optional `fts_rank`/`vec_rank`, `created_at` ms, `metadata`, `annotations: Vec<AnnotationDto>`), `AnnotationDto` (`kind` mapped from `AnnotationKind` to one of `supersedes`/`superseded_by`/`conflicts_with`/`contested_by`, lines 118–123; plus `other_id`, optional `other_title`/`reason`), `ObservationDto`, `SessionDto`, `LinkDto`, `RelationDto`, `StatsResponse`/`ObservationStats`/`SessionStats`/`CountBucket`, and `EmbedderInfo`. Each has an idiomatic `From<CoreType>` impl converting timestamps and `as_str()` enum labels (note `SearchHitDto` converts from `&SearchHit` by reference, lines 111–144, whereas `ObservationDto`/`SessionDto`/`LinkDto`/`RelationDto` consume an owned core value).

The parsing helpers centralize boundary validation:
- `parse_id(s, label) -> Result<SeeleId, ApiError>` — `s.parse::<SeeleId>()`, mapping failure to `BadRequest("invalid {label}: {e}")` (lines 475–478).
- `parse_scope(Option<&str>)` — `None`→default (`Scope::Project`), `"project"`/`"personal"` accepted, anything else 400 (lines 481–490).
- `parse_type(&str) -> ObservationType` — **infallible**, unknown strings become `ObservationType::Other(...)` via `from_str_relaxed` (lines 494–496).
- `parse_metadata(Value) -> Metadata` — null→empty `Metadata`, otherwise `Metadata::from_value` (lines 499–505).
- `parse_session_status` / `parse_relation_kind` / `parse_judgment_status` (lines 267–275, 410–416, 418–426) — strict parses via the core `from_str_strict` constructors, with descriptive 400 messages enumerating valid values.

The asymmetry is deliberate: observation *type* is relaxed (custom types allowed), but *scope*, *session status*, *relation kind*, and *judgment status* are strict closed sets.

### 9.7 `handlers.rs` — endpoint handlers

Each handler is the thinnest possible wrapper: extract from path/query/body, call a `SeeleService` method, serialize (`handlers.rs`, lines 1–4). They return `Result<Json<T>>` for bodies, `Result<StatusCode>` for no-content actions, or bare `Json<T>` for infallible ones (`get_embedder_info` at lines 194–196, `chat_info` at lines 237–252). Status-code conventions:

- **Mutations that return a body** (`save_memory`, `create_link`, `create_relation`, `start_session`) → **200 OK** with the DTO (not 201 — a deliberate simplification).
- **Lifecycle/no-content actions** (`soft_delete_memory`, `restore_memory`, `end_session`, `abort_session`, `delete_link`, `judge_relation`) → **204 No Content** (`StatusCode::NO_CONTENT`).
- **Reads** → 200 with body, or **404** via `ApiError::NotFound` when `get_memory`/`get_session` find nothing (lines 47, 100).
- **Validation failures** → 400 via the parse helpers / service guards.

`search_memories` (lines 31–38) is the only handler that runs the `enforce_search_query_or_filter` gate before delegating. `list_pending_conflicts` uses a small local `ConflictsQuery { limit: Option<u32> }` query type (struct lines 175–179, handler lines 181–186).

#### Chat endpoint (wiring `seele-chat`)

`/chat` and `/chat/info` integrate the standalone `seele-chat` crate (imports at `handlers.rs` lines 200–205). `chat_info` (lines 237–252) reports `{enabled, provider, model}` from the optional `ChatProviderConfig`, returning `enabled: false` with `provider`/`model` `None` when chat is disabled. The `chat` handler (lines 254–376) is substantial:

1. Pulls both `State<Arc<SeeleService>>` and `State<Option<Arc<ChatProviderConfig>>>`.
2. Resolves provider config in priority order **per-request fields > CLI config** for `provider`, `api_key`, `model`, `endpoint`. Missing `provider` or `api_key` after both sources → `BadRequest` with a panel-oriented message (lines 261–289).
3. Defines a single tool, `seele_search`, with a JSON-schema (`query` required, `limit` default 5/max 20, optional `project`) (lines 290–302).
4. Picks the provider: `"anthropic"` (case-insensitive via `eq_ignore_ascii_case`) → `AnthropicProvider`; anything else → `OpenAICompatibleProvider` using `endpoint_override` or `default_endpoint_for(provider)` (lines 304–315).
5. Builds an async `ToolHandler` closure capturing a cloned `Arc<SeeleService>`. The closure deserializes the model's JSON args, clamps `limit` to ≤20 (`args.limit.unwrap_or(5).min(20)`), constructs a `SearchRequest` (no scope/type filters, `include_purist=false`, `score_boost_multiplier=1.0`), calls `svc.search_observations`, and returns a compact JSON summary truncating each hit's content to 280 chars (lines 317–360).
6. Builds `ChatConfig::default()` (the baked SEELE system prompt) and overrides `config.system_prompt` if `req.system_prompt` is present (lines 362–365).
7. Calls `seele_chat::run_chat(provider.as_ref(), &tool_handler, req.messages, &config).await`, mapping failure to `ApiError::Internal("chat failed: ...")`, and returns `ChatResponse { messages, provider, model }` (lines 367–375).

`default_model_for` / `default_endpoint_for` (lines 378–401) hardcode per-provider defaults for `minimax`, `openai`, `openrouter`, `together`, `groq`, `deepseek`, `anthropic`, falling back to OpenAI's `gpt-4o-mini` / `https://api.openai.com/v1/chat/completions` for any other label. Note `default_model_for` is duplicated verbatim in the CLI's `serve.rs` (lines 97–108 there) — a small DRY gotcha. The `ChatRequest` re-uses `seele_chat::Message` directly as its `messages` field (`handlers.rs` line 209), so the wire format mirrors the chat crate's message model. The `system_prompt` override is one knob exposed beyond per-request provider/key/model/endpoint. Importantly, `/chat` and `/chat/info` are **not** in the OpenAPI spec (see §9.9) — they are routed but undocumented, which currently breaks the consistency test (see §9.9 / §9.11).

### 9.8 `error.rs` — domain-to-HTTP mapping

`ApiError` (lines 14–30) is a `thiserror` enum with five variants: `BadRequest(String)`, `Unauthorized`, `NotFound(String)`, `Conflict(String)`, `Internal(String)`. `ErrorBody` (lines 32–36) is the stable JSON envelope `{ code: &'static str, message: String }` and itself derives `ToSchema` (so it is registered in the OpenAPI components). The `IntoResponse` impl (lines 38–56) maps each variant to a `(StatusCode, code)` pair and serializes the envelope (the `message` is `self.to_string()`, i.e. the `thiserror` `#[error("...")]` text such as `"bad request: ..."`):

| Variant | Status | `code` |
|---|---|---|
| `BadRequest` | 400 | `BAD_REQUEST` |
| `Unauthorized` | 401 | `UNAUTHORIZED` |
| `NotFound` | 404 | `NOT_FOUND` |
| `Conflict` | 409 | `CONFLICT` |
| `Internal` | 500 | `INTERNAL` |

`From` conversions translate each inner error tree (lines 58–96): `StorageError::{NotFound→404, InvalidInput→400, Conflict→409, _→500}`, `SearchError::{InvalidInput→400, _→500}`, `EmbedderError→500` (always `Internal`, prefixed `embedder: `), and `SeeleError::{InvalidInput→400, NotFound→404, Conflict→409, _→500}`. The `?` operator in service/handlers thus auto-maps any inner failure to the right HTTP status. Gotcha: the `ApiError::Unauthorized` variant exists and maps to 401 with an `ErrorBody`, **but the auth middleware never produces it** — `require_bearer` returns a bare `StatusCode::UNAUTHORIZED`, so auth 401s have an empty body while a (hypothetical) handler-returned `Unauthorized` would carry the envelope. (`Unauthorized` is, however, consumed by `seele-mcp`'s `From<ApiError> for ToolError` impl, which maps it to an internal error — `crates/seele-mcp/src/tools.rs`, line 48.) `Result<T>` is the crate alias `std::result::Result<T, ApiError>` (line 98).

### 9.9 `openapi.rs` — OpenAPI + Swagger UI

The strategy (lines 1–7) is hybrid: use the `#[derive(OpenApi)]` macro only to **register component schemas** (the `SchemaRegistry` ZST lists all DTOs — the `schemas(...)` list spans lines 28–50, struct declared at line 51), but author the **paths by hand** in `build_paths()` rather than decorating each handler with `#[utoipa::path]`, to keep the router clean and avoid duplicating signature info.

`build_openapi()` (lines 55–70) calls `SchemaRegistry::openapi()`, sets `info.title = "SEELE HTTP API"`, `info.version = CARGO_PKG_VERSION`, a description noting the `{code, message}` error envelope, a single server `http://127.0.0.1:7777` ("Default loopback"), and replaces `doc.paths` with `build_paths()`.

`build_paths()` (lines 72–346) enumerates every documented path/method with summary, parameters, request body, and response schema using a small DSL of helpers: `json_body::<T>` (required `application/json` request body referencing a schema by **explicit `&str` name argument**), `json_response::<T>(status, desc)` (single object response), `json_array::<T>` (array response), `no_content` (204), `json_ok` (loose 200 for health/version), `path_param`, `query_param`. The schema name in `json_response`/`json_array` is derived at runtime from `std::any::type_name::<T>()` via `rsplit("::").next()` (lines 368–371, 389–392) — a slightly fragile reflection trick. Note that `json_body` does NOT use this reflection; it takes the schema name as a literal `&str` argument (e.g. `json_body::<SaveRequest>("SaveRequest")`), and `schema_ref::<T>` actually discards its `type_name` call (line 351) and uses the passed-in `name`. So a renamed DTO would need its `json_body("Name")` literal updated to avoid a dangling `$ref`, while `json_response`/`json_array` derive the name from the type and would self-correct. The spec documents `/health`, `/version`, and all the canonical data paths, but deliberately **omits** the legacy ENGRAM aliases (ADR-13) and **omits** `/chat`/`/chat/info`. The spec ends up with 20 path keys (`/health`, `/version`, plus 18 canonical data path keys).

`routes()` (lines 454–459) mounts `SwaggerUi::new("/docs").url("/openapi.json", build_openapi())` into an `axum::Router`. The comment (lines 451–453) explains it is merged outside the auth block so no middleware fires for docs — the spec describes schemas only, no data leak. Tests verify Swagger HTML at `/docs/` and JSON at `/openapi.json` (`handlers_d_auth_openapi.rs`).

The **anti-drift** test `openapi_consistency.rs` is a notable safeguard: because axum 0.8 exposes no route introspection, the test regex-parses `src/server.rs` for `.route("…")` literals **up to the `if self.config.legacy_engram_paths` cutoff** (so the two legacy aliases are excluded), adds `/health` and `/version`, and asserts the set is exactly equal to the keys of `/openapi.json`. Adding a route without a matching `paths.path(...)` (or vice versa) fails the test with a `MISSING`/`EXTRA` diff and a remediation hint (lines 91–100). It also guards against vacuous passes by asserting ≥15 paths (lines 106–110).

**This test is currently FAILING.** The `/chat` and `/chat/info` routes are wired at `server.rs` lines 151–152 — *before* the `legacy_engram_paths` cutoff at line 154 — so the regex captures them into the router-side set, but they are absent from `build_paths()`. Running `cargo test -p seele-http --test openapi_consistency` panics with:

```
router has routes not declared in OpenAPI spec: ["/chat/info", "/chat"].
Add a `paths.path("<path>", ...)` entry in crates/seele-http/src/openapi.rs.
```

In other words, the chat routes' deliberate absence from the spec and the consistency test's exact-match contract are in genuine conflict, and the test does not pass as the code stands today. (This is a concrete defect for the improvement pass, not a hypothetical "tension": either add the two `/chat*` paths to `build_paths()`, or move the chat routes below the legacy cutoff / teach the regex to skip them.)

### 9.10 How it connects to the rest of SEELE

- **CLI (`seele serve`)** builds a `SeeleService` via `crate::app::build_service` and constructs `ServerConfig` from flags `--port`, `--bind`, `--legacy-engram-paths`, `--auth-bearer`, repeatable `--cors-allow`, and the `--chat-*` family, then calls `Server::run()` (`crates/seele-cli/src/commands/serve.rs`). The CLI resolves `--chat-key $ENVVAR` to an env lookup via `resolve_key` (the value is read from the named env var only when it starts with `$`, else used literally — lines 88–95) and enforces that `--chat-provider` and `--chat-key` are set together (else `anyhow::bail!`, lines 53–72).
- **MCP (`seele-mcp`)** is the biggest consumer of this crate's *library* surface: it holds a `SeeleService`, reuses the request DTOs (`SaveRequest`, `SearchRequest`, `ListRequest`, `LinkCreateRequest`, etc.), reuses `enforce_search_query_or_filter` (`crates/seele-mcp/src/tool_impls/memories.rs`, lines 5–6, 28), and converts `seele_http::ApiError` into its own `ToolError` (`crates/seele-mcp/src/tools.rs`, lines 41–52). This is the concrete realization of the "shared service layer" promise — MCP tools are thin shims over the same methods the HTTP handlers call. (`seele-mcp`'s tool handlers are *synchronous* `fn(&SeeleService, Value) -> Result<Value, ToolError>` — `tools.rs` line 60 — unlike the async axum handlers.)
- **TUI (`seele-tui`)** also uses the library surface directly **in-process**: it imports `SeeleService`, the DTOs (`ObservationDto`, `SearchHitDto`, `StatsResponse`), and `enforce_search_query_or_filter` (`crates/seele-tui/src/app.rs`, lines 7–9; `lib.rs` line 21 and its module doc explicitly note "no HTTP round trip, no MCP"). The TUI does NOT talk to a running HTTP server.
- **Data crossing the boundary**: JSON DTOs in/out for HTTP; the same Rust DTO structs in-process for MCP and TUI. Inner domain types (`Observation`, `Session`, `Link`, `MemoryRelation`, `SearchHit`) never cross the wire directly — they are always projected through the `From` impls into DTOs.

### 9.11 Edge cases, gotchas, and limitations

- **`score_boost_multiplier` default is `0.0`.** `SearchRequest` derives `Default` and uses `#[serde(default)]` on `score_boost_multiplier: f64` (`dto.rs`, lines 71–72), so an omitted field yields `0.0`, not `1.0`. Whether the search engine treats `0.0` as "no boost" or as "zero out boosted scores" depends on `seele-search` semantics; callers who want a neutral multiplier must send `1.0` explicitly (the chat tool handler does, `handlers.rs` line 339). This is a likely foot-gun.
- **Embedding is best-effort and post-save.** A failing embedder (or a failing embedding write) yields a persisted row with no vector and only a `tracing::warn!` (`service.rs`, lines 96–107); the row is invisible to the vec branch of search until reindexed. No reindex endpoint exists in this crate.
- **No 201/Location on create.** All creates return 200 with the resource body, not 201 Created.
- **Auth is not constant-time** and emits empty-body 401s (no `ErrorBody`), diverging from the documented envelope contract.
- **CORS is all-or-nothing.** Any non-empty `cors_origins` opens `Access-Control-Allow-Origin: *` — the per-origin allowlist is explicitly on the backlog (`server.rs` comment lines 108–116; CLI flag doc lines 22–25).
- **No graceful shutdown** is wired into `Server::run`.
- **Schema-name handling** in `openapi.rs` is split: `json_body` takes an explicit `&str` literal (renaming a DTO without updating the literal silently produces a dangling `$ref`), while `json_response`/`json_array` derive the name via `type_name::<T>()` string-splitting (self-correcting on rename, but fragile if a type's path-tail collides).
- **`/chat` and `/chat/info` are undocumented in OpenAPI**, and because they are routed *before* the legacy cutoff the `openapi_consistency` test currently **fails** with `router has routes not declared in OpenAPI spec: ["/chat/info", "/chat"]` (verified via `cargo test -p seele-http --test openapi_consistency`). This is a live defect, not a latent one.
- **`merge_observation_metadata` and `list_projects` have no REST routes.** Both service methods exist but are exposed only through the MCP tool surface (and `list_projects` via the CLI); there is no `PATCH /memories/{id}/metadata` or `GET /projects` in the router.
- **No rate limiting, request-size cap, or body-size limit** is configured beyond axum/tower defaults.
