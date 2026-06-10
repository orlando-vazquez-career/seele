# Plan táctico — Horizonte 1: quick wins GRAIL → SEELE

**Fecha**: 2026-06-10
**Branch**: `sprint/grail-h1-quickwins`
**Fuente**: [Plan de mejoras](../../analysis/2026-06-09-plan-mejoras-grail-en-seele.md) §3 (Horizonte 1), derivado del [análisis comparativo](../../analysis/2026-06-09-analisis-grail-vs-seele.md) de 10 dimensiones verificado adversarialmente.
**Gate 1 (aprobación humana)**: otorgado por Orlando el 2026-06-10 — *"ok he leido ambos y entiendo las mejoras ... aplicalas"* — tras leer análisis y plan completos. Alcance autorizado: Horizonte 1 (Q1–Q11) + ronda de optimizables encontrados en el camino.

## Alcance

Los 11 quick wins del plan de mejoras, en orden de ejecución por dependencias:

| Orden | ID | Bloque | Crates tocados |
|-------|----|--------|----------------|
| 1 | Q2 | Envelope JSON uniforme + contrato de error CLI | seele-cli |
| 2 | Q5 | Cablear `embeddings_meta` + guard en doctor | seele-storage, seele-http, seele-cli, seele-mcp, seele-eval, seele-search (helpers) |
| 3 | Q7 | nDCG@10 + `--suite-file` + suites v2 (paraphrase 6→15, multi-hop 3→10) | seele-eval, seele-cli |
| 4 | Q3 | Tercer path RRF `fts_loose` (gate eval contra suite v2) | seele-search |
| 5 | Q4 | `near_duplicates` informacional post-save (umbral L2 ~0.37) | seele-search, seele-http, seele-mcp |
| 6 | Q8 | `seele search --explain` (trace v1 CLI-only) | seele-search, seele-cli |
| 7 | Q6 | `find_similar` + `seele_compare` modo suggest (JW≥0.92, cos≥0.93) | seele-http (service), seele-mcp, seele-core (config) |
| 8 | Q9 | ChatProvider: timeout/retry/strip/dispatch + primera suite de tests | seele-chat, seele-http |
| 9 | Q10 | `topic-families.toml` real | seele-core, seele-http (service), seele-mcp |
| 10 | Q11 | COMPARISON.md + mermaid README | docs |
| 11 | Q1 | release.yml + RELEASING.md + deriva CHANGELOG/CLAUDE.md | .github, docs |

**Nota Q1**: la reparación del workflow y el runbook entran en este sprint; el **tag v0.2.1 es acción pública** que decide Orlando al cierre (main ya contiene material v0.3-α — decidir tag desde main vs branch release con cherry-picks).

**Nota Q3 (gate eval)**: el merge del tercer path queda condicionado a que `seele eval` con la suite v2 muestre paraphrase r@5 > baseline sin degradar single-fact/temporal. Si ONNX no está disponible en esta máquina, el gate corre con FakeEmbedder solo para smoke y la validación real queda documentada como pendiente de la corrida con ONNX.

## Disciplina

- AEGIS: sin sub-agentes; ejecución en consola, bloque por bloque.
- Cada bloque cierra con: tests del crate afectado verdes → `cargo test --workspace` → `clippy -D warnings` → `fmt --check` → STELE residual → commit `h1-grail bloque-NN — <título>`.
- CHANGELOG `[Unreleased]` se actualiza en el mismo commit de cada bloque que cambie API/behavior visible.
- Optimizables encontrados en el camino: se anotan en este archivo (§ Optimizables) y se aplican en una **ronda 2** al final, no inline (para no contaminar el diff de cada bloque).
- Cierre del sprint: devlog `2026-06-10-sprint-h1-grail-quickwins.md`, este plan → `executed/`, `docs/INDEX.md` + `CLAUDE.md` actualizados, cost-ledger.

## Estado de bloques

- [x] Bloque 0 — baseline gates verde en branch (limpieza: 20,8 GiB de target/ viejos; root cause de los fallos de build era pagefile/disk-full)
- [x] Bloque 1 — Q2 (envelope `ok/data/warnings` + errores JSON a stdout + kind tipado; E2E migrados + 3 tests nuevos de contrato)
- [x] Bloque 2 — Q5 (set_embedding atómico vector+meta; `embedding_provenance()` + `mix_warning()` compartido; doctor CLI+MCP; **bonus: bug latente vec0 INSERT OR REPLACE descubierto y arreglado**)
- [x] Bloque 3 — Q7 (nDCG@10 + --suite-file + suite v2: paraphrase 15, multi-hop 10; baseline PRE capturado con ONNX real — primer first-run download exitoso del fix hf-hub 0.5)
- [x] Bloque 4 — Q3+Q8 (**GATE APROBADO**: paraphrase r@5 +0.133 / MRR +0.161, multi-hop r@5 1.000, cero degradación; --explain expone fts_candidates=0 en paráfrasis; artefactos baseline-v2-pre/post-q3.json)
- [x] Bloque 5 — Q4 (knn_by_vector + near_duplicates en SaveResponse + hint MCP + suggest_topic_key por vecino; umbral L2 0.37 ≈ cos 0.93; 5 tests nuevos con StubEmbedder)
- [x] Bloque 6 — Q8 (entregado junto al bloque 4: search_traced + --explain CLI)
- [x] Bloque 7 — Q6 (find_similar 2 señales + compare suggest/confirm con confidence real; umbrales en seele_core::similarity; get_embedding nuevo; 5 tests)
- [x] Bloque 8 — Q9 (timeouts + retry/429 + strip_thinking server-side + dispatch por nombre; primera suite de seele-chat: 7 tests)
- [x] Bloque 9 — Q10 (seele_core::families + resolución en capas + E2E hermético por env var; suggest reporta families_source)
- [x] Bloque 10 — Q11 (COMPARISON.md fechada con números del harness + primer mermaid del README)
- [x] Bloque 11 — Q1 (release.yml: timeouts + tag-check + 14 crates; workspace.dependencies con version; RELEASING.md; CHANGELOG refs + doble Fixed consolidado; CLAUDE.md estado)
- [x] Ronda 2 — optimizables (pick_embedder dedup −28 LOC; status() removida; path-deps centralizados −12 líneas; profile.dev line-tables-only)
- [x] Cierre AEGIS (devlog 2026-06-10 + INDEX + cost-ledger + plan → executed/)

## Optimizables encontrados en el camino

Aplicados en Ronda 2:

1. **Dup `pick_embedder`/`build_embedder`** (app.rs vs commands/eval.rs) —
   unificado en `app::pick_embedder_boxed`; eval mantiene su warning de
   baseline sobre la fuente única.
2. **`output::status()` huérfana** tras el envelope Q2 — removida.
3. **Path-deps sin `version`** (bloqueante crates.io) — centralizados en
   `workspace.dependencies` (13 entradas), members con `.workspace = true`.
4. **Footprint del target debug** (>18 GB, llenó el disco 2×) —
   `[profile.dev] debug = "line-tables-only"`.

Anotados para H2/H3 (no aplicados acá):

- Doctor duplicado CLI/MCP — la consolidación natural llega con `seele
  probe` (T5).
- `default_model_for` triplicado en chat (handlers/serve/ChatPanel) — lo
  resuelve el registry de providers (E6).
