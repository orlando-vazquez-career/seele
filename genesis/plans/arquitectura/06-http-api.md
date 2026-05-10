# ADR-06 — HTTP REST API con axum

**Estado**: Aceptado · 2026-05-09
**Decisión**: HTTP REST API con axum 0.8+, OpenAPI 3.1 docs auto-generadas con utoipa, auth Bearer opcional, JSON body in/out.

## Contexto

HTTP API es el segundo transport (después de MCP stdio) que MNEMA y otros consumers usan. Casos:

- MNEMA orchestrator (TS+Bun) → HTTP SEELE corriendo en otro proceso/host.
- Frontend MNEMA (Astro+React) → HTTP SEELE para visualizar memorias.
- OpenClaw remoto → HTTP SEELE con auth.
- Tools de monitoring / scripts → HTTP SEELE.

## Endpoints

### Base
```
GET  /health
GET  /stats
GET  /version
```

### Memories
```
POST   /memories                  # save
GET    /memories?filter[...]&limit=&offset= # list
GET    /memories/{id}             # show
PUT    /memories/{id}/metadata    # update metadata
DELETE /memories/{id}             # soft delete
POST   /memories/{id}/restore     # restore
```

### Search
```
POST /search                      # body: { query, filters, top_k }
```

### Links
```
POST   /links                     # create
GET    /memories/{id}/links       # list links of memory
DELETE /links/{id}                # remove
```

### Embedder
```
GET    /embedder                  # current model + dim
POST   /embedder/test             # body: { text }, returns { embedding }
```

### Schema (admin)
```
GET    /schema/version
POST   /schema/virtual-column     # body: { name, jsonpath }
DELETE /schema/virtual-column/{name}
```

## Convenciones REST

- **Paths kebab-case**.
- **Query params snake_case** (no camelCase).
- **JSON body camelCase** (convention para JSON in JS land).
- **Response envelope minimalista**: data directo, sin wrapping `{ data: ..., meta: ... }` salvo cuando hay paginación.
- **Errors uniformes**:

```json
{
  "error": {
    "code": "MEMORY_NOT_FOUND",
    "message": "Memory with id 01HW3X... not found",
    "details": { "id": "01HW3X..." }
  }
}
```

## Auth

### Default v0.1: NO auth

Si SEELE corre en localhost, asumimos trusted environment. Cualquier proceso que pueda hablar al port puede leer/escribir.

Ergonomía > security para v0.1 single-user. Nadie tiene que setear tokens para correr local.

### Opcional Bearer token

Habilitable via:

```bash
seele serve --auth-bearer <token>

# o
SEELE_AUTH_BEARER=secret123 seele serve
```

Cuando habilitado, todas las requests requieren `Authorization: Bearer <token>`. 401 si missing/wrong.

### CORS

Default: same-origin only (sin CORS headers). Habilitable:

```bash
seele serve --cors-origin https://mnema.example.com
```

Permite múltiples origins separados por coma.

## OpenAPI docs

Via crate `utoipa` con derive macros:

```rust
#[derive(Serialize, ToSchema)]
struct Memory { ... }

#[utoipa::path(
    post, path = "/search",
    request_body = SearchRequest,
    responses(
        (status = 200, body = SearchResponse),
        (status = 400, body = ErrorResponse)
    )
)]
async fn search_handler(...) -> ... { ... }
```

Endpoint `GET /docs` sirve Swagger UI auto-generado.

Endpoint `GET /openapi.json` sirve el spec.

## Middleware stack (axum tower)

```
Request
  ├─ tracing middleware (request ID, latency)
  ├─ cors middleware (si habilitado)
  ├─ auth middleware (si habilitado)
  ├─ compression middleware (gzip on responses > 1KB)
  └─ handler
Response
```

## Performance

axum 0.8 sobre tokio: routing time-O(1) por radix tree, handler async sin overhead.

Connection pool DB: r2d2 con `max_size = 16` por default (configurable).

Embedder: singleton compartido entre handlers (Arc<Embedder>).

Targets:
- `GET /health`: <1ms.
- `GET /memories/{id}`: <10ms.
- `POST /search` (10K memorias): <300ms.
- `POST /memories` (con embed): <50ms (embed dominates).

## Configuración

```toml
# ~/.seele/config.toml o passed via env vars / flags
[server]
host = "127.0.0.1"
port = 7777
auth_bearer = ""              # vacío = no auth
cors_origins = []
max_request_size_mb = 10

[db]
path = "~/.seele/seele.db"
pool_size = 16

[embedder]
model = "all-MiniLM-L6-v2"
quantized = true
```

CLI flags override env vars override config.toml override defaults.

## Manejo de errores

`anyhow::Error` en handlers, mapped a HTTP via custom `IntoResponse` impl:

```rust
struct ApiError(StatusCode, ErrorBody);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(self.1)).into_response()
    }
}
```

Mapping:
- `MemoryNotFound` → 404 + `MEMORY_NOT_FOUND`
- `EmbedderError` → 500 + `EMBEDDER_FAILED`
- `InvalidQuery` → 400 + `INVALID_QUERY`
- `Unauthorized` → 401 + `UNAUTHORIZED`
- Catch-all → 500 + `INTERNAL_ERROR` (con `req_id` en logs).

## Testing

- Unit tests por handler usando `axum::Router` + `tower::ServiceExt::oneshot()`.
- Integration tests: spawn server real en port random, hit con `reqwest`.
- OpenAPI doc validation: parse `openapi.json` y verificar que todos los endpoints están documented.

## Out of scope v0.1

- **WebSockets / SSE para streaming**: v0.3.
- **Rate limiting**: v0.2 cuando emerja necesidad.
- **gRPC**: nunca (REST + MCP cubren el espectro).
- **GraphQL**: nunca (over-engineering para este uso).

## Referencias

- axum docs: https://docs.rs/axum/latest/axum/
- utoipa: https://github.com/juhaku/utoipa
- Tower middleware ecosystem: https://docs.rs/tower-http
