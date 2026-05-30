## 21. Interface Catalog — MCP Tools × HTTP Endpoints × CLI Subcommands

SEELE exposes one storage/service core (`seele_http::SeeleService`) through three transports. The invariant that makes this catalog coherent: **all three transports funnel into the same `SeeleService` methods** (`crates/seele-http/src/service.rs`). The HTTP handlers (`handlers.rs`) and the MCP tool shims (`tool_impls/`) are deliberately thin — deserialize, call a `SeeleService` method, serialize. The CLI commands call `SeeleService` directly in-process (no HTTP round trip). Therefore the *capability* surface is shared, but the *shape* of each transport (param names, defaults, response envelope, error mapping) differs. This section reconciles those differences exactly.

Two cross-cutting facts hold across every table:

- **MCP reuses HTTP's DTOs.** Across its `tool_impls/` modules `seele-mcp` imports `seele_http::dto::{SaveRequest, SearchRequest, ListRequest, LinkCreateRequest, JudgeRequest, RelationCreateRequest, SessionStartRequest, SessionEndRequest}`, plus `seele_http::SeeleService` (the crate-root re-export of `service::SeeleService`) and `seele_http::service::enforce_search_query_or_filter`. The MCP crate depends on `seele-http` precisely to avoid re-declaring the request/response types — there is no single aggregated import; the DTOs are pulled in per-module (`memories.rs`, `relations.rs`, `sessions.rs`).
- **The anti-empty-query gate is shared.** `enforce_search_query_or_filter` (`service.rs:442`) is called by *both* the HTTP `/search` handler (`handlers.rs:35`) and the MCP `seele_search` shim (`tool_impls/memories.rs:28`). It rejects a blank query when no `project`/`scope`/`type` filter is present (`service.rs:442-453`), mitigating a list-all-DB exfiltration vector Cloven flagged (2026-05-10). The CLI applies the same gate (`commands/search.rs:47`).

### 21.1 Table 1 — The 19 MCP `seele_*` Tools

Registered statically in `all_tools()` (`crates/seele-mcp/src/tools.rs:79`). Names may be rewritten at the boundary by `build_index(prefix)` (`tools.rs:205`), which falls through `translate_name` (`tools.rs:225`). The `mnema_*` alias column reflects `RENAMES` (`tools.rs:223`): only one suffix is remapped — `search` → `recall` — because ENGRAM called search "recall". Every other tool is a literal `seele_<x>` → `mnema_<x>` rename. (A `Some("seele")` prefix is also special-cased to keep canonical names, `tools.rs:210`.) The `tools/list` JSON-RPC method returns descriptors sorted by name (`server.rs:99`); `tools/call` is dispatched by `dispatch_tool_call` (`server.rs:113`) and, on success, the handler's JSON is stringified into a single `{content:[{type:"text",text}],isError:false}` block (`server.rs:147-153`).

