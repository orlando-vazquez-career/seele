//! SEELE storage — SQLite + FTS5 + sqlite-vec layer.

pub mod error;
pub mod migrations;
pub mod pool;
pub mod vec0_install;
pub mod vec0_loader;

pub use error::{Result, StorageError};
pub use pool::{init_pool, Pool, PoolConfig};

use std::path::Path;

/// Open a DB at `path`, applying canonical PRAGMAs, loading vec0, and
/// running pending migrations. Idempotent.
pub fn init_db(path: impl AsRef<Path>) -> Result<Pool> {
    let pool = init_pool(PoolConfig::with_path(path))?;
    let mut conn = pool.get()?;
    migrations::run_pending(&mut conn)?;
    Ok(pool)
}
