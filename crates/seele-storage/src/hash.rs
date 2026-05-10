//! Content normalization + sha256 hash for dedup detection.
//!
//! The dedup window in ENGRAM is `(normalized_hash, project, scope, type, title)`
//! within ~24h. We compute the normalized hash here; the time/scope check
//! belongs to the upsert layer.

use sha2::{Digest, Sha256};

/// Compute a stable normalized hash over `content`:
/// 1. Lowercase
/// 2. Collapse whitespace runs to single spaces
/// 3. Trim leading/trailing whitespace
/// 4. sha256 hex (lowercase)
///
/// The transform is intentionally aggressive: small reformat-only edits
/// to the same content should hash identically and be merged as duplicates.
pub fn normalized_hash(content: &str) -> String {
    let normalized = normalize(content);
    let digest = Sha256::digest(normalized.as_bytes());
    hex::encode(digest)
}

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_content_same_hash() {
        let a = normalized_hash("Hello world");
        let b = normalized_hash("Hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn whitespace_normalized() {
        let a = normalized_hash("hello   world");
        let b = normalized_hash("hello world");
        let c = normalized_hash("hello\t\nworld");
        assert_eq!(a, b);
        assert_eq!(b, c);
    }

    #[test]
    fn case_normalized() {
        let a = normalized_hash("HELLO World");
        let b = normalized_hash("hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn leading_trailing_trimmed() {
        let a = normalized_hash("  hello world  ");
        let b = normalized_hash("hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn different_content_different_hash() {
        let a = normalized_hash("hello world");
        let b = normalized_hash("hello world!");
        assert_ne!(a, b);
    }

    #[test]
    fn output_is_hex_64_chars() {
        let h = normalized_hash("anything");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
