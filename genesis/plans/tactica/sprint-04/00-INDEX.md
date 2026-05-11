# Sprint-04 — Ops & UX

**Fecha**: 2026-05-10
**Estado**: Tactica escrita, ejecucion pendiente
**Pre-requisitos**: Sprint-03 BE Interfaces cerrado (tag `sprint-03-interfaces`) + post-cloven fixes (commit `3cc3e32`).

## Objetivo

Cerrar los 5 crates restantes (`seele-project`, `seele-setup`, `seele-sync`, `seele-tui`, `seele-cli` completo con clap) y entregar el `seele import --from-engram` (ADR-13) que MNEMA necesita para migrar.

Al cerrar este sprint, SEELE tiene todo lo que un usuario espera de un memory engine local:
- CLI completa para uso humano directo (`seele save`, `seele search`, `seele list`, `seele show`, `seele import`, `seele sync`, `seele setup`).
- TUI navegable con `ratatui`.
- Sync multi-maquina via chunks comprimidos git-friendly.
- Setup wizard que configura los 8 agentes que SEELE espera (Claude Code, Cursor, Windsurf, OpenCode, Aider, Cody, Continue, Zed).
- Project detection que reconoce 5 casos (monorepo, multi-package, lang-mixto, single, embedded).

## Bloques

**A — `seele-project` (project detection)**: 5-case detection (monorepo / multi-package / lang-mixto / single / embedded) + child scan + skip noise dirs (`node_modules`, `target`, `.git`, etc.). API: `Project::detect(path) -> ProjectInfo`. ~250-350 LOC + ~10 tests.

**B — `seele-setup` (wizard 8 agentes)**: instala configs para Claude Code, Cursor, Windsurf, OpenCode, Aider, Cody, Continue, Zed. Cada agente tiene su path canonico de config + formato propio. Flags: `--all`, `--agent <name>`, `--dry-run`, `--backup`. Idempotente: re-correr no duplica entries. API: `Setup::install(agents, opts) -> InstallReport`. ~400-500 LOC + ~15 tests.

**C — `seele-sync` (chunks comprimidos git-friendly)**: serializa observations en chunks JSON gzip-comprimidos, ~1MB cada uno, en `.seele-sync/<project>/chunks/<sha256>.json.gz`. Tabla `sync_chunks` (ya existe en schema) trackea (chunk_id, sha256, size, observation_ids). API: `Sync::export(project) -> ChunkSet`, `Sync::import(chunks) -> ImportReport`. Dedup via sha256 — los chunks identicos no se re-importan. ~350-450 LOC + ~12 tests.

**D — `seele-cli` clap completo (incluye `import --from-engram`)**:

- D.1 — Migracion a clap. Reemplaza el argv parser hand-rolled de Sprint-03 con `clap derive`. Subcommands: `save`, `search`, `show`, `list`, `delete`, `restore`, `link`, `sync`, `import`, `setup`, `stats`, `doctor`, `mcp`, `serve`, `tui`. ~300 LOC.
- D.2 — Comandos save/search/show/list que llaman al service local (no HTTP). Output flag `--json` / `--human` (default human). ~250 LOC + ~8 tests.
- D.3 — `seele import --from-engram <db-path> [--re-embed] [--dry-run]` (ADR-13). Lee `memories` table ENGRAM, mapea a `observations`, preserva ULIDs, migra `metadata.linked_to[]` a `links`. Idempotente. ~300 LOC + ~6 tests.

**E — `seele-tui` (ratatui)**:

- E.1 — Setup ratatui + crossterm + event loop + 5 vistas declaradas. Vistas: Home (overview/stats), Browse (lista filtrable), Search (input + resultados), Detail (1 observation + links + annotations), Stats (charts ASCII). Keybindings vi-style (j/k navegacion, / search, q quit, Tab cambiar vista). ~600-800 LOC.
- E.2 — Snapshot tests con `insta` + `TestBackend`. Cobertura: 5 vistas + transiciones + edge cases (DB vacia, error). ~10 snapshots.

**F — State-sync + tag**:
- Devlog `docs/aegis/devlogs/YYYY-MM-DD-sprint-04-ops-ux.md`.
- Plan a `executed/`.
- CHANGELOG + INDEX + CLAUDE.md + memoria.
- Tag `sprint-04-ops-ux`.
- Cloven review.

## Decisiones tecnicas locked-in

1. **CLI llama al service local, no al HTTP.** El CLI usa `SeeleService::new(pool, embedder)` directo. HTTP es para consumers externos (web UI, scripts no-Rust). Para uso humano local, no hay razon de pasar por el loopback.

2. **TUI llama al service local, no al HTTP.** Mismo razonamiento.

3. **`seele-cli` cambia de FakeEmbedder a OnnxEmbedder por default** (Cloven sight Sprint-03). FakeEmbedder solo via `--fake-embedder` (testing). Si la descarga del modelo falla, warn explicito + caer a Fake.

4. **`seele-sync` deduplica por SHA256 del payload completo del chunk** (no por observation_id), permite que dos exports independientes del mismo set produzcan el mismo chunk-id.

5. **`seele-setup` es idempotente por `[seele]` block markers** en cada config. Re-correr actualiza el bloque, no duplica.

6. **`seele-project` no asume Rust** — es lang-agnostic. Detecta `package.json`, `Cargo.toml`, `pyproject.toml`, `go.mod`, `pom.xml`, etc. La heuristica determina los 5 casos.

## Riesgos identificados

| Riesgo | Mitigacion |
|---|---|
| TUI snapshot tests fragiles en CI (terminal width diff Linux/Mac/Win) | `TestBackend` fija width=120 explicito. |
| Wizard agentes con paths que cambian entre OS | Usar `dirs::config_dir()` y feature-flag por OS si el path varia. |
| Import ENGRAM bug que corrompe DB | `--dry-run` + backup automatico del source pre-import + idempotencia testeada con E2E. |
| Sprint-04 demasiado grande, no cierra en una pasada | Subdividir agresivamente en bloques. Si E se atrasa, cerrar Sprint-04 con A/B/C/D y dejar E para Sprint-04.5. Patron heredado del split C.1/C.2 del Sprint-03. |

## Cloven sights de Sprint-03 incorporados en Sprint-04

Vienen del review post-Bloque F.

- **ONNX por default en CLI** (Cloven [MEDIO]): Bloque D.1.
- **`seele doctor` debe gritar si esta en FakeEmbedder mode**: Bloque D.1.
- **Asegurar que el switch a clap sea reemplazo completo** del argv parser (Cloven [NIT]): Bloque D.1 explicitamente elimina `main.rs` viejo.
- **Property tests workspace-wide**: defer a Sprint-05 (Cloven concuerda).

## Criterios de aceptacion del Sprint-04

1. `cargo build --release -p seele-cli` produce binary que dispatcha todos los subcommands.
2. `seele save`, `seele search`, `seele show`, `seele list` funcionan contra DB local.
3. `seele import --from-engram <path>` migra una DB ENGRAM existente con asserts E2E.
4. `seele sync export` + `seele sync import` round-trip funciona entre dos DBs.
5. `seele setup --agent claude-code --dry-run` muestra el diff sin tocar archivos.
6. `seele tui` arranca, navega entre las 5 vistas, sale con q.
7. ~280-320 tests verde (vs 203 hoy). Clippy + fmt + STELE green.
8. Cloven review post-sprint con findings documentados o cerrados.
