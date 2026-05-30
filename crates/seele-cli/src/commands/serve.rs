use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Args as ClapArgs;
use seele_http::server::ChatProviderConfig;
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
    /// AI provider for the chat-with-DB endpoint (POST /chat).
    /// Supported: `minimax`, `openai`, `openrouter`, `together`, `groq`,
    /// `deepseek`, `anthropic`, or any other label (treated as
    /// OpenAI-compatible, needs --chat-endpoint).
    #[arg(long = "chat-provider", value_name = "NAME")]
    pub chat_provider: Option<String>,
    /// API key for the chat provider. Reads from the env var named here if
    /// the value starts with `$`, otherwise used literally. E.g.
    /// `--chat-key $MINIMAX_API_KEY`.
    #[arg(long = "chat-key", value_name = "KEY_OR_ENVVAR")]
    pub chat_key: Option<String>,
    /// Model name to use for chat completions. Defaults vary per provider.
    #[arg(long = "chat-model", value_name = "MODEL")]
    pub chat_model: Option<String>,
    /// Override the OpenAI-compatible chat completions URL. Ignored for
    /// `anthropic`. If omitted, a sensible default per --chat-provider is
    /// used.
    #[arg(long = "chat-endpoint", value_name = "URL")]
    pub chat_endpoint: Option<String>,
}

pub async fn run(args: Args, db: &Option<PathBuf>, fake_embedder: bool) -> anyhow::Result<()> {
    let svc = crate::app::build_service(db, fake_embedder)?;
    let addr: SocketAddr = format!("{}:{}", args.bind, args.port).parse()?;

    let chat = match (args.chat_provider, args.chat_key) {
        (Some(provider), Some(key_or_env)) => {
            let api_key = resolve_key(&key_or_env)?;
            let model = args
                .chat_model
                .unwrap_or_else(|| default_model_for(&provider));
            Some(ChatProviderConfig {
                provider,
                api_key,
                model,
                endpoint: args.chat_endpoint,
            })
        }
        (None, None) => None,
        _ => {
            anyhow::bail!("--chat-provider and --chat-key must be set together (or both omitted)");
        }
    };

    Server::new(
        svc,
        ServerConfig {
            addr,
            cors_origins: args.cors_allow,
            auth_bearer: args.auth_bearer,
            legacy_engram_paths: args.legacy_engram_paths,
            chat,
        },
    )
    .run()
    .await
}

fn resolve_key(raw: &str) -> anyhow::Result<String> {
    if let Some(envvar) = raw.strip_prefix('$') {
        std::env::var(envvar)
            .map_err(|_| anyhow::anyhow!("chat key references env var ${envvar} but it is not set"))
    } else {
        Ok(raw.to_string())
    }
}

fn default_model_for(provider: &str) -> String {
    match provider.to_ascii_lowercase().as_str() {
        "minimax" => "MiniMax-M2".to_string(),
        "openai" => "gpt-4o-mini".to_string(),
        "openrouter" => "openai/gpt-4o-mini".to_string(),
        "together" => "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo".to_string(),
        "groq" => "llama-3.3-70b-versatile".to_string(),
        "deepseek" => "deepseek-chat".to_string(),
        "anthropic" => "claude-haiku-4-5-20251001".to_string(),
        _ => "gpt-4o-mini".to_string(),
    }
}
