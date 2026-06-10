//! Migration + write-path tests for `embeddings_meta` (ADR-14, V002 + Q5).
//!
//! V002 creates the table; Q5 wires the write path: every
//! `set_embedding` persists provenance atomically, `delete_embedding`
//! clears it, and `embedding_provenance()` + `mix_warning()` power the
//! doctor guard against silently mixed vectors.

use seele_core::id::SeeleId;
use seele_core::memory::{ObservationType, Scope};
use seele_core::metadata::Metadata;
use seele_storage::{init_db, EmbeddingMeta, ObservationStore, SaveInput};
use tempfile::TempDir;

fn meta(model_id: &str) -> EmbeddingMeta {
    EmbeddingMeta {
        model_id: model_id.to_string(),
        dim: 384,
        contextualized: false,
    }
}

fn save_one(store: &ObservationStore, title: &str) -> SeeleId {
    store
        .save(SaveInput {
            session_id: None,
            kind: ObservationType::Memory,
            title: title.to_string(),
            content: format!("content for {title}"),
            tool_name: None,
            project: Some("p".to_string()),
            scope: Scope::Project,
            topic_key: None,
            metadata: Metadata::new(),
        })
        .unwrap()
        .id()
}

#[test]
fn embeddings_meta_exists_and_is_empty_on_fresh_db() {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let conn = pool.get().unwrap();

    // Table exists (V002 applied) and starts empty on a clean DB.
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM embeddings_meta", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0, "V002 must create an initially empty embeddings_meta");

    // Re-opening the same DB re-runs migrations idempotently.
    drop(conn);
    drop(pool);
    let pool2 = init_db(td.path().join("seele.db")).unwrap();
    let conn2 = pool2.get().unwrap();
    let n2: i64 = conn2
        .query_row("SELECT COUNT(*) FROM embeddings_meta", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n2, 0);
}

#[test]
fn set_embedding_writes_provenance_row_atomically() {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let store = ObservationStore::new(pool.clone());
    let id = save_one(&store, "with-vector");

    store
        .set_embedding(id, &vec![0.1f32; 384], &meta("all-MiniLM-L6-v2"))
        .unwrap();

    let conn = pool.get().unwrap();
    let (model, dim, ctx): (String, i64, i64) = conn
        .query_row(
            "SELECT model_id, dim, contextualized FROM embeddings_meta WHERE observation_id = ?1",
            [id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("provenance row must exist after set_embedding");
    assert_eq!(model, "all-MiniLM-L6-v2");
    assert_eq!(dim, 384);
    assert_eq!(ctx, 0);
}

#[test]
fn reembedding_with_other_model_replaces_provenance() {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let store = ObservationStore::new(pool.clone());
    let id = save_one(&store, "remodeled");

    store
        .set_embedding(id, &vec![0.1f32; 384], &meta("all-MiniLM-L6-v2"))
        .unwrap();
    store
        .set_embedding(
            id,
            &vec![0.2f32; 384],
            &meta("paraphrase-multilingual-MiniLM-L12-v2"),
        )
        .unwrap();

    let conn = pool.get().unwrap();
    let (n, model): (i64, String) = conn
        .query_row(
            "SELECT COUNT(*), MAX(model_id) FROM embeddings_meta WHERE observation_id = ?1",
            [id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(n, 1, "INSERT OR REPLACE must keep exactly one row");
    assert_eq!(model, "paraphrase-multilingual-MiniLM-L12-v2");
}

#[test]
fn delete_embedding_clears_provenance_too() {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let store = ObservationStore::new(pool.clone());
    let id = save_one(&store, "cleared");

    store
        .set_embedding(id, &vec![0.1f32; 384], &meta("all-MiniLM-L6-v2"))
        .unwrap();
    store.delete_embedding(id).unwrap();

    let conn = pool.get().unwrap();
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM embeddings_meta WHERE observation_id = ?1",
            [id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 0, "provenance must not outlive its vector");
}

#[test]
fn embedding_provenance_reports_combos_and_vectorless_actives() {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let store = ObservationStore::new(pool.clone());

    let a = save_one(&store, "vec-a");
    let b = save_one(&store, "vec-b");
    let _c = save_one(&store, "no-vector"); // never embedded

    store
        .set_embedding(a, &vec![0.1f32; 384], &meta("all-MiniLM-L6-v2"))
        .unwrap();
    store
        .set_embedding(b, &vec![0.2f32; 384], &meta("seele/fake-embedder"))
        .unwrap();

    let prov = store.embedding_provenance().unwrap();
    assert_eq!(prov.models.len(), 2, "two distinct model combos");
    assert_eq!(prov.active_without_vector, 1, "one row never embedded");

    // Mixed combos must trip the doctor guard regardless of active model.
    assert!(prov.mix_warning("all-MiniLM-L6-v2", 384).is_some());
}

#[test]
fn mix_warning_is_none_only_when_stored_matches_active() {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let store = ObservationStore::new(pool.clone());
    let id = save_one(&store, "homogeneous");
    store
        .set_embedding(id, &vec![0.1f32; 384], &meta("all-MiniLM-L6-v2"))
        .unwrap();

    let prov = store.embedding_provenance().unwrap();
    assert!(prov.mix_warning("all-MiniLM-L6-v2", 384).is_none());
    // Same dim, different model: the genuinely silent A2 swap — must warn.
    assert!(prov
        .mix_warning("paraphrase-multilingual-MiniLM-L12-v2", 384)
        .is_some());
    // Empty table (fresh DB) never warns.
    let empty_pool = init_db(td.path().join("fresh.db")).unwrap();
    let empty = ObservationStore::new(empty_pool)
        .embedding_provenance()
        .unwrap();
    assert!(empty.mix_warning("anything", 384).is_none());
}

#[test]
fn get_embedding_roundtrips_stored_vector() {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let store = ObservationStore::new(pool);
    let id = save_one(&store, "roundtrip");

    assert!(store.get_embedding(id).unwrap().is_none(), "no vector yet");

    let mut v = vec![0.0f32; 384];
    v[0] = 0.6;
    v[1] = 0.8;
    store.set_embedding(id, &v, &meta("all-MiniLM-L6-v2")).unwrap();

    let back = store.get_embedding(id).unwrap().expect("vector stored");
    assert_eq!(back.len(), 384);
    assert!((back[0] - 0.6).abs() < 1e-6);
    assert!((back[1] - 0.8).abs() < 1e-6);
    assert!(back[2].abs() < 1e-6);
}
