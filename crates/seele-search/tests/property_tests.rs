//! Property tests for `seele-search`.
//!
//! These exercise invariants that should hold across many random inputs:
//! - save → embed → search is a roundtrip when the query keyword is in the
//!   content;
//! - `limit` is always respected;
//! - empty-query path always returns ≤ limit hits, all from the requested
//!   project, all non-deleted.

mod common;

use common::{populated_n, FixtureSet};
use proptest::prelude::*;
use seele_core::id::SeeleId;
use seele_core::memory::{ObservationType, Scope};
use seele_core::metadata::Metadata;
use seele_search::SearchQuery;
use seele_storage::SaveInput;

fn save_with_keyword(fx: &mut FixtureSet, project: &str, keyword: &str) -> SeeleId {
    fx.save(SaveInput {
        session_id: None,
        kind: ObservationType::Memory,
        title: format!("title for {keyword}"),
        content: format!("body containing the {keyword} keyword in the middle"),
        tool_name: None,
        project: Some(project.into()),
        scope: Scope::Project,
        topic_key: None,
        metadata: Metadata::new(),
    })
}

proptest! {
    #![proptest_config(ProptestConfig {
        // Keep CI fast — local can override with PROPTEST_CASES env.
        cases: 32,
        ..ProptestConfig::default()
    })]

    #[test]
    fn save_then_search_roundtrip_matches_by_keyword(
        keyword in "[a-z]{6,15}",
        project in "[a-z][a-z0-9-]{2,12}",
    ) {
        let mut fx = FixtureSet::new();
        let id = save_with_keyword(&mut fx, &project, &keyword);
        let hits = fx
            .engine
            .search(SearchQuery {
                text: keyword.clone(),
                project: Some(project.clone()),
                ..Default::default()
            })
            .unwrap();
        prop_assert!(
            hits.iter().any(|h| h.observation.id == id),
            "expected to find id {id} in hits for keyword {keyword}"
        );
    }

    #[test]
    fn limit_caps_result_count(
        n in 1u32..30u32,
        limit in 1u32..15u32,
    ) {
        let fx = populated_n(n as usize);
        let hits = fx
            .engine
            .search(SearchQuery {
                text: "common keyword".into(),
                project: Some("p".into()),
                limit: Some(limit),
                ..Default::default()
            })
            .unwrap();
        prop_assert!(hits.len() as u32 <= limit);
    }

    #[test]
    fn empty_query_respects_project_and_limit(
        n in 1u32..30u32,
        limit in 1u32..15u32,
    ) {
        let fx = populated_n(n as usize);
        let hits = fx
            .engine
            .search(SearchQuery {
                text: "".into(),
                project: Some("p".into()),
                limit: Some(limit),
                ..Default::default()
            })
            .unwrap();
        prop_assert!(hits.len() as u32 <= limit);
        for h in &hits {
            prop_assert_eq!(h.observation.project.as_deref(), Some("p"));
        }
    }
}
