# Sprint-03 BE — Interfaces (HTTP + MCP)

**Fecha**: 2026-05-10
**Estado**: Cerrado
**Tag**: `sprint-03-interfaces`

## Resumen

Sprint que materializa los dos transports externos de SEELE — REST HTTP (axum 0.8) y MCP stdio (JSON-RPC 2.0) — sobre un service layer común (`SeeleService`). Más el binary mínimo `seele [mcp|serve|--version]` y la compat shim para ENGRAM (ADR-13).

El service layer se diseñó para ser DRY entre transports: cada handler HTTP y cada tool MCP es un wrapper thin que parsea input → llama al service → serializa output. El anti-empty-query gate (mitigación Cloven 2026-05-10 del list-all-DB exfiltration) vive una sola vez en `service::enforce_search_query_or_filter` y se aplica en ambos transports.

## Cambios entregados

### Bloque A — HTTP skeleton + service layer (commit `d9c84c7`)

- Crate `seele-http` con deps reales (axum, tower, tower-http, utoipa, chrono, anyhow + reqwest dev-dep).
- Módulos: `dto.rs`, `error.rs`, `handlers.rs`, `server.rs`, `service.rs`.
- `SeeleService` (Clone) wraps `ObservationStore + SessionStore + LinkStore + RelationStore + PromptStore + ChunkStore + SearchEngine + Embedder + Pool`. Bridge `ArcEmbedder` para compartir `Arc<dyn Embedder>` con `SearchEngine`.
- `Server::router()` con TraceLayer + CorsLayer + CompressionLayer.
- Endpoints públicos: `GET /health`, `GET /version`.
- 6 stores marcados `#[derive(Clone)]` (requerido por `SeeleService: Clone`).
- 3 tests E2E (`tests/skeleton.rs`).

### Bloque B — Handlers básicos (commit `76aaa96`)

- 4 endpoints: `POST /memories`, `POST /search`, `GET /memories/{id}`, `GET /memories?...`.
- DTOs: `SaveRequest/Response`, `SearchRequest/Response/SearchHitDto/AnnotationDto`, `ObservationDto`, `ListRequest`.
- Best-effort embedding write post-save (failure logs, no 5xx).
- **Anti-empty-query gate** (`enforce_search_query_or_filter`): empty query + no filters → 400. Cloven sight 2026-05-10.
- 8 tests E2E (`tests/handlers_basicos.rs`).

### Bloque C.1 — Lifecycle (commit `dc448dd`)

- 10 endpoints: soft_delete + restore, sessions (start/list/get/end/abort), links (create/list-for-memory/delete).
- DTOs: `SessionStartRequest/SessionListQuery/SessionEndRequest/SessionDto`, `LinkCreateRequest/LinkDto`.
- `list_links_for_observation` hace merge source+target via BTreeMap dedup.
- `parse_session_status` helper.
- 13 tests E2E (`tests/handlers_c1_lifecycle.rs`).

### Bloque C.2 — Relations + stats + embedder (commit `c22beff`)

- 6 endpoints: `POST/GET /relations`, `PUT /relations/{id}/judge`, `GET /conflicts`, `GET /stats`, `GET /embedder`.
- Storage: `count_active/count_deleted/count_projects/count_by_type/count_by_scope` (observations) + `count_total/count_by_status` (sessions).
- Service: `create_relation` (valida sync_id, source≠target), `list_relations`, `judge_relation`, `list_pending_conflicts`, `stats`, `embedder_info`.
- DTOs: `RelationCreateRequest/RelationListQuery/JudgeRequest/RelationDto`, `StatsResponse/ObservationStats/SessionStats/CountBucket`, `EmbedderInfo`.
- 10 tests E2E (`tests/handlers_c2_relations_stats.rs`).

### Bloque D — Auth bearer + OpenAPI + legacy paths (commit `3f54b88`)

