//! r2d2 connection pool over rusqlite with vec0 extension preloaded.

use std::path::{Path, PathBuf};
use std::time::Duration;

use r2d2_sqlite::SqliteConnectionManager;

use crate::error::{Result, StorageError};
use crate::vec0_install;

/// Per-connection lock wait (T-01): SQLite retries SQLITE_BUSY internally
/// for up to this long before surfacing "database is locked". Only takes
/// effect for connections that begin as writers — a deferred tx upgraded
/// read→write mid-flight is refused BUSY without the busy handler.
pub const BUSY_TIMEOUT_MS: u32 = 5000;

/// Backoff before the single write-path retry on SQLITE_BUSY. The
/// busy_timeout above already absorbed lock waits shorter than itself;
/// this pause only runs when a writer held the lock past the timeout and
/// the whole operation is replayed on a fresh checkout.
const BUSY_RETRY_BACKOFF: Duration = Duration::from_millis(50);

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
             PRAGMA temp_store=MEMORY;
             PRAGMA busy_timeout=5000;",
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

/// Run `op` once, retrying a single time after a short backoff when it
/// fails with SQLITE_BUSY ("database is locked"). Write-path safety net
/// over `PRAGMA busy_timeout`, for the case the timeout cannot absorb: a
/// writer holding the lock longer than BUSY_TIMEOUT_MS.
pub(crate) fn with_busy_retry<T>(mut op: impl FnMut() -> Result<T>) -> Result<T> {
    match op() {
        Err(StorageError::Sqlite(e)) if is_busy(&e) => {
            std::thread::sleep(BUSY_RETRY_BACKOFF);
            op()
        }
        other => other,
    }
}

/// True for SQLITE_BUSY regardless of the extended code (plain BUSY and
/// BUSY_SNAPSHOT alike).
fn is_busy(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(e, _) if e.code == rusqlite::ErrorCode::DatabaseBusy
    )
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

        let busy_timeout: i64 = conn
            .query_row("PRAGMA busy_timeout", [], |r| r.get(0))
            .unwrap();
        assert_eq!(busy_timeout, i64::from(BUSY_TIMEOUT_MS));
    }

    fn busy_err() -> StorageError {
        StorageError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
            Some("database is locked".into()),
        ))
    }

    #[test]
    fn busy_retry_recovers_from_single_busy_error() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let attempts = AtomicUsize::new(0);
        let out = with_busy_retry(|| {
            if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(busy_err())
            } else {
                Ok(7)
            }
        })
        .unwrap();
        assert_eq!(out, 7);
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn busy_retry_gives_up_after_one_retry() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let attempts = AtomicUsize::new(0);
        let err = with_busy_retry(|| -> Result<()> {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err(busy_err())
        })
        .unwrap_err();
        assert!(matches!(err, StorageError::Sqlite(_)));
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            2,
            "retry-once means exactly two attempts"
        );
    }

    #[test]
    fn busy_retry_does_not_retry_other_errors() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let attempts = AtomicUsize::new(0);
        let err = with_busy_retry(|| -> Result<()> {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err(StorageError::Conflict("nope".into()))
        })
        .unwrap_err();
        assert!(matches!(err, StorageError::Conflict(_)));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
}
