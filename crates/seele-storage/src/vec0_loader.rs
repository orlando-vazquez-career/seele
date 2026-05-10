//! Vendored sqlite-vec (`vec0`) loader.
//!
//! At build time, the matching binary for the host target is embedded into
//! the crate via `include_bytes!`. At runtime, callers write the bytes to a
//! temporary file and call `rusqlite::Connection::load_extension` against it.
//!
//! Source: <https://github.com/asg017/sqlite-vec> v0.1.9 (MIT, Alex Garcia).
//! See `vendor/sqlite-vec/README.md` for licence + provenance + checksums.

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const VEC0_BYTES: Option<&[u8]> = Some(include_bytes!("../vendor/sqlite-vec/linux-x86_64/vec0.so"));

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const VEC0_BYTES: Option<&[u8]> =
    Some(include_bytes!("../vendor/sqlite-vec/linux-aarch64/vec0.so"));

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const VEC0_BYTES: Option<&[u8]> = Some(include_bytes!(
    "../vendor/sqlite-vec/macos-x86_64/vec0.dylib"
));

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const VEC0_BYTES: Option<&[u8]> = Some(include_bytes!(
    "../vendor/sqlite-vec/macos-aarch64/vec0.dylib"
));

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const VEC0_BYTES: Option<&[u8]> = Some(include_bytes!(
    "../vendor/sqlite-vec/windows-x86_64/vec0.dll"
));

#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "windows", target_arch = "x86_64"),
)))]
const VEC0_BYTES: Option<&[u8]> = None;

/// Bytes of the vendored `vec0` loadable extension for the current target.
///
/// Returns `None` for targets we don't ship a binary for (e.g. android, iOS,
/// 32-bit linux). In that case callers should surface a clear error.
pub fn vec0_bytes() -> Option<&'static [u8]> {
    VEC0_BYTES
}

/// File extension for the loadable on the current OS.
///
/// SQLite's `load_extension` infers nothing from the filename, but the OS
/// loader needs the correct suffix for `dlopen` / `LoadLibrary` to succeed.
pub fn vec0_extension_suffix() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        ".so"
    }
    #[cfg(target_os = "macos")]
    {
        ".dylib"
    }
    #[cfg(target_os = "windows")]
    {
        ".dll"
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec0_bytes_present_on_known_targets() {
        if let Some(bytes) = vec0_bytes() {
            assert!(!bytes.is_empty(), "vendored vec0 must not be empty");
            assert!(bytes.len() > 1024, "vec0 binary suspiciously small");
        }
    }

    #[test]
    fn extension_suffix_is_known() {
        let sfx = vec0_extension_suffix();
        assert!(matches!(sfx, ".so" | ".dylib" | ".dll" | ""));
    }
}
