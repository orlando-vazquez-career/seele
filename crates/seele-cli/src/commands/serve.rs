use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Args as ClapArgs;
use seele_http::{Server, ServerConfig};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Listen port. `0` lets the OS pick (binding announced via stderr).
    #[arg(long, default_value_t = 7777)]
    pub port: u16,
    /// Bind address.
    #[arg(long, default_value = "127.0.0.1")]
    pub bind: String,
    /// Expose ENGRAM-compatible path aliases per ADR-13.
    #[arg(long)]
    pub legacy_engram_paths: bool,
    /// Require `Authorization: Bearer <token>` for non-public routes.
    #[arg(long)]
    pub auth_bearer: Option<String>,
    /// Enable CORS for the given origin. Repeatable for multiple origins.
    /// Empty = CORS disabled (default; safe for local-only use). Today any
    /// non-empty value enables permissive `Access-Control-Allow-Origin: *`;
    /// per-origin allowlist refinement is on the backlog.
    #[arg(long = "cors-allow", value_name = "ORIGIN")]
    pub cors_allow: Vec<String>,
}

pub async fn run(args: Args, db: &Option<PathBuf>, fake_embedder: bool) -> anyhow::Result<()> {
    let svc = crate::app::build_service(db, fake_embedder)?;
    let addr: SocketAddr = format!("{}:{}", args.bind, args.port).parse()?;
    Server::new(
        svc,
        ServerConfig {
            addr,
            cors_origins: args.cors_allow,
            auth_bearer: args.auth_bearer,
            legacy_engram_paths: args.legacy_engram_paths,
        },
    )
    .run()
    .await
}
