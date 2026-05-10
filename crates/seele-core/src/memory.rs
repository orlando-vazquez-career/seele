use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::id::SeeleId;
use crate::metadata::Metadata;

/// Observation = a single memory unit. The core entity of SEELE.
///
/// Inherits ENGRAM's data shape (sessions, type, scope, topic_key, normalized_hash,
/// revision_count, duplicate_count) and adds vector embedding via `sqlite-vec`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: SeeleId,
    pub session_id: Option<SeeleId>,
    #[serde(rename = "type")]
    pub kind: ObservationType,
    pub title: String,
    pub content: String,
    pub tool_name: Option<String>,
    pub project: Option<String>,
    pub scope: Scope,
    pub topic_key: Option<String>,
    pub normalized_hash: Option<String>,
    pub revision_count: u32,
    pub duplicate_count: u32,
    pub last_seen_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub metadata: Metadata,
}

/// Observation type — extensible. Stored as TEXT in SQLite (no enum CHECK).
///
/// First 7 are ENGRAM-inherited (coding agents); next 5 are MNEMA-extended.
/// Consumers may use `Other(String)` for custom types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationType {
    Decision,
    Architecture,
    Bugfix,
    Pattern,
    Config,
    Discovery,
    Learning,
    Memory,
    Skill,
    AdvisorOutput,
    Review,
    Verdict,
    #[serde(untagged)]
    Other(String),
}

impl ObservationType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Decision => "decision",
            Self::Architecture => "architecture",
            Self::Bugfix => "bugfix",
            Self::Pattern => "pattern",
            Self::Config => "config",
            Self::Discovery => "discovery",
            Self::Learning => "learning",
            Self::Memory => "memory",
            Self::Skill => "skill",
            Self::AdvisorOutput => "advisor_output",
            Self::Review => "review",
            Self::Verdict => "verdict",
            Self::Other(s) => s.as_str(),
        }
    }

    /// Parse a string to `ObservationType`. Falls back to `Other(s)` for unknown values.
    pub fn from_str_relaxed(s: &str) -> Self {
        match s {
            "decision" => Self::Decision,
            "architecture" => Self::Architecture,
            "bugfix" => Self::Bugfix,
            "pattern" => Self::Pattern,
            "config" => Self::Config,
            "discovery" => Self::Discovery,
            "learning" => Self::Learning,
            "memory" => Self::Memory,
            "skill" => Self::Skill,
            "advisor_output" => Self::AdvisorOutput,
            "review" => Self::Review,
            "verdict" => Self::Verdict,
            other => Self::Other(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    #[default]
    Project,
    Personal,
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Personal => "personal",
        }
    }

    pub fn from_str_strict(s: &str) -> Option<Self> {
        match s {
            "project" => Some(Self::Project),
            "personal" => Some(Self::Personal),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_type_roundtrip_known() {
        let known = [
            ("decision", ObservationType::Decision),
            ("architecture", ObservationType::Architecture),
            ("bugfix", ObservationType::Bugfix),
            ("pattern", ObservationType::Pattern),
            ("config", ObservationType::Config),
            ("discovery", ObservationType::Discovery),
            ("learning", ObservationType::Learning),
            ("memory", ObservationType::Memory),
            ("skill", ObservationType::Skill),
            ("advisor_output", ObservationType::AdvisorOutput),
            ("review", ObservationType::Review),
            ("verdict", ObservationType::Verdict),
        ];
        for (s, expected) in known {
            let parsed = ObservationType::from_str_relaxed(s);
            assert_eq!(parsed, expected);
            assert_eq!(parsed.as_str(), s);
        }
    }

    #[test]
    fn observation_type_other_passthrough() {
        let other = ObservationType::from_str_relaxed("custom_kind");
        assert_eq!(other.as_str(), "custom_kind");
        match other {
            ObservationType::Other(s) => assert_eq!(s, "custom_kind"),
            _ => panic!("expected Other"),
        }
    }

    #[test]
    fn scope_default_is_project() {
        let s: Scope = Default::default();
        assert_eq!(s, Scope::Project);
    }

    #[test]
    fn scope_strict_parse() {
        assert_eq!(Scope::from_str_strict("project"), Some(Scope::Project));
        assert_eq!(Scope::from_str_strict("personal"), Some(Scope::Personal));
        assert_eq!(Scope::from_str_strict("nonsense"), None);
    }
}
