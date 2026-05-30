//! OpenAPI spec + Swagger UI mount.
//!
//! Strategy: use `utoipa::OpenApi` derive to register schemas, but author
//! the paths section by hand. Decorating every handler with
//! `#[utoipa::path]` would duplicate a lot of signature info that already
//! lives in the router. Instead, we describe the API surface in one place
//! and let the router stay clean.

use utoipa::openapi::path::HttpMethod;
use utoipa::openapi::path::{OperationBuilder, ParameterIn};
use utoipa::openapi::request_body::RequestBodyBuilder;
use utoipa::openapi::{ContentBuilder, RefOr, Schema};
use utoipa::openapi::{
    PathItem, PathsBuilder, Ref, Required, ResponseBuilder, ResponsesBuilder, ServerBuilder,
};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

use crate::dto::{
    AnnotationDto, CountBucket, EmbedderInfo, JudgeRequest, LinkCreateRequest, LinkDto,
    ObservationDto, ObservationStats, RelationCreateRequest, RelationDto, SaveRequest,
    SaveResponse, SearchHitDto, SearchRequest, SearchResponse, SessionDto, SessionEndRequest,
    SessionStartRequest, SessionStats, StatsResponse,
};
use crate::error::ErrorBody;

#[derive(OpenApi)]
#[openapi(components(schemas(
    SaveRequest,
    SaveResponse,
    SearchRequest,
    SearchResponse,
    SearchHitDto,
    AnnotationDto,
    ObservationDto,
    SessionStartRequest,
    SessionEndRequest,
    SessionDto,
    LinkCreateRequest,
    LinkDto,
    RelationCreateRequest,
    RelationDto,
    JudgeRequest,
    StatsResponse,
    ObservationStats,
    SessionStats,
    CountBucket,
    EmbedderInfo,
    ErrorBody,
)))]
struct SchemaRegistry;

/// Build the OpenAPI document. Schemas come from `SchemaRegistry`; paths
/// are described by hand below so handlers stay free of utoipa macros.
pub fn build_openapi() -> utoipa::openapi::OpenApi {
    let mut doc = SchemaRegistry::openapi();
    doc.info.title = "SEELE HTTP API".to_string();
    doc.info.version = env!("CARGO_PKG_VERSION").to_string();
    doc.info.description = Some(
        "Local-first memory engine. Endpoints follow REST conventions; \
         error envelope is `{code, message}` per `ErrorBody`."
            .to_string(),
    );
    doc.servers = Some(vec![ServerBuilder::new()
        .url("http://127.0.0.1:7777")
        .description(Some("Default loopback"))
        .build()]);
    doc.paths = build_paths();
    doc
}