| MCP tool (`seele_*`) | `--tool-prefix mnema` alias | Required params | Optional params | Effect / underlying op |
|---|---|---|---|---|
| `seele_save` | `mnema_save` | `title`, `content` | `type`(=memory), `project`, `scope`, `topic_key`, `session_id`, `tool_name`, `metadata` | `SaveRequest` → `svc.save_observation` → `ObservationStore::save` (privacy strip, topic-key upsert, dedup) + best-effort embedding write |
| `seele_search` | `mnema_recall` | — (query OR filter) | `query`, `project`, `scope`, `type`, `limit`, `include_purist`, `include_annotations`, `score_boost_multiplier`, `max_vec_distance` | gate → `svc.search_observations` → `SearchEngine::search` (FTS+vec+RRF) |
| `seele_show` | `mnema_show` | `id` (ULID) | — | `svc.get_observation`; 404→`ToolError::NotFound` |
| `seele_list` | `mnema_list` | — | `project`, `scope`, `type`, `topic_key`, `session_id`, `limit`, `include_deleted` | `ListRequest` → `svc.list_observations` |
| `seele_update_metadata` | `mnema_update_metadata` | `id`, `metadata_patch`(object) | — | `svc.merge_observation_metadata` (flat shallow merge); returns `{ok,id}` |
| `seele_delete` | `mnema_delete` | `id` | — | `svc.soft_delete_observation` (sets `deleted_at`); returns `{ok,id}` |
| `seele_restore` | `mnema_restore` | `id` | — | `svc.restore_observation` (clears `deleted_at`); returns `{ok,id}` |
| `seele_link` | `mnema_link` | `from_id`, `to_id`, `link_type` | `metadata` | `LinkCreateRequest` → `svc.create_link` → `LinkStore::create` |
| `seele_stats` | `mnema_stats` | — (empty object) | — | `svc.stats` → `StatsResponse` |
| `seele_session_start` | `mnema_session_start` | `project` | `directory` | `svc.start_session` → `SessionStore::start` |
| `seele_session_end` | `mnema_session_end` | `id` | `summary` | `svc.end_session(id, {summary})`; returns `{ok,id}` |
| `seele_session_summary` | `mnema_session_summary` | `session_id`, `title`, `summary` | `project` | composes a `SaveRequest` (`type=memory`, `topic_key="session/<id>"`, `tool_name="seele_session_summary"`) → `svc.save_observation` (so repeated summaries upsert in place) |
| `seele_capture_passive` | `mnema_capture_passive` | `transcript` | `project`, `session_id` | parses `## Key Learnings:` bullets, saves each as a `type=learning` observation; returns `{saved:[ids],count}` |
| `seele_judge` | `mnema_judge` | `relation_id`, `status` | `reason`, `evidence`, `confidence` | `svc.judge_relation` → `RelationStore::judge`; returns `{ok,id}` |
| `seele_compare` | `mnema_compare` | `sync_id`, `source_id`, `target_id` | `reason` | `svc.create_relation` with hard-coded `relation="conflicts_with"`, `marked_by_actor="seele_compare"`, `marked_by_kind="tool"` |
| `seele_suggest_topic_key` | `mnema_suggest_topic_key` | `title` | `content` | pure local heuristic (no DB): keyword-match against 7 families (architecture, bug, decision, pattern, config, discovery, learning); returns `{family,score,suggestion:"<family>/auto"}` for the highest-scoring family, or `{family:null,suggestion:null}` (no `score` key) when nothing matches |
| `seele_projects` | `mnema_projects` | — (empty object) | — | `svc.list_projects`; returns `{projects:[...]}` |
| `seele_doctor` | `mnema_doctor` | — (empty object) | — | `svc.embedder_info` + `svc.stats`; returns `{status,embedder{model_id,dim,expected_sha256},observations_active,sessions_total,schema_version}` |
| `seele_version` | `mnema_version` | — (empty object) | — | static `{name:"seele",version:CARGO_PKG_VERSION}` (no `svc` touch) |

Gotchas worth flagging for the improvement pass:

- **`seele_doctor` mislabels the version.** It returns `"schema_version": env!("CARGO_PKG_VERSION")` (`tool_impls/meta.rs:30`) — that is the *package* version, not a DB schema version. The key name is misleading. Note also `"status": "ok"` is hard-coded (`meta.rs:22`), not derived from a real liveness check — the tool's own description string ("returns DB pool status") overstates what it actually computes.
- **`seele_compare` cannot set confidence/evidence.** It hard-codes `relation: "conflicts_with"`, `evidence: None`, `confidence: None`, `marked_by_actor: Some("seele_compare")`, `marked_by_kind: Some("tool")`, `marked_by_model: None`, and `session_id: None` (`relations.rs:66-78`), accepting only `sync_id`/`source_id`/`target_id` (required) and an optional `reason`. So the richer relation fields (evidence, confidence, session_id, and any relation kind other than `conflicts_with`) are reachable only via HTTP `POST /relations`.
- **No MCP coverage for sessions list/get, relations list, conflicts, link delete/list, or chat.** Those capabilities exist only on HTTP (see §21.4). MCP exposes session *lifecycle* (start/end/summary) but not session *query*.

