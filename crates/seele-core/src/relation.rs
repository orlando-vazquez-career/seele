use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::id::SeeleId;

/// Memory relation — for conflict / judgment lifecycle (heredado de ENGRAM).
///
/// Distinct from `Link`: `MemoryRelation` carries a judgment status that goes
/// through pending → judged (with reason/evidence/confidence) → orphaned/ignored.
/// Used for "decision X is invalidated by decision Y" type relationships
/// where auditability matters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRelation {
    pub id: SeeleId,
    pub sync_id: String,
    pub source_id: SeeleId,
    pub target_id: SeeleId,
    pub relation: RelationKind,
    pub judgment_status: JudgmentStatus,
    pub reason: Option<String>,
    pub evidence: Option<String>,
    pub confidence: Option<f64>,
    pub marked_by_actor: Option<String>,
    pub marked_by_kind: Option<String>,
    pub marked_by_model: Option<String>,
    pub session_id: Option<SeeleId>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    Supersedes,
    ConflictsWith,
    Scoped,
    Related,
    Compatible,
    NotConflict,
}

impl RelationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Supersedes => "supersedes",
            Self::ConflictsWith => "conflicts_with",
            Self::Scoped => "scoped",
            Self::Related => "related",
            Self::Compatible => "compatible",
            Self::NotConflict => "not_conflict",
        }
    }

    pub fn from_str_strict(s: &str) -> Option<Self> {
        match s {
            "supersedes" => Some(Self::Supersedes),
            "conflicts_with" => Some(Self::ConflictsWith),
            "scoped" => Some(Self::Scoped),
            "related" => Some(Self::Related),
            "compatible" => Some(Self::Compatible),
            "not_conflict" => Some(Self::NotConflict),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JudgmentStatus {
    Pending,
    Judged,
    Orphaned,
    Ignored,
}

impl JudgmentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Judged => "judged",
            Self::Orphaned => "orphaned",
            Self::Ignored => "ignored",
        }
    }

    pub fn from_str_strict(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "judged" => Some(Self::Judged),
            "orphaned" => Some(Self::Orphaned),
            "ignored" => Some(Self::Ignored),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_kind_roundtrip() {
        let all = [
            RelationKind::Supersedes,
            RelationKind::ConflictsWith,
            RelationKind::Scoped,
            RelationKind::Related,
            RelationKind::Compatible,
            RelationKind::NotConflict,
        ];
        for r in all {
            let s = r.as_str();
            assert_eq!(RelationKind::from_str_strict(s), Some(r));
        }
    }

    #[test]
    fn judgment_status_roundtrip() {
        let all = [
            JudgmentStatus::Pending,
            JudgmentStatus::Judged,
            JudgmentStatus::Orphaned,
            JudgmentStatus::Ignored,
        ];
        for js in all {
            let s = js.as_str();
            assert_eq!(JudgmentStatus::from_str_strict(s), Some(js));
        }
    }

    #[test]
    fn unknown_strings_return_none() {
        assert_eq!(RelationKind::from_str_strict("nonsense"), None);
        assert_eq!(JudgmentStatus::from_str_strict("nonsense"), None);
    }
}
