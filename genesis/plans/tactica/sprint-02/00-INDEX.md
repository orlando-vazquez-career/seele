# Sprint-02 BE — Embedder + Search

**Fecha**: 2026-05-10
**Tema**: Polish y completado de `seele-embedder` (ONNX local + auto-download) y `seele-search` (FTS + vec0 + RRF + boost). Cierra el path completo write→embed→search end-to-end con tests integración sólidos.
**Salida**: embedder en `~/.seele/embedder/` con SHA256 verification + INT8 quantization opcional + singleton global; search engine con boost por metadata score + annotation lines de relations + empty-query-list path; suite de tests integración con DB poblada (~50 fixtures realistas).

## Estado inicial heredado del commit `8d67f48` (no por tactica)

El Sprint-02 ya tiene código commiteado avanzado fuera del flujo AEGIS. Esta tactica lo absorbe como **estado inicial** en vez de re-implementarlo. Lo enumera para que los bloques nuevos cubran solo los gaps reales.

### Embedder ya implementado (`crates/seele-embedder/`)

- `Embedder` trait con `embed`, `embed_batch`, `dim`, `model_id`. Default `embed_batch` itera `embed`.
- `OnnxEmbedder` (production):
  - Modelo `sentence-transformers/all-MiniLM-L6-v2` (384-dim, max_seq_len 256).
  - Auto-download via `hf-hub::api::sync::Api` cacheado en `~/.cache/huggingface/`.
  - Tokenizer WordPiece via `tokenizers::Tokenizer::from_file`.
  - Inferencia: tokenize batch → ONNX session run → mean-pool con attention-mask weighting → L2 normalize.
  - Mutex per-instance sobre `Session`. Lock liberado antes del CPU-bound pooling.
  - Errores tipados: `EmbedderError::EmptyInput`, `Ort`, `DimensionMismatch`, `HfHub`, `Tokenizer`.
- `FakeEmbedder`: SHA-256 hash + scaling determinístico → 384-dim L2-normalized. Para tests downstream que no deben pegar a HF.
- `OnnxConfig` con `model_repo`, `max_seq_len`, `dim`. `with_config()` para overrides.
- Tests: 6 unit (4 unit + 2 fake correctness) + 2 ignored (descarga ONNX real).

### Search ya implementado (`crates/seele-search/`)

- `SearchQuery` con `text`, `project`, `scope`, `kind`, `per_method_limit` (default 50), `limit` (default 10), `include_purist` (default false).
- `SearchHit` con `observation`, `score` (RRF score), `fts_rank`, `vec_rank`.
- `SearchEngine::new(pool, embedder)` + `with_rrf_k(k)`.
- `search(query)` ejecuta:
  1. `fts_query` — FTS5 MATCH con escape (wraps en `"…"` + duplica `"`), filtros project/scope/kind/include_purist, ORDER BY rank.
  2. `vec_query` — embed query → vec0 MATCH con `k = limit`, mismos filtros.
  3. `rrf::combine` con k=60 default — combina rankings, devuelve `RrfHit` ordenados por score desc.
  4. `hydrate` — selecciona observation rows desde IDs ordenados.
- RRF combiner standalone (`rrf.rs`):
  - `combine(rankings, k)` itera rankings, suma `1.0 / (k + rank)`, mantiene `per_source` con (source_name, rank).
  - Sort estable: ties preservan orden de inserción.
- Errores tipados: `SearchError::InvalidInput`, `DimensionMismatch`, `Storage`, `Embedder`.
- Tests: 7 unit RRF (single source, both sources, rank-1 dominance, empty, k effects, per_source) + 1 escape_fts + 7 integration (`hybrid_search.rs`: fts_match, vec_only_match, project_filter, purist_exclude_default + opt-in, deleted_excluded, empty_error, limit_respected).

### Storage embedding API ya implementado (`crates/seele-storage/src/observations.rs`)

- `ObservationStore::set_embedding(id, &embedding)` — INSERT OR REPLACE en `observations_vec(rowid, embedding)`. Resolve `int_id` desde `observations.id`. Caller responsible para L2-norm.
- `ObservationStore::delete_embedding(id)` — DELETE WHERE rowid en sub-select. Idempotent.
- `hard_delete(id)` ya limpia el row de `observations_vec` automáticamente.
- Embedding column dim 384 (matches MiniLM-L6-v2).

### Lo que NO está hecho (gap real para Sprint-02)

#### Embedder
- ❌ INT8 quantized model como default. Plan estrategia/04 dice "INT8 quantization por default (~30% más rápido, ~2% drop)". Código actual usa `onnx/model.onnx` (full precision).
- ❌ Path de cache controlado por SEELE — actualmente `~/.cache/huggingface/`. Plan ADR-04 dice `~/.seele/embedder/<model>/`.
- ❌ SHA256 verification del modelo descargado contra hash hardcodeado.
- ❌ Singleton global pattern (`OnceCell<Arc<dyn Embedder>>`) para HTTP/MCP servers que comparten una instancia.
- ❌ Detección de model swap (hash change en config) → warning + opción `embedder reembed-all` (esa CLI llega Sprint-04, pero el primitivo se prepara aquí).
- ❌ `Embedder::expected_sha256()` o equivalente para audit.

