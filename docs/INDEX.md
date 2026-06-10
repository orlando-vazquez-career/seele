# SEELE — docs index

Índice canónico de documentación del repo. Toda nueva entrada de devlog, ADR ejecutado, plan cerrado o decisión persistente se referencia desde acá.

## Project

- [`README.md`](../README.md) — qué es SEELE, qué hace, créditos a ENGRAM.
- [`CHANGELOG.md`](../CHANGELOG.md) — Keep-a-Changelog (Unreleased + histórico de bumps de deps vendorizadas).
- [`CREDITS.md`](../CREDITS.md) — atribución detallada a ENGRAM (MIT, Copyright Gentleman-Programming).
- [`LICENSE`](../LICENSE) — MIT 2026 DevZen SpA.
- [`CLAUDE.md`](../CLAUDE.md) — reglas operativas del repo para Claude Code.

## Guías de usuario (Sprint-05)

- [`docs/INSTALLATION.md`](INSTALLATION.md) — install matrix (scripts +
  cargo + source), embedder cache, troubleshooting.
- [`docs/AGENT-SETUP.md`](AGENT-SETUP.md) — wiring SEELE como MCP
  server en Claude Code / Cursor / Windsurf + status de los 5 skeletons.
- [`docs/ENGRAM-MIGRATION.md`](ENGRAM-MIGRATION.md) — pasar de
  ENGRAM/MNEMA a SEELE: `import from-engram`, compat layer
  `--tool-prefix mnema`, caveats.
- [`docs/COMPARISON.md`](COMPARISON.md) — SEELE vs ENGRAM vs GRAIL,
  fechada, con números reproducibles del harness (GRAIL-H1, Q11).
- [`docs/RELEASING.md`](RELEASING.md) — runbook de release: antes /
  tagear / después / fallos comunes (GRAIL-H1, Q1).

## Análisis comparativos (`docs/analysis/`)

- [`2026-06-09-analisis-grail-vs-seele.md`](analysis/2026-06-09-analisis-grail-vs-seele.md)
  — análisis multi-agente de 10 dimensiones GRAIL→SEELE, verificado
  adversarialmente claim por claim contra ambos repos.
- [`2026-06-09-plan-mejoras-grail-en-seele.md`](analysis/2026-06-09-plan-mejoras-grail-en-seele.md)
  — 37 propuestas consolidadas en 3 horizontes (H1 quick wins ejecutado
  en sprint GRAIL-H1; H2 v0.3; H3 v0.4+) + sección "no adoptar".

## Genesis (planes históricos del diseño)

Estrategia, arquitectura y táctica del Sprint 01 viven bajo `genesis/plans/`. El subdirectorio `executed/` recibe planes que ya cerraron su ciclo AEGIS.

### Estrategia

- [`genesis/plans/estrategia/00-INDEX.md`](../genesis/plans/estrategia/00-INDEX.md) — overview de la fase.
- `01-overview.md`, `02-reimplementacion-inspirada.md`, `03-naming-options.md`, `04-scope-mvp.md`, `05-engram-feature-audit.md`.

### Arquitectura

- [`genesis/plans/arquitectura/00-INDEX.md`](../genesis/plans/arquitectura/00-INDEX.md) — overview de los ADRs.
- ADRs `01-rust-y-crates.md` … `10-mapping-mnema-seele.md` + `11` (sqlite-vec vendorizado, follow-up Cloven) + `12` (embedder hardening followups, Sprint-02) + `13` (engram compatibility, Sprint-04).

### Táctica

- [`genesis/plans/tactica/00-INDEX.md`](../genesis/plans/tactica/00-INDEX.md) — sprints v0.1.

### Ejecutados (planes cerrados)

- [`genesis/plans/executed/tactica/sprint-01/00-INDEX.md`](../genesis/plans/executed/tactica/sprint-01/00-INDEX.md) — Sprint-01 BE Foundation, cerrado 2026-05-10. Devlog: [`docs/aegis/devlogs/2026-05-10-sprint-01-foundation.md`](aegis/devlogs/2026-05-10-sprint-01-foundation.md).
- [`genesis/plans/executed/tactica/sprint-02/00-INDEX.md`](../genesis/plans/executed/tactica/sprint-02/00-INDEX.md) — Sprint-02 BE Embedder + Search, cerrado 2026-05-10. Devlog: [`docs/aegis/devlogs/2026-05-10-sprint-02-embedder-search.md`](aegis/devlogs/2026-05-10-sprint-02-embedder-search.md).
- [`genesis/plans/executed/tactica/sprint-03/00-INDEX.md`](../genesis/plans/executed/tactica/sprint-03/00-INDEX.md) — Sprint-03 BE Interfaces (HTTP + MCP), cerrado 2026-05-10. Devlog: [`docs/aegis/devlogs/2026-05-10-sprint-03-interfaces.md`](aegis/devlogs/2026-05-10-sprint-03-interfaces.md). Tag `sprint-03-interfaces`.
- [`genesis/plans/executed/tactica/sprint-04/00-INDEX.md`](../genesis/plans/executed/tactica/sprint-04/00-INDEX.md) — Sprint-04 Ops & UX, cerrado 2026-05-10. Devlog: [`docs/aegis/devlogs/2026-05-10-sprint-04-ops-ux.md`](aegis/devlogs/2026-05-10-sprint-04-ops-ux.md). Tag `sprint-04-ops-ux`.
- [`genesis/plans/executed/tactica/sprint-05/00-INDEX.md`](../genesis/plans/executed/tactica/sprint-05/00-INDEX.md) — Sprint-05 Polish + CI/CD + Release, cerrado 2026-05-11. Devlog: [`docs/aegis/devlogs/2026-05-11-sprint-05-polish-release.md`](aegis/devlogs/2026-05-11-sprint-05-polish-release.md). Tag AEGIS `sprint-05-polish-release`; SemVer release tag `v0.1.0`.

