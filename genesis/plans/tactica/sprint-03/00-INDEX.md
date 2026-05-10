# Sprint-03 BE — Interfaces (HTTP + MCP)

**Fecha**: 2026-05-10
**Tema**: Exponer SEELE via HTTP REST API (axum) + MCP server stdio (JSON-RPC 2.0). Ambas interfaces consumen el mismo service layer interno (DRY entre transports).
**Salida**: `seele-http` con ~25 endpoints (auth bearer opcional + utoipa OpenAPI). `seele-mcp` con 19 tools `seele_*`. Tests E2E spawn del binary contra ambos transports.

## Por qué subdividimos en 6 bloques (no 3)

Recomendación de Cloven (review post Sprint-02): Sprint-03 es el sprint más grande del v0.1 (~2-2.5K LOC). Subdividir en bloques chicos da gates humanos más finos + previene cuelgues de contexto. Si MCP rompe, HTTP queda commiteado por separado y no hay que revertir todo.

## Bloques

- `01-bloque-A-http-skeleton.md` — `seele-http`: axum App + Router + `AppState` + middleware (trace, cors, compression) + service layer compartido + `/health` + `/version`. Sin handlers reales.
- `02-bloque-B-http-handlers-basicos.md` — POST `/memories` (save), POST `/search`, GET `/memories/{id}`, GET `/memories` (list). Incluye protección **empty-query requires filter** (regla Cloven 2026-05-10).
- `03-bloque-C-http-handlers-avanzados.md` — sessions (5 endpoints), links (3), relations + conflicts (4), stats (1), schema (3). Aprox 16 endpoints.
- `04-bloque-D-http-auth-openapi.md` — auth bearer middleware (opt-in via `SEELE_AUTH_BEARER` env / flag) + utoipa derives + `/docs` Swagger UI + `/openapi.json`.
- `05-bloque-E-mcp-stdio.md` — `seele-mcp`: JSON-RPC 2.0 sobre stdin/stdout + 19 tools `seele_*` que reusan el service layer del HTTP.
- `06-bloque-F-tests-e2e-state-sync.md` — Tests E2E del binary spawn + cierre AEGIS (devlog + executed/ + CHANGELOG + INDEX + CLAUDE.md + memoria + tag).

## Dependencias

```
A (skeleton) ─→ B (basicos) ─→ C (avanzados) ─→ D (auth+openapi)
                                                              │
A ─────────────────────────────────────────────→ E (mcp) ─────┤
                                                              ▼
                                                  F (E2E + state-sync)
```

Estricto: B/C dependen del service layer + AppState que crea A. D opera sobre handlers existentes. E puede iniciarse en paralelo a B/C/D porque solo necesita el service layer de A (pero práctico: ejecutarlos en orden para no fragmentar contexto). F requiere todo lo anterior.

## Service layer compartido (decisión central del Sprint)

HTTP handlers y MCP tools NO duplican lógica. Ambos llaman a un trait+structs en `seele-http::service` (o `seele-core::service` si conviene moverlo después).

```rust
// crates/seele-http/src/service.rs (o seele-core)
pub struct SeeleService {
    pub store: ObservationStore,
    pub sessions: SessionStore,
    pub links: LinkStore,
    pub relations: RelationStore,
    pub prompts: PromptStore,
    pub chunks: ChunkStore,
    pub search: SearchEngine,
    pub embedder: Arc<dyn Embedder>,
}

impl SeeleService {
    pub fn save(&self, input: SaveRequest) -> Result<SaveResponse>;
    pub fn search(&self, input: SearchRequest) -> Result<SearchResponse>;
    pub fn get(&self, id: SeeleId) -> Result<Option<Observation>>;
    pub fn list(&self, query: ListQuery) -> Result<Vec<Observation>>;
    // ...etc para las 19 ops del MCP / 25 endpoints del HTTP
}
```

HTTP handlers: thin wrappers `axum::extract::State<SeeleService>` → `service.save(req.into())` → `Json(resp.into())`.
MCP tools: thin wrappers `parse JSON-RPC params` → `service.save(...)` → `serialize JSON-RPC response`.