- `src/auth.rs`: `check_bearer` + `require_bearer` middleware. Wired opt-in via `ServerConfig.auth_bearer`. `/health`, `/version`, `/openapi.json`, `/docs/*` stay public.
- `src/openapi.rs`: `SchemaRegistry` (utoipa derive) registra 20 DTOs + ErrorBody. `build_openapi()` autor a-mano de los paths (evita decorar cada handler con `#[utoipa::path]`).
- Swagger UI servido en `/docs/`, spec en `/openapi.json`.
- **ADR-13 `--legacy-engram-paths`**: cuando `ServerConfig.legacy_engram_paths = true`, expone `POST /save` + `GET /show/{id}` como aliases. Sessions/relations/stats NO se aliasean (no existían en ENGRAM).
- Bump `utoipa-swagger-ui` 8 → 9 (8 solo soporta axum 0.7; SEELE usa axum 0.8).
- 14 tests E2E (`tests/handlers_d_auth_openapi.rs`).

### Bloque E — MCP stdio + 19 tools + CLI dispatch (commit `2300631`)

- Crate `seele-mcp` con:
  - `jsonrpc.rs`: tipos Request/Response/ErrorObject + std codes + 1001 TOOL_ERROR.
  - `server.rs`: `McpServer` + `McpServerConfig.tool_prefix`. `run_io` es transport-generic (`AsyncRead + AsyncWrite`) para tests con `tokio::io::duplex`. `run_stdio` para stdin/stdout real.
  - `tools.rs`: registro de 19 tools + `build_index(prefix)`. **ADR-13 alias**: `mnema_recall` para `seele_search` (ENGRAM usaba "recall").
  - `tool_impls/`: memories (8), sessions (4 — start/end/summary/capture_passive), relations (2 — judge/compare), meta (5 — stats/projects/doctor/version/suggest_topic_key).
  - `capture_passive` parsea `## Key Learnings` bullets (`-` y `*`, con continuaciones).
  - `suggest_topic_key` usa heurísticas ENGRAM-inherited (architecture/bug/decision/pattern/config/discovery/learning).
- Service additions: `merge_observation_metadata`, `list_projects`.
- `seele-cli/src/main.rs`: argv parser hand-rolled (clap completo en Sprint-04). Soporta `--version`, `--help`, `mcp [--tool-prefix --db]`, `serve [--port --bind --legacy-engram-paths --auth-bearer --db]`.
- Default DB: `~/.seele/seele.db`. Embedder: FakeEmbedder (real ONNX behind flag en Sprint-04).
- 18 tests (6 unit + 12 stdio_e2e).

### Bloque F — E2E binary + state-sync (este commit)

- `crates/seele-cli/tests/binary_e2e.rs`: 6 tests que spawn el binary real:
  - `seele --version`, `--help`, unknown-command exit.
  - `seele mcp` recibe `tools/list` por stdin, retorna 19 tools.
  - `seele mcp --tool-prefix mnema` retorna `mnema_recall` y NO retorna ningún `seele_*`.
  - `seele serve --port <p> --db <tmp>` arranca, `/health` retorna 200 en ≤5s.
- Devlog (este archivo), executed/, CHANGELOG, INDEX, CLAUDE.md, memoria persistente, cost-ledger.

## Decisiones técnicas

1. **Service layer compartido** entre HTTP y MCP. Cada operación de negocio vive una sola vez en `SeeleService` (~14 métodos). Handlers HTTP + tool impls MCP son ambos shims thin sobre eso. Costo: ~120 LOC en service.rs + ~80 LOC en handlers.rs + ~150 LOC en tool_impls/ vs ~350+ duplicados si cada transport tuviera su propia lógica.

2. **Anti-empty-query gate** una sola vez en `enforce_search_query_or_filter` aplicado en HTTP `/search` y MCP `seele_search`. Mitiga list-all-DB exfiltration (Cloven 2026-05-10).

3. **OpenAPI paths a-mano** en lugar de `#[utoipa::path]` por handler. Trade-off: la spec vive en `openapi.rs` separado de los handlers (cost: drift posible si se agrega un endpoint sin actualizar la spec). Beneficio: handlers libres de macro noise.

4. **MCP transport-generic**: `McpServer::run_io<R, W: AsyncRead+AsyncWrite>` permite testing con `tokio::io::duplex` sin tocar stdin/stdout reales. `run_stdio` es un thin wrapper sobre la real.

5. **Tool-prefix alias** vive en `build_index(prefix)`, no en los handlers. Renames son boundary-only (los handlers internos siempre referencian `seele_*`). Esto deja el ENGRAM-compat (ADR-13) sin ramas dentro de los tool impls.