## Planes post-génesis (`docs/plans/`)

- **Arquitectura**: [`14-evaluation-harness.md`](plans/arquitectura/14-evaluation-harness.md) — ADR-14 (harness eval-first + `embeddings_meta`); [`15-binary-size-footprint.md`](plans/arquitectura/15-binary-size-footprint.md) — ADR-15 (footprint del binario, feature-gating propuesto).
- **Sprint GRAIL-H1 (ejecutado)**: [`executed/tactica/2026-06-10-h1-grail-quickwins.md`](plans/executed/tactica/2026-06-10-h1-grail-quickwins.md) — 11 quick wins GRAIL→SEELE, cerrado 2026-06-10. Devlog: [`2026-06-10-sprint-grail-h1-quickwins.md`](aegis/devlogs/2026-06-10-sprint-grail-h1-quickwins.md). Baselines del gate: `baseline-v2-{pre,post}-q3.json`.
- **v0.3-α «calidad de memoria»**: [`estrategia/00-overview.md`](plans/estrategia/v0.3-calidad-memoria/00-overview.md), [`tactica/00-bloques.md`](plans/executed/tactica/v0.3-calidad-memoria/00-bloques.md), [`baseline-v0.2.json`](plans/executed/tactica/v0.3-calidad-memoria/baseline-v0.2.json).

## Devlogs

- [`docs/aegis/devlogs/2026-05-10-sprint-01-foundation.md`](aegis/devlogs/2026-05-10-sprint-01-foundation.md) — Sprint-01 BE Foundation cerrado. 99 tests verde, CI matrix verde, schema completo + CRUD funcional.
- [`docs/aegis/devlogs/2026-05-10-sprint-02-embedder-search.md`](aegis/devlogs/2026-05-10-sprint-02-embedder-search.md) — Sprint-02 BE Embedder + Search cerrado. Embedder polish (cache + INT8 + SHA256 + singleton) + search polish (boost + empty-query + annotations + max-distance) + 31 tests nuevos. Total 130 verde.
- [`docs/aegis/devlogs/2026-05-10-sprint-03-interfaces.md`](aegis/devlogs/2026-05-10-sprint-03-interfaces.md) — Sprint-03 BE Interfaces cerrado. HTTP axum ~25 endpoints (auth bearer + OpenAPI + Swagger UI + legacy-engram-paths) + MCP stdio JSON-RPC 19 tools + binary `seele [mcp|serve|--version]`. Total 202 verde + 4 ignored.
- [`docs/aegis/devlogs/2026-05-10-sprint-04-ops-ux.md`](aegis/devlogs/2026-05-10-sprint-04-ops-ux.md) — Sprint-04 Ops & UX cerrado. 5 nuevos crates (project, setup, sync, engram-import, tui) + binary `seele` con clap derive 17 subcomandos. ENGRAM migration tool desbloquea MNEMA → SEELE (ADR-13). Total 304 verde + 4 ignored.
- [`docs/aegis/devlogs/2026-05-11-sprint-05-polish-release.md`](aegis/devlogs/2026-05-11-sprint-05-polish-release.md) — Sprint-05 Polish + CI/CD + Release cerrado. Property tests workspace-wide + ONNX default + release pipeline 5 targets + install scripts + docs polish (README + 3 guías) + smoke acceptance. Total 322 verde + 4 ignored. v0.1.0 cierra v0.1.
- [`docs/aegis/devlogs/2026-05-20-patch-mcp-call-tool-result.md`](aegis/devlogs/2026-05-20-patch-mcp-call-tool-result.md) — patch del envelope `CallToolResult` (MCP `tools/call`) + test de regresión.
- [`docs/aegis/devlogs/2026-05-29-sprint-v0.3-alpha-eval-harness.md`](aegis/devlogs/2026-05-29-sprint-v0.3-alpha-eval-harness.md) — Sprint v0.3-α eval-first: crate `seele-eval` (recall@k/MRR), baseline v0.2 (coding-memory r@5 0.83), fix silent-drop `int_id`, `embeddings_meta` (V002), ADR-15. (Pendiente clippy + merge a main.)
- [`docs/aegis/devlogs/2026-06-10-sprint-grail-h1-quickwins.md`](aegis/devlogs/2026-06-10-sprint-grail-h1-quickwins.md) — Sprint GRAIL-H1: los 11 quick wins del plan GRAIL→SEELE. Gate eval de `fts_loose`: paraphrase r@5 0.733→0.867, TOTAL MRR 0.657→0.788, cero degradación. Bug latente vec0 INSERT OR REPLACE cazado y arreglado. Release pipeline reparado.

## Cost Ledger

- [`docs/aegis/devlogs/cost-ledger.jsonl`](aegis/devlogs/cost-ledger.jsonl) — append-only por plan, modelo, fase. Ver convenciones en `C:/dev/protocols/AEGIS/guides/cost-ledger.md`.

## Operación

- [`scripts/check-no-stele-residual.sh`](../scripts/check-no-stele-residual.sh) y `.ps1` — static check para residuos del nombre legacy. Corre en CI.
- [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) — matriz Linux+macOS+Windows + jobs lint y static-checks.
- [`.github/dependabot.yml`](../.github/dependabot.yml) — monitor de bumps (especialmente `ort`).
