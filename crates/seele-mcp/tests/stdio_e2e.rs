//! Bloque E — MCP stdio E2E tests.
//!
//! Uses `tokio::io::duplex` so each test gets an in-memory transport
//! pair without touching real stdin/stdout.

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

/// Extract and parse the JSON payload from a successful `tools/call`
/// response. MCP spec wraps the handler output in `result.content[0].text`
/// as a stringified JSON. This helper unwraps it so test assertions can
/// keep looking at the handler-level shape without each test re-parsing
/// the envelope.
fn tool_result(r: &Value) -> Value {
    let text = r["result"]["content"][0]["text"]
        .as_str()
        .expect("tools/call result missing content[0].text");
    serde_json::from_str(text).expect("tools/call result text is not valid JSON")
}

/// Spawn the MCP server against a duplex pair, write one request, read
/// one line of response, and return the parsed JSON.
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
    // Close the client side so the server's read loop exits cleanly.
    drop(reader);
    drop(client);
    // Server task completes on EOF.
    let _ = handle.await;
    serde_json::from_str(&buf).unwrap()
}

// -------- protocol --------

#[tokio::test]
async fn initialize_returns_server_info() {
    let (_td, svc) = make_service();
    let server = McpServer::new(svc, McpServerConfig::default());
    let r = round_trip(
        server,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"}),
    )
    .await;
    assert_eq!(r["id"], 1);
    assert_eq!(r["result"]["serverInfo"]["name"], "seele");
}

#[tokio::test]
async fn tools_list_returns_19_tools_with_canonical_seele_prefix() {
    let (_td, svc) = make_service();
    let server = McpServer::new(svc, McpServerConfig::default());
    let r = round_trip(
        server,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    )
    .await;
    let tools = r["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 19, "expected 19 tools, got {}", tools.len());
    for t in tools {
        let name = t["name"].as_str().unwrap();
        assert!(
            name.starts_with("seele_"),
            "canonical prefix expected, got {name}"
        );
        assert!(t["description"].is_string());
        assert!(t["inputSchema"].is_object());
    }
}

#[tokio::test]
async fn tools_list_with_mnema_prefix_aliases_recall() {
    let (_td, svc) = make_service();
    let server = McpServer::new(
        svc,
        McpServerConfig {
            tool_prefix: Some("mnema".to_string()),
        },
    );
    let r = round_trip(
        server,
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list"}),
    )
    .await;
    let names: Vec<&str> = r["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"mnema_save"),
        "expected mnema_save, got {names:?}"
    );
    assert!(
        names.contains(&"mnema_recall"),
        "expected mnema_recall (ENGRAM compat), got {names:?}"
    );
    assert!(!names.iter().any(|n| n.starts_with("seele_")));
}

#[tokio::test]
async fn unknown_method_returns_method_not_found_minus_32601() {
    let (_td, svc) = make_service();
    let server = McpServer::new(svc, McpServerConfig::default());
    let r = round_trip(
        server,
        json!({"jsonrpc": "2.0", "id": 4, "method": "no_such_method"}),
    )
    .await;
    assert_eq!(r["error"]["code"], -32601);
}

#[tokio::test]
async fn parse_error_returns_minus_32700_with_null_id() {
    let (_td, svc) = make_service();
    let server = McpServer::new(svc, McpServerConfig::default());
    let (mut client, server_side) = tokio::io::duplex(64 * 1024);
    let (client_read, client_write) = tokio::io::split(server_side);
    let handle = tokio::spawn(async move { server.run_io(client_read, client_write).await });
    client.write_all(b"not-json\n").await.unwrap();
    let mut reader = BufReader::new(&mut client);
    let mut buf = String::new();
    reader.read_line(&mut buf).await.unwrap();
    drop(reader);
    drop(client);
    let _ = handle.await;
    let r: Value = serde_json::from_str(&buf).unwrap();
    assert!(r["id"].is_null());
    assert_eq!(r["error"]["code"], -32700);
}

#[tokio::test]
async fn notification_no_id_produces_no_response() {
    let (_td, svc) = make_service();
    let server = McpServer::new(svc, McpServerConfig::default());
    let (mut client, server_side) = tokio::io::duplex(64 * 1024);
    let (client_read, client_write) = tokio::io::split(server_side);
    let handle = tokio::spawn(async move { server.run_io(client_read, client_write).await });
    // Send a notification (no id), then a real request to flush.
    client
        .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"initialize\"}\n")
        .await
        .unwrap();
    client
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":99,\"method\":\"initialize\"}\n")
        .await
        .unwrap();
    let mut reader = BufReader::new(&mut client);
    let mut first = String::new();
    reader.read_line(&mut first).await.unwrap();
    let v: Value = serde_json::from_str(&first).unwrap();
    // The first response we see MUST be the second request's id; the
    // notification produced no output.
    assert_eq!(v["id"], 99);
    drop(reader);
    drop(client);
    let _ = handle.await;
}

