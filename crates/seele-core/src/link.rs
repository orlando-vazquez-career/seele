use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::id::SeeleId;
use crate::metadata::Metadata;

/// General-purpose link between two observations (graph edge, no judgment lifecycle).
/// For invalidation/conflict relationships with auditing, use `MemoryRelation` instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub id: SeeleId,
    pub from_id: SeeleId,
    pub to_id: SeeleId,
    pub link_type: String,
    pub metadata: Metadata,
    pub created_at: DateTime<Utc>,
}

/// Recommended link types — TEXT is free-form, these are conventions.
pub mod link_types {
    pub const DERIVES_FROM: &str = "derives_from";
    pub const SUPERSEDES: &str = "supersedes";
    pub const RELATED_TO: &str = "related_to";
    pub const CONTRADICTS: &str = "contradicts";
    pub const EVIDENCE_FOR: &str = "evidence_for";
    pub const PART_OF_VERDICT: &str = "part_of_verdict";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_type_constants_are_snake_case() {
        for s in [
            link_types::DERIVES_FROM,
            link_types::SUPERSEDES,
            link_types::RELATED_TO,
            link_types::CONTRADICTS,
            link_types::EVIDENCE_FOR,
            link_types::PART_OF_VERDICT,
        ] {
            assert!(s.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
            assert!(!s.starts_with('_'));
            assert!(!s.ends_with('_'));
        }
    }
}