### 21.2 Table 2 — HTTP Endpoints

Routes are wired in `Server::router` (`crates/seele-http/src/server.rs:99`). `/health` and `/version` are registered on the outer router, and `/openapi.json` + `/docs/*` (Swagger UI) are merged in from `openapi::routes()` — all four sit outside the `protected` sub-router, so they are always public; every other route sits in the `protected` sub-router. Auth is opt-in: when `ServerConfig.auth_bearer = Some(token)`, the `require_bearer` middleware (`auth.rs:37`) requires `Authorization: Bearer <token>` on all protected routes. The check (`check_bearer`, `auth.rs:17`) strips the mandatory `"Bearer "` prefix, then `.trim()`s the remainder before an exact byte comparison against the configured token; a missing header, missing prefix, or mismatch all map to `401`. Errors elsewhere flow through `ApiError::into_response` (`error.rs:38`): `BadRequest→400`, `Unauthorized→401`, `NotFound→404`, `Conflict→409`, `Internal→500`, all with body `{code,message}` per `ErrorBody` (`error.rs:32`).

The two legacy ENGRAM aliases (`POST /save`, `GET /show/{id}`) are mounted **only** when `legacy_engram_paths` is set (`server.rs:154`); they reuse the same handlers as `POST /memories` and `GET /memories/{id}`. No other route gets a legacy alias.

| Method | Path | Legacy alias | Auth* | Request DTO | Response DTO / status | Maps to |
|---|---|---|---|---|---|---|
| GET | `/health` | — | public | — | `{status:"ok"}` | inline `health()` |
| GET | `/version` | — | public | — | `{name,version}` | inline `version()` |
| GET | `/openapi.json` | — | public | — | OpenAPI 3.1 doc | `openapi::build_openapi` |
| GET | `/docs/*` | — | public | — | Swagger UI HTML | `utoipa_swagger_ui::SwaggerUi` |
| POST | `/memories` | `POST /save` | ✓ | `SaveRequest` (JSON) | `SaveResponse` 200 | `save_memory` → `save_observation` |
| GET | `/memories` | — | ✓ | `ListRequest` (query) | `Vec<ObservationDto>` 200 | `list_memories` → `list_observations` |
| GET | `/memories/{id}` | `GET /show/{id}` | ✓ | path `id` | `ObservationDto` 200 / 404 | `get_memory` → `get_observation` |
| DELETE | `/memories/{id}` | — | ✓ | path `id` | 204 | `soft_delete_memory` → `soft_delete_observation` |
| POST | `/memories/{id}/restore` | — | ✓ | path `id` | 204 | `restore_memory` → `restore_observation` |
| GET | `/memories/{id}/links` | — | ✓ | path `id` | `Vec<LinkDto>` 200 | `list_links_for_memory` (links on either side, deduped) |
| POST | `/search` | — | ✓ | `SearchRequest` (JSON) | `SearchResponse` 200 | `search_memories` (gate first) → `search_observations` |
| POST | `/sessions` | — | ✓ | `SessionStartRequest` | `SessionDto` 200 | `start_session` |
| GET | `/sessions` | — | ✓ | `SessionListQuery` (query) | `Vec<SessionDto>` 200 | `list_sessions` |
| GET | `/sessions/{id}` | — | ✓ | path `id` | `SessionDto` 200 / 404 | `get_session` |
| PUT | `/sessions/{id}/end` | — | ✓ | `SessionEndRequest` | 204 | `end_session` |
| PUT | `/sessions/{id}/abort` | — | ✓ | path `id` | 204 | `abort_session` |
| POST | `/links` | — | ✓ | `LinkCreateRequest` | `LinkDto` 200 | `create_link` |
| DELETE | `/links/{id}` | — | ✓ | path `id` | 204 | `delete_link` |
| POST | `/relations` | — | ✓ | `RelationCreateRequest` | `RelationDto` 200 | `create_relation` |
| GET | `/relations` | — | ✓ | `RelationListQuery` (query) | `Vec<RelationDto>` 200 | `list_relations` |
| PUT | `/relations/{id}/judge` | — | ✓ | `JudgeRequest` | 204 | `judge_relation` |
| GET | `/conflicts` | — | ✓ | `ConflictsQuery` (`limit`) | `Vec<RelationDto>` 200 | `list_pending_conflicts` (conflicts_with + pending) |
| GET | `/stats` | — | ✓ | — | `StatsResponse` 200 | `get_stats` |
| GET | `/embedder` | — | ✓ | — | `EmbedderInfo` 200 | `get_embedder_info` |
| POST | `/chat` | — | ✓ | `ChatRequest` | `ChatResponse` 200 | `chat` (AI provider w/ `seele_search` tool-use) |
| GET | `/chat/info` | — | ✓ | — | `ChatInfoResponse` 200 | `chat_info` |

