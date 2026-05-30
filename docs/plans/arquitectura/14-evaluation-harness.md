# ADR-14 — Harness de evaluación de calidad de memoria (`seele-eval`) + `embeddings_meta`

**Estado**: Propuesto · 2026-05-29 · **Pendiente Gate 1**
**Continúa la secuencia de ADRs** del repo (01-13 en `genesis/plans/arquitectura/`). Por ser post-v0.1, este ADR vive en `docs/plans/arquitectura/` per la regla del `CLAUDE.md` ("cuando salga v0.1, las nuevas features van a `docs/plans/`").
**Origen**: veredicto del Counsel MNEMA 2026-05-29 (ver reporte fuente en sandbox).

## Decisión

1. **SEELE adopta un harness de evaluación de calidad de memoria como prerequisito que gatea cualquier cambio de retrieval** (reranking A1, swap de embedder A2, query expansion A4). Se materializa en un crate `seele-eval` (#13 del workspace) que corre suites de preguntas con ground-truth contra el pipeline de búsqueda y emite métricas reproducibles.
2. **`embeddings_meta` (provenance de embeddings) es prerequisito duro** de A2 (swap de embedder) y B3 (Contextual Retrieval). Ningún vector entra a `vec0` sin registrar `(model_id, dim, contextualized: bool, created_at)`. Esto cierra el fallo fatal #2 del counsel: dos corpus de embeddings incompatibles (con/sin contexto LLM, o de modelos distintos) **no deben mezclarse en silencio** en la misma tabla.

## Contexto

El estado del arte 2026 es benchmark-driven (LoCoMo, LongMemEval). SEELE no mide calidad de memoria — solo perf. El counsel (argumento mejor puntuado, 13.8/15) concluyó que **optimizar sin medir contamina la línea base**: una vez mergeados A1/A2 sin un "antes", no se puede aislar qué ganó o perdió calidad. El re-framing de Primeros Principios y el plan del Ejecutor convergen en lo mismo.

## Decisión detallada

### Crate `seele-eval`

- **Forma**: crate de workspace con un runner; expuesto como `seele eval --suite <name>` (subcomando del CLI) y como test `#[ignore]` (`cargo test -p seele-eval -- --ignored`) para no correr en CI por default (descarga/coste).
- **Suites (doble pista)**:
  - `longmemeval-subset`: ~50 de las 500 preguntas (categorías priorizadas: multi-session recall, knowledge update, temporal reasoning).
  - `coding-memory`: mini-corpus propio (~20-40 Q&A) de observaciones representativas de coding agents (topic-keys `architecture/*`, `bug/*`, `decision/*`), para no medir lo que SEELE nunca hará (sight Contrarian).
- **Contrato de fixtures**: JSON con `{ corpus: [{id, body, metadata}], queries: [{q, expected_ids[], category}] }`. Se ingiere el corpus en una DB temporal, se corre `search`, se compara el top-k con `expected_ids`.
- **Métrica**: recall@k binaria por query (acierta si algún `expected_id` está en top-k) + agregados por categoría; secundarias MRR y nDCG@k. `k` configurable (default 5 y 10).
- **Output**: JSON `{ suite, model_id, totals: {recall@5, recall@10, mrr}, by_category: {...} }` + tabla a stdout. Apto para versionar el baseline.

### `embeddings_meta`

```sql
CREATE TABLE embeddings_meta (
    observation_id TEXT PRIMARY KEY REFERENCES observations(id) ON DELETE CASCADE,
    model_id       TEXT NOT NULL,        -- e.g. 'all-MiniLM-L6-v2', 'embeddinggemma-300m'
    dim            INTEGER NOT NULL,     -- 384, 768, ...
    contextualized INTEGER NOT NULL DEFAULT 0,  -- 1 si B3 reescribió el body antes de embeber
    created_at     INTEGER NOT NULL
) WITHOUT ROWID;
```

- En search, si conviven varios `model_id`/`dim`, SEELE **no** mezcla distancias: filtra al `model_id` activo (o exige re-embed). `contextualized` permite distinguir y, si hace falta, segmentar.
- Habilita `seele embedder reembed-all` seguro (sabe qué falta migrar) y cierra la migración futura prevista en ADR-02 ("v0.2 tabla `embeddings_meta`").

## Alternativas rechazadas

- **Empezar por A1/A2 y medir después** — rechazada: es el fallo fatal #1 del counsel (línea base contaminada).
- **Solo el subset académico (sin corpus propio)** — rechazada: riesgo de métrica engañosa (LoCoMo ≠ coding-agent memory).
- **`embeddings_meta` recién cuando entre A2** — rechazada: B3 (Contextual Retrieval) también lo necesita; sin provenance, vectores heterogéneos se mezclan en `vec0` en silencio.
- **Reescribir SEELE hacia grafo (B5) o bi-temporal (B4) ahora** — rechazada/diferida: scope que hunde a un dev solo y rompe idempotencia de sync/import ENGRAM.

## Riesgos

| Riesgo | Mitigación |
|---|---|
| El subset LongMemEval es pesado de parsear | Fallback: harness casero de 20 Q&A a mano sobre memorias sintéticas, recall@5 (sight Ejecutor) |
| `int_id`: ¿colisión rompe el mapeo a `vec0`? | El mapeo usa el tail aleatorio (bytes 9..16), NO el timestamp; `int_id` es `UNIQUE` y fresh-save reintenta → el baseline **no** está en riesgo. El riesgo real es el silent-drop en import (`save_raw` con `INSERT OR IGNORE`), cubierto en T-A2. |
| Suite propia sesgada por el autor | Documentar metodología; mantener la pista académica como contrapeso |
| Licencias de modelos nuevos (EmbeddingGemma/BGE) | Verificar model card antes de adoptar; A2 es sprint posterior |

## Consecuencias

- Cada PR de retrieval futuro pasa de "creo que mejora" a "subió/bajó X en la suite". Falsable.
- Primer score local-first auditable publicable (activo de marketing, sight Expansionista).
- `embeddings_meta` desbloquea swap de embedder y Contextual Retrieval sin corromper la `vec0`.

## Referencias

- Reporte fuente (counsel + veredicto): sandbox `Reporte_SEELE_Auditoria-Memory-Engines_2026-05-29.md`.
- LongMemEval arXiv 2410.10813; Mem0 arXiv 2504.19413; Zep arXiv 2501.13956; Anthropic Contextual Retrieval.
- ADR-02 (`embeddings_meta` previsto), ADR-03 (reranking diferido), ADR-04 (swap de embedder).
