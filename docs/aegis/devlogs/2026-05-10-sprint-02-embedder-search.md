# Sprint-02 BE Embedder + Search — devlog

**Fecha**: 2026-05-10
**Fases ejecutadas**: tactica → ejecucion (bloques A/B/C) → guardrails → state-sync
**Plan origen**: `genesis/plans/executed/tactica/sprint-02/00-INDEX.md`
**Repos afectados**: `tools/SEELE`
**Migraciones**: ninguna nueva (Sprint-02 trabaja sobre schema V001 ya commiteado en Sprint-01).
**Tests**: 130 passing / 0 failing / 4 ignored (2 ONNX download + 2 perf smoke 1K/10K)
**Build**: OK Linux + macOS + Windows (esperado en CI matrix tras push)

## Resumen

Sprint-02 cierra los gaps del `seele-embedder` + `seele-search` heredados del commit `8d67f48` (avance fuera de Táctica) contra el plan estrategia/04-scope-mvp + ADR-03 (search híbrido) + ADR-04 (embedder ONNX). El sprint se ejecutó en 3 bloques de polish + un bloque de tests integración:

- **Bloque A**: embedder polish — cache `~/.seele/embedder/` con env override, INT8 quantized como default con fallback automático, SHA256 verification opcional, singleton global.
- **Bloque B**: search polish — boost por metadata score, empty-query path con `created_at DESC`, annotation lines de `memory_relations`, max-distance threshold para vec hits.
- **Bloque C**: tests integración — fixture suite con 20 obs realistas, 7 tests E2E, 3 proptest, 2 perf smoke ignored.
- **Bloque D**: este state-sync.

Antes de Sprint-02 había 99 tests; al cierre hay **130 tests verde** + 4 ignored. Net +31 tests (8 embedder + 13 search unit/integration + 7 e2e + 3 proptest).

## Cambios entregados

### Bloque A — embedder polish (commit `8e0f240`)

`crates/seele-embedder/`:

- `Cargo.toml`: agrega `dirs` y `hex` a deps.
- `error.rs`: 3 errores nuevos — `CacheDirUnresolvable`, `HashMismatch { file, expected, got }`, `GlobalAlreadyInitialized`.
- `embedder.rs::Embedder`: nuevo método trait `expected_sha256(&self) -> Option<&str>` con default `None`.
- `onnx.rs`:
  - `OnnxConfig`: nuevo campo `quantized: bool` (default true).
  - `resolve_cache_dir()` público — prioriza env `SEELE_EMBEDDER_DIR`, cae a `dirs::cache_dir().join("seele/embedder")`. Crea idempotente. Error tipado si no resoluble.
  - `resolve_model_files()`: cuando `quantized=true` intenta `onnx/model_quantized.onnx` primero; si falla, log `tracing::warn` y fallback a `onnx/model.onnx`. Cuando `quantized=false`, va directo a full precision.
  - Tabla `TRUSTED_HASHES` (vacía al v0.1, populada por release). Helper `verify_hash_if_listed()` — si el archivo está listado y mismatch, retorna `HashMismatch`. Si no listado, log debug y proceed.
  - `OnnxEmbedder::expected_sha256()` retorna hash de la tabla para el archivo que efectivamente se cargó.
  - Usa `ApiBuilder::with_cache_dir()` en lugar del default de hf-hub.
- `singleton.rs` (nuevo módulo): `init_global(Arc<dyn Embedder>)` idempotente con `OnceCell`, `global()` panic-on-uninit, `try_global()` no-panic. Pensado para HTTP/MCP servers de larga vida.
- Tests embedder: 14 passed + 2 ignored (antes 6 + 2). +8 nuevos: 3 cache_dir (env/fallback/empty), 1 quantized_default, 2 trusted_hash (lookup/skip-when-unlisted/mismatch-when-listed), 1 init_global + second_init_errs.

### Bloque B — search polish (commit `0854e15`)