// -------- tools/call --------

#[tokio::test]
async fn tools_call_seele_save_then_search() {
    let (_td, svc) = make_service();
    let server = McpServer::new(svc, McpServerConfig::default());
    let (mut client, server_side) = tokio::io::duplex(64 * 1024);
    let (client_read, client_write) = tokio::io::split(server_side);
    let handle = tokio::spawn(async move { server.run_io(client_read, client_write).await });

    let req1 = json!({
        "jsonrpc": "2.0", "id": 10, "method": "tools/call",
        "params": {
            "name": "seele_save",
            "arguments": {"title": "deploy notes", "content": "rolled out v0.1.0", "project": "p"},
        }
    });
    let mut line = serde_json::to_vec(&req1).unwrap();
    line.push(b'\n');
    client.write_all(&line).await.unwrap();

    let req2 = json!({
        "jsonrpc": "2.0", "id": 11, "method": "tools/call",
        "params": {
            "name": "seele_search",
            "arguments": {"query": "deploy"},
        }
    });
    let mut line = serde_json::to_vec(&req2).unwrap();
    line.push(b'\n');
    client.write_all(&line).await.unwrap();

    let mut reader = BufReader::new(&mut client);
    let mut a = String::new();
    let mut b = String::new();
    reader.read_line(&mut a).await.unwrap();
    reader.read_line(&mut b).await.unwrap();
    drop(reader);
    drop(client);
    let _ = handle.await;

    let saved: Value = serde_json::from_str(&a).unwrap();
    assert_eq!(saved["id"], 10);
    assert!(tool_result(&saved)["id"].is_string());

    let searched: Value = serde_json::from_str(&b).unwrap();
    assert_eq!(searched["id"], 11);
    let searched_payload = tool_result(&searched);
    let hits = searched_payload["hits"].as_array().unwrap();
    assert!(!hits.is_empty(), "expected at least one hit");
}

#[tokio::test]
async fn tools_call_seele_search_empty_query_returns_tool_error() {
    let (_td, svc) = make_service();
    let server = McpServer::new(svc, McpServerConfig::default());
    let r = round_trip(
        server,
        json!({
            "jsonrpc": "2.0", "id": 20, "method": "tools/call",
            "params": { "name": "seele_search", "arguments": {} }
        }),
    )
    .await;
    // Empty query with no filters → BadParams (mapped to -32602).
    assert_eq!(r["error"]["code"], -32602);
}

#[tokio::test]
async fn tools_call_unknown_tool_returns_invalid_params() {
    let (_td, svc) = make_service();
    let server = McpServer::new(svc, McpServerConfig::default());
    let r = round_trip(
        server,
        json!({
            "jsonrpc": "2.0", "id": 30, "method": "tools/call",
            "params": { "name": "seele_doesnt_exist", "arguments": {} }
        }),
    )
    .await;
    assert_eq!(r["error"]["code"], -32602);
}

#[tokio::test]
async fn tools_call_under_mnema_prefix_routes_to_canonical_handler() {
    let (_td, svc) = make_service();
    let server = McpServer::new(
        svc,
        McpServerConfig {
            tool_prefix: Some("mnema".to_string()),
        },
    );
    let r = round_trip(
        server,
        json!({
            "jsonrpc": "2.0", "id": 40, "method": "tools/call",
            "params": {
                "name": "mnema_save",
                "arguments": {"title": "t", "content": "c"},
            }
        }),
    )
    .await;
    assert!(tool_result(&r)["id"].is_string());
}

#[tokio::test]
async fn tools_call_seele_doctor_returns_health_report() {
    let (_td, svc) = make_service();
    let server = McpServer::new(svc, McpServerConfig::default());
    let r = round_trip(
        server,
        json!({
            "jsonrpc": "2.0", "id": 50, "method": "tools/call",
            "params": { "name": "seele_doctor", "arguments": {} }
        }),
    )
    .await;
    let payload = tool_result(&r);
    assert_eq!(payload["status"], "ok");
    assert!(payload["embedder"]["model_id"].is_string());
    assert_eq!(payload["observations_active"], 0);
}

#[tokio::test]
async fn tools_call_seele_capture_passive_extracts_learnings() {
    let (_td, svc) = make_service();
    let server = McpServer::new(svc, McpServerConfig::default());
    let transcript = "## Key Learnings\n- alpha\n- beta\n## End";
    let r = round_trip(
        server,
        json!({
            "jsonrpc": "2.0", "id": 60, "method": "tools/call",
            "params": {
                "name": "seele_capture_passive",
                "arguments": {"transcript": transcript, "project": "p"},
            }
        }),
    )
    .await;
    let payload = tool_result(&r);
    assert_eq!(payload["count"], 2);
    let saved = payload["saved"].as_array().unwrap();
    assert_eq!(saved.len(), 2);
    for v in saved {
        assert!(v.is_string());
    }
}
