# Scope MVP — qué entra en SEELE v0.1

## Filosofía del MVP

v0.1 debe ser **funcional para que MNEMA lo use en producción** y **paritaria con ENGRAM en lo local** (sin cloud). Si MNEMA no puede consumir SEELE para Recall + Encode con la misma comodidad que tendría con ENGRAM, el engine está incompleto.

El alcance es más grande que la primera versión del plan porque la auditoría de ENGRAM (`05-engram-feature-audit.md`) reveló features importantes que no estaban contempladas: sessions, projects, scope, topic keys, type field, memory relations, git sync, project detection, privacy stripping, capture passive, doctor, export/import, agent setup wizard.

## In-scope para v0.1

### Storage layer

- ✅ SQLite con FTS5 builtin (Rust crate `rusqlite` con feature `bundled`).
- ✅ sqlite-vec para embeddings (loadable extension via `rusqlite::loadable_extension`).
- ✅ Tabla `sessions` con id (ULID), project, directory, started_at, ended_at, summary, status.
- ✅ Tabla `observations` con id (ULID), session_id, type, title, content, tool_name, project, scope, topic_key, normalized_hash, revision_count, duplicate_count, last_seen_at, created_at, updated_at, deleted_at, embedding via vec0.
- ✅ Tabla `user_prompts` linked a sessions.
- ✅ Tabla `memory_relations` con judgment lifecycle (pending/judged/orphaned/ignored).
- ✅ Tabla `sync_chunks` para git sync deduplication.
- ✅ Tabla `links` general purpose (separada de memory_relations).
- ✅ FTS5 virtual tables para `observations_fts` y `prompts_fts` con triggers de sync.
- ✅ vec0 virtual table para embeddings (dim 384).
- ✅ Virtual generated columns sobre JSON metadata + B-tree partial indexes.
- ✅ Migrations versionadas (refinery).
- ✅ Topic key upserts (revision_count en mismo project+scope+topic_key).
- ✅ Normalized hash dedup window (duplicate_count incrementa, last_seen_at actualiza).
- ✅ Privacy stripping `<private>...</private>` en store layer.
- ✅ Soft delete con `deleted_at`.

### Embedder

- ✅ Modelo `all-MiniLM-L6-v2` (default, dim 384).
- ✅ Runtime ONNX via crate `ort` (CPU only, sin requerir GPU).
- ✅ Auto-download del modelo en el primer init (cached en `~/.seele/embedder/`).
- ✅ Singleton pattern para no reinicializar runtime entre queries.
- ✅ Mean-pooling + L2 normalize sobre token embeddings.
- ✅ INT8 quantization por default (~30% más rápido, ~2% drop).

### Search

- ✅ Full-text search via FTS5 con tokenize `porter unicode61 remove_diacritics 2`.
- ✅ Vector similarity (cosine) via sqlite-vec.
- ✅ Híbrido FTS + vector + filter metadata con **Reciprocal Rank Fusion (RRF)** k=60.
- ✅ Filter por type, project, scope, topic_key, custom virtual columns.
- ✅ Top-K configurable (default 10).
- ✅ Annotation lines en results: `supersedes:`, `superseded_by:`, `conflicts:`, `conflict: contested by`.
- ✅ Boost opcional por metadata score.
- ✅ Empty query + filtros = list por created_at desc.

### Project detection

- ✅ Algoritmo 5-case: `.seele/config.json` → git remote → git root → git child scan (depth 1, max 20, 200ms timeout, skip noise dirs) → dir basename.
- ✅ NEVER returns error — siempre devuelve algo + warning si ambiguo.
- ✅ Output: `{project, project_source, project_path, cwd, available_projects, warning?}`.

### Topic keys

