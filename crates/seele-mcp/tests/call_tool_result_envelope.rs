//! Patch — MCP `tools/call` envelope shape.
//!
//! Verifies that `tools/call` responses follow the `CallToolResult` shape
//! required by the MCP spec (protocolVersion 2024-11-05+):
//!
//! ```json
//! {
//!   "jsonrpc": "2.0",
//!   "id": <n>,
//!   "result": {
//!     "content": [{ "type": "text", "text": "<stringified JSON>" }],
//!     "isError": false
//!   }
//! }
//! ```
//!
//! Prior to the 2026-05-20 patch, `dispatch_tool_call` returned the
//! handler's raw JSON as `result` directly. MCP clients (Claude Code,
//! Cursor, Windsurf) look for `result.content[0].text`, found
//! `undefined`, and rendered "completed with no output". This regression
//! survived v0.1.0 and v0.2.0 because the existing test suite asserted
//! against the handler-level shape, not against the wire envelope.
//!
//! This file is intentionally narrow: it only checks the envelope
//! structure. The end-to-end behavior of every tool (save/search/doctor
//! …) is exercised in `stdio_e2e.rs`.

use std::sync::Arc;

use seele_embedder::{Embedder, FakeEmbedder};
use seele_http::SeeleService;
use seele_mcp::{McpServer, McpServerConfig};
use seele_storage::init_db;
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn make_service() -> (TempDir, SeeleService) {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let e: Arc<dyn Embedder> = Arc::new(FakeEmbedder);
    let svc = SeeleService::new(pool, e);
    (td, svc)
}

async fn round_trip(server: McpServer, req: Value) -> Value {
    let (mut client, server_side) = tokio::io::duplex(64 * 1024);
    let (client_read, client_write) = tokio::io::split(server_side);
    let handle = tokio::spawn(async move { server.run_io(client_read, client_write).await });

    let mut line = serde_json::to_vec(&req).unwrap();
    line.push(b'\n');
    client.write_all(&line).await.unwrap();
    let mut reader = BufReader::new(&mut client);
    let mut buf = String::new();
    reader.read_line(&mut buf).await.unwrap();
    drop(reader);
    drop(client);
    let _ = handle.await;
    serde_json::from_str(&buf).unwrap()
}

/// `seele_doctor` is the cheapest tool to exercise the envelope: no
/// arguments, no filesystem outside the test tempdir, deterministic
/// payload shape (`status`/`embedder`/`observations_active`/...).
#[tokio::test]
async fn tools_call_wraps_handler_json_in_call_tool_result() {
    let (_td, svc) = make_service();
    let server = McpServer::new(svc, McpServerConfig::default());
    let r = round_trip(
        server,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "seele_doctor", "arguments": {} }
        }),
    )
    .await;

    // JSON-RPC envelope intact.
    assert_eq!(r["jsonrpc"], "2.0");
    assert_eq!(r["id"], 1);
    assert!(r["result"].is_object(), "result missing or not an object");
    assert!(r["error"].is_null(), "error must be absent on success");

    // CallToolResult envelope.
    let content = r["result"]["content"]
        .as_array()
        .expect("result.content must be an array");
    assert_eq!(
        content.len(),
        1,
        "expected exactly one content block, got {}",
        content.len()
    );
    assert_eq!(
        content[0]["type"], "text",
        "content block must be type=text"
    );
    let text = content[0]["text"]
        .as_str()
        .expect("content[0].text must be a string");

    // isError must be present and false on success.
    assert_eq!(
        r["result"]["isError"], false,
        "isError must be `false` on success"
    );

    // The text payload must be parseable JSON and carry the handler's
    // doctor payload (sanity check: the handler actually ran).
    let payload: Value = serde_json::from_str(text).expect("content[0].text must be valid JSON");
    assert_eq!(payload["status"], "ok");
    assert!(
        payload["embedder"]["model_id"].is_string(),
        "doctor must report embedder.model_id"
    );
}

/// Even when the handler returns an empty-ish payload, the envelope must
/// still be present. `seele_version` returns `{name, version}` — small
/// but valid JSON; the wire response must wrap it the same way.
#[tokio::test]
async fn tools_call_envelope_present_even_for_minimal_payload() {
    let (_td, svc) = make_service();
    let server = McpServer::new(svc, McpServerConfig::default());
    let r = round_trip(
        server,
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "seele_version", "arguments": {} }
        }),
    )
    .await;

    let text = r["result"]["content"][0]["text"]
        .as_str()
        .expect("envelope missing for seele_version");
    let payload: Value = serde_json::from_str(text).unwrap();
    assert_eq!(payload["name"], "seele");
    assert!(payload["version"].is_string());
    assert_eq!(r["result"]["isError"], false);
}
