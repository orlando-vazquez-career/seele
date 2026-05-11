//! Property tests for the RRF combiner (`seele_search::rrf::combine`).
//!
//! Invariants we pin down (derived from the Sprint-05 plan):
//! - Every output hit has `score >= 0`.
//! - Output IDs equal the deduplicated union of the input IDs.
//! - Output length equals `|union(inputs)|`.
//! - Output is sorted by descending score.
//! - A document appearing in N sources has `per_source.len() == N`.

use std::collections::HashSet;

use proptest::prelude::*;
use seele_search::rrf::{combine, DEFAULT_K};

/// Generate a single ranking — a vec of unique `usize` ids with length
/// up to `max_len`. We keep the universe small (0..32) so that two
/// independent rankings have a good chance of overlapping.
fn ranking_strategy(max_len: usize) -> impl Strategy<Value = Vec<usize>> {
    proptest::collection::vec(0usize..32usize, 0..=max_len).prop_map(|v| {
        // dedup while preserving first-seen order
        let mut seen = HashSet::new();
        v.into_iter().filter(|x| seen.insert(*x)).collect()
    })
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 32,
        ..ProptestConfig::default()
    })]

    #[test]
    fn scores_are_non_negative(
        fts in ranking_strategy(12),
        vec in ranking_strategy(12),
    ) {
        let out = combine(&[("fts", fts), ("vec", vec)], DEFAULT_K);
        for h in &out {
            prop_assert!(h.score >= 0.0, "expected score >= 0, got {}", h.score);
        }
    }

    #[test]
    fn output_ids_equal_union_of_inputs(
        fts in ranking_strategy(12),
        vec in ranking_strategy(12),
    ) {
        let expected: HashSet<usize> = fts.iter().copied().chain(vec.iter().copied()).collect();
        let out = combine(&[("fts", fts), ("vec", vec)], DEFAULT_K);
        let got: HashSet<usize> = out.iter().map(|h| h.id).collect();
        prop_assert_eq!(got, expected);
    }

    #[test]
    fn output_length_equals_union_size(
        fts in ranking_strategy(12),
        vec in ranking_strategy(12),
    ) {
        let expected_len = fts
            .iter()
            .copied()
            .chain(vec.iter().copied())
            .collect::<HashSet<usize>>()
            .len();
        let out = combine(&[("fts", fts), ("vec", vec)], DEFAULT_K);
        prop_assert_eq!(out.len(), expected_len);
    }

    #[test]
    fn output_is_sorted_descending_by_score(
        fts in ranking_strategy(12),
        vec in ranking_strategy(12),
    ) {
        let out = combine(&[("fts", fts), ("vec", vec)], DEFAULT_K);
        for w in out.windows(2) {
            prop_assert!(
                w[0].score >= w[1].score,
                "not descending: {} then {}",
                w[0].score,
                w[1].score,
            );
        }
    }

    #[test]
    fn per_source_counts_match_appearances(
        fts in ranking_strategy(12),
        vec in ranking_strategy(12),
    ) {
        let in_fts: HashSet<usize> = fts.iter().copied().collect();
        let in_vec: HashSet<usize> = vec.iter().copied().collect();
        let out = combine(&[("fts", fts), ("vec", vec)], DEFAULT_K);
        for h in &out {
            let expected = (in_fts.contains(&h.id) as usize) + (in_vec.contains(&h.id) as usize);
            prop_assert_eq!(h.per_source.len(), expected);
        }
    }

    #[test]
    fn doc_in_both_sources_outscores_doc_in_one(
        only_a in 0usize..32usize,
        only_b in 0usize..32usize,
        both in 0usize..32usize,
    ) {
        prop_assume!(only_a != only_b && only_a != both && only_b != both);
        let out = combine(
            &[
                ("fts", vec![only_a, both]),
                ("vec", vec![only_b, both]),
            ],
            DEFAULT_K,
        );
        let both_score = out.iter().find(|h| h.id == both).unwrap().score;
        let a_score = out.iter().find(|h| h.id == only_a).unwrap().score;
        let b_score = out.iter().find(|h| h.id == only_b).unwrap().score;
        prop_assert!(both_score > a_score);
        prop_assert!(both_score > b_score);
    }
}
