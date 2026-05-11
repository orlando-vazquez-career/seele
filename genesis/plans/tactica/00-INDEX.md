# SEELE Genesis — Táctica

**Fecha**: 2026-05-10
**Estado**: en ejecución
**Fase previa**: Arquitectura (cerrada con 10 ADRs locked-in).

## Estructura de sprints v0.1

5 sprints estimados para llegar a v0.1 funcional (decisión del User: scope completo, sin cuts). Cada sprint es ejecutable end-to-end con tests verdes y código pusheado antes de pasar al siguiente.

| Sprint | Tema | Crates principales | LOC est. |
|---|---|---|---|
| **01** | Foundation | `seele-core`, `seele-storage` | ~2-3K |
| **02** | Embedder + Search | `seele-embedder`, `seele-search` | ~1.5-2K |
| **03** | Interfaces | `seele-http`, `seele-mcp` | ~2-2.5K |
| **04** | Ops & UX | `seele-tui`, `seele-sync`, `seele-setup`, `seele-project`, `seele-cli` | ~3-4K |
| **05** | Polish + CI/CD + Release | (cross-cutting + scripts + docs) | ~1-1.5K |

Total estimado v0.1: ~10-13K LOC + ~3-5K LOC de tests.

## Sprint-01 — Foundation

**Estado**: ✅ ejecutado — cerrado 2026-05-10. Plan archivado en `../executed/tactica/sprint-01/00-INDEX.md`. Devlog: [`../../../docs/aegis/devlogs/2026-05-10-sprint-01-foundation.md`](../../../docs/aegis/devlogs/2026-05-10-sprint-01-foundation.md).

Salida: `seele-core` y `seele-storage` funcionales con migraciones, CRUD sobre `sessions`/`observations`/`user_prompts`/`memory_relations`/`sync_chunks`/`links`, FTS5 + vec0 virtual tables, virtual generated columns + indexes, schema_version table, soft delete, topic key upserts, normalized hash dedup, privacy stripping. 99 tests verde en CI matrix Linux+macOS+Windows.

## Sprint-02 — Embedder + Search

**Estado**: ✅ ejecutado — cerrado 2026-05-10. Plan archivado en `../executed/tactica/sprint-02/00-INDEX.md`. Devlog: [`../../../docs/aegis/devlogs/2026-05-10-sprint-02-embedder-search.md`](../../../docs/aegis/devlogs/2026-05-10-sprint-02-embedder-search.md).

Salida: `seele-embedder` con cache `~/.seele/embedder/` + INT8 quantized default + SHA256 verify infra + singleton global. `seele-search` con boost por meta_score + empty-query path + annotation lines de relations + max_vec_distance. 130 tests verde (97 directos + 33 nuevos del Sprint-02) + 4 ignored (ONNX + perf smoke).

## Sprint-03 — Interfaces

**Estado**: ✅ ejecutado — cerrado 2026-05-10. Plan archivado en `../executed/tactica/sprint-03/00-INDEX.md`. Devlog: [`../../../docs/aegis/devlogs/2026-05-10-sprint-03-interfaces.md`](../../../docs/aegis/devlogs/2026-05-10-sprint-03-interfaces.md). Tag git: `sprint-03-interfaces`.

Salida: `seele-http` (~25 endpoints axum 0.8 con auth bearer opt-in + utoipa OpenAPI + Swagger UI + legacy ENGRAM paths per ADR-13). `seele-mcp` (JSON-RPC 2.0 stdio + 19 tools `seele_*` + `--tool-prefix mnema` ENGRAM-compat). Binary mínimo `seele [mcp|serve|--version]` con argv parser hand-rolled. 202 tests verde + 4 ignored. Bloques subdivididos: A skeleton, B handlers básicos, C.1 lifecycle, C.2 relations/stats/embedder, D auth+openapi+legacy-paths, E mcp+19 tools+cli, F E2E binary + state-sync.

## Sprint-04 — Ops & UX

**Estado**: ✅ ejecutado — cerrado 2026-05-10. Plan archivado en `../executed/tactica/sprint-04/00-INDEX.md`. Devlog: [`../../../docs/aegis/devlogs/2026-05-10-sprint-04-ops-ux.md`](../../../docs/aegis/devlogs/2026-05-10-sprint-04-ops-ux.md). Tag git: `sprint-04-ops-ux`.

Salida: 5 nuevos crates (`seele-project` 5-case detection, `seele-setup` wizard 3 agentes implementados + 5 skeleton, `seele-sync` gzip JSON chunks git-friendly, `seele-engram-import` ADR-13 migration tool, `seele-tui` ratatui 5 vistas) + `seele-cli` reescrito a `clap derive` con 17 subcommands. `seele import --from-engram` cierra la primera decisión de ADR-13 (MNEMA → SEELE transition). 304 tests verde + 4 ignored. Bloques: A `seele-project`, B `seele-setup`, C `seele-sync`, D.1+D.2 `seele-cli` clap, post-Cloven fixes, D.3 `seele-engram-import`, E `seele-tui`, F state-sync.

## Sprint-05 — Polish + CI/CD + Release

Bloques previstos:
- A: tests E2E completo (CLI + HTTP + MCP + TUI smoke).
- B: GitHub Actions CI matrix Linux + macOS + Windows + clippy + fmt + cargo-deny.
- C: Release pipeline (5 targets binarios + sha256 + auto-changelog).
- D: docs API HTTP autogenerada + READMEs + AGENT-SETUP.md + INSTALLATION.md.
- E: install scripts (sh + ps1) + Homebrew tap (defer v0.2) + crates.io publish.

## Convenciones operativas

- **Tests por bloque**: cada bloque cierra con tests verdes en el OS local. CI verde es criterio de cierre del sprint completo.
- **Commits por bloque**: cada bloque commitea + pushea al cierre. Mensaje: `sprint-NN bloque-X — <título corto>`.
- **Devlog por sprint**: al cerrar el sprint, devlog en `docs/aegis/devlogs/YYYY-MM-DD-sprint-NN-<tema>.md`.
- **Cost ledger**: append por sprint en `docs/aegis/devlogs/cost-ledger.jsonl`.
- **Static check STELE residual**: corre en cada commit (script existe).
- **Sin agentes en background**: regla heredada de AEGIS v2.0.0. Todo el trabajo en consola del orquestador.

## Criterio de salida v0.1

Cumplir los 11 criterios del `estrategia/04-scope-mvp.md` sección "Criterio de aceptación de v0.1":

1. Install via `cargo install seele` o binary.
2. CLI completo funcional.
3. MCP stdio conectable desde Claude Code con 19 tools.
4. HTTP API completo (~26 endpoints).
5. Performance search < 300ms a 10K observations.
6. Project detection paritario con ENGRAM en casos de prueba.
7. Git sync entre dos máquinas funciona.
8. TUI navegable (5 vistas).
9. Tests verdes en Linux + macOS + Windows.
10. Release binarios en GitHub Releases.
11. Crédito a ENGRAM en README + CREDITS + release notes.
