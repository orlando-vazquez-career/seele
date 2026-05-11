//! Property tests for `seele-sync`.
//!
//! `compute_chunk_id` is the function that decides whether two chunks
//! represent the same observation set. These properties pin down its
//! contract:
//! - Order of observations in the payload is irrelevant.
//! - `exported_at` and `seele_version` (informational fields) do not
//!   influence the id.
//! - Any change to an observation's content changes the id.

use chrono::{Duration, Utc};
use proptest::prelude::*;
use seele_core::id::SeeleId;
use seele_core::memory::{Observation, ObservationType, Scope};
use seele_core::metadata::Metadata;
use seele_sync::{compute_chunk_id, ChunkPayload};

fn fake_observation(idx: usize) -> Observation {
    let now = Utc::now();
    Observation {
        id: SeeleId::new(),
        session_id: None,
        kind: ObservationType::Memory,
        title: format!("title_{idx}"),
        content: format!("content_{idx}"),
        tool_name: None,
        project: Some("p".to_string()),
        scope: Scope::Project,
        topic_key: None,
        normalized_hash: None,
        revision_count: 0,
        duplicate_count: 0,
        last_seen_at: now,
        created_at: now,
        updated_at: now,
        deleted_at: None,
        metadata: Metadata::new(),
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 32,
        ..ProptestConfig::default()
    })]

    #[test]
    fn chunk_id_ignores_observation_order(n in 1usize..10usize) {
        let obs: Vec<Observation> = (0..n).map(fake_observation).collect();
        let p1 = ChunkPayload {
            format_version: 1,
            seele_version: "0.0.99".to_string(),
            exported_at: Utc::now(),
            project: Some("p".to_string()),
            observations: obs.clone(),
        };
        let mut p2 = p1.clone();
        p2.observations.reverse();
        prop_assert_eq!(
            compute_chunk_id(&p1).unwrap(),
            compute_chunk_id(&p2).unwrap(),
        );
    }

    #[test]
    fn chunk_id_ignores_exported_at_and_version(
        n in 1usize..10usize,
        delta_secs in 1i64..(86_400 * 30),
        v1 in "[0-9]\\.[0-9]\\.[0-9]",
        v2 in "[0-9]\\.[0-9]\\.[0-9]",
    ) {
        let obs: Vec<Observation> = (0..n).map(fake_observation).collect();
        let t1 = Utc::now();
        let t2 = t1 + Duration::seconds(delta_secs);
        let p1 = ChunkPayload {
            format_version: 1,
            seele_version: v1,
            exported_at: t1,
            project: None,
            observations: obs.clone(),
        };
        let p2 = ChunkPayload {
            format_version: 1,
            seele_version: v2,
            exported_at: t2,
            project: None,
            observations: obs,
        };
        prop_assert_eq!(
            compute_chunk_id(&p1).unwrap(),
            compute_chunk_id(&p2).unwrap(),
        );
    }

    #[test]
    fn chunk_id_changes_when_observation_content_changes(
        n in 1usize..10usize,
        new_title in "[a-z]{5,20}",
    ) {
        let obs: Vec<Observation> = (0..n).map(fake_observation).collect();
        let p1 = ChunkPayload {
            format_version: 1,
            seele_version: "0.0.99".to_string(),
            exported_at: Utc::now(),
            project: None,
            observations: obs,
        };
        let mut p2 = p1.clone();
        // Force a real change: only mutate if the new title isn't the
        // existing one — the regex generator is broad enough that
        // collisions are vanishingly rare, but the guard makes the
        // invariant unconditional.
        let first = p2.observations.first_mut().unwrap();
        if first.title == new_title {
            first.title.push('!');
        } else {
            first.title = new_title;
        }
        prop_assert_ne!(
            compute_chunk_id(&p1).unwrap(),
            compute_chunk_id(&p2).unwrap(),
        );
    }

    #[test]
    fn chunk_id_changes_when_project_filter_changes(n in 1usize..6usize) {
        let obs: Vec<Observation> = (0..n).map(fake_observation).collect();
        let p1 = ChunkPayload {
            format_version: 1,
            seele_version: "0.0.99".to_string(),
            exported_at: Utc::now(),
            project: None,
            observations: obs.clone(),
        };
        let p2 = ChunkPayload {
            project: Some("p".to_string()),
            ..p1.clone()
        };
        prop_assert_ne!(
            compute_chunk_id(&p1).unwrap(),
            compute_chunk_id(&p2).unwrap(),
        );
    }
}
