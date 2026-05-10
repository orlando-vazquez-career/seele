//! r2d2 connection pool over rusqlite with vec0 extension preloaded.

use std::path::{Path, PathBuf};

use r2d2_sqlite::SqliteConnectionManager;

use crate::error::{Result, StorageError};
use crate::vec0_install;

pub type Pool = r2d2::Pool<SqliteConnectionManager>;

#[derive(Debug, Clone)]
pub struct PoolConfig {
    pub path: PathBuf,
    pub max_size: u32,
}

impl PoolConfig {
    pub fn with_path(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            max_size: 8,
        }
    }
}

/// Build a pool that, on every checked-out connection, applies the canonical
/// PRAGMAs and loads the vec0 extension. Failure to load the extension is
/// fatal — SEELE depends on hybrid search.
pub fn init_pool(cfg: PoolConfig) -> Result<Pool> {
    let vec0_path = vec0_install::ensure_vec0_extension()?;

    let manager = SqliteConnectionManager::file(&cfg.path).with_init(move |conn| {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             PRAGMA temp_store=MEMORY;",
        )?;
        load_vec0(conn, &vec0_path)
    });

    let pool = r2d2::Pool::builder()
        .max_size(cfg.max_size)
        .build(manager)
        .map_err(StorageError::Pool)?;
    Ok(pool)
}

fn load_vec0(conn: &rusqlite::Connection, path: &Path) -> rusqlite::Result<()> {
    // SAFETY: we trust the vendored binary; load_extension is unsafe in
    // rusqlite because arbitrary native code can be loaded.
    unsafe {
        let _guard = rusqlite::LoadExtensionGuard::new(conn)?;
        // Explicit entry point: `vec0` is the loadable filename, but the
        // exported init function in sqlite-vec is `sqlite3_vec_init` (no
        // `0` suffix). Using `None` would compute the wrong symbol.
        conn.load_extension(path, Some("sqlite3_vec_init"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
    ))]
    #[test]
    fn pool_loads_vec0_and_pragmas() {
        let td = TempDir::new().unwrap();
        let cfg = PoolConfig::with_path(td.path().join("seele.db"));
        let pool = init_pool(cfg).expect("pool init");
        let conn = pool.get().expect("checkout");

        // vec_version() must respond if the extension loaded.
        let v: String = conn
            .query_row("SELECT vec_version()", [], |r| r.get(0))
            .expect("vec_version available");
        assert!(!v.is_empty(), "vec_version returned empty: {v}");

        // PRAGMAs must be set.
        let journal: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(journal.to_lowercase(), "wal");

        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }
}