6. **`utoipa-swagger-ui` 8 → 9 bump**: 8 solo soporta axum 0.7; SEELE eligió axum 0.8 en Sprint-03 Bloque A. 9.0.2 ya soporta axum 0.8 nativo.

7. **Auth opt-in, default OFF**: bearer middleware solo se wirea cuando `auth_bearer = Some(...)`. Default para uso local (consumer en localhost) sin fricción.

## Incidentes durante la ejecución

- **utoipa-swagger-ui 8 incompat axum 0.8** → bump a 9.0.2 resolvió.
- **6 stores faltaba `Clone` derivation** → agregado en Bloque A para que `SeeleService` pudiera derivar Clone.
- **Cargo fmt reformateó archivos varias veces** post-Write (cosmético, sin regresiones).
- **Linker LNK1104 sporadic en Windows** (file locked) → resuelto re-corriendo. No bloquea CI Linux.
- **Cloven sight 2026-05-10 [ALTO/RIESGO timeline]**: MNEMA estaba conectada a ENGRAM, migrar naive rompía queries. Cerrado vía SEELE ADR-13 (commit `424b096`) + MNEMA ADR-011 (commit MNEMA `ba5a2c0`). Sprint-03 D y E implementaron `--legacy-engram-paths` y `--tool-prefix mnema` respectivamente.

## Cómo reproducir

```powershell
# Tests workspace
cargo test --workspace                # 202 verde + 4 ignored
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bash scripts/check-no-stele-residual.sh

# HTTP server local
cargo run --release -p seele-cli -- serve --port 7777
curl http://127.0.0.1:7777/health     # → {"status":"ok"}
curl http://127.0.0.1:7777/openapi.json | head -20
# Swagger UI en http://127.0.0.1:7777/docs/

# MCP stdio
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | cargo run -q -p seele-cli -- mcp

# MCP con compat MNEMA (ADR-13)
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | \
  cargo run -q -p seele-cli -- mcp --tool-prefix mnema

# HTTP con legacy paths ENGRAM (ADR-13)
cargo run --release -p seele-cli -- serve --legacy-engram-paths
curl -X POST http://127.0.0.1:7777/save -H "content-type: application/json" \
  -d '{"title":"t","content":"c"}'    # alias de POST /memories
```

## Pendiente para próximos sprints

- **Sprint-04 (Ops & UX)**:
  - CLI completa con clap (subcommands save/search/show/list/sync/import/setup).
  - `seele import --from-engram <path>` per ADR-13.
  - TUI con ratatui.
  - seele-sync (chunks comprimidos git-friendly).
  - seele-setup (wizard 8 agentes).
  - seele-project (5-case detection).
- **Sprint-05 (Polish + CI/CD + Release)**:
  - HTTP transport MCP (v0.2).
  - Rate limiting (v0.2).
  - Property tests workspace-wide.
  - Release pipeline (crates.io + GitHub Releases binaries).
  - Docs `ENGRAM-MIGRATION.md` linkeada desde README.

## Uso y costo

| Sesión | Modelo | Tokens (estimated) | Notas |
|---|---|---|---|
| Sprint-03 ejecución (A→F) | claude-opus-4-7 | ~480k input + ~95k output | 6 bloques + tests + ADR-13 + ADR-MNEMA-011 |
| Sprint-03 state-sync | claude-opus-4-7 | ~25k input + ~12k output | Este devlog + cierre |

Cost-ledger: `docs/aegis/devlogs/cost-ledger.jsonl` (entries con `"estimated": true`).

## Referencias

- Commits: `d9c84c7` (A), `76aaa96` (B), `dc448dd` (C.1), `c22beff` (C.2), `3f54b88` (D), `2300631` (E), este (F).
- ADRs: `genesis/plans/arquitectura/12-embedder-hardening-followups.md`, `13-engram-compatibility.md`.
- Cloven reviews: 2026-05-10 [post C.1], 2026-05-10 [timeline MNEMA→SEELE] → cerrado por ADR-13 + ADR-MNEMA-011.
- Plan táctico ejecutado: `genesis/plans/executed/tactica/sprint-03/`.