DRY garantizada: si Bloque B implementa `service.save()` correctamente, Bloque E lo reusa sin re-implementar.

## Regla operacional: empty-query sin filtros (Cloven 2026-05-10)

Crítica de timeline: el path empty-query del Sprint-02 (`list_by_filters`) sin filters retorna los 10 más recientes globales. Si HTTP expone POST `/search` con `{query: "", filters: {}}` sin auth, un cliente puede hacer `curl -X POST localhost:7777/search -d '{}'` y obtener data sensible.

**Mitigación**: en Bloque B, el handler de `/search` valida:

```rust
if req.query.is_empty()
    && req.filters.project.is_none()
    && req.filters.scope.is_none()
    && req.filters.kind.is_none() {
    return Err(ApiError::bad_request(
        "empty query requires at least one filter (project, scope, or kind)"
    ));
}
```

Mismo gate en el handler MCP `seele_search`.

## Criterios de cierre del sprint

1. ✅ `cargo build --workspace` verde.
2. ✅ `cargo test --workspace` verde con nuevos tests E2E (~150-180 tests total esperado).
3. ✅ `cargo clippy --workspace --all-targets -- -D warnings` verde.
4. ✅ `cargo fmt --check` verde.
5. ✅ Static check STELE residual verde.
6. ✅ Spawn `seele serve --port 0` (random port) + curl ~10 endpoints → respuestas OK.
7. ✅ Spawn `seele mcp` + write `tools/list` JSON-RPC → recibir 19 tools.
8. ✅ Spawn `seele mcp` + write `tools/call` para `seele_save` + `seele_search` → respuestas OK.
9. ✅ Empty-query sin filters retorna 400 (HTTP) o JSON-RPC error -32602 (MCP).
10. ✅ Auth bearer: con `SEELE_AUTH_BEARER=secret` set, request sin Authorization → 401.
11. ✅ `/openapi.json` retorna spec válida OpenAPI 3.1.
12. ✅ Devlog del sprint, plans a executed/, CHANGELOG, INDEX, CLAUDE.md, memoria persistente actualizados.
13. ✅ Tag git `sprint-03-interfaces`.

## Out-of-scope del sprint-03

- HTTP transport para MCP (v0.2 — solo stdio en v0.1).
- WebSockets / SSE streaming en HTTP (v0.3).
- Rate limiting HTTP (v0.2).
- Setup wizard para configurar consumers (Sprint-04).
- TUI (Sprint-04).
- Sync chunks compressed (Sprint-04).

## Tamaño esperado

- LOC Rust nuevo: ~2-2.5K (mitad HTTP, mitad MCP, ~200 LOC service layer compartido).
- Tests nuevos: ~30-50 entre unit, integration y E2E.
- Tiempo: 2-3 sesiones orquestadas (estimado generoso).

## Riesgos identificados

| Riesgo | Mitigación |
|---|---|
| Service layer crece monolítico (god-object) con 25+ métodos | Split en sub-services por dominio: `MemoryService`, `SessionService`, `LinkService`, `RelationService`. Cada uno con ~5 métodos. |
| utoipa derive macros explotan compile time | Si `cargo check` pasa de 20s a 60s, evaluar si conviene mantener utoipa o escribir el spec a mano. v0.1 budget razonable. |
| JSON-RPC framing incorrecto (newline-delimited vs Content-Length) | Spec MCP: newline-delimited JSON. Implementar `BufReader::lines()` desde stdin. Tests E2E lo validan. |
| Auth bearer middleware bypasseable via path traversal o flags raras | Aplicar middleware como `tower::Layer` global, no per-route. Test específico: `curl -H "Authorization: Bearer wrong" → 401`. |
| MCP tool handlers que llaman embedder bloquean stdin loop | Service layer `async` donde aplique. Embedder `.embed()` es sync pero rápido (~5ms). Bloqueo aceptable a v0.1. |
| HTTP test E2E con port-binding race en runners CI | Usar `port=0` (let OS pick) + parsear el port real del servidor. tokio::net::TcpListener::bind(":0"). |
