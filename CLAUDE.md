# CLAUDE.md — SEELE repo

Reglas operativas para Claude Code cuando trabaje en este repo. Específico al stack y dominio. El protocolo AEGIS general vive en `C:/dev/protocols/AEGIS/AEGIS-PROTOCOL.md`; este archivo es la capa fina por encima.

## Qué es SEELE

Memory engine en Rust para agentes de IA. Local-first: SQLite con FTS5 + vec0 (sqlite-vec vendorizado) + ONNX embeddings (`all-MiniLM-L6-v2`) + búsqueda híbrida con Reciprocal Rank Fusion. Expone CLI + HTTP REST API (~26 endpoints) + MCP server stdio (19 tools `seele_*`) + TUI con `ratatui`. Sync multi-máquina via chunks comprimidos git-friendly.

Reimplementación clean-room inspirada en [ENGRAM](https://github.com/Gentleman-Programming/engram) (MIT, Copyright Gentleman-Programming). Ver `CREDITS.md` para atribución detallada.

## Estado actual

- **v0.1.0 released** (2026-05-11). 5 sprints AEGIS cerrados:
  - Sprint-01 BE Foundation (`sprint-01-foundation`, devlog `2026-05-10-sprint-01-foundation.md`).
  - Sprint-02 BE Embedder + Search (`sprint-02-embedder-search`, devlog `2026-05-10-sprint-02-embedder-search.md`).
  - Sprint-03 BE Interfaces (`sprint-03-interfaces`, devlog `2026-05-10-sprint-03-interfaces.md`).
  - Sprint-04 Ops & UX (`sprint-04-ops-ux`, devlog `2026-05-10-sprint-04-ops-ux.md`).
  - Sprint-05 Polish + CI/CD + Release (`sprint-05-polish-release`, devlog `2026-05-11-sprint-05-polish-release.md`). SemVer release tag `v0.1.0` (precedido por `v0.1.0-rc.1` para validar `release.yml`).
- Sprint GRAIL-H1 (2026-06-10, branch `sprint/grail-h1-quickwins`): 11 quick wins del plan `docs/analysis/2026-06-09-plan-mejoras-grail-en-seele.md` — envelope JSON CLI, embeddings_meta cableada, tercer path RRF `fts_loose` (gate eval: paraphrase r@5 0.733→0.867), `--explain`, near_duplicates en save, find_similar + compare suggest, ChatProvider endurecido, topic-families.toml real, eval nDCG@10 + suite v2, COMPARISON.md + mermaid, release.yml reparado + RELEASING.md.
- ~370 tests verde + 5 ignored (ONNX descarga + perf smoke). Clippy + fmt + STELE residual checks pasando.
- v0.2 trabaja sobre `[Unreleased]` en `CHANGELOG.md`. Próximas features candidatas: 5 skeleton agents (`opencode`/`aider`/`cody`/`continue`/`zed`), sync chunk splitter (~1 MB cap), TUI editing in-place, `claude mcp add` delegación, Homebrew tap, project-detection wired in `seele save`.

### Lo que ya corre

- `seele serve [--port 7777] [--bind 127.0.0.1] [--legacy-engram-paths] [--auth-bearer <token>] [--db <path>]` — HTTP REST API con Swagger UI en `/docs`, OpenAPI 3.1 en `/openapi.json`.
- `seele mcp [--tool-prefix <p>] [--db <path>]` — MCP stdio JSON-RPC 2.0 con 19 tools. Conectable desde Claude Code, Cursor, OpenCode. Per ADR-13, `--tool-prefix mnema` expone `mnema_save`, `mnema_recall`, etc para drop-in compat con consumers ENGRAM.
- `seele [save|search|show|list|delete|restore|link|stats|doctor|projects]` — clap CLI completa contra el service local (no HTTP). Flags globales `--db`/`--json`/`--fake-embedder`. Con `--json` toda la CLI habla el envelope `{ok,data,warnings}` (errores `{ok:false,error,kind}` a stdout). `seele search --explain` vuelca el trace del pipeline (candidatos por path, distancias vec, params). `seele eval --suite <name>|--suite-file <path>` corre el harness con recall@k/MRR/nDCG@10. ONNX es default en v0.1; `--fake-embedder` (o `SEELE_FAKE_EMBEDDER=1`) fuerza FakeEmbedder. Si ONNX init falla, fallback transparente a Fake con warn.
- `seele sync [export|import]` — git-friendly chunks JSON gzip. Re-imports idempotent por SHA-256.
- `seele import from-engram <path> [--dry-run] [--re-embed]` — migración one-shot de ENGRAM SQLite (ADR-13). Preserva ULIDs, mapea `linked_to[]` a tabla `links`. Idempotente.
- `seele setup [--agent <name>|--all|--list] [--dry-run] [--no-backup]` — wizard MCP install. 3 agentes implementados (claude-code/cursor/windsurf), 5 skeleton (opencode/aider/cody/continue/zed).
- `seele tui` — ratatui interactivo, 5 vistas (Home/Browse/Search/Detail/Stats), keymap vi-style.

## Stack

- **Rust** 1.85+ (`rust-toolchain.toml`).
- **Edición** 2021.
- **MSRV bump 1.83 → 1.85** decidido para usar `clap_lex` con `edition2024`. Ver `CHANGELOG.md`.
- **Workspace** con 14 crates en `crates/`:
  - `seele-core` — tipos canónicos, errores, IDs (ULID via `SeeleId`).
  - `seele-storage` — SQLite + FTS5 + vec0 + CRUD + migrations refinery. `save_raw_in_tx` para migration paths.
  - `seele-embedder` — ONNX runtime via `ort` + `tokenizers` + `hf-hub` + auto-download.
  - `seele-search` — FTS + vec híbrido con RRF combiner.
  - `seele-mcp` — MCP server stdio (sprint-03).
  - `seele-http` — REST API axum (sprint-03).
  - `seele-chat` — `ChatProvider` (OpenAI-compatible + Anthropic) para chat-with-DB; tool-use loop sobre `seele_search` (v0.2, sprints LUMEN).
  - `seele-tui` — TUI ratatui 5 vistas (sprint-04).
  - `seele-sync` — git-friendly chunks gzip JSON (sprint-04).
  - `seele-setup` — wizard 3 implementados + 5 skeleton (sprint-04).
  - `seele-project` — 5-case project detection (sprint-04).
  - `seele-engram-import` — migration ENGRAM → SEELE ADR-13 (sprint-04).
  - `seele-cli` — binary `seele` clap derive, 18 subcomandos (sprint-04; `eval` agregado en v0.3).
  - `seele-eval` — harness de evaluación de calidad de memoria (recall@k/MRR por categoría); suites embebidas + subcomando `seele eval` (v0.3-α, ADR-14).
- **DB**: SQLite con `rusqlite` (feature `bundled` + `load_extension`) + `sqlite-vec` v0.1.9 vendorizado para 5 targets.
- **Async**: tokio 1.42 multi-thread.
- **Errores**: `thiserror` para errores tipados; `anyhow` solo en CLI.
- **Tracing**: `tracing` + `tracing-subscriber` con env-filter.

## Pins críticos

- `ort = "=2.0.0-rc.10"` — pin exacto, no hay 2.0.0 stable a 2026-05. Despinear cuando upstream haga release stable. Dependabot abre PR automático.
- `sqlite-vec` vendorizado en `crates/seele-storage/vendor/sqlite-vec/` v0.1.9 (NO crate Rust). Procedimiento de bump documentado en el README de esa carpeta.

## Convenciones

### Tests

- **Unit tests**: en el mismo archivo del módulo bajo `#[cfg(test)] mod tests`.
- **Integration tests**: en `crates/<crate>/tests/<feature>.rs`.
- **Workspace-level tests** (E2E del binary): llegan en sprint-03 (HTTP/MCP) y sprint-05.
- **Property tests** con `proptest` — workspace-wide a partir de Sprint-05. Casos en `crates/seele-core/tests/types_roundtrip.rs`, `crates/seele-storage/tests/properties.rs`, `crates/seele-search/tests/{property_tests,rrf_properties}.rs`, `crates/seele-sync/tests/properties.rs`. Default 32 casos por property (8 para los DB-touching de storage).
- **TUI snapshots** con `insta` + `TestBackend` (sprint-04).
- **Comandos**: `cargo test --workspace --all-features`. Tests `#[ignore]` para descargas reales de modelos ONNX se corren con `cargo test -p seele-embedder -- --ignored`.

### Lint y formato

- **Clippy**: `cargo clippy --workspace --all-targets -- -D warnings`. Verde es criterio de cierre.
- **Fmt**: `cargo fmt --all -- --check`. `rustfmt.toml` minimal: `edition = "2021"`.
- **Static check STELE residual**: `bash scripts/check-no-stele-residual.sh` (Linux/Mac) o `pwsh scripts/check-no-stele-residual.ps1` (Windows). Asegura que ningún archivo nuevo introduzca el nombre legacy "STELE" fuera del allowlist.

### Naming

- **Tablas SQL**: `snake_case`, plural (`observations`, `sessions`, `memory_relations`).
- **Índices**: `idx_<tabla>_<col>` o `idx_<tabla>_<col1>_<col2>`. Partial cuando aplique (`WHERE deleted_at IS NULL` para queries vivas).
- **Triggers FTS**: `<tabla>_ai` (after insert), `<tabla>_ad` (after delete), `<tabla>_au` (after update).
- **Crates**: `seele-<area>`. Sin prefijos adicionales.
- **Tipos públicos**: `PascalCase`, derivan al menos `Debug + Clone`. `Serialize + Deserialize` cuando cruzan boundary.
- **Errores**: `<Area>Error` enum con `thiserror`.

### IDs

- `SeeleId(Ulid)` para todo. ULID textual en SQL. Bridge a vec0 INTEGER rowid via `SeeleId::as_i64()` (tail aleatorio: bytes 9..16 del ULID, 56 bits, byte alto en cero) — esa es la implementación correcta. `int_id` es una columna INTEGER real `UNIQUE` poblada desde Rust en el INSERT (no una virtual column); la colisión del tail se maneja con retry en fresh-save (`ID_COLLISION_RETRIES`).

### Privacy

- Strip `<private>...</private>` en `ObservationStore::save()` y `PromptStore::save()` via regex `(?si)<private>.*?</private>`. Aplica a `title` + `content` antes de `normalized_hash` y antes de FTS.

### Topic keys

- Default heuristics: `architecture/*`, `bug/*`, `decision/*`, `pattern/*`, `config/*`, `discovery/*`, `learning/*` (heredadas de ENGRAM, válidas para coding agents).
- **Configurabilidad por consumer (implementado en GRAIL-H1)**: `seele_core::families` resuelve `$SEELE_TOPIC_FAMILIES` > `./.seele/topic-families.toml` > `~/.seele/topic-families.toml` > 7 builtin ENGRAM, una vez al boot del service. TOML inválido = warn + fallthrough, nunca panic. `seele_suggest_topic_key` además propone el topic_key del vecino vectorial más cercano (source `neighbor`) antes del fallback por familias.
- Upsert: misma `(project, scope, topic_key)` activa → update + revision_count++.

## Comandos comunes

```powershell
# Build full workspace
cargo build --workspace

# Tests (sin descargas de modelos)
cargo test --workspace

# Tests con ONNX real (descarga ~90MB primera vez)
cargo test -p seele-embedder -- --ignored

# Lint + fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

# Static check residuos STELE
bash scripts/check-no-stele-residual.sh

# Construir binary release
cargo build --release -p seele-cli

# Correr binary (cuando esté implementado, sprint-04)
cargo run --bin seele -- --help
```

## AEGIS — disciplina del repo

Aplicamos AEGIS v2.0.0 (sin agentes en background, todo en consola). Resumen:

- Plan vive en `genesis/plans/` (estamos en fase genesis hasta v0.1; cuando salga, las nuevas features irán a `docs/plans/`).
- Plan muere en `genesis/plans/executed/` (o `docs/plans/executed/` post-genesis).
- Devlog del sprint en `docs/aegis/devlogs/YYYY-MM-DD-sprint-NN-<tema>.md`.
- Cost ledger append en `docs/aegis/devlogs/cost-ledger.jsonl`.
- Cada bloque del sprint commitea + pushea al cierre. Mensaje: `sprint-NN bloque-X — <título corto>`.
- Cada sprint cierra con devlog + tag `sprint-NN-<tema>`.

**Dos gates humanos obligatorios**:
- Gate 1: revisión del plan táctico antes de Ejecución.
- Gate 2: aprobación del cierre antes del state-sync.

**Regla de oro AEGIS**: si al cerrar un ciclo falta `executed/` o devlog o `CLAUDE.md`/`INDEX.md` actualizados, **el ciclo no está cerrado**.

## Cloven

`/cloven` se invoca entre ciclos para revisión externa. Cloven vive en `C:/dev/buddys/cloven/`. Vió este repo desde la fase genesis y ya advirtió:

- (2026-05-10) NIT — asegurar que ningún archivo Rust generado contenga residuos del nombre legacy "STELE". Cerrado vía `scripts/check-no-stele-residual.{sh,ps1}` + CI jobs.
- (2026-05-10) Cloven follow-up — licencia debe ser MIT pura (no Apache-2.0/MIT mixta), tests en order, dependabot configurado para `ort` pin. Cerrado en commits `47d87ac` y `9478201`.
- (2026-05-10) Sprint-04 mid-review — 4 findings: CRITICO 1 `seele-sync::import` sin transacción (cerrado), CRITICO 2 `seele-setup::write_atomic` con `std::fs::write` no atómico (cerrado), ALTO `setup --all` itera skeletons + `--fake-embedder` dead UI (cerrado, hidden + filter), MEDIO `seele-project` git subprocess sin timeout (cerrado, 1500ms thread+mpsc cap). Todos en commit `d152842`.

## No hacer

- ❌ Spawnear `Agent(...)` con `subagent_type` que delegue trabajo. AEGIS v2.0.0 lo prohíbe.
- ❌ Escribir tests trivial `assert!(true)` — clippy 1.95 los rechaza por `assertions_on_constants`. Si querés un sentinel del crate, usá un check con valor genuino o eliminá el `mod tests` vacío.
- ❌ Agregar dependencia "STELE" en cualquier `Cargo.toml` o `lib.rs` o config. El static check rompe CI.
- ❌ Mover archivos de `genesis/plans/` a `executed/` antes de cerrar el sprint con devlog. La regla de oro AEGIS lo prohíbe explícitamente.
- ❌ Despinear `ort` manualmente — esperar PR de Dependabot cuando upstream haga release stable.
- ❌ Tocar el binary `vec0.{so,dylib,dll}` vendorizado sin actualizar `crates/seele-storage/vendor/sqlite-vec/README.md` con el procedimiento de bump documentado.

## Hacer

- ✅ Cada PR/commit corre tests + clippy + fmt + STELE residual antes de pushear.
- ✅ Crear plan táctico **antes** de codear cualquier feature nueva no trivial.
- ✅ Devlog al cierre de cada sprint.
- ✅ Append cost-ledger por sesión (ver convención en `C:/dev/protocols/AEGIS/guides/cost-ledger.md`).
- ✅ `genesis/plans/<fase>/00-INDEX.md` actualizado al agregar/mover docs.
- ✅ Update `CHANGELOG.md` (sección `Unreleased`) con todo cambio que afecte API pública o behavior visible.
- ✅ ADR para decisiones técnicas no triviales (`genesis/plans/arquitectura/<NN>-<slug>.md`). Sólo cuando la decisión no es obvia.

## Referencias

- AEGIS-PROTOCOL.md: `C:/dev/protocols/AEGIS/AEGIS-PROTOCOL.md`.
- LUMEN-PROTOCOL: `C:/dev/protocols/LUMEN/` (cuando llegue el frontend del SEELE TUI / web — v0.3 si emerge).
- ENGRAM: https://github.com/Gentleman-Programming/engram.
- Anthropic MCP spec: https://modelcontextprotocol.io.
- sqlite-vec docs: https://github.com/asg017/sqlite-vec.