- ✅ Default family heuristics (de ENGRAM, para coding agents): `architecture/*`, `bug/*`, `decision/*`, `pattern/*`, `config/*`, `discovery/*`, `learning/*`.
- ✅ **Configurabilidad por consumer**: SEELE expone API para que un consumer (MNEMA u otro) registre **su propio set de family prefixes** via config file (`~/.seele/topic-families.toml`) o CLI (`seele topic-keys add-family <prefix> <pattern>`).
- ✅ Las heuristics default son útiles para coding agents; MNEMA define las suyas (`verdict/*`, `axiomatica/*`, `skill/*`, `disenso/*`) sin que SEELE imponga las de ENGRAM.
- ✅ Suggestion via `seele suggest-topic-key <type> <title>` usa el set activo del config.
- ✅ Upsert behavior cuando topic_key se reusa en mismo project+scope (independiente del set de families).

### Privacy stripping

- ✅ Regex strip de `<private>...</private>` en store layer (`stripPrivateTags()`).
- ✅ Aplicado en `AddObservation()` y `AddPrompt()`.
- ✅ Plugin layer (TS para integraciones externas) v0.2.

### Capture passive

- ✅ Parser de output text para `## Key Learnings:` sections.
- ✅ Save c/u (bullet o numbered) como observation independiente.
- ✅ MCP tool `seele_capture_passive`.

### Sessions

- ✅ Lifecycle start/end/summary.
- ✅ `seele_session_summary` con structure: `## Goal`, `## Discoveries`, `## Accomplished`, `## Next Steps`, `## Relevant Files`.
- ✅ Auto-inject context de sesión previa al iniciar nueva (3-layer progressive disclosure).

### Memory relations / conflict storage (read-only en v0.1)

- ✅ Schema completo: relations table con judgment lifecycle.
- ✅ MCP tools `seele_judge`, `seele_compare` para escribir relations manualmente.
- ✅ HTTP/CLI read-only ops: `conflicts list`, `conflicts show`, `conflicts stats`.
- ✅ Annotation lines en search output muestran relations existentes.
- 📅 v0.2: `conflicts scan --semantic` con LLM judging.

### Git sync (chunks comprimidos)

- ✅ Export memorias nuevas como chunks comprimidos en `.seele/`.
- ✅ Git-friendly: chunks deterministic, no merge conflicts.
- ✅ Import desde otra máquina via `seele sync --import`.
- ✅ `sync --status` muestra estado.
- ✅ `sync_chunks` table previene re-import.

### Interfaces

- ✅ CLI `seele` con verbos completos: `init`, `save`, `search`, `show`, `list`, `delete`, `serve`, `mcp`, `tui`, `timeline`, `context`, `stats`, `export`, `import`, `sync`, `doctor`, `setup`, `projects`, `conflicts list/show/stats`, `version`.
- ✅ HTTP REST API (axum) con auth opcional bearer token, ~26 endpoints (ver ADR-06 actualizado).
- ✅ MCP server stdio con 19 tools `seele_*` (ver ADR-05 actualizado).
- ✅ Output formats: human-readable + JSON (`--json` flag).

### Lifecycle (primitivas básicas)

- ✅ Soft delete (`deleted_at` timestamp; queries excluyen por default; `--include-deleted` flag opcional).
- ✅ `axiomatic` flag (lectura/escritura via metadata).
- ✅ Custom score field via metadata.

### Doctor / observability

- ✅ `seele doctor [--json] [--project]` con health checks: DB schema versión, embedder loaded, ANTHROPIC_API_KEY (si aplica), model files, project detection report.
- ✅ `seele stats` — total sessions/observations/prompts/projects, DB size, last save, breakdown por type/scope.
- ✅ Logging estructurado (`tracing` + `tracing-subscriber`).
- ✅ Stats endpoint HTTP `GET /sync/status` y `GET /stats`.

### Export / Import

- ✅ `seele export [file] [--project NAME]` — JSON dump.
- ✅ `seele import <file>` — JSON ingest.
- ✅ HTTP `GET /export?project=NAME` y `POST /import` con ExportData JSON.

### Setup wizard (v0.1: completo para todos los agentes, decisión User 2026-05-10)

