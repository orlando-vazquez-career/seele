use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::error::{Result, SeeleError};

/// SEELE-wide identifier. Wraps `ulid::Ulid` for typed contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SeeleId(Ulid);

impl SeeleId {
    /// Generate a new ULID using the current time + random tail.
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    pub fn from_ulid(u: Ulid) -> Self {
        Self(u)
    }

    pub fn as_ulid(&self) -> Ulid {
        self.0
    }

    /// Maps the random tail (last 56 bits) of the ULID to a non-negative i64.
    /// Used to bridge ULID PK to vec0 INTEGER rowid in storage layer.
    ///
    /// The 80-bit random part of a ULID has 56 bits taken here (bytes 9..16);
    /// collision probability for 10^6 IDs is roughly 1 in 10^4 (birthday-bound).
    /// For SEELE's expected DB size (~100K observations) this is acceptable;
    /// when collisions occur on insert, the storage layer regenerates the ULID.
    pub fn as_i64(&self) -> i64 {
        let bytes = self.0.to_bytes();
        let mut int_bytes = [0u8; 8];
        // Top byte zeroed to keep value positive (sign bit clear).
        int_bytes[1..8].copy_from_slice(&bytes[9..16]);
        i64::from_be_bytes(int_bytes)
    }

    pub fn timestamp_ms(&self) -> u64 {
        self.0.timestamp_ms()
    }
}

impl Default for SeeleId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SeeleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SeeleId {
    type Err = SeeleError;

    fn from_str(s: &str) -> Result<Self> {
        Ulid::from_str(s)
            .map(Self)
            .map_err(|e| SeeleError::InvalidInput(format!("invalid ULID '{s}': {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_ids_unique() {
        let a = SeeleId::new();
        let b = SeeleId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn roundtrip_string() {
        let id = SeeleId::new();
        let s = id.to_string();
        let parsed: SeeleId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn invalid_string_errors() {
        let bad: Result<SeeleId> = "not-a-ulid".parse();
        assert!(bad.is_err());
    }

    #[test]
    fn as_i64_is_positive() {
        // Top byte cleared, so value is always non-negative.
        for _ in 0..100 {
            let id = SeeleId::new();
            assert!(id.as_i64() >= 0);
        }
    }

    #[test]
    fn as_i64_unique_in_batch() {
        // 10K fresh IDs should produce ~10K unique i64 mappings.
        // Birthday bound for 56 random bits is ~2^28 ≈ 268M before 50% collision;
        // 10K is far below that.
        let mut seen = std::collections::HashSet::new();
        for _ in 0..10_000 {
            let id = SeeleId::new();
            assert!(seen.insert(id.as_i64()), "i64 collision detected");
        }
    }

    #[test]
    fn as_i64_deterministic_for_same_ulid() {
        let id = SeeleId::new();
        let a = id.as_i64();
        let b = id.as_i64();
        assert_eq!(a, b);
    }
}
