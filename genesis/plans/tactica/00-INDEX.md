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

Documento detallado: `sprint-01/00-INDEX.md`.

Salida: `seele-core` y `seele-storage` funcionales con migraciones, CRUD sobre `sessions`/`observations`/`user_prompts`/`memory_relations`/`sync_chunks`/`links`, FTS5 + vec0 virtual tables, virtual generated columns + indexes, schema_version table, soft delete, topic key upserts, normalized hash dedup, privacy stripping. Tests integración pasando.

## Sprint-02 — Embedder + Search

Bloques previstos:
- A: `seele-embedder` (ort + tokenizers + hf-hub + auto-download + singleton).
- B: `seele-search` (fts + vec + RRF combiner + boost por score).
- C: tests integración con DB poblada.

## Sprint-03 — Interfaces

Bloques previstos:
- A: `seele-http` (axum + ~26 handlers + auth bearer + utoipa OpenAPI + middleware).
- B: `seele-mcp` (stdio + JSON-RPC 2.0 + 19 tools `seele_*`).
- C: tests E2E del binary contra HTTP + MCP.

## Sprint-04 — Ops & UX

Bloques previstos:
- A: `seele-tui` (ratatui + crossterm + Home/Browse/Search/Detail/Stats + keybindings vi-style).
- B: `seele-sync` (compressed chunks + git-friendly + sync_chunks dedup).
- C: `seele-setup` (8 agentes + `--all` + idempotent + backup).
- D: `seele-project` (5-case detection + child scan + skip noise dirs).
- E: `seele-cli` (clap + dispatch a todos + output JSON/human).

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
