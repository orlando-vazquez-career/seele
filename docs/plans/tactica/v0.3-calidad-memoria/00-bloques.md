# Táctica — Sprint v0.3-α (B1 harness + A6 higiene) · pre-Gate 1

**Estado**: Borrador pre-stageado · 2026-05-29 · **⏸ Espera Gate 1 humano antes de Ejecución**
**Plan macro**: `docs/plans/estrategia/v0.3-calidad-memoria/00-overview.md`
**ADR**: `docs/plans/arquitectura/14-evaluation-harness.md`

> Todas las tareas las ejecuta el orquestador en consola (AEGIS v2.x, sin sub-agentes). Cada bloque commitea + pushea al cierre. Devlog al final del sprint. **No mover este plan a `executed/` hasta el Gate 2.**

## Bloque A — A6: higiene y correctitud (rápido, primero)

- **T-A1** · Sincronizar versión en `README.md` (`Status: v0.1.0` → `v0.2.0`) con lo que dice `CHANGELOG.md`. Revisar otras menciones de versión.
  - *Done*: `grep -rn "v0.1.0" README.md` no devuelve el badge/status desactualizado; CI verde.
- **T-A2** · `int_id` — riesgo real (redirigido en Gate 1). `SeeleId::as_i64()` usa el **tail aleatorio** (bytes 9..16, 56 bits), no los primeros 6 bytes; fresh-save ya reintenta en colisión (`ID_COLLISION_RETRIES`) y la unicidad en 10K ya está testeada (`seele-core/src/id.rs`). El hueco es el **silent-drop en import**: `save_raw_in_tx` usa `INSERT OR IGNORE`, así que un ULID preservado con `int_id` colisionante se descarta en silencio (data loss enmascarado como skip idempotente). **Fix (decisión A):** `save_raw` detecta filas-afectadas=0 con PK inexistente = colisión y lo **reporta** en vez de tragárselo.
  - *Done*: test en `seele-storage` que construye 2 ULIDs con `as_i64()` colisionante y verifica que el import **reporta** la colisión (no la descarta); fix aplicado en `save_raw_in_tx`.
- **T-A3** · Append nota a `CHANGELOG.md` `[Unreleased]` documentando el inicio del trabajo v0.3 (sin marcar release).

## Bloque B — B1: crate `seele-eval` (núcleo del sprint)

- **T-B1** · Scaffold del crate `seele-eval` (#13 del workspace): `Cargo.toml`, `lib.rs`, registro en el workspace root. Depende de `seele-search`/`seele-storage`/`seele-core`.
  - *Done*: `cargo build -p seele-eval` verde.
- **T-B2** · Loader de fixtures JSON con el contrato del ADR-14 (`{corpus:[...], queries:[...]}`). Ingesta a DB temporal (`--db` efímero o tmpfile).
  - *Done*: test unitario que carga un fixture mínimo y verifica el parse + ingest.
- **T-B3** · Runner: por cada query, correr el `search` híbrido actual (FTS+vec+RRF, embedder real), comparar top-k con `expected_ids`, computar recall@k (5 y 10) + MRR, agregado por categoría. Emitir JSON + tabla stdout.
  - *Done*: `cargo test -p seele-eval -- --ignored` corre end-to-end sobre el fixture mínimo y emite el JSON.
- **T-B4** · Subcomando `seele eval --suite <name> [--k N] [--json]` en `seele-cli` que invoca el runner.
  - *Done*: `seele eval --suite coding-memory --json` produce salida válida; `--help` lo lista.
- **T-B5** · Construir `coding-memory` (mini-corpus propio, ~20-40 Q&A con topic-keys reales) y `longmemeval-subset` (~50/500, categorías multi-session/knowledge-update/temporal). Si LongMemEval pesa, aplicar el fallback del ADR (harness casero) y dejar la pista académica como TODO con issue.
  - *Done*: ambas suites cargan; queda al menos una corrida completa.
- **T-B6** · **Capturar el baseline v0.2** (all-MiniLM + RRF, sin reranker) sobre ambas suites; versionar el JSON en `docs/plans/tactica/v0.3-calidad-memoria/baseline-v0.2.json` y resumir en el devlog.
  - *Capturar con*: `cargo test -p seele-eval --test baseline -- --ignored --nocapture` (o `seele eval --suite <name> --json`) en una máquina con acceso a HuggingFace. La descarga del modelo ONNX no corre en el sandbox de CI (mismo motivo por el que los tests `#[ignore]` del embedder son "run locally") — el baseline se captura local.
  - *Done*: número por categoría registrado y reproducible.

## Bloque C — Arquitectura `embeddings_meta` (solo diseño/migración, no consumo aún)

- **T-C1** · Migración refinery que crea la tabla `embeddings_meta` (esquema del ADR-14) + índice. Backfill de las filas existentes con `model_id='all-MiniLM-L6-v2', dim=384, contextualized=0`.
  - *Done*: migración up aplica idempotente; tests de storage verdes. (El **consumo** en search llega con A2/B3, sprints posteriores.)

## Guardrails (antes del Gate 2)

- `cargo test --workspace` verde (+ `seele-eval` con `--ignored` corrido manualmente).
- `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt --all -- --check`.
- `bash scripts/check-no-stele-residual.sh` (o `.ps1`) verde.
- El harness **no** corre por default en CI (marcado `#[ignore]`).
- Code review propio del diff; sin secrets; sin tests `assert!(true)`.

## State-sync (post Gate 2)

1. Mover este plan a `docs/plans/executed/tactica/v0.3-calidad-memoria/`.
2. Devlog `docs/aegis/devlogs/2026-MM-DD-sprint-v0.3-alpha-eval-harness.md`.
3. Actualizar `CLAUDE.md` (nuevo crate `seele-eval` #13, baseline, `embeddings_meta`) + `docs/INDEX.md`.
4. `CHANGELOG.md` `[Unreleased]`: Added `seele-eval`, `embeddings_meta`; Fixed README drift + int_id.
5. `seele save --type decision --topic-key sprint/v0.3/eval-first` con resumen + link al devlog (query-before-act / save-after-decide de AEGIS v2.1).
6. Commit + push. Append `cost-ledger.jsonl`.

## Dependencias

- T-A2 (int_id) **antes** de confiar en el baseline T-B6.
- Bloque B depende del scaffold T-B1.
- T-C1 es independiente (puede ir en paralelo); su consumo se difiere a A2/B3.
- **Gate 1**: el humano revisa este plan antes de Ejecución. **Gate 2**: aprueba el cierre antes de State-sync.
