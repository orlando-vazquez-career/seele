# Sprint-02 Bloque C — Tests integración

**Tema**: Suite de tests integración con DB poblada para validar el flow embedder → storage → search end-to-end. Incluye property tests con `proptest` y un smoke perf con N=10K.

**Pre-requisitos**: bloques A + B cerrados.

## Tareas atómicas

### C.1 — Fixture suite

**Path**: `crates/seele-search/tests/fixtures.rs` (helper module shared) + `crates/seele-search/tests/fixtures_data.json`.

**Cambios**:
- `fixtures.rs` expone:
  ```rust
  pub struct FixtureSet {
      pub td: TempDir,
      pub pool: Pool,
      pub store: ObservationStore,
      pub embedder: FakeEmbedder,
      pub engine: SearchEngine,
      pub ids: Vec<SeeleId>,
  }

  pub fn populated_50() -> FixtureSet;
  pub fn populated_n(n: usize) -> FixtureSet;
  ```
- `populated_50()` carga 50 observations realistas:
  - **6 sessions** (3 dev-zen, 2 mnema, 1 personal-scope).
  - **types mixtos**: 12 decision, 10 architecture, 8 bugfix, 5 advisor_output, 5 review, 5 verdict, 5 skill.
  - **topic_keys** con family heuristics (architecture/api-rate-limit, decision/embedder-choice, bug/fts-trigger-race, etc).
  - **10 con metadata.context_mode='purist'** (advisor_outputs).
  - **20 con metadata.axiomatic=true**.
  - **5 soft-deleted**.
  - **30 con linked_to** (para tests de relations).
  - **8 con memory_relations**: 5 pending, 3 judged.
  - cada observation se inserta + `set_embedding(id, FakeEmbedder.embed(content))`.

### C.2 — Tests integración cross-feature

**Path**: `crates/seele-search/tests/end_to_end.rs`.

**Tests**:
- `e2e_save_embed_search_full_lifecycle` — usa `populated_50`, busca por keyword común, verifica que el resultado más reciente del project esperado encabeza.
- `e2e_purist_excluded_by_default_with_real_volume` — usa `populated_50`, verifica que ninguno de los 10 purist aparece en search default; `include_purist=true` los incluye.
- `e2e_soft_deleted_excluded_consistently` — los 5 soft-deleted nunca aparecen en search.
- `e2e_topic_key_filter_returns_only_matching_family` — filter por topic_key="architecture/api-rate-limit" → solo hits con ese exact match (Sprint-04 agrega prefix matching, esto es exact).
- `e2e_annotations_in_real_dataset` — habilita annotations, busca un keyword que toque dos memorias con `supersedes`. Verifica que el winner tiene `Supersedes` y el loser `SupersededBy`.
- `e2e_boost_with_meta_score_promotes_axiomatic` — los 20 axiomatic tienen `meta_score=5.0`. Con `score_boost_multiplier=0.1`, en un search ambiguo los axiomatic encabezan vs los sin score.
- `e2e_empty_query_returns_recent_in_project_filter`.

### C.3 — Property tests

**Path**: `crates/seele-search/tests/property_tests.rs`.

**Tests** (con `proptest!`, default 256 casos local / 64 CI):

```rust
proptest! {
    #[test]
    fn save_embed_search_roundtrip(
        title in "\\PC{1,100}",
        content in "\\PC{20,500}",
        project in "[a-z][a-z0-9-]{2,15}",
    ) {
        let (td, store, engine) = fresh_engine();
        let id = save_with_embedding(&store, &FakeEmbedder, &title, &content, &project, Metadata::new());
        let hits = engine.search(SearchQuery {
            text: extract_keyword(&content),
            project: Some(project.clone()),
            ..Default::default()
        }).unwrap();
        prop_assert!(hits.iter().any(|h| h.observation.id == id));
    }

    #[test]
    fn rrf_score_increases_when_doc_appears_in_more_methods(
        // ...
    ) { ... }

    #[test]
    fn limit_caps_result_count(
        n in 1u32..50u32,
        limit in 1u32..30u32,
    ) {
        let (td, store, engine) = populated_n(n as usize).split();
        let hits = engine.search(SearchQuery {
            text: "common keyword".into(),
            limit: Some(limit),
            ..Default::default()
        }).unwrap();
        prop_assert!(hits.len() as u32 <= limit);
    }
}
```

(`extract_keyword` toma la primera palabra >5 chars del content para garantizar match no degenerado.)

Acompañar con `crates/seele-storage/tests/property_tests.rs` con los proptest deferidos del Sprint-01 Bloque D (originalmente plan original):
- `save_then_get_roundtrip`
- `topic_key_upsert_increments_revision`
- `normalized_hash_dedup_increments_duplicate_count`
- `soft_delete_hides_from_list`

### C.4 — Smoke perf con 10K observations

**Path**: `crates/seele-search/tests/perf_smoke.rs`.

**Test**:
```rust
#[test]
#[ignore = "smoke perf with 10K observations; run with --ignored"]
fn search_under_300ms_with_10k_observations() {
    let (_td, _store, engine) = populated_n(10_000).split();
    let start = std::time::Instant::now();
    let hits = engine.search(SearchQuery {
        text: "common keyword".into(),
        limit: Some(10),
        ..Default::default()
    }).unwrap();
    let elapsed = start.elapsed();
    assert!(!hits.is_empty());
    assert!(
        elapsed.as_millis() < 300,
        "expected sub-300ms, got {}ms",
        elapsed.as_millis()
    );
}
```

Notas:
- Usa `FakeEmbedder` para no descargar ONNX en runners.
- `#[ignore]` por default — local con `--ignored`. Si el runner no aguanta sub-300ms con FakeEmbedder + SQLite real, el problema es real (no bench artifact).

### C.5 — Update de criterios de aceptación

Actualizar `Sprint-02/00-INDEX.md` (criterios) cuando esto cierre.

## Criterios de aceptación del bloque C

1. `cargo test --workspace` verde con count nuevo (~30+ tests sumados entre fixtures, e2e, proptest, perf-smoke).
2. `cargo test -- --ignored` corre el perf smoke local sub-300ms.
3. Property tests cap a 64 casos en CI via `PROPTEST_CASES=64` env (configurable). Local default 256.
4. Fixtures reusables desde otros crates (sprint-03 los va a usar para tests E2E de HTTP/MCP).

## Commit del bloque C

```
git add -A
git commit -m "sprint-02 bloque-C — tests integracion + property tests + perf smoke"
git push
```
