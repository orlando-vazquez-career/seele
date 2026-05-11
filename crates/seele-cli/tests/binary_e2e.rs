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

/// Wrap `Command::new(SEELE_BIN)` with `SEELE_FAKE_EMBEDDER=1` so tests
/// never try to download the ONNX model (~90 MB on first run) and stay
/// deterministic in CI. `--version` / `--help` / unknown-command tests
/// don't strictly need it, but consistency wins.
fn seele() -> Command {
    let mut c = Command::new(SEELE_BIN);
    c.env("SEELE_FAKE_EMBEDDER", "1");
    c
}

#[test]
fn version_prints_seele_and_pkg_version() {
    let out = seele().arg("--version").output().expect("spawn");
    assert!(out.status.success(), "exit {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.starts_with("seele "), "stdout: {stdout}");
}

#[test]
fn help_lists_mcp_and_serve_commands() {
    let out = seele().arg("--help").output().expect("spawn");
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("mcp"), "stdout: {s}");
    assert!(s.contains("serve"), "stdout: {s}");
}

#[test]
fn unknown_command_exits_nonzero() {
    let out = seele().arg("does-not-exist").output().expect("spawn");
    assert!(!out.status.success());
}

// -------- mcp --------

#[test]
fn mcp_tools_list_returns_19_tools() {
    let td = TempDir::new().unwrap();
    let db_path = td.path().join("seele.db");
    let mut child = seele()
        .arg("mcp")
        .arg("--db")
        .arg(&db_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut stdin = child.stdin.take().unwrap();
    stdin
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n")
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
    let mut child = seele()
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
    stdin
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n")
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

    // Let the OS pick the port (--port 0) and parse the chosen address
    // from the server's stderr line. Avoids the bind-then-drop race
    // where another process could grab the port before the server does.
    let mut child = seele()
        .arg("serve")
        .arg("--port")
        .arg("0")
        .arg("--db")
        .arg(&db_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");

    let stderr = child.stderr.take().unwrap();
    let mut reader = BufReader::new(stderr);
    let mut line = String::new();
    reader.read_line(&mut line).expect("read stderr line");
    // Expected: "seele http listening on http://127.0.0.1:43217"
    let base = line
        .trim()
        .strip_prefix("seele http listening on ")
        .map(str::to_string)
        .unwrap_or_else(|| panic!("unexpected stderr line: {line:?}"));

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
    assert!(ok, "server did not become healthy within 5s at {base}");
}