- ✅ `seele setup` arranca wizard interactivo (detecta agentes instalados, ofrece setup paso a paso).
- ✅ `seele setup claude-code` — instala plugin via marketplace / actualiza `~/.claude/.../mcp.json`.
- ✅ `seele setup cursor` — escribe `~/.cursor/mcp.json` con SEELE como MCP server.
- ✅ `seele setup vs-code` — comando `code --add-mcp '{"name":"seele","command":"seele","args":["mcp"]}'`.
- ✅ `seele setup opencode` — config en `~/.config/opencode/mcp.json` style.
- ✅ `seele setup gemini-cli` — config en path de gemini-cli.
- ✅ `seele setup codex` — config en path de codex CLI (OpenAI).
- ✅ `seele setup windsurf` — config en `~/.windsurf/...`.
- ✅ `seele setup antigravity` — config en path de antigravity.
- ✅ `seele setup generic` — printa MCP config JSON copiable para cualquier otro consumer.
- ✅ `seele setup --all` — instala en todos los agentes detectados.
- ✅ Detección automática del agente activo via `which`/`Get-Command` + paths conocidos.
- ✅ Idempotente: re-run no duplica config.
- ✅ Backup del archivo previo antes de modificar (`<file>.seele-backup-<ts>`).

Rationale del User: setup wizard parcial fuerza al consumer a hacer manual la mitad — eso es exactamente el tipo de MVP parcial que el User no quiere. Ocho agentes son ocho implementaciones, pero todas siguen el mismo patrón (escribir/mergear un JSON en un path conocido), así que el costo marginal por agente extra es bajo una vez resuelto el primero.

### TUI (incluida en v0.1)

- ✅ Interfaz interactiva con `ratatui` + `crossterm`.
- ✅ Vistas: Home / Browse / Search / Detail / Stats.
- ✅ Browse con filtros por type/project/scope/tag.
- ✅ Search live con debounce 200ms.
- ✅ Detail con preview body + metadata + linked memories + relations.
- ✅ Quick actions: axiomatic toggle, soft delete con confirmación.
- ✅ Keybindings vi-style con footer hint bar.
- ✅ External `$EDITOR` para edit body / metadata.
- 📅 v0.2: visualización graph de relations, edición inline, undo, theme alternativo (Catppuccin Mocha como opción).

### Observability

- ✅ `tracing` + `tracing-subscriber` con env-filter.
- ✅ Levels: TRACE, DEBUG, INFO, WARN, ERROR.

### Testing

- ✅ Unit tests por crate.
- ✅ Integration tests del binary (`assert_cmd` + `predicates`).
- ✅ Smoke tests del MCP server (spawn + JSON-RPC handshake).
- ✅ Property tests con `proptest` para schema/queries.
- ✅ Snapshot tests TUI con `insta` + `TestBackend`.
- ✅ CI con GitHub Actions: build + test + clippy + fmt en Linux + macOS + Windows.

### Documentación

- ✅ README con quickstart + epígrafe latino + disclaim sobre el nombre.
- ✅ CREDITS.md con crédito detallado a ENGRAM.
- ✅ docs/aegis/ con planes y devlogs.
- ✅ docs API HTTP autogenerada (utoipa OpenAPI 3.1).
- ✅ docs/ARCHITECTURE.md, docs/INSTALLATION.md, docs/AGENT-SETUP.md (heredados conceptualmente de ENGRAM).

### Distribución

- ✅ Releases binarios via GitHub Actions (5 targets: Linux x86_64 GNU+musl, macOS arm64+x86_64, Windows x86_64).
- ✅ Cargo crates publicados en crates.io: `seele-core` + binary `seele`.
- ✅ Install scripts: bash one-liner (`curl … | sh`) + PowerShell.
- 📅 v0.2: Homebrew tap.

## Out-of-scope para v0.1

### Cloud sync (Postgres + dashboard)

- ❌ Server cloud completo. **Rationale**: scope enorme (Postgres, dashboard templ-style, auth JWT, autosync, audit log). Local-first puro alcanza para v0.1; multi-machine se cubre con git sync chunks.
- 📅 v0.2-v0.3 cuando emerja necesidad real.

