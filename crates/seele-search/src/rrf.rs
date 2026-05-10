//! Reciprocal Rank Fusion combiner.
//!
//! For each retrieval method we have a ranked list. The RRF score for a
//! document is `sum_methods 1 / (k + rank_in_method)` (rank starts at 1).
//! Higher score = better. `k=60` is the standard from the original RRF paper
//! (Cormack et al. 2009) and what ENGRAM/sqlite-vec docs default to.
//!
//! Documents missing from a method's ranking simply contribute 0 from that
//! method — RRF gracefully handles partial recall.

use std::collections::HashMap;
use std::hash::Hash;

/// Standard RRF constant from Cormack et al. 2009.
pub const DEFAULT_K: usize = 60;

#[derive(Debug, Clone)]
pub struct RrfHit<I> {
    pub id: I,
    pub score: f64,
    pub per_source: Vec<(&'static str, usize)>, // (source name, rank starting at 1)
}

/// Combine multiple named rankings into a single RRF-scored list.
///
/// `rankings`: slice of `(source_name, doc_ids_in_rank_order)`.
/// `k`: RRF constant.
///
/// Output is sorted by descending score; ties resolved by stable input order
/// so callers get reproducible results.
pub fn combine<I>(rankings: &[(&'static str, Vec<I>)], k: usize) -> Vec<RrfHit<I>>
where
    I: Eq + Hash + Clone,
{
    let mut score_map: HashMap<I, f64> = HashMap::new();
    let mut sources_map: HashMap<I, Vec<(&'static str, usize)>> = HashMap::new();
    let mut order: Vec<I> = Vec::new();

    for (source_name, ids) in rankings {
        for (rank0, id) in ids.iter().enumerate() {
            let rank = rank0 + 1;
            let increment = 1.0 / (k as f64 + rank as f64);
            let entry = score_map.entry(id.clone()).or_insert_with(|| {
                order.push(id.clone());
                0.0
            });
            *entry += increment;
            sources_map
                .entry(id.clone())
                .or_default()
                .push((*source_name, rank));
        }
    }

    let mut hits: Vec<RrfHit<I>> = order
        .into_iter()
        .map(|id| {
            let score = score_map[&id];
            let per_source = sources_map.remove(&id).unwrap_or_default();
            RrfHit {
                id,
                score,
                per_source,
            }
        })
        .collect();

    // Stable sort: ties keep insertion order (first-seen first).
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_source_preserves_order_with_decreasing_scores() {
        let r = combine(&[("fts", vec!["a", "b", "c"])], DEFAULT_K);
        assert_eq!(r.iter().map(|h| h.id).collect::<Vec<_>>(), ["a", "b", "c"]);
        assert!(r[0].score > r[1].score);
        assert!(r[1].score > r[2].score);
    }

    #[test]
    fn doc_in_both_sources_outranks_doc_in_one() {
        let combined = combine(
            &[("fts", vec!["x", "y", "z"]), ("vec", vec!["y", "w", "z"])],
            DEFAULT_K,
        );
        // y is rank 2 in fts and rank 1 in vec → present in both.
        // x is only rank 1 in fts.
        let y = combined.iter().find(|h| h.id == "y").unwrap();
        let x = combined.iter().find(|h| h.id == "x").unwrap();
        assert!(y.score > x.score, "y={} x={}", y.score, x.score);
    }

    #[test]
    fn rank_1_in_both_sources_beats_rank_1_in_one() {
        let combined = combine(
            &[("fts", vec!["a", "b"]), ("vec", vec!["a", "c"])],
            DEFAULT_K,
        );
        let a = combined.iter().find(|h| h.id == "a").unwrap();
        let b = combined.iter().find(|h| h.id == "b").unwrap();
        let c = combined.iter().find(|h| h.id == "c").unwrap();
        assert!(a.score > b.score && a.score > c.score);
        assert_eq!(a.per_source.len(), 2);
    }

    #[test]
    fn empty_rankings_produce_empty_result() {
        let r: Vec<RrfHit<&str>> = combine(&[], DEFAULT_K);
        assert!(r.is_empty());
    }

    #[test]
    fn smaller_k_amplifies_top_rank_advantage() {
        let rankings = vec![("fts", vec!["a", "b", "c"])];
        let strong = combine(&rankings, 0); // pretend k=0
        let weak = combine(&rankings, 1000);
        let ratio_strong = strong[0].score / strong[2].score;
        let ratio_weak = weak[0].score / weak[2].score;
        assert!(
            ratio_strong > ratio_weak,
            "small k should make top rank more dominant"
        );
    }

    #[test]
    fn per_source_tracks_all_appearances() {
        let r = combine(
            &[("fts", vec!["a", "b"]), ("vec", vec!["b", "a"])],
            DEFAULT_K,
        );
        let b = r.iter().find(|h| h.id == "b").unwrap();
        let names: Vec<&str> = b.per_source.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"fts"));
        assert!(names.contains(&"vec"));
    }
}
