//! Bloque F — E2E tests that spawn the real `seele` binary.
//!
//! These exercise the CLI dispatch end-to-end: `seele --version`,
//! `seele mcp` over piped stdio, and `seele serve` over a real TCP port.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use tempfile::TempDir;

const SEELE_BIN: &str = env!("CARGO_BIN_EXE_seele");

#[test]
fn version_prints_seele_and_pkg_version() {
    let out = Command::new(SEELE_BIN)
        .arg("--version")
        .output()
        .expect("spawn");
    assert!(out.status.success(), "exit {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.starts_with("seele "), "stdout: {stdout}");
}

#[test]
fn help_lists_mcp_and_serve_commands() {
    let out = Command::new(SEELE_BIN).arg("--help").output().expect("spawn");
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("mcp"), "stdout: {s}");
    assert!(s.contains("serve"), "stdout: {s}");
}

#[test]
fn unknown_command_exits_nonzero() {
    let out = Command::new(SEELE_BIN)
        .arg("does-not-exist")
        .output()
        .expect("spawn");
    assert!(!out.status.success());
}

// -------- mcp --------

#[test]
fn mcp_tools_list_returns_19_tools() {
    let td = TempDir::new().unwrap();
    let db_path = td.path().join("seele.db");
    let mut child = Command::new(SEELE_BIN)
        .arg("mcp")
        .arg("--db")
        .arg(&db_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut stdin = child.stdin.take().unwrap();
    writeln!(
        stdin,
        "{}",
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#
    )
    .unwrap();
    drop(stdin); // signal EOF so server exits after responding
    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);
    let line = reader
        .lines()
        .next()
        .expect("at least one line")
        .expect("read line");
    let v: Value = serde_json::from_str(&line).expect("parse json");
    let tools = v["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 19);
    child.wait().unwrap();
}

#[test]
fn mcp_tool_prefix_mnema_aliases_recall() {
    let td = TempDir::new().unwrap();
    let db_path = td.path().join("seele.db");
    let mut child = Command::new(SEELE_BIN)
        .arg("mcp")
        .arg("--tool-prefix")
        .arg("mnema")
        .arg("--db")
        .arg(&db_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut stdin = child.stdin.take().unwrap();
    writeln!(
        stdin,
        "{}",
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#
    )
    .unwrap();
    drop(stdin);
    let stdout = child.stdout.take().unwrap();
    let line = BufReader::new(stdout)
        .lines()
        .next()
        .expect("line")
        .expect("read");
    let v: Value = serde_json::from_str(&line).unwrap();
    let names: Vec<&str> = v["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"mnema_save"));
    assert!(names.contains(&"mnema_recall"));
    assert!(!names.iter().any(|n| n.starts_with("seele_")));
    child.wait().unwrap();
}

// -------- serve --------

#[test]
fn serve_health_returns_200_over_real_tcp() {
    let td = TempDir::new().unwrap();
    let db_path = td.path().join("seele.db");
    // Pick a free port via OS, then close it before spawning the server.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let mut child = Command::new(SEELE_BIN)
        .arg("serve")
        .arg("--port")
        .arg(port.to_string())
        .arg("--db")
        .arg(&db_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn");

    // Wait for the server to come up — poll /health for up to ~5s.
    let base = format!("http://127.0.0.1:{port}");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let ok = rt.block_on(async {
        let client = reqwest::Client::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match client.get(format!("{base}/health")).send().await {
                Ok(r) if r.status() == 200 => return true,
                _ => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
        false
    });
    let _ = child.kill();
    let _ = child.wait();
    assert!(ok, "server did not become healthy within 5s on port {port}");
}
