use thiserror::Error;

#[derive(Debug, Error)]
pub enum SeeleError {
    #[error("storage error: {0}")]
    Storage(String),

    #[error("embedder error: {0}")]
    Embedder(String),

    #[error("search error: {0}")]
    Search(String),

    #[error("mcp error: {0}")]
    Mcp(String),

    #[error("http error: {0}")]
    Http(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, SeeleError>;