fn build_paths() -> utoipa::openapi::Paths {
    let mut paths = PathsBuilder::new();

    // GET /health
    paths = paths.path(
        "/health",
        PathItem::new(
            HttpMethod::Get,
            OperationBuilder::new()
                .summary(Some("Liveness probe"))
                .description(Some(
                    "Returns `{status: \"ok\"}` if the server is responsive. \
                     Always public (auth bypass).",
                ))
                .responses(json_ok("health payload")),
        ),
    );

    // GET /version
    paths = paths.path(
        "/version",
        PathItem::new(
            HttpMethod::Get,
            OperationBuilder::new()
                .summary(Some("Build version"))
                .responses(json_ok("version payload")),
        ),
    );

    // POST /memories — save
    paths = paths.path(
        "/memories",
        PathItem::new(
            HttpMethod::Post,
            OperationBuilder::new()
                .summary(Some("Save observation"))
                .request_body(Some(json_body::<SaveRequest>("SaveRequest")))
                .responses(json_response::<SaveResponse>(200, "Saved observation")),
        ),
    );

    // GET /memories
    paths = paths.path(
        "/memories",
        PathItem::new(
            HttpMethod::Get,
            OperationBuilder::new()
                .summary(Some("List observations"))
                .parameters(Some(vec![
                    query_param("project", "Filter by project", false),
                    query_param("scope", "Filter by scope (project|personal)", false),
                    query_param("type", "Filter by observation type", false),
                    query_param("topic_key", "Filter by topic_key", false),
                    query_param("session_id", "Filter by session", false),
                    query_param("limit", "Max rows", false),
                    query_param("include_deleted", "Include soft-deleted", false),
                ]))
                .responses(json_array::<ObservationDto>(200, "Observation list")),
        ),
    );

    // GET /memories/{id}
    paths = paths.path(
        "/memories/{id}",
        PathItem::new(
            HttpMethod::Get,
            OperationBuilder::new()
                .summary(Some("Get observation by id"))
                .parameters(Some(vec![path_param("id", "ULID")]))
                .responses(json_response::<ObservationDto>(200, "Observation")),
        ),
    );

    // DELETE /memories/{id} — soft delete
    paths = paths.path(
        "/memories/{id}",
        PathItem::new(
            HttpMethod::Delete,
            OperationBuilder::new()
                .summary(Some("Soft delete observation"))
                .parameters(Some(vec![path_param("id", "ULID")]))
                .responses(no_content("Deleted")),
        ),
    );

    // POST /memories/{id}/restore
    paths = paths.path(
        "/memories/{id}/restore",
        PathItem::new(
            HttpMethod::Post,
            OperationBuilder::new()
                .summary(Some("Restore soft-deleted observation"))
                .parameters(Some(vec![path_param("id", "ULID")]))
                .responses(no_content("Restored")),
        ),
    );

    // GET /memories/{id}/links
    paths = paths.path(
        "/memories/{id}/links",
        PathItem::new(
            HttpMethod::Get,
            OperationBuilder::new()
                .summary(Some("List links touching an observation"))
                .parameters(Some(vec![path_param("id", "ULID")]))
                .responses(json_array::<LinkDto>(200, "Link list")),
        ),
    );

    // POST /search
    paths = paths.path(
        "/search",
        PathItem::new(
            HttpMethod::Post,
            OperationBuilder::new()
                .summary(Some("Hybrid search (FTS + vec + RRF)"))
                .request_body(Some(json_body::<SearchRequest>("SearchRequest")))
                .responses(json_response::<SearchResponse>(200, "Search hits")),
        ),
    );

    // Sessions
    paths = paths.path(
        "/sessions",
        PathItem::new(
            HttpMethod::Post,
            OperationBuilder::new()
                .summary(Some("Start session"))
                .request_body(Some(json_body::<SessionStartRequest>(
                    "SessionStartRequest",
                )))
                .responses(json_response::<SessionDto>(200, "Session started")),
        ),
    );
    paths = paths.path(
        "/sessions",
        PathItem::new(
            HttpMethod::Get,
            OperationBuilder::new()
                .summary(Some("List sessions"))
                .parameters(Some(vec![
                    query_param("project", "Filter by project", false),
                    query_param("status", "active|ended|aborted", false),
                    query_param("limit", "Max rows", false),
                ]))
                .responses(json_array::<SessionDto>(200, "Session list")),
        ),
    );
    paths = paths.path(
        "/sessions/{id}",
        PathItem::new(
            HttpMethod::Get,
            OperationBuilder::new()
                .summary(Some("Get session"))
                .parameters(Some(vec![path_param("id", "ULID")]))
                .responses(json_response::<SessionDto>(200, "Session")),
        ),
    );
    paths = paths.path(
        "/sessions/{id}/end",
        PathItem::new(
            HttpMethod::Put,
            OperationBuilder::new()
                .summary(Some("End session"))
                .parameters(Some(vec![path_param("id", "ULID")]))
                .request_body(Some(json_body::<SessionEndRequest>("SessionEndRequest")))
                .responses(no_content("Ended")),
        ),
    );
    paths = paths.path(
        "/sessions/{id}/abort",
        PathItem::new(
            HttpMethod::Put,
            OperationBuilder::new()
                .summary(Some("Abort session"))
                .parameters(Some(vec![path_param("id", "ULID")]))
                .responses(no_content("Aborted")),
        ),
    );

    // Links
    paths = paths.path(
        "/links",
        PathItem::new(
            HttpMethod::Post,
            OperationBuilder::new()
                .summary(Some("Create link between observations"))
                .request_body(Some(json_body::<LinkCreateRequest>("LinkCreateRequest")))
                .responses(json_response::<LinkDto>(200, "Link created")),
        ),
    );
    paths = paths.path(
        "/links/{id}",
        PathItem::new(
            HttpMethod::Delete,
            OperationBuilder::new()
                .summary(Some("Delete link"))
                .parameters(Some(vec![path_param("id", "ULID")]))
                .responses(no_content("Deleted")),
        ),
    );

    // Relations
    paths = paths.path(
        "/relations",
        PathItem::new(
            HttpMethod::Post,
            OperationBuilder::new()
                .summary(Some("Create memory relation"))
                .request_body(Some(json_body::<RelationCreateRequest>(
                    "RelationCreateRequest",
                )))
                .responses(json_response::<RelationDto>(200, "Relation created")),
        ),
    );
    paths = paths.path(
        "/relations",
        PathItem::new(
            HttpMethod::Get,
            OperationBuilder::new()
                .summary(Some("List relations"))
                .parameters(Some(vec![
                    query_param("source_id", "Filter by source", false),
                    query_param("target_id", "Filter by target", false),
                    query_param("relation", "Filter by relation kind", false),
                    query_param("status", "pending|judged|orphaned|ignored", false),
                    query_param("limit", "Max rows", false),
                ]))
                .responses(json_array::<RelationDto>(200, "Relation list")),
        ),
    );
    paths = paths.path(
        "/relations/{id}/judge",
        PathItem::new(
            HttpMethod::Put,
            OperationBuilder::new()
                .summary(Some("Apply judgment to a relation"))
                .parameters(Some(vec![path_param("id", "ULID")]))
                .request_body(Some(json_body::<JudgeRequest>("JudgeRequest")))
                .responses(no_content("Judged")),
        ),
    );
    paths = paths.path(
        "/conflicts",
        PathItem::new(
            HttpMethod::Get,
            OperationBuilder::new()
                .summary(Some("List pending conflicts_with relations"))
                .parameters(Some(vec![query_param("limit", "Max rows", false)]))
                .responses(json_array::<RelationDto>(200, "Pending conflicts")),
        ),
    );

    // Stats + embedder
    paths = paths.path(
        "/stats",
        PathItem::new(
            HttpMethod::Get,
            OperationBuilder::new()
                .summary(Some("Aggregate stats"))
                .responses(json_response::<StatsResponse>(200, "Stats payload")),
        ),
    );
    paths = paths.path(
        "/embedder",
        PathItem::new(
            HttpMethod::Get,
            OperationBuilder::new()
                .summary(Some("Current embedder info"))
                .responses(json_response::<EmbedderInfo>(200, "Embedder info")),
        ),
    );

    // POST /chat — AI chat with server-side tool-use against /search
    paths = paths.path(
        "/chat",
        PathItem::new(
            HttpMethod::Post,
            OperationBuilder::new()
                .summary(Some("AI chat with tool-use over hybrid search"))
                .description(Some(
                    "Runs the server-side tool-use loop: the model may call \
                     `seele_search` against the hybrid engine, results are fed \
                     back, looping until it returns text. Request body \
                     `{messages, system_prompt?, provider?, api_key?, model?, \
                     endpoint?}` — per-request overrides beat the \
                     `seele serve --chat-*` config; `api_key` is used once and \
                     never persisted. Response `{messages, provider, model}`. \
                     Returns 500 if chat is not configured.",
                ))
                .responses(json_ok("Full message history after the tool-use loop")),
        ),
    );

    // GET /chat/info — chat availability
    paths = paths.path(
        "/chat/info",
        PathItem::new(
            HttpMethod::Get,
            OperationBuilder::new()
                .summary(Some("Chat availability + configured provider/model"))
                .description(Some(
                    "Returns `{enabled, provider, model}` so the frontend can \
                     render the right state. `enabled=false` when the server was \
                     started without `--chat-provider`.",
                ))
                .responses(json_ok("Chat info payload")),
        ),
    );

    paths.build()
}

