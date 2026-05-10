use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::id::SeeleId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SeeleId,
    pub project: String,
    pub directory: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub summary: Option<String>,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Active,
    Ended,
    Aborted,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Ended => "ended",
            Self::Aborted => "aborted",
        }
    }

    pub fn from_str_strict(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "ended" => Some(Self::Ended),
            "aborted" => Some(Self::Aborted),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_strict_parse() {
        assert_eq!(
            SessionStatus::from_str_strict("active"),
            Some(SessionStatus::Active)
        );
        assert_eq!(
            SessionStatus::from_str_strict("ended"),
            Some(SessionStatus::Ended)
        );
        assert_eq!(
            SessionStatus::from_str_strict("aborted"),
            Some(SessionStatus::Aborted)
        );
        assert_eq!(SessionStatus::from_str_strict("nonsense"), None);
    }

    #[test]
    fn status_serde_roundtrip() {
        let s = SessionStatus::Ended;
        let j = serde_json::to_string(&s).unwrap();
        assert_eq!(j, "\"ended\"");
        let back: SessionStatus = serde_json::from_str(&j).unwrap();
        assert_eq!(back, s);
    }
}