\* "Auth ✓" means the route is gated **only when** `--auth-bearer` is configured; with no token, every route is open.

Notes on the HTTP surface:

- **OpenAPI/route drift.** `openapi.rs` registers schemas via the `utoipa::OpenApi` derive but hand-authors the paths section in `build_paths()` (`openapi.rs:72`). It does **not** describe `/chat`, `/chat/info`, the `/save` or `/show/{id}` legacy aliases. It also documents only the `200`/`204` success responses — no `404` or error responses are attached to any path, even though the `ErrorBody` schema *is* registered in `components.schemas` (`openapi.rs:49`) and goes otherwise unreferenced. The router (`server.rs`) is the source of truth for what actually serves; the spec is a curated subset. This is a known doc-vs-reality gap worth surfacing to the improvement agent.
- **`SearchRequest.score_boost_multiplier` default trap.** It is `#[serde(default)]` on `f64`, so a JSON request omitting it sends `0.0`. The CLI deliberately passes `0.0` too (`commands/search.rs:44`), while the chat tool-handler passes `1.0` (`handlers.rs:339`). Whether `0.0` zeroes the boost or is treated as "unset" by the search engine is a §7 concern, but the inconsistency originates here.
- **`/chat` keeps secrets local.** `ChatProviderConfig` (`server.rs:62`) and per-request `api_key` never persist; the handler (`chat`, `handlers.rs:254`) resolves provider/key/model/endpoint in priority order (per-request fields > CLI config) and runs `seele_chat::run_chat` with a single `seele_search` tool whose handler closure (`handlers.rs:318`) calls back into `svc.search_observations`. Provider selection is `provider_name.eq_ignore_ascii_case("anthropic")` → `AnthropicProvider`, else `OpenAICompatibleProvider` (`handlers.rs:304`). `default_model_for`/`default_endpoint_for` (`handlers.rs:378`/`391`) map provider labels (`minimax`, `openai`, `openrouter`, `together`, `groq`, `deepseek`, `anthropic`) to defaults; the catch-all (`_`) falls back to the OpenAI model/endpoint, so anything non-`anthropic` is treated as OpenAI-compatible. The tool-call `limit` is clamped to `min(20)` with a default of 5 (`handlers.rs:331`).

### 21.3 Table 3 — The CLI Subcommands

The clap command tree is `Command` in `crates/seele-cli/src/app.rs:44`. Sixteen top-level variants; `Sync` and `Import` each nest a subcommand, and the documented "17 subcommands" count reflects the leaf operations (`sync export`, `sync import`, `import from-engram`). Three **global** flags apply to all (`app.rs:25`): `--db <path>` (default `~/.seele/seele.db` via `default_db_path`, `main.rs:42`), `--fake-embedder` (or env `SEELE_FAKE_EMBEDDER` non-empty → `FakeEmbedder`, else ONNX with transparent fallback on init failure, `app.rs:144`), and `--json` (machine output). DB-touching commands build the service via `build_service` (`app.rs:128`).

