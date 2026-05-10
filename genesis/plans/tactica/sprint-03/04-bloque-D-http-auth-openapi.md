# Sprint-03 Bloque D — Auth bearer + OpenAPI

**Tema**: Auth bearer middleware opt-in + utoipa derives + Swagger UI.

**Pre-requisitos**: Bloques A/B/C cerrados.

## Tareas atómicas

### D.1 — Auth bearer middleware

`crates/seele-http/src/auth.rs`:

```rust
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use axum::body::Body;

pub async fn require_bearer(
    expected_token: String,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let header = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok());
    let provided = header.and_then(|h| h.strip_prefix("Bearer ").map(str::trim));
    if provided == Some(expected_token.as_str()) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
```

Wire en `Server::router()` cuando `config.auth_bearer.is_some()`:

```rust
let mut app = Router::new()
    .route("/health", get(handlers::health))     // public
    .route("/version", get(handlers::version))   // public
    // ...
    ;
if let Some(token) = self.config.auth_bearer.clone() {
    app = app.layer(axum::middleware::from_fn(move |req, next| {
        let token = token.clone();
        async move { require_bearer(token, req, next).await }
    }));
}
```

(`/health` y `/version` quedan public para health checks externos.)

### D.2 — utoipa derives en DTOs

Cambio en cada DTO:

```rust
#[derive(Debug, Deserialize, ToSchema)]   // ToSchema agregado
pub struct SaveRequest { ... }
```

Agregar `utoipa::path` a cada handler:

```rust
#[utoipa::path(
    post, path = "/memories",
    request_body = SaveRequest,
    responses(
        (status = 200, body = SaveResponse),
        (status = 400, body = ErrorBody),
    )
)]
pub async fn save(...) -> ...
```

### D.3 — Swagger UI

`crates/seele-http/src/openapi.rs`:

```rust
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::memories::save,
        handlers::memories::search,
        handlers::memories::get_by_id,
        handlers::memories::list,
        // ...todos los handlers
    ),
    components(schemas(
        SaveRequest, SaveResponse, SearchRequest, SearchResponse,
        SearchHitDto, AnnotationDto, ListRequest,
        // ...
    )),
)]
pub struct ApiDoc;
```

Wire en router:

```rust
app = app.merge(SwaggerUi::new("/docs").url("/openapi.json", ApiDoc::openapi()));
```

### D.4 — Tests auth

```rust
#[tokio::test]
async fn auth_disabled_by_default_no_token_required() { ... }

#[tokio::test]
async fn auth_enabled_rejects_missing_token_401() { ... }

#[tokio::test]
async fn auth_enabled_rejects_wrong_token_401() { ... }

#[tokio::test]
async fn auth_enabled_accepts_valid_token() { ... }

#[tokio::test]
async fn auth_enabled_health_endpoint_remains_public() { ... }

#[tokio::test]
async fn openapi_spec_lists_all_handler_paths() {
    // GET /openapi.json y verificar que el spec tiene los ~22 paths.
}
```

## Criterios de aceptación del bloque D

1. `cargo test -p seele-http` verde con tests de auth + OpenAPI.
2. `cargo clippy -p seele-http -- -D warnings` verde (utoipa macros generan warnings a veces).
3. `GET /openapi.json` retorna spec válida OpenAPI 3.1 con todos los handlers.
4. `GET /docs` sirve Swagger UI.
5. Auth opt-in via `ServerConfig::auth_bearer = Some(...)`. Health y version siguen public.

## Commit del bloque D

```
git add -A
git commit -m "sprint-03 bloque-D — http auth bearer + utoipa OpenAPI + Swagger UI"
```
