use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("connection pool error: {0}")]
    Pool(#[from] r2d2::Error),

    #[error("migration error: {0}")]
    Migration(#[from] refinery::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("vec0 vendored binary not available for current target ({arch} on {os})")]
    Vec0NotSupportedTarget {
        os: &'static str,
        arch: &'static str,
    },

    #[error("vec0 cache path could not be resolved (no user cache dir)")]
    Vec0CacheUnresolvable,

    #[error("vec0 vendored binary failed integrity check at {path}: expected sha256 prefix {expected}, got {actual}")]
    Vec0IntegrityMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("conflict: {0}")]
    Conflict(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;
