use thiserror::Error;

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("storage error: {0}")]
    Storage(#[from] seele_storage::StorageError),

    #[error("embedder error: {0}")]
    Embedder(#[from] seele_embedder::EmbedderError),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("dimension mismatch: query embedding {query} != db embedding {db}")]
    DimensionMismatch { query: usize, db: usize },
}

pub type Result<T> = std::result::Result<T, SearchError>;
