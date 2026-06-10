//! Loader/ingest test (T-B2): the `coding-memory` fixture parses, ingests as
//! distinct rows (no topic-upsert collapse), and every item is retrievable.

use seele_embedder::FakeEmbedder;
use seele_eval::{ingest, load_suite};
use seele_storage::{init_db, ObservationStore};
use tempfile::TempDir;

#[test]
fn loads_and_ingests_coding_memory_fixture() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/coding-memory.json");
    let suite = load_suite(path).unwrap();
    assert_eq!(suite.corpus.len(), 22, "fixture corpus size");
    // Suite v2 (Q7): 18 originales + 9 paraphrase + 7 multi-hop nuevas.
    assert_eq!(suite.queries.len(), 34, "fixture query count (v2)");

    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("eval.db")).unwrap();
    let store = ObservationStore::new(pool);

    let map = ingest(&suite, &store, &FakeEmbedder::new()).unwrap();

    // Every corpus item is its own row — including the superseding pair
    // ld-05 / ld-06 that share topic_key `decision/embedder`. With `save`
    // (topic-upsert) this would be 21 distinct ids; with `save_raw` it's 22.
    assert_eq!(map.len(), 22, "all corpus items map to distinct ids");
    assert_ne!(
        map["ld-05"], map["ld-06"],
        "superseding pair must not collapse"
    );

    for fid in ["ld-01", "ld-06", "ld-22"] {
        let id = map[fid];
        assert!(
            store.get(id).unwrap().is_some(),
            "{fid} should be retrievable"
        );
    }
}
