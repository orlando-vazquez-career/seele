//! Runtime installer for the vendored `vec0` extension.
//!
//! At runtime, when SEELE first opens a DB, it resolves a stable cache path
//! for the loadable extension and writes the embedded bytes there if not
//! already present (idempotent + integrity-checked).
//!
//! Override: env var `SEELE_VEC_PATH` forces use of an external loadable.
//! Useful for unsupported targets or debugging.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{Result, StorageError};
use crate::vec0_loader;

const VEC0_ENV_OVERRIDE: &str = "SEELE_VEC_PATH";
const SHA_PREFIX_LEN: usize = 16;

/// Resolve the path to a usable `vec0` loadable on disk.
///
/// Order of precedence:
/// 1. `$SEELE_VEC_PATH` env var, if set and the file exists.
/// 2. Vendored binary written to `<user-cache>/seele/vec0-<sha>.<suffix>`.
/// 3. Error `Vec0NotSupportedTarget` if target has no vendored binary
///    and override is not set.
pub fn ensure_vec0_extension() -> Result<PathBuf> {
    if let Ok(override_path) = env::var(VEC0_ENV_OVERRIDE) {
        let p = PathBuf::from(&override_path);
        if p.is_file() {
            return Ok(p);
        }
        return Err(StorageError::InvalidInput(format!(
            "{VEC0_ENV_OVERRIDE} set to '{override_path}' but file does not exist"
        )));
    }

    let bytes = vec0_loader::vec0_bytes().ok_or(StorageError::Vec0NotSupportedTarget {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
    })?;

    let cache_dir = resolve_cache_dir()?;
    fs::create_dir_all(&cache_dir)?;

    let sha = sha256_prefix(bytes);
    let suffix = vec0_loader::vec0_extension_suffix();
    let target_path = cache_dir.join(format!("vec0-{sha}{suffix}"));

    if target_path.is_file() {
        verify_integrity(&target_path, &sha)?;
        return Ok(target_path);
    }

    write_atomic(&target_path, bytes)?;
    Ok(target_path)
}

fn resolve_cache_dir() -> Result<PathBuf> {
    let base = dirs::cache_dir().ok_or(StorageError::Vec0CacheUnresolvable)?;
    Ok(base.join("seele"))
}

fn sha256_prefix(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex = hex::encode(digest);
    hex.chars().take(SHA_PREFIX_LEN).collect()
}

fn write_atomic(target: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = target.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, target)?;
    Ok(())
}

fn verify_integrity(path: &Path, expected_prefix: &str) -> Result<()> {
    let bytes = fs::read(path)?;
    let actual = sha256_prefix(&bytes);
    if actual != expected_prefix {
        return Err(StorageError::Vec0IntegrityMismatch {
            path: path.display().to_string(),
            expected: expected_prefix.to_string(),
            actual,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn sha256_prefix_is_stable_and_short() {
        let a = sha256_prefix(b"hello");
        let b = sha256_prefix(b"hello");
        assert_eq!(a, b);
        assert_eq!(a.len(), SHA_PREFIX_LEN);
    }

    #[test]
    fn write_atomic_writes_and_renames() {
        let td = TempDir::new().unwrap();
        let target = td.path().join("subdir").join("file.bin");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        write_atomic(&target, b"hello world").unwrap();
        let got = fs::read(&target).unwrap();
        assert_eq!(got, b"hello world");
    }

    #[test]
    fn verify_integrity_rejects_mismatch() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("vec0.bin");
        fs::write(&p, b"contents").unwrap();
        let real = sha256_prefix(b"contents");
        let fake = "0".repeat(SHA_PREFIX_LEN);
        verify_integrity(&p, &real).unwrap();
        let err = verify_integrity(&p, &fake).unwrap_err();
        assert!(matches!(err, StorageError::Vec0IntegrityMismatch { .. }));
    }

    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
    ))]
    #[test]
    fn ensure_vec0_extension_returns_existing_file() {
        // We don't override the cache dir here — the function uses dirs::cache_dir().
        // On CI runners this works (HOME is set). The test is gated to supported
        // targets so vec0_bytes() returns Some.
        let path = ensure_vec0_extension().expect("ensure_vec0_extension must succeed");
        assert!(
            path.is_file(),
            "returned path must exist: {}",
            path.display()
        );
        assert!(
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("vec0-")),
            "filename must follow vec0-<sha> pattern: {}",
            path.display()
        );
    }
}
