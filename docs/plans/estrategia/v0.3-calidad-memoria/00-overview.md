# Estrategia v0.3 — Calidad de memoria (evaluation-first)

**Estado**: Borrador pre-stageado · 2026-05-29 · **Pendiente Gate 1 humano**
**Origen**: Auditoría externa "SEELE vs SOTA memory engines" corrida en Cowork con un Counsel MNEMA completo (5 advisors + 5 reviewers ciegos + veredicto, `claude-opus-4-8`).
**Reporte fuente**: `C:/dev/sandbox/experiments/claude-cowork-dispatch/Reporte_SEELE_Auditoria-Memory-Engines_2026-05-29.md`
**ADR asociado**: `docs/plans/arquitectura/14-evaluation-harness.md` (ADR-14).
**Táctica**: `docs/plans/executed/tactica/v0.3-calidad-memoria/00-bloques.md`.

> ⚠️ Este documento fue redactado fuera de Claude Code (en Cowork) como handoff. La ejecución se hace en Claude Code bajo AEGIS. **No mover a `executed/` ni commitear sin el Gate 1 + Gate 2** correspondientes.

## TL;DR de la decisión

v0.3 es un release **evaluation-first**. No se mergea ninguna mejora de retrieval (reranking, swap de embedder, query expansion) **hasta tener un número de baseline reproducible**. El veredicto del counsel (argumento mejor puntuado, 13.8/15, "más sólido" para los 5 reviewers): *optimizar sin medir contamina la línea base y vuelve injustificable todo lo demás*.

## Por qué ahora

El estado del arte 2026 de memory engines para agentes (Mem0, Zep/Graphiti, Letta, cognee, A-MEM) es **benchmark-driven**: todos reportan LoCoMo / LongMemEval. SEELE **no tiene ninguna evaluación de calidad de memoria** — solo perf smoke a 1K/10K. El README afirma una "apuesta de retrieval distinta a ENGRAM" que hoy no se puede sostener con datos. Cerrar esa brecha es prerequisito de cualquier otra mejora y, a la vez, un activo publicable (primer score local-first auditable).

## Alcance (in)

- **B1 — Crate `seele-eval`**: harness que corre un subset de LongMemEval **+ un mini-corpus de coding-agent memory** contra el pipeline de búsqueda actual y emite recall@k / MRR por categoría a JSON. Baseline de v0.2 registrado.
- **A6 — Higiene y correctitud**: sincronizar `README.md` (decía v0.1.0) con `CHANGELOG.md` (v0.2.0) + reflejar features v0.2 (web + `seele-chat`); corregir doc-drift de `int_id` (`as_i64` usa el tail aleatorio bytes 9..16, no los primeros 6) y conteo de crates (13); arreglar el **silent-drop de `int_id` en import** (`save_raw` con `INSERT OR IGNORE`).
- **A7 — Footprint del binario** (nuevo, ADR-15): investigar/reducir el tamaño en disco (binario release 42.55 MB; feature-gating de `onnx` + HTTP/Swagger). Ortogonal al eval — no contamina el baseline.
- Decisión de arquitectura para **`embeddings_meta`** (provenance de embeddings) como **prerequisito duro** de A2/B3 — diseñada en ADR-14, implementada cuando A2/B3 entren.

## Alcance (out / diferido)

- **A1 reranking, A2 embedder multilingüe**: planificados pero **gated** por el número de B1 (sprint v0.3-β). No entran en este sprint.
- **B2 consolidación LLM, B3 Contextual Retrieval**: sprint v0.3-γ, opt-in y local-capable, **después** de `embeddings_meta`.
- **B4 capa bi-temporal, B5 grafo de entidades**: **diferidos** (riesgo de charter + carga de mantenimiento para dev solo + rompen idempotencia de sync/import ENGRAM). Sustituir su necesidad temporal subiendo el `earn_score` decay de MNEMA a SEELE-core como señal opt-in (v0.4, a evaluar).
- **A4 query expansion/HyDE, A5 ANN**: parqueados (ADR-03 ya los ubicaba en v0.3+; ANN solo si hay presión real >1M).

## Actores y restricciones

- **Mantenedor**: un dev solo. Criterio rector: cada feature debe (a) mover un número de calidad medible, o (b) reducir la fricción de adopción/confianza de un usuario que no es el autor.
- **Charter (reescrito explícito, cierra el sight del Outsider)**: local-first, **offline-by-default con LLM opt-in y local-capable** (no "offline-siempre"), CPU-only, single-binary, deps mínimas, agnóstico del schema del consumer. `seele-chat` + endpoint local (Ollama/llama.cpp) hace que B2/B3 sean 100% locales si se apunta a `localhost`.

## Criterios de éxito del sprint v0.3-α

1. `cargo test -p seele-eval -- --ignored` corre end-to-end y emite un JSON con recall@k por categoría.
2. Existe un **baseline numérico de v0.2** (all-MiniLM + RRF, sin reranker) sobre subset LongMemEval **y** sobre el mini-corpus de coding-memory.
3. `README.md` y `CHANGELOG.md` consistentes en versión.
4. Test que verifica que el mapeo `int_id` no colisiona en el corpus de prueba (o documenta el riesgo + mitigación).
5. Clippy + fmt + STELE residual checks en verde; el harness no corre por default en CI (marcado `#[ignore]` por descarga/coste).

## Decisiones abiertas para Arquitectura (resueltas en ADR-14)

- Tamaño del subset LongMemEval (~50/500) y composición del mini-corpus coding-memory.
- Métrica primaria (recall@k binaria vs MRR vs nDCG) y `k`.
- Forma del crate `seele-eval` (binario aparte vs subcomando `seele eval`).
- Esquema de `embeddings_meta` y su relación con el re-embed.

## Bloqueantes / riesgos

- **Representatividad del benchmark** (sight Contrarian): LoCoMo/LongMemEval miden memoria conversacional; SEELE guarda coding-agent memory. Mitigación: doble pista (académico + corpus propio).
- **`int_id`**: si colisiona, todo número de retrieval es ruido. Verificar **antes** de confiar en el baseline.
- No expandir scope a A1/A2 "de paso" — es exactamente el fallo fatal #1 del counsel.

## Referencias

- Veredicto y counsel completo: §5 del reporte fuente.
- ADRs SEELE relacionados: `genesis/plans/arquitectura/02-schema-sqlite.md`, `03-search-hybrid.md`, `04-embedder-onnx.md` (este sprint extiende sus "out of scope v0.2/v0.3").
- AEGIS: `C:/dev/protocols/AEGIS/AEGIS-PROTOCOL.md` (query-before-act / save-after-decide).