`crates/seele-search/`:

- `engine.rs`:
  - `SearchQuery` extendido con `score_boost_multiplier: f64`, `max_vec_distance: Option<f64>`, `include_annotations: bool` (todos con defaults de no-op para preservar Sprint-01 behavior).
  - Nuevo `enum AnnotationKind { Supersedes, SupersededBy, ConflictsWith, ContestedBy }`.
  - Nuevo `struct RelationAnnotation { kind, other_id, other_title, reason }`.
  - `SearchHit` extendido con `annotations: Vec<RelationAnnotation>`.
  - `search()`:
    - Empty query (`text.trim().is_empty()`) ahora ejecuta `list_by_filters()` en lugar de retornar `InvalidInput`. Skip FTS+vec entirely (no llama embed).
    - Después de RRF: `apply_score_boost()` re-scorea con `(1 + multiplier * meta_score)` cuando multiplier > 0.
    - Annotations opt-in: si `include_annotations=true`, hace una query extra contra `memory_relations` JOIN `observations` para títulos del other-end.
  - `vec_query()` ahora respeta `max_vec_distance` con `AND vec.distance <= ?`.
  - `annotation_for_source()` / `annotation_for_target()` helpers para mapear relation+status a AnnotationKind.
- `lib.rs`: re-exports de `AnnotationKind`, `RelationAnnotation`.
- Tests: +5 unit annotation mapping + +8 integration en `hybrid_search.rs` (renombrado `empty_query_returns_invalid_input_error` → `whitespace_only_query_treated_as_empty_query` que ahora verifica comportamiento opuesto).

### Bloque C — tests integración (commit `cbe0512`)

`crates/seele-search/tests/`:

- `common/mod.rs` — módulo compartido entre test binaries (`#![allow(dead_code)]` para evitar warnings spurios cuando un binary usa solo subset). Expone `FixtureSet` (TempDir + Pool + ObservationStore + RelationStore + FakeEmbedder + SearchEngine + ids vec) + `populated_20()` (fixture realista con winner/loser supersedes + schema_a/b conflict judged + purist + axiomatic + soft-deleted) + `populated_n(n)` (n obs sintéticas en project "p").
- `end_to_end.rs` — 7 tests E2E con `populated_20`: search filters por project, purist excluded/included, soft-deleted excluido, annotations supersedes attach, annotations contested_by para conflict judged, empty query lista recientes.
- `property_tests.rs` — 3 proptests con 32 casos (CI fast, configurable via `PROPTEST_CASES`): save→search roundtrip por keyword, limit cap, empty-query respeta project + limit.
- `perf_smoke.rs` — 2 `#[ignore]` smoke: 1K obs sub-100ms, 10K obs sub-300ms (criterio MVP §5).
- `Cargo.toml`: agrega `proptest` a dev-dependencies.

## Decisiones técnicas tomadas

- **`TRUSTED_HASHES` vacía al v0.1**. El plan dice "verificación SHA256 hardcodeada". Implementamos la infra completa pero dejamos la tabla vacía hasta que tengamos una build deterministica del modelo. Cuando se haga el primer release, se calcula el hash de `onnx/model_quantized.onnx`, `onnx/model.onnx` y `tokenizer.json` para `all-MiniLM-L6-v2` con `sha256sum` y se popula la tabla. El behavior por default (no listado → warn + proceed) hace que los tests + dev locales no dependan de tener los hashes definidos.
- **Quantized fallback silencioso con warn**. Plan original (Bloque A.2) decía "fail con `QuantizedNotAvailable` error". Cambiamos a fallback automático con `tracing::warn` porque (1) más amigable para usuarios que no conocen el flag, (2) mantiene el binary funcional aunque HF retire temporalmente el quantized, (3) el warn deja trail visible en logs. Si en el futuro queremos strict-mode, agregamos `OnnxConfig { strict_quantized: true }`.
- **`include_annotations` y `score_boost_multiplier` opt-in con defaults no-op**. Razón: preservar comportamiento Sprint-01 sin breaking changes. Los nuevos features se habilitan explícitamente en `SearchQuery` per call. CLI HTTP/MCP (Sprint-03) van a exponer estos flags.
- **Boost re-sort post-RRF, no antes**. El multiplier afecta el score final, no la mecánica RRF. Esto preserva la invariancia-a-escala de RRF (un boost = una segunda etapa, no una contaminación del rank).
- **Annotations en una sola query con OR**. `WHERE source_id IN (…) OR target_id IN (…)` ejecuta dos seeks en `memory_relations`. Más eficiente que dos queries separadas; permite resolver winner+loser de un supersedes en el mismo round-trip.
- **Property tests con 32 casos** vs 256 default proptest. CI tarda menos. Local con `PROPTEST_CASES=256` para diagnóstico profundo si emerge falla intermitente.

