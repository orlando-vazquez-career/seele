use serde::{Deserialize, Serialize};

use crate::memory::Scope;

/// Filter for observation queries. The storage layer translates this to SQL WHERE clauses.
///
/// `include_purist` defaults to `false` — `metadata.context_mode = 'purist'` outputs
/// are excluded from Recall by default. See ADR-10 (mapping-mnema-seele) capa 4.5.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataFilter {
    pub kind: Option<String>,
    pub project: Option<String>,
    pub scope: Option<Scope>,
    pub topic_key: Option<String>,
    pub axiomatic: Option<bool>,
    #[serde(default)]
    pub include_deleted: bool,
    #[serde(default)]
    pub include_purist: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationQuery {
    pub query: Option<String>,
    pub filter: MetadataFilter,
    pub limit: u32,
    pub offset: u32,
}

impl Default for ObservationQuery {
    fn default() -> Self {
        Self {
            query: None,
            filter: MetadataFilter::default(),
            limit: 10,
            offset: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_defaults_excludes_deleted_and_purist() {
        let f = MetadataFilter::default();
        assert!(!f.include_deleted);
        assert!(!f.include_purist);
    }

    #[test]
    fn query_default_limit_10() {
        let q = ObservationQuery::default();
        assert_eq!(q.limit, 10);
        assert_eq!(q.offset, 0);
        assert!(q.query.is_none());
    }

    #[test]
    fn filter_serde_roundtrip() {
        let f = MetadataFilter {
            kind: Some("decision".into()),
            project: Some("dev-zen".into()),
            scope: Some(Scope::Project),
            topic_key: None,
            axiomatic: Some(true),
            include_deleted: false,
            include_purist: false,
        };
        let s = serde_json::to_string(&f).unwrap();
        let back: MetadataFilter = serde_json::from_str(&s).unwrap();
        assert_eq!(back.kind, f.kind);
        assert_eq!(back.project, f.project);
        assert_eq!(back.scope, f.scope);
        assert_eq!(back.axiomatic, f.axiomatic);
    }
}
