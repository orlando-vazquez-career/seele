//! Regression test for the `int_id` silent-drop on the raw import path.
//!
//! `SeeleId::as_i64()` maps the ULID's random tail (bytes 9..16) to the
//! `int_id` UNIQUE column. On the fresh-save path a collision is retried;
//! on the raw import path (`save_raw_in_tx`, `INSERT OR IGNORE`) a
//! *preserved* ULID whose tail collides with an existing row used to be
//! swallowed and mislabelled `AlreadyExisted` — silent data loss. It must
//! now surface as `RawSaveOutcome::IntIdCollision`.

use chrono::Utc;
use seele_core::id::SeeleId;
use seele_core::memory::{ObservationType, Scope};
use seele_core::metadata::Metadata;
use seele_storage::{init_db, ObservationStore, RawSaveInput, RawSaveOutcome};
use tempfile::TempDir;
use ulid::Ulid;

fn raw_input(id: SeeleId) -> RawSaveInput {
    let now = Utc::now();
    RawSaveInput {
        id,
        session_id: None,
        kind: ObservationType::Memory,
        title: "title".to_string(),
        content: "content".to_string(),
        tool_name: None,
        project: Some("p".to_string()),
        scope: Scope::Project,
        topic_key: None,
        created_at: now,
        updated_at: now,
        metadata: Metadata::new(),
    }
}

/// Two distinct ULIDs that share the low 56 bits (bytes 9..16) → identical
/// `as_i64()`, different ULID. They differ only above bit 64.
fn colliding_ids() -> (SeeleId, SeeleId) {
    let low: u128 = 0x00_00_00_00_DE_AD_BE_EF; // < 2^56
    let a = SeeleId::from_ulid(Ulid::from((1u128 << 64) | low));
    let b = SeeleId::from_ulid(Ulid::from((2u128 << 64) | low));
    (a, b)
}

#[test]
fn raw_import_surfaces_int_id_collision_instead_of_silent_drop() {
    let (a, b) = colliding_ids();
    assert_eq!(a.as_i64(), b.as_i64(), "setup: ids must collide on int_id");
    assert_ne!(a, b, "setup: ids must be distinct ULIDs");

    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let store = ObservationStore::new(pool);
    let mut conn = store.pool().get().unwrap();
    let tx = conn.transaction().unwrap();

    // First row inserts cleanly.
    assert_eq!(
        ObservationStore::save_raw_in_tx(&tx, raw_input(a)).unwrap(),
        RawSaveOutcome::Inserted
    );

    // Second row: different id, colliding int_id → must be reported as a
    // collision, NOT silently mislabelled `AlreadyExisted`.
    assert_eq!(
        ObservationStore::save_raw_in_tx(&tx, raw_input(b)).unwrap(),
        RawSaveOutcome::IntIdCollision
    );

    // A genuine re-import of the SAME id is still an idempotent skip.
    assert_eq!(
        ObservationStore::save_raw_in_tx(&tx, raw_input(a)).unwrap(),
        RawSaveOutcome::AlreadyExisted
    );

    tx.commit().unwrap();
}