// ---------- Helpers for the path-builder DSL ----------

fn schema_ref<T: ToSchema>(name: &str) -> RefOr<Schema> {
    let _ = std::any::type_name::<T>();
    RefOr::Ref(Ref::from_schema_name(name))
}

fn json_body<T: ToSchema>(schema_name: &str) -> utoipa::openapi::request_body::RequestBody {
    RequestBodyBuilder::new()
        .content(
            "application/json",
            ContentBuilder::new()
                .schema(Some(schema_ref::<T>(schema_name)))
                .build(),
        )
        .required(Some(Required::True))
        .build()
}

fn json_response<T: ToSchema>(status: u16, description: &str) -> utoipa::openapi::Responses {
    let schema_name = std::any::type_name::<T>()
        .rsplit("::")
        .next()
        .unwrap_or("payload");
    ResponsesBuilder::new()
        .response(
            status.to_string(),
            ResponseBuilder::new()
                .description(description)
                .content(
                    "application/json",
                    ContentBuilder::new()
                        .schema(Some(schema_ref::<T>(schema_name)))
                        .build(),
                )
                .build(),
        )
        .build()
}

fn json_array<T: ToSchema>(status: u16, description: &str) -> utoipa::openapi::Responses {
    let schema_name = std::any::type_name::<T>()
        .rsplit("::")
        .next()
        .unwrap_or("payload");
    let mut array_schema = utoipa::openapi::ArrayBuilder::new();
    array_schema = array_schema.items(schema_ref::<T>(schema_name));
    ResponsesBuilder::new()
        .response(
            status.to_string(),
            ResponseBuilder::new()
                .description(description)
                .content(
                    "application/json",
                    ContentBuilder::new()
                        .schema(Some(RefOr::T(Schema::Array(array_schema.build()))))
                        .build(),
                )
                .build(),
        )
        .build()
}

