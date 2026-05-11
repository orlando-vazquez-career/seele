//! SEELE CLI binary — minimum dispatch to `seele mcp` and `seele serve`
//! for Sprint-03 Bloque E + F. Full clap-based command surface lands in
//! Sprint-04.
//!
//! Recognized invocations:
//! - `seele --version`        — print version + exit
//! - `seele --help`           — usage banner
//! - `seele mcp [--tool-prefix <p>] [--db <path>]`
//! - `seele serve [--port <p>] [--bind <addr>] [--legacy-engram-paths]
//!                [--auth-bearer <token>] [--db <path>]`

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use seele_embedder::{Embedder, FakeEmbedder};
use seele_http::{SeeleService, Server, ServerConfig};
use seele_mcp::{McpServer, McpServerConfig};
use seele_storage::init_db;

const HELP: &str = "\
seele — local-first memory engine

USAGE:
    seele <command> [options]

COMMANDS:
    mcp                     Run MCP server over stdio
    serve                   Run HTTP REST API
    --version               Print version and exit
    --help                  Print this help and exit

MCP OPTIONS:
    --tool-prefix <name>    Rename exposed tools (e.g. 'mnema' for ENGRAM compat)
    --db <path>             Database file (default: ~/.seele/seele.db)

SERVE OPTIONS:
    --port <port>           Listen port (default: 7777)
    --bind <addr>           Bind address (default: 127.0.0.1)
    --legacy-engram-paths   Expose POST /save + GET /show/{id} aliases
    --auth-bearer <token>   Require Authorization: Bearer <token>
    --db <path>             Database file (default: ~/.seele/seele.db)

The full clap-based interface lands in Sprint-04.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("--help") | Some("-h") => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Some("--version") | Some("-V") => {
            println!("seele {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("mcp") => run_async(run_mcp(parse_mcp_args(&args[1..]))),
        Some("serve") => run_async(run_serve(parse_serve_args(&args[1..]))),
        Some(other) => {
            eprintln!("unknown command: {other}\n{HELP}");
            ExitCode::FAILURE
        }
    }
}

fn run_async(f: impl std::future::Future<Output = anyhow::Result<()>>) -> ExitCode {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("failed to start tokio runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    match rt.block_on(f) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("seele error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

// -------- mcp --------

struct McpArgs {
    tool_prefix: Option<String>,
    db: Option<PathBuf>,
}

fn parse_mcp_args(args: &[String]) -> McpArgs {
    let mut out = McpArgs {
        tool_prefix: None,
        db: None,
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--tool-prefix" => {
                out.tool_prefix = args.get(i + 1).cloned();
                i += 2;
            }
            "--db" => {
                out.db = args.get(i + 1).map(PathBuf::from);
                i += 2;
            }
            _ => i += 1,
        }
    }
    out
}

async fn run_mcp(args: McpArgs) -> anyhow::Result<()> {
    let service = build_service(args.db)?;
    let server = McpServer::new(
        service,
        McpServerConfig {
            tool_prefix: args.tool_prefix,
        },
    );
    server.run_stdio().await
}

// -------- serve --------

struct ServeArgs {
    port: u16,
    bind: String,
    legacy_engram_paths: bool,
    auth_bearer: Option<String>,
    db: Option<PathBuf>,
}

fn parse_serve_args(args: &[String]) -> ServeArgs {
    let mut out = ServeArgs {
        port: 7777,
        bind: "127.0.0.1".to_string(),
        legacy_engram_paths: false,
        auth_bearer: None,
        db: None,
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                if let Some(v) = args.get(i + 1).and_then(|s| s.parse().ok()) {
                    out.port = v;
                }
                i += 2;
            }
            "--bind" => {
                if let Some(v) = args.get(i + 1) {
                    out.bind = v.clone();
                }
                i += 2;
            }
            "--legacy-engram-paths" => {
                out.legacy_engram_paths = true;
                i += 1;
            }
            "--auth-bearer" => {
                out.auth_bearer = args.get(i + 1).cloned();
                i += 2;
            }
            "--db" => {
                out.db = args.get(i + 1).map(PathBuf::from);
                i += 2;
            }
            _ => i += 1,
        }
    }
    out
}

async fn run_serve(args: ServeArgs) -> anyhow::Result<()> {
    let service = build_service(args.db)?;
    let addr: SocketAddr = format!("{}:{}", args.bind, args.port).parse()?;
    let server = Server::new(
        service,
        ServerConfig {
            addr,
            cors_origins: vec![],
            auth_bearer: args.auth_bearer,
            legacy_engram_paths: args.legacy_engram_paths,
        },
    );
    server.run().await
}

// -------- helpers --------

fn build_service(db: Option<PathBuf>) -> anyhow::Result<SeeleService> {
    let path = db.unwrap_or_else(default_db_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let pool = init_db(&path)?;
    // FakeEmbedder for the v0.1 CLI quick-path. Sprint-04 wires the real
    // OnnxEmbedder behind a `--embedder` flag.
    let embedder: Arc<dyn Embedder> = Arc::new(FakeEmbedder);
    Ok(SeeleService::new(pool, embedder))
}

fn default_db_path() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".seele").join("seele.db")
    } else {
        PathBuf::from(".seele/seele.db")
    }
}
