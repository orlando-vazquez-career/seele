//! Performance smoke test for hybrid search.
//!
//! Verifies that searching a populated database returns within the
//! sub-300ms target from `genesis/plans/estrategia/04-scope-mvp.md`
//! criterion §5 ("Performance: search híbrido sub-300ms con 10K
//! observations en CPU mid-range").
//!
//! Marked `#[ignore]` because populating 10K rows + running search takes
//! several seconds and the CI runners shouldn't pay that cost on every
//! push. Run locally with:
//!
//! ```bash
//! cargo test --package seele-search --test perf_smoke -- --ignored
//! ```

mod common;

use std::time::Instant;

use common::populated_n;
use seele_search::SearchQuery;

#[test]
#[ignore = "smoke perf with 10K observations; run with --ignored"]
fn search_under_300ms_with_10k_observations() {
    let setup_start = Instant::now();
    let fx = populated_n(10_000);
    let setup_elapsed = setup_start.elapsed();
    eprintln!("setup (insert + embed 10K rows) took {setup_elapsed:?}");

    let search_start = Instant::now();
    let hits = fx
        .engine
        .search(SearchQuery {
            text: "common keyword".into(),
            project: Some("p".into()),
            limit: Some(10),
            ..Default::default()
        })
        .unwrap();
    let elapsed = search_start.elapsed();
    eprintln!("search elapsed: {elapsed:?}, hits: {}", hits.len());

    assert!(!hits.is_empty(), "expected at least one hit");
    assert!(
        elapsed.as_millis() < 300,
        "expected sub-300ms, got {}ms (setup {}ms)",
        elapsed.as_millis(),
        setup_elapsed.as_millis()
    );
}

#[test]
#[ignore = "smoke perf with 1K observations; faster than 10K but still ignored"]
fn search_under_100ms_with_1k_observations() {
    let fx = populated_n(1_000);
    let start = Instant::now();
    let hits = fx
        .engine
        .search(SearchQuery {
            text: "common keyword".into(),
            project: Some("p".into()),
            limit: Some(10),
            ..Default::default()
        })
        .unwrap();
    let elapsed = start.elapsed();
    eprintln!("1K search elapsed: {elapsed:?}, hits: {}", hits.len());
    assert!(!hits.is_empty());
    assert!(elapsed.as_millis() < 100, "expected sub-100ms with 1K");
}
