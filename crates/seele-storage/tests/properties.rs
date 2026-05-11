//! Property tests for `seele-storage`.
//!
//! Pure-function invariants:
//! - `normalized_hash` is deterministic across runs and ignores
//!   whitespace runs + ASCII case.
//! - `normalized_hash` always emits 64 lowercase hex chars.
//! - `strip_private_tags` is idempotent — applying twice equals once.
//!
//! Integration invariants (real SQLite via `tempfile`, smaller case count
//! to keep CI fast):
//! - `save → get(id)` returns a row whose title/content match the input
//!   verbatim when no `<private>` tags are present.
//! - `save → list(project=…)` retrieves the row that was just saved.

use proptest::prelude::*;
use seele_core::memory::{ObservationType, Scope};
use seele_core::metadata::Metadata;
use seele_storage::hash::normalized_hash;
use seele_storage::privacy::strip_private_tags;
use seele_storage::{init_db, ObservationQuery, ObservationStore, SaveInput};
use tempfile::TempDir;

fn fresh_store() -> (TempDir, ObservationStore) {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    (td, ObservationStore::new(pool))
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 32,
        ..ProptestConfig::default()
    })]

    #[test]
    fn normalized_hash_is_deterministic(s in "[ -~]{0,200}") {
        let a = normalized_hash(&s);
        let b = normalized_hash(&s);
        prop_assert_eq!(a, b);
    }

    #[test]
    fn normalized_hash_collapses_whitespace_runs(
        a in "[a-z]{1,20}",
        b in "[a-z]{1,20}",
        spaces in 1usize..10usize,
    ) {
        let single = format!("{a} {b}");
        let multi = format!("{a}{}{b}", " ".repeat(spaces));
        prop_assert_eq!(normalized_hash(&single), normalized_hash(&multi));
    }

    #[test]
    fn normalized_hash_is_ascii_case_insensitive(s in "[a-z][a-z ]{1,40}") {
        prop_assert_eq!(
            normalized_hash(&s),
            normalized_hash(&s.to_uppercase()),
        );
    }

    #[test]
    fn normalized_hash_output_is_hex_64(s in "[ -~]{0,200}") {
        let h = normalized_hash(&s);
        prop_assert_eq!(h.len(), 64);
        prop_assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn strip_private_tags_is_idempotent(
        prefix in "[a-z]{0,20}",
        secret in "[a-z0-9 ]{1,40}",
        suffix in "[a-z]{0,20}",
    ) {
        let original = format!("{prefix}<private>{secret}</private>{suffix}");
        let once = strip_private_tags(&original);
        let twice = strip_private_tags(&once);
        prop_assert_eq!(&once, &twice);
        prop_assert!(!once.contains("<private>"));
        prop_assert!(!once.contains("</private>"));
    }
}

// DB-touching tests use a smaller case count: each iteration runs
// migrations + vec0 load + insert + read, so 8 cases ≈ 1-3 s locally
// and stays well inside the CI test budget.
proptest! {
    #![proptest_config(ProptestConfig {
        cases: 8,
        ..ProptestConfig::default()
    })]

    #[test]
    fn save_then_get_roundtrips_title_and_content(
        title in "[a-zA-Z][a-zA-Z0-9 ]{2,40}",
        content in "[a-zA-Z][a-zA-Z0-9 ]{2,200}",
        project in "[a-z][a-z0-9-]{2,12}",
    ) {
        let (_td, store) = fresh_store();
        let outcome = store
            .save(SaveInput {
                session_id: None,
                kind: ObservationType::Memory,
                title: title.clone(),
                content: content.clone(),
                tool_name: None,
                project: Some(project),
                scope: Scope::Project,
                topic_key: None,
                metadata: Metadata::new(),
            })
            .unwrap();
        let got = store.get(outcome.id()).unwrap().expect("must exist");
        prop_assert_eq!(got.title, title);
        prop_assert_eq!(got.content, content);
    }

    #[test]
    fn save_then_list_by_project_retrieves_row(
        title in "[a-zA-Z][a-zA-Z0-9 ]{2,40}",
        content in "[a-zA-Z][a-zA-Z0-9 ]{2,200}",
        project in "[a-z][a-z0-9-]{2,12}",
    ) {
        let (_td, store) = fresh_store();
        let outcome = store
            .save(SaveInput {
                session_id: None,
                kind: ObservationType::Memory,
                title,
                content,
                tool_name: None,
                project: Some(project.clone()),
                scope: Scope::Project,
                topic_key: None,
                metadata: Metadata::new(),
            })
            .unwrap();
        let rows = store
            .list(ObservationQuery {
                project: Some(project),
                ..Default::default()
            })
            .unwrap();
        prop_assert!(rows.iter().any(|r| r.id == outcome.id()));
    }
}
