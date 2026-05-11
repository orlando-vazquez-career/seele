# Sprint-03 Bloque E — MCP server stdio + 19 tools

**Tema**: `seele-mcp` crate con JSON-RPC 2.0 sobre stdin/stdout y 19 tools `seele_*` que reusan el service layer del Bloque A.

**Pre-requisitos**: Bloque A (service layer) + bloques B/C (operaciones implementadas en service). Bloque D opcional (auth no aplica a stdio).

## 19 tools `seele_*` v0.1

| # | Tool | Op interna | ADR ref |
|---|---|---|---|
| 1 | `seele_save` | `service.save_observation` | ADR-05 |
| 2 | `seele_search` | `service.search_observations` (+ anti-empty-query) | ADR-05 |
| 3 | `seele_show` | `service.get_observation` (+ links opt-in) | ADR-05 |
| 4 | `seele_list` | `service.list_observations` | ADR-05 |
| 5 | `seele_update_metadata` | observation patch metadata | ADR-05 |
| 6 | `seele_delete` | soft_delete | ADR-05 |
| 7 | `seele_restore` | restore | ADR-05 |
| 8 | `seele_link` | `service.create_link` | ADR-05 |
| 9 | `seele_stats` | `service.stats` | ADR-05 |
| 10 | `seele_session_start` | `service.start_session` | scope MVP |
| 11 | `seele_session_end` | `service.end_session` | scope MVP |
| 12 | `seele_session_summary` | session structured summary save | scope MVP |
| 13 | `seele_capture_passive` | parser de `## Key Learnings:` + save batch | scope MVP |
| 14 | `seele_judge` | `service.judge_relation` | scope MVP |
| 15 | `seele_compare` | `service.create_relation(ConflictsWith pending)` | scope MVP |
| 16 | `seele_suggest_topic_key` | match contra family heuristics | scope MVP |
| 17 | `seele_projects` | list projects (DISTINCT project FROM observations) | scope MVP |
| 18 | `seele_doctor` | health checks: DB version, embedder loaded, paths | scope MVP |
| 19 | `seele_version` | `pkg version + schema version` | scope MVP |

## Estructura del crate

```
crates/seele-mcp/
├── Cargo.toml
├── src/
│   ├── lib.rs           # public API
│   ├── server.rs        # McpServer + run_stdio
│   ├── jsonrpc.rs       # JSON-RPC 2.0 request/response types
│   ├── tools.rs         # registry de tools + dispatch
│   └── tool_impls/      # implementación por tool (1 archivo por dominio)
│       ├── memories.rs
│       ├── sessions.rs
│       ├── relations.rs
│       ├── meta.rs
│       └── ...
└── tests/
    └── stdio_e2e.rs
```

## Tareas atómicas

### E.1 — Cargo.toml

```toml
[dependencies]
seele-core = { path = "../seele-core" }
seele-storage = { path = "../seele-storage" }
seele-search = { path = "../seele-search" }
seele-embedder = { path = "../seele-embedder" }
seele-http = { path = "../seele-http" }  # para reusar SeeleService
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true
```

### E.2 — JSON-RPC 2.0 types

`jsonrpc.rs`:

```rust
#[derive(Debug, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct Response {
    pub jsonrpc: &'static str, // "2.0"
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorObject>,
}

#[derive(Debug, Serialize)]
pub struct ErrorObject {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

// Codes: -32700 parse, -32600 invalid req, -32601 method not found,
//        -32602 invalid params, -32603 internal, 1001+ SEELE-specific.
```

### E.3 — Tool registry

`tools.rs`:

```rust
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: serde_json::Value,
    pub handler: fn(&SeeleService, serde_json::Value) -> Result<serde_json::Value, ToolError>,
}

pub fn all_tools() -> Vec<Tool> {
    vec![
        Tool { name: "seele_save", description: "...", input_schema: ..., handler: tool_impls::memories::save },
        // ...19 tools
    ]
}
```

### E.4 — Server stdio

`server.rs`:

```rust
pub struct McpServer {
    service: SeeleService,
    tools: HashMap<&'static str, Tool>,
}

impl McpServer {
    pub fn new(service: SeeleService) -> Self {
        let tools = all_tools().into_iter().map(|t| (t.name, t)).collect();
        Self { service, tools }
    }

    pub async fn run_stdio(&self) -> anyhow::Result<()> {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let reader = tokio::io::BufReader::new(stdin);
        let writer = tokio::sync::Mutex::new(stdout);
        let mut lines = reader.lines();
        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() { continue; }
            let response = self.handle_line(&line);
            let mut w = writer.lock().await;
            let s = serde_json::to_string(&response)?;
            tokio::io::AsyncWriteExt::write_all(&mut *w, s.as_bytes()).await?;
            tokio::io::AsyncWriteExt::write_all(&mut *w, b"\n").await?;
            tokio::io::AsyncWriteExt::flush(&mut *w).await?;
        }
        Ok(())
    }

    fn handle_line(&self, line: &str) -> Response {
        // parse → method dispatch → tool call → response.
    }
}
```

Methods dispatched:
- `initialize` → returns server info + capabilities.
- `tools/list` → returns 19 tool descriptors.
- `tools/call` → dispatch a `Tool.handler`.

### E.5 — Tool implementations (19 archivos pequeños)

Cada tool es un fn corto que parsea params + llama al service + serializa response:

```rust
pub fn save(svc: &SeeleService, params: serde_json::Value) -> Result<serde_json::Value, ToolError> {
    let req: SaveRequest = serde_json::from_value(params)?;
    let resp = svc.save_observation(req)?;
    Ok(serde_json::to_value(resp)?)
}
```

### E.6 — Tests stdio E2E

`tests/stdio_e2e.rs`:

```rust
#[tokio::test]
async fn tools_list_returns_19_tools() {
    // arrancar McpServer en background con channels in-memory en lugar
    // de stdin/stdout reales para no depender de I/O OS-level.
    // Hacer una conversación: write `tools/list` → read response.
}

#[tokio::test]
async fn tools_call_seele_save() { ... }

#[tokio::test]
async fn tools_call_seele_search_empty_query_returns_error() {
    // Mismo gate anti-empty-query que HTTP.
}

#[tokio::test]
async fn unknown_method_returns_method_not_found_minus_32601() { ... }

#[tokio::test]
async fn unknown_tool_returns_invalid_params_minus_32602() { ... }
```

Para los tests, el `run_stdio` se puede generalizar con un trait `Transport { fn read_line(); fn write_line(); }` que tenga impl para stdin/stdout real + impl para `tokio::sync::mpsc` testeable. O usar `tokio::io::duplex()` que da reader+writer in-memory.

## Binary `seele mcp` integration (preparación para Sprint-04 CLI)

`seele-cli/src/main.rs` ya existe como stub. En este Bloque E lo extendemos minimalmente para soportar `seele mcp`:

```rust
fn main() -> anyhow::Result<()> {
    let mode = std::env::args().nth(1).as_deref().unwrap_or("--help").to_string();
    match mode.as_str() {
        "mcp" => run_mcp(),
        "serve" => run_serve(),
        "--version" => { println!("seele {}", env!("CARGO_PKG_VERSION")); Ok(()) }
        _ => { eprintln!("usage: seele [mcp|serve|--version]"); Ok(()) }
    }
}
```

(CLI completa con clap llega en Sprint-04. Aquí es solo lo mínimo para que el binario expose ambos transports.)

## Criterios de aceptación del bloque E

1. `cargo test -p seele-mcp` verde con tests de tools/list, tools/call para 2-3 tools clave, error handling.
2. `cargo build -p seele-cli --release` produce binary que responde a `seele --version`, `seele mcp` y `seele serve`.
3. Spawn `seele mcp` en test (subprocess), pipe stdin con `tools/list`, leer stdout → recibir 19 tools.
4. Empty-query check para `seele_search` retorna JSON-RPC error con code adecuado.

## Commit del bloque E

```
git add -A
git commit -m "sprint-03 bloque-E — mcp stdio + 19 tools + cli mcp/serve dispatch"
```