### Conflict scan semantic (LLM-based)

- ❌ `seele conflicts scan --semantic`. **Rationale**: requiere integración con LLM CLI (claude / opencode / openrouter). Schema de relations sí está en v0.1; el scan es v0.2.
- 📅 v0.2.

### Decay automático

- ❌ Comando `seele decay` con fórmula built-in. **Rationale**: la fórmula es decisión del consumidor (MNEMA tiene la suya). SEELE expone primitives; el caller corre cron.
- 📅 v0.2 si emerge demanda.

### Embeddings remotos

- ❌ OpenAI Embeddings, Voyage, Cohere. **Rationale**: local-first.
- 📅 v0.2 detrás de adapter trait.

### Múltiples embedders simultáneos

- ❌ Soportar dos modelos en la misma DB. **Rationale**: complejidad alta, beneficio bajo.
- 📅 sin fecha.

### Sharding / réplicas

- ❌ Out of charter — SQLite local-first, single-writer.

### Encryption at rest

- ❌ SQLCipher. **Rationale**: agregable como feature flag después.
- 📅 v0.2 detrás de feature.

### Web UI / dashboard servido

- ❌ SEELE no tiene frontend web propio. Si MNEMA tiene su propio FE bajo LUMEN, ese consume el HTTP API.
- 📅 nunca como parte de SEELE.

### Obsidian export

- ❌ `seele obsidian-export`. **Rationale**: nicho específico, scope adicional.
- 📅 v0.2 si hay tracción.

### Repair scripts

- ❌ `tools/repair-*.sh`. **Rationale**: solo necesario con cloud sync (que es v0.2).
- 📅 con cloud.

### Catppuccin Mocha theme

- ❌ Theme alternativo. **Rationale**: SEELE v0.1 tiene un theme propio sobrio. Catppuccin no es prioridad.
- 📅 v0.2 como theme alternativo opcional.

## Criterio de aceptación de v0.1

v0.1 se considera **completo** cuando:

1. ✅ MNEMA puede instalar SEELE con `cargo install seele` o el binary.
2. ✅ MNEMA puede llamar TODO el ciclo de uso de ENGRAM local: init, sessions, save, search, timeline, context, stats, export, import, sync, doctor, setup.
3. ✅ MNEMA puede correr `seele mcp` y conectarse desde Claude Code con las 19 tools `seele_*`.
4. ✅ MNEMA puede correr `seele serve` y consumir el HTTP API completo (~26 endpoints).
5. ✅ Performance: search híbrido sub-300ms con 10K observations en CPU mid-range.
6. ✅ Project detection algorithm produce el mismo resultado que ENGRAM en casos de prueba (parity check).
7. ✅ Git sync entre dos máquinas: máquina A → commit chunks → máquina B → import → mismas observations consultables.
8. ✅ TUI navegable, vistas Home/Browse/Search/Detail funcionales.
9. ✅ Tests verde en Linux + macOS + Windows.
10. ✅ Release binario disponible en GitHub Releases para 5 targets.
11. ✅ Crédito a ENGRAM presente y detallado en README, CREDITS y release notes.

## Tamaño esperado actualizado

- **Crates**: ~11 (core, storage, embedder, search, mcp, http, tui, sync, setup, project, cli).
- **LOC Rust**: ~10K-15K en v0.1.
- **Tests**: ~4K-6K LOC.
- **Sprints estimados**: 5-7 sprints AEGIS para llegar a v0.1 funcional.

## Camino post-v0.1

- **v0.2**: cloud sync (Postgres + autosync), conflict scan --semantic, setup wizards completos, Catppuccin theme alternativo, MCP HTTP transport, embeddings remotos, encryption at rest, plugin system para embedders, repair scripts, Obsidian export.
- **v0.3**: TUI graph view de relations + edición inline (sprint LUMEN dedicado), reranking LLM post-RRF.
- **v1.0**: estable después de 3-6 meses de uso real por MNEMA en producción y al menos 2 consumers externos.
