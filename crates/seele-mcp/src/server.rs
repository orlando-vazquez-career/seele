//! MCP server: dispatches `initialize`, `tools/list`, `tools/call` over a
//! line-delimited JSON-RPC 2.0 transport.
//!
//! The transport is generic so tests can use `tokio::io::duplex` or
//! in-memory channels instead of touching real stdin/stdout.

use std::collections::HashMap;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use seele_http::SeeleService;

use crate::jsonrpc::{codes, ErrorObject, Request, Response};
use crate::tools::{build_index, Tool, ToolDescriptor};

#[derive(Clone, Default)]
pub struct McpServerConfig {
    /// Prefix for exposed tool names. `None` keeps canonical `seele_*`.
    /// `"mnema"` triggers ADR-13 ENGRAM-compat naming (e.g.
    /// `mnema_recall`).
    pub tool_prefix: Option<String>,
}

pub struct McpServer {
    service: SeeleService,
    tools: HashMap<String, Tool>,
}

impl McpServer {
    pub fn new(service: SeeleService, config: McpServerConfig) -> Self {
        let tools = build_index(config.tool_prefix.as_deref());
        Self { service, tools }
    }

    /// Run against the real `tokio::io::stdin/stdout`. Blocks until EOF
    /// on stdin.
    pub async fn run_stdio(&self) -> anyhow::Result<()> {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        self.run_io(stdin, stdout).await
    }

    /// Run against any pair implementing `AsyncRead + AsyncWrite`. Used
    /// by tests with `tokio::io::duplex`.
    pub async fn run_io<R, W>(&self, reader: R, mut writer: W) -> anyhow::Result<()>
    where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        let mut lines = BufReader::new(reader).lines();
        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }
            if let Some(resp) = self.handle_line(&line) {
                let s = serde_json::to_string(&resp)?;
                writer.write_all(s.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
        }
        Ok(())
    }

    /// Parse + dispatch one line. Returns `None` for notifications (no
    /// `id`); the caller must not send a response in that case.
    pub fn handle_line(&self, line: &str) -> Option<Response> {
        let req: Request = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => {
                return Some(Response::err(
                    Value::Null,
                    ErrorObject::new(codes::PARSE_ERROR, format!("parse error: {e}")),
                ));
            }
        };
        // Notification: no id, no response.
        let id = req.id.clone()?;
        match req.method.as_str() {
            "initialize" => Some(Response::ok(
                id,
                serde_json::json!({
                    "serverInfo": { "name": "seele", "version": env!("CARGO_PKG_VERSION") },
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                }),
            )),
            "tools/list" => {
                let mut entries: Vec<ToolDescriptor<'_>> = self
                    .tools
                    .iter()
                    .map(|(name, t)| ToolDescriptor {
                        name: name.clone(),
                        description: t.description,
                        input_schema: &t.input_schema,
                    })
                    .collect();
                entries.sort_by(|a, b| a.name.cmp(&b.name));
                Some(Response::ok(id, serde_json::json!({ "tools": entries })))
            }
            "tools/call" => Some(self.dispatch_tool_call(id, req.params)),
            other => Some(Response::err(
                id,
                ErrorObject::new(
                    codes::METHOD_NOT_FOUND,
                    format!("method not found: {other}"),
                ),
            )),
        }
    }

    fn dispatch_tool_call(&self, id: Value, params: Value) -> Response {
        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => {
                return Response::err(
                    id,
                    ErrorObject::new(codes::INVALID_PARAMS, "params.name required"),
                );
            }
        };
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(Value::Object(Default::default()));

        let tool = match self.tools.get(&name) {
            Some(t) => t,
            None => {
                return Response::err(
                    id,
                    ErrorObject::new(codes::INVALID_PARAMS, format!("unknown tool: {name}")),
                );
            }
        };
        match (tool.handler)(&self.service, arguments) {
            Ok(v) => {
                // MCP spec (protocolVersion 2024-11-05+) requires `tools/call`
                // to return a `CallToolResult` shaped like
                // `{ content: [{ type: "text", text: <...> }], isError: bool }`.
                // Handlers produce arbitrary JSON; we stringify it into a
                // single text block. Migration to `structuredContent`
                // (2025-06-18) is deferred.
                let text = serde_json::to_string(&v)
                    .unwrap_or_else(|_| "<unserializable tool output>".into());
                Response::ok(
                    id,
                    serde_json::json!({
                        "content": [{ "type": "text", "text": text }],
                        "isError": false,
                    }),
                )
            }
            Err(e) => Response::err(id, e.to_jsonrpc()),
        }
    }
}