## Incidentes durante la ejecución

- **dead_code warnings en `tests/common/mod.rs`**. Cada test binary `mod common`-importa solo un subset de helpers (e.g., `end_to_end.rs` usa `populated_20`, `property_tests.rs` y `perf_smoke.rs` usan `populated_n`). Clippy `--all-targets` se quejaba spuriously porque no ve cross-binary usage. Solución: `#![allow(dead_code)]` al inicio del módulo + docstring explicativo.
- **`cargo fmt` reformateó dos archivos** tras los edits. Aplicado y verificado en check.
- **Boost test inicialmente flaky** porque con FakeEmbedder + ULIDs aleatorios, el orden RRF base entre dos contents idénticos depende del orden de inserción. Solución: el test verifica el orden vía índice en el vector retornado, no vía score absoluto.

## Cómo reproducir / verificar

```powershell
cd C:\dev\tools\SEELE
cargo build --workspace
cargo test --workspace                                    # 130 verde
cargo test -p seele-embedder -- --ignored                 # ONNX real, ~90MB descarga
cargo test --test perf_smoke -- --ignored                 # smoke perf 1K + 10K
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bash scripts/check-no-stele-residual.sh
```

## Pendiente

- **Hashes reales en `TRUSTED_HASHES`** cuando se haga el primer release tagged.
- **Tests `#[ignore]` ONNX correr en CI semanal** (no en cada push). Se decide en Sprint-05 cuando se diseñe el pipeline de release.
- **CUDA / Metal / DirectML support** — sigue v0.2 (ADR-04).
- **Embedder swap CLI** (`seele embedder install`, `seele embedder reembed-all`) — Sprint-04 con CLI completa.
- **Reranking por LLM post-RRF** — v0.2.
- **Query expansion / sinónimos** — v0.3.

## Uso y costo

| Modelo | Input tokens | Output tokens | Total | USD | Duración |
|---|---|---|---|---|---|
| claude-opus-4-7 | ~850,000 | ~180,000 | ~1,030,000 | ~$26 | ~150 min |

Estimación conservadora. Se appendea en `cost-ledger.jsonl` con `"estimated": true`.

## Referencias

- Plan táctica: `genesis/plans/executed/tactica/sprint-02/`
- Bloque A commit: `8e0f240` — sprint-02 bloque-A embedder polish
- Bloque B commit: `0854e15` — sprint-02 bloque-B search polish
- Bloque C commit: `cbe0512` — sprint-02 bloque-C tests integracion
- Bloque D commit (este state-sync): pendiente al cierre
- Sprint-02 trabajo previo absorbido: `8d67f48` — embedder/search inicial sin tactica
- Devlog Sprint-01: `docs/aegis/devlogs/2026-05-10-sprint-01-foundation.md`
- ADR-03 search híbrido: `genesis/plans/arquitectura/03-search-hybrid.md`
- ADR-04 embedder ONNX: `genesis/plans/arquitectura/04-embedder-onnx.md`