| Subcommand | Key flags / args | Behavior |
|---|---|---|
| `save <title> <content>` | `--type`(=memory), `--project`, `--scope`, `--topic-key`, `--metadata <json>` | builds `SaveRequest` with `tool_name="seele-cli"`, `session_id=None`; `save_observation`; prints `saved <id> (<outcome>)` |
| `search [query]` | `--project`, `--scope`, `--type`, `--limit`(=20), `--include-purist`, `--include-annotations` | gate (empty query needs a filter) → `search_observations`; sends `score_boost_multiplier=0.0` |
| `show <id>` | — | `get_observation`; errors if not found; text view shows id/type/scope/project/title/indented content |
| `list` | `--project`, `--scope`, `--type`, `--topic-key`, `--session-id`, `--limit`(=50), `--include-deleted` | `ListRequest` → `list_observations` |
| `delete <id>` | — | `soft_delete_observation`; prints `soft-deleted <id>` |
| `restore <id>` | — | `restore_observation`; prints `restored <id>` |
| `link <from_id> <to_id> <link_type>` | `--metadata <json>` | `create_link`; prints `linked <from> -[<type>]-> <to> (id=<id>)` |
| `stats` | — | `stats`; renders observation/session counters incl. by_type/by_scope/by_status |
| `doctor` | — | `stats` + `embedder_info`; `DoctorReport` with a `fake_embedder_warning` when `model_id` contains "fake" |
| `projects` | — | `list_projects` |
| `sync export <dir>` | `--project` | `seele_sync::export_to_dir(&svc.observations, dir, filter)` → gzip JSON chunk |
| `sync import <path>` | `--target-key <key>` (required) | `seele_sync::import_from_file(&svc.observations,&svc.chunks,target_key,path)`; idempotent per (target_key, chunk_id) |
| `import from-engram <path>` | `--re-embed` (no-op in v0.1), `--dry-run` | builds its own `ObservationStore`+`LinkStore` (does not use `build_service`), runs `EngramImporter::import_from` (preserves ULIDs, maps `linked_to[]`) |
| `setup` | `--agent <name>` \| `--all` \| `--list`, `--dry-run`, `--no-backup`, `--seele-binary <path>` | installs MCP entry into agent configs via `seele_setup::install`; `--all` iterates only implemented agents (`implemented_agent_names`) |
| `mcp` | `--tool-prefix <p>` | `McpServer::new(...).run_stdio()` — JSON-RPC 2.0 stdio loop; `mnema` activates ENGRAM aliases |
| `serve` | `--port`(=7777), `--bind`(=127.0.0.1), `--legacy-engram-paths`, `--auth-bearer`, `--cors-allow <origin>`(repeatable), `--chat-provider`, `--chat-key`, `--chat-model`, `--chat-endpoint` | builds `ServerConfig` + optional `ChatProviderConfig`, runs the axum server; `--chat-key $ENV` resolves from env (`resolve_key`, `serve.rs:88`); `--chat-provider` and `--chat-key` must be set together or both omitted (else hard error) |
| `tui` | `--smoke` (hidden) | interactive path calls `seele_tui::run_tui(svc).await`; `--smoke` instead calls `seele_tui::run_tui_smoke(svc)`, which renders one off-screen frame, prints `tui smoke ok`, and exits 0 |

CLI-specific notes:

- **No CLI command for sessions, relations, judge, conflicts, update-metadata, or chat.** Those are HTTP/MCP-only. The CLI is the *operator* surface (save/search/introspect/sync/migrate/serve), not the full memory-graph API.
- **`save --metadata` and `link --metadata`** parse a raw JSON string; invalid JSON is a hard CLI error (`save.rs:45`, `link.rs:30`).
- **`import --re-embed` is recognized but a documented no-op** in v0.1 (`commands/import.rs:64`, logs a `tracing::warn!`).
- **`mcp`, `serve`, `tui` are long-running processes**, not request/response ops; they host the *other two* transports. `tui` and `mcp` talk to the in-process `SeeleService` with no HTTP hop.

### 21.4 Capability Cross-Map (MCP × HTTP × CLI)