#### Search
- ❌ Boost opcional por metadata score (ADR-03 capa 5: `final_score = rrf_score * (1.0 + 0.1 * meta_score.unwrap_or(1.0))`). Configurable via flag.
- ❌ Empty-query path: `search("", filters=…)` → list ordenado por `created_at DESC` sin pasar por FTS/vec. Hoy retorna `InvalidInput`.
- ❌ Annotation lines en results: para cada hit, anotar `supersedes:`, `superseded_by:`, `conflicts:`, `conflict: contested by` en función de `memory_relations`. Requiere consulta adicional + extender `SearchHit` con `annotations: Vec<RelationAnnotation>`.
- ❌ `--max-distance` threshold para excluir vec hits demasiado lejos.
- ❌ Performance test concreto: search híbrido < 300ms con 10K observations (criterio MVP §5).

#### Tests integración
- ❌ Suite con DB poblada (~50 fixtures realistas que cubran types, projects, scopes, topic_keys, axiomatic, purist, soft-deleted, relations).
- ❌ Property tests con `proptest` (deferidos también desde Sprint-01).
- ❌ Smoke perf con N=10K observations.

## Bloques

- `01-bloque-A-embedder-polish.md` — INT8 quantized + cache `~/.seele/embedder/` + SHA256 verify + singleton global + tests.
- `02-bloque-B-search-polish.md` — boost por metadata score + empty-query path + annotation lines de relations + max-distance + tests.
- `03-bloque-C-tests-integration.md` — fixtures suite (~50 observations) + property tests + smoke perf.
- `04-bloque-D-state-sync.md` — devlog + plan a executed + CHANGELOG + INDEX + cost-ledger + commit + tag `sprint-02-embedder-search`.

## Dependencias entre bloques

```
A (embedder polish) ─┐
                     ├─→ C (tests integracion) ─→ D (state-sync)
B (search polish) ───┘
```

A y B son independientes (distintos crates), pueden ejecutarse en paralelo dentro del mismo turno (no en agentes separados — ambos los ejecuta el orquestador en consola). C depende de A+B porque las fixtures usan APIs polished. D depende de A+B+C cerrados con tests verdes.

## Criterios de cierre del sprint

1. ✅ `cargo build --workspace` verde.
2. ✅ `cargo test --workspace` verde con tests nuevos del Bloque C agregados al count (esperado: ~110-130 tests).
3. ✅ `cargo clippy --workspace --all-targets -- -D warnings` verde.
4. ✅ `cargo fmt --check` verde.
5. ✅ Static check STELE residual verde.
6. ✅ INT8 quantized model carga + funciona + tests adaptados.
7. ✅ Cache path `~/.seele/embedder/` resolvible + idempotente + override por env `SEELE_EMBEDDER_DIR`.
8. ✅ Search con boost: caso unit donde una observation con `meta_score=10.0` ranquea por encima de una con `meta_score=null` con misma RRF score base.
9. ✅ Search con empty query + project filter retorna observaciones más recientes de ese project.
10. ✅ Annotation lines: caso integration donde un hit con relation `supersedes` contra otro hit incluye annotation `supersedes: <id>`.
11. ✅ Performance smoke: search híbrido sub-300ms con 10K observations en CPU local (asume FakeEmbedder para no descargar).
12. ✅ Commit + push de cada bloque + devlog del sprint en `docs/aegis/devlogs/2026-05-MM-sprint-02-embedder-search.md`.
13. ✅ Cost-ledger append para Sprint-02.
14. ✅ Tag git `sprint-02-embedder-search`.

## Out-of-scope del sprint-02

- CUDA / Metal / DirectML support — sigue v0.2 (ADR-04).
- Embedder swap CLI (`seele embedder install <model>`, `seele embedder reembed-all`) — Sprint-04 con CLI completa.
- Reranking por LLM post-RRF — v0.2.
- Query expansion / sinónimos — v0.3.
- HTTP / MCP transport del search — Sprint-03.
- Setup wizard para descarga inicial — Sprint-04.

## Tamaño esperado

- LOC Rust nuevo: ~600-1000 (mayormente seele-search + tests integración).
- Tests nuevos: ~25-40 incluyendo property tests.
- Tiempo: 1 sesión orquestada (estimado).

## Riesgos identificados

| Riesgo | Mitigación |
|---|---|
| INT8 quantized model no disponible en HF para `all-MiniLM-L6-v2` con paths estándar | Verificar paths antes de bloque A. Fallback: full precision con flag `--no-quantize` (config). Si HF no expone `onnx/model_quantized.onnx`, agregar entry en `OnnxConfig` para path explícito. |
| `~/.seele/embedder/` no existe en runners CI | Bloque A crea el directorio idempotente con `dirs::cache_dir().join("seele/embedder")`. Tests usan `TempDir`. |
| Annotation lines requieren consulta extra → +50ms en search | Hacer annotation **opt-in** en `SearchQuery` (`include_annotations: false` default). Si MNEMA quiere las anotaciones las pide explícitamente. |
| Performance smoke 10K observations no alcanza el sub-300ms en runner CI | Marcar el test como `#[ignore]` con flag `-- --ignored`. Sirve como regression check local; CI corre el suite normal sin él. |
| Property tests con proptest agregan tiempo de CI | Cap a 64 casos por property (`PROPTEST_CASES=64`) en CI. Local default sigue 256. |

## Notas operativas

- Cada bloque cierra con tests verdes + clippy verde + commit pusheado al cierre del bloque (`sprint-02 bloque-X — <título>`).
- Devlog del sprint al cerrar bloque D, incluyendo absorción del estado inicial commit `8d67f48` + el delta nuevo de los bloques A/B/C.
- Si Bloque A descubre que el path quantized no existe en HF, **detener Bloque A**, escribir mini-ADR en `genesis/plans/arquitectura/12-embedder-quantization-fallback.md` y consultar gate humano antes de seguir.
