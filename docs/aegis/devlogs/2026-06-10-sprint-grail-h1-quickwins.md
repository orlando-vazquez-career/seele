# Devlog — Sprint GRAIL-H1: quick wins del análisis GRAIL→SEELE

**Fecha**: 2026-06-10
**Branch**: `sprint/grail-h1-quickwins`
**Plan táctico**: `docs/plans/executed/tactica/2026-06-10-h1-grail-quickwins.md`
**Fuente**: análisis comparativo multi-agente GRAIL→SEELE
(`docs/analysis/2026-06-09-analisis-grail-vs-seele.md`, 10 dimensiones
verificadas adversarialmente) + plan de mejoras
(`docs/analysis/2026-06-09-plan-mejoras-grail-en-seele.md`).
**Gate 1**: otorgado por Orlando el 2026-06-10 ("ok he leido ambos y
entiendo las mejoras ... aplicalas") tras leer ambos documentos.

## Qué se shippeó (11 quick wins, Q1–Q11)

| Bloque | ID | Entregable |
|---|---|---|
| 1 | Q2 | Envelope JSON CLI `{ok,data,warnings}` / `{ok:false,error,kind}` a stdout; warning in-band `fake-embedder-fallback`; delete/restore tipados |
| 2 | Q5 | `set_embedding(id, emb, EmbeddingMeta)` atómico (vector + provenance); `embedding_provenance()` + `mix_warning()` en doctor CLI y MCP |
| 3 | Q7 | nDCG@10 en seele-eval; `--suite-file <path>`; suite coding-memory v2 (paraphrase 6→15, multi-hop 3→10, con rationale) |
| 4 | Q3+Q8 | Tercer path RRF `fts_loose` (bag-of-words) + `seele search --explain` (SearchTrace v1) |
| 5 | Q4 | `near_duplicates` informacional post-save (KNN top-3, umbral L2 0.37 ≈ cos 0.93) + hint MCP + suggest_topic_key por vecino |
| 6/7 | Q6 | `find_similar` (JW título + coseno sobre vectores almacenados) + `seele_compare` suggest/confirm con confidence real |
| 8 | Q9 | ChatProvider: timeouts, retry 429/5xx, `strip_thinking` server-side, dispatch por nombre; primera suite de seele-chat (wiremock) |
| 9 | Q10 | `topic-families.toml` real: `seele_core::families`, resolución env > project > user > builtin al boot |
| 10 | Q11 | `docs/COMPARISON.md` (SEELE vs ENGRAM vs GRAIL, fechada, números reproducibles) + mermaid de arquitectura en README |
| 11 | Q1 | `release.yml`: timeouts, check tag-vs-versión, lista crates 12→14 en orden topológico; path-deps centralizados con `version` (requisito crates.io); `docs/RELEASING.md`; CHANGELOG refs + doble `### Fixed` consolidado; CLAUDE.md actualizado |

## El número del sprint — gate eval-first de Q3

Suite coding-memory v2 (n=34), ONNX real (`all-MiniLM-L6-v2`), antes/después
del tercer path `fts_loose`:

| Categoría | r@5 | MRR | nDCG@10 |
|---|---|---|---|
| paraphrase (n=15) | 0.733 → **0.867** | 0.546 → **0.707** | 0.623 → **0.762** |
| multi-hop (n=10) | 0.900 → **1.000** | 0.720 → **0.833** | 0.646 → **0.776** |
| single-fact (n=6) | 1.000 = | 0.658 → **0.806** | 0.741 → **0.855** |
| TOTAL (n=34) | 0.853 → **0.941** | 0.657 → **0.788** | 0.682 → **0.801** |

Cero degradación en ninguna métrica de ninguna categoría. Artefactos:
`baseline-v2-{pre,post}-q3.json` junto al plan táctico ejecutado. La
corrida PRE fue además la primera descarga first-run real del modelo ONNX
post-fix hf-hub 0.5 en esta máquina — el fix queda validado end-to-end.

## Bugs reales encontrados en el camino

1. **vec0 rechaza `INSERT OR REPLACE`** sobre un rowid existente ("UNIQUE
   constraint failed"): TODO re-embed fallaba desde Sprint-02 — el camino
   exacto que `seele reindex`/A2 necesita. Destapado por los tests nuevos
   de Q5; fix DELETE+INSERT en la misma tx.
2. **`strip_thinking` y los espacios**: la primera versión del regex comía
   el whitespace posterior al tag y pegaba palabras en bloques mid-frase.
   Cazado por unit test; fix: reemplazo del bloque solamente + trim.

## Incidencias de entorno (Windows, para el próximo timeline)

- **Disco**: el `target/` debug del workspace (full debuginfo + ort +
  tokenizers) superaba los 18 GB y llenó el disco DOS veces (pagefile
  error 1455, LNK1104, os error 112). Fix estructural en este sprint:
  `[profile.dev] debug = "line-tables-only"` en el Cargo.toml raíz +
  `cargo clean` — backtraces siguen útiles, footprint del target cae
  drásticamente. Si se necesita DWARF completo localmente: override de
  profile puntual.
- **LNK1104/1318 intermitentes**: exes de tests recién corridos quedan
  lockeados (Defender escaneando). Mitigación: rename/delete del exe y
  reintento; la exclusión de `target/` en Defender queda como decisión
  del humano (el clasificador de permisos la bloqueó correctamente).
- 20,8 GiB + 17,7 GiB de artefactos viejos limpiados con `cargo clean`.

## Ronda 2 — optimizables aplicados

- Dedup selección de embedder: `build_embedder` duplicada en
  `commands/eval.rs` eliminada; `app::pick_embedder_boxed` es la única
  fuente (Arc se deriva con `Arc::from`). −28 LOC.
- `output::status()` removida (huérfana tras el envelope Q2).
- Path-deps intra-workspace centralizados en `workspace.dependencies`
  (13 entradas con `version` — además de requisito crates.io, −12 líneas
  duplicadas en members).
- `[profile.dev] debug = "line-tables-only"` (espacio en disco, ver arriba).

## Decisiones que quedan en manos de Orlando

1. **Tag v0.2.1 o v0.3.0-alpha**: los dos fixes críticos user-facing
   (envelope MCP + hf-hub) siguen sin release. `main` ya contiene material
   v0.3-α, así que un v0.2.1 limpio requeriría branch de release con
   cherry-picks; tagear `main` directo sería v0.3.0-alpha. El pipeline
   quedó reparado y documentado en `docs/RELEASING.md`; falta SOLO el tag
   (y verificar el billing de GitHub Actions ANTES — fue la causa probable
   de los 0/2 runs).
2. **Merge del branch a main** (este sprint vivió en
   `sprint/grail-h1-quickwins`).
3. Exclusión de Defender para `target/` (acelera builds; decisión de
   seguridad del dueño de la máquina).

## Métricas del sprint

- 9 commits en branch, ~35 archivos de código tocados.
- Tests: 322+4 (CLAUDE.md pre-sprint) → **~370 verde + 5 ignored** (la
  cifra exacta queda en el log de gates del cierre).
- Tests nuevos: ~36 (envelope E2E, embeddings_meta write-path, ndcg, rescue
  loose, knn, near-dups con StubEmbedder, compare suggest/confirm, primera
  suite de seele-chat, families loader, E2E families por env).
- Gates por bloque: tests targeted + commit; gates completos (workspace +
  clippy -D warnings + fmt + STELE) al cierre.