Rows are logical capabilities; a marker means a first-class operation exists on that transport. `✓` = present; `—` = absent; parenthetical notes show divergences.

| Capability | MCP tool | HTTP endpoint | CLI subcommand |
|---|---|---|---|
| Save observation | ✓ `seele_save` | ✓ `POST /memories` (+`/save` legacy) | ✓ `save` |
| Hybrid search | ✓ `seele_search`/`mnema_recall` | ✓ `POST /search` | ✓ `search` |
| Show one by id | ✓ `seele_show` | ✓ `GET /memories/{id}` (+`/show/{id}` legacy) | ✓ `show` |
| List observations | ✓ `seele_list` | ✓ `GET /memories` | ✓ `list` |
| Soft-delete | ✓ `seele_delete` | ✓ `DELETE /memories/{id}` | ✓ `delete` |
| Restore | ✓ `seele_restore` | ✓ `POST /memories/{id}/restore` | ✓ `restore` |
| Update metadata (merge) | ✓ `seele_update_metadata` | — (only via re-save/upsert) | — |
| Create link | ✓ `seele_link` | ✓ `POST /links` | ✓ `link` |
| List links for memory | — | ✓ `GET /memories/{id}/links` | — |
| Delete link | — | ✓ `DELETE /links/{id}` | — |
| Create relation | ✓ `seele_compare` (conflicts_with only) | ✓ `POST /relations` (all 6 kinds) | — |
| List relations | — | ✓ `GET /relations` | — |
| Judge relation | ✓ `seele_judge` | ✓ `PUT /relations/{id}/judge` | — |
| List pending conflicts | — | ✓ `GET /conflicts` | — |
| Session start | ✓ `seele_session_start` | ✓ `POST /sessions` | — |
| Session end | ✓ `seele_session_end` | ✓ `PUT /sessions/{id}/end` | — |
| Session abort | — | ✓ `PUT /sessions/{id}/abort` | — |
| Session get / list | — | ✓ `GET /sessions/{id}` · `GET /sessions` | — |
| Session summary | ✓ `seele_session_summary` | — (compose `POST /memories` manually) | — |
| Passive capture (Key Learnings) | ✓ `seele_capture_passive` | — | — |
| Stats | ✓ `seele_stats` | ✓ `GET /stats` | ✓ `stats` |
| Projects list | ✓ `seele_projects` | — (no dedicated endpoint) | ✓ `projects` |
| Embedder info | (folded into `seele_doctor`) | ✓ `GET /embedder` | (folded into `doctor`) |
| Doctor / health | ✓ `seele_doctor` | ✓ `GET /health` (+`/version`) | ✓ `doctor` |
| Version | ✓ `seele_version` | ✓ `GET /version` | (clap `--version`) |
| Suggest topic key | ✓ `seele_suggest_topic_key` | — | — |
| Chat-with-DB | — | ✓ `POST /chat` · `GET /chat/info` | — (start via `serve --chat-*`) |
| Sync export/import | — | — | ✓ `sync export` / `sync import` |
| ENGRAM import | — | — | ✓ `import from-engram` |
| Agent MCP setup | — | — | ✓ `setup` |
| Run MCP server | (is the transport) | — | ✓ `mcp` |
| Run HTTP server | — | (is the transport) | ✓ `serve` |
| Run TUI | — | — | ✓ `tui` |

The asymmetries are the actionable findings: **operational capabilities (sync, ENGRAM import, setup, server bootstrap, TUI) are CLI-exclusive**; **memory-graph query capabilities (relation listing, conflicts, session query, link listing/deletion) are HTTP-exclusive**; **`update_metadata`, `suggest_topic_key`, `capture_passive`, and `session_summary` are MCP-exclusive niceties** (the last two are conveniences MCP composes from primitives the other transports also expose). The single deepest seam is `POST /relations` vs `seele_compare`: HTTP can create any of the six `RelationKind` values with full evidence/confidence, while MCP can only create a bare `conflicts_with`. No transport is a strict superset of another.