fn no_content(description: &str) -> utoipa::openapi::Responses {
    ResponsesBuilder::new()
        .response(
            "204",
            ResponseBuilder::new().description(description).build(),
        )
        .build()
}

fn json_ok(description: &str) -> utoipa::openapi::Responses {
    ResponsesBuilder::new()
        .response(
            "200",
            ResponseBuilder::new().description(description).build(),
        )
        .build()
}

fn path_param(name: &str, description: &str) -> utoipa::openapi::path::Parameter {
    utoipa::openapi::path::ParameterBuilder::new()
        .name(name)
        .parameter_in(ParameterIn::Path)
        .required(Required::True)
        .description(Some(description))
        .build()
}

fn query_param(name: &str, description: &str, required: bool) -> utoipa::openapi::path::Parameter {
    utoipa::openapi::path::ParameterBuilder::new()
        .name(name)
        .parameter_in(ParameterIn::Query)
        .required(if required {
            Required::True
        } else {
            Required::False
        })
        .description(Some(description))
        .build()
}

/// Router that serves `/openapi.json` + Swagger UI at `/docs/*`. Mounted
/// from `Server::router`. Public so that no auth middleware fires for
/// docs (the spec only describes the schema, no data leak).
pub fn routes() -> axum::Router {
    let ui: axum::Router = SwaggerUi::new("/docs")
        .url("/openapi.json", build_openapi())
        .into();
    ui
}
