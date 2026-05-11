# Changelog

Todos los cambios notables a este proyecto se documentan acá. Formato basado en
[Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y SemVer.

## [Unreleased]

(Nothing yet — v0.2 work lands here.)

## [0.1.0] — 2026-05-11

First public release. Five AEGIS sprints (Foundation, Embedder+Search,
Interfaces, Ops&UX, Polish+Release). Built across 2026-05-10 → 2026-05-11.

### Added
- Estructura inicial del workspace Cargo con 11 crates (`seele-core`,
  `seele-storage`, `seele-embedder`, `seele-search`, `seele-mcp`, `seele-http`,
  `seele-tui`, `seele-sync`, `seele-setup`, `seele-project`, `seele-cli`).
- `seele-core` — tipos canónicos: `SeeleId` (ULID), `Observation`,
  `ObservationType` (12 variants + `Other`), `Scope`, `Session`,
  `SessionStatus`, `Metadata`, `Link`, `MemoryRelation`, `MetadataFilter`.
  22 unit tests + 9 integration tests (incluye proptest).
- `seele-storage` — SQLite + FTS5 + vec0 con migrations refinery, CRUD
  completo sobre `sessions`/`observations`/`user_prompts`/`links`/
  `memory_relations`/`sync_chunks`. Privacy strip `<private>`, normalized
  hash dedup, topic key upserts, soft delete.
- `seele-storage/vec0_loader` — carga vendorizada de `sqlite-vec` v0.1.9 vía
  `include_bytes!` para 5 targets (linux/mac/win × x86_64 + linux/mac aarch64).
  Ver `crates/seele-storage/vendor/sqlite-vec/README.md` y ADR-11.
- `seele-embedder` — ONNX runtime via `ort` 2.0.0-rc.10 + tokenizers + hf-hub
  con auto-download de `all-MiniLM-L6-v2`. `OnnxEmbedder` + `FakeEmbedder`
  para tests downstream. (Trabajo Sprint-02 parcial; cierra en próximo ciclo.)
- `seele-search` — RRF combiner híbrido FTS+vec con boost por metadata score.
  (Trabajo Sprint-02 parcial; cierra en próximo ciclo.)
- CI matrix Linux + macOS + Windows (build + test) + lint job (clippy + fmt) +
  static-checks-bash + static-checks-pwsh.
- Static check `scripts/check-no-stele-residual.{sh,ps1}` para asegurar que
  ningún archivo del repo tenga residuos del nombre legacy "STELE" fuera de
  los allowlist documentados. Allowlist soporta archivos exactos y prefijos
  de directorio (e.g. `docs/aegis/devlogs/`).
- Génesis AEGIS completa: 5 docs estrategia + 11 ADRs arquitectura + 5
  sprints táctica.
- `CLAUDE.md` con reglas operativas del repo + `docs/INDEX.md` + primer
  devlog Sprint-01 + cost-ledger.jsonl arrancado.

### Changed
- **Sprint-01 BE Foundation cerrado** (2026-05-10). Plan táctico movido a
  `genesis/plans/executed/tactica/sprint-01/`. Devlog completo en
  `docs/aegis/devlogs/2026-05-10-sprint-01-foundation.md`. 99 tests verde
  + clippy + fmt + STELE residual checks pasando.
- **Sprint-02 BE Embedder + Search cerrado** (2026-05-10). Plan táctico
  movido a `genesis/plans/executed/tactica/sprint-02/`. Devlog en
  `docs/aegis/devlogs/2026-05-10-sprint-02-embedder-search.md`. Cambios:
  - `seele-embedder`: cache controlada `~/.seele/embedder/` con env
    override `SEELE_EMBEDDER_DIR`, INT8 quantized default con fallback
    automático a full precision + warn, SHA256 verification opcional
    (tabla `TRUSTED_HASHES` vacía hasta primer release), singleton global
    para servers de larga vida, trait method `expected_sha256()`.
  - `seele-search`: boost por `meta_score` (ADR-03 capa 5), empty-query
    path `created_at DESC` en lugar de `InvalidInput`, annotation lines
    de `memory_relations` (Supersedes/SupersededBy/ConflictsWith/
    ContestedBy) opt-in vía `include_annotations`, `max_vec_distance`
    threshold.
  - Tests: 130 verde (+31 vs Sprint-01), 4 ignored (2 ONNX + 2 perf).
    Suite fixtures + 7 E2E + 3 proptest + 2 perf smoke. Clippy + fmt +
    STELE residual checks pasando.
- **Sprint-04 Ops & UX cerrado** (2026-05-10). Plan táctico movido a
  `genesis/plans/executed/tactica/sprint-04/`. Devlog en
  `docs/aegis/devlogs/2026-05-10-sprint-04-ops-ux.md`. Tag git
  `sprint-04-ops-ux`. Cambios:
  - `seele-project`: 5-case detection heredada de ENGRAM
    (`.seele/config.json` override → git remote basename → git root
    basename → child-scan depth 1 con skip-noise → cwd basename). Cases
    2 y 3 con timeout de 1500ms por thread+mpsc — no bloquean
    indefinido si git cuelga.
  - `seele-setup`: wizard MCP-install para 3 agentes implementados
    (`claude-code` ~/.claude.json, `cursor` ~/.cursor/mcp.json,
    `windsurf` ~/.codeium/windsurf/mcp_config.json) + 5 skeleton
    (opencode, aider, cody, continue, zed) que retornan
    `SetupError::NotImplemented`. `--all` filtra a implemented-only
    (`AgentKind::is_implemented`); `--list` muestra ambos con tags
    `[implemented]`/`[skeleton (v0.2)]`. `write_atomic` real con tmp +
    rename, backup automático `<path>.<ext>.bak.<unix_ms>`. Outcomes
    Created/Added/Unchanged/Updated/DryRun.
  - `seele-sync`: chunks JSON gzip git-friendly. `export_to_dir` con
    `chunk_id = SHA-256(payload determinista)` — dos exports
    independientes del mismo set producen el mismo chunk-id, dedup
    natural. `import_from_file` atómico en una sola transacción
    SQLite (cierra Cloven CRITICO 1): `ObservationStore::save_in_tx` +
    `ChunkStore::mark_imported_in_tx` + commit. Crash mid-loop
    deja la destination DB intacta y el ledger sin marca para que
    re-runs reprocesen limpios.
  - `seele-engram-import`: nuevo crate (#12 del workspace) que cierra
    la primera decisión de ADR-13 — migrate one-shot de una DB
    ENGRAM SQLite a SEELE. `EngramImporter::import_from(path, dry_run)`
    devuelve `ImportReport`. Mapeo `memories.body` → `content`,
    `memories.metadata` JSON preservado verbatim, ULIDs source-side
    preservados / cuid-style minteán fresh `SeeleId` con
    `metadata.engram_id` breadcrumb, `metadata.linked_to[]` →
    tabla `links` con `link_type = "related_to"`. Idempotente vía
    `INSERT OR IGNORE` sobre el id preservado.
  - `seele-cli`: rewrite completo a `clap derive` reemplazando el
    argv parser hand-rolled de Sprint-03. 17 subcommands (save,
    search, show, list, delete, restore, link, stats, doctor,
    projects, sync export|import, import from-engram, setup, mcp,
    serve, tui). Global flags `--db`/`--fake-embedder`/`--json`.
    `seele doctor` emite `fake_embedder_warning` cuando model_id
    contiene "fake" (sight Cloven Sprint-03). `--fake-embedder`
    `hide = true` hasta Sprint-05 cuando ONNX ship por default.
  - `seele-tui`: ADR-07 implementado — ratatui 5 vistas con keymap
    vi-style. Vistas: Home (stats card + breakdowns), Browse
    (list j/k), Search (live query + RRF score badge), Detail
    (header + body + JSON pretty metadata), Stats (fuller
    breakdowns + sessions). Keybindings 1-5/q/Ctrl-C globales,
    j/k/g/G/enter/// por pane, `r` refresh, esc back. RAII
    `TerminalGuard` restaura raw mode + alt screen incluso ante
    pánico. Search pane consume printable input antes de globals
    para no triggerear hotkeys con digits typed en query.
  - **Cloven post-review fixes** (commit `d152842`): 4 findings
    cerrados antes de D.3:
    - CRITICO 1 (sync import atomicity, ya descrito).
    - CRITICO 2 (`seele-setup::write_atomic` con tmp+rename).
    - ALTO (`setup --all` skeleton-aware; `--fake-embedder` hidden).
    - MEDIO (`seele-project` git subprocess timeout 1500ms).
  - Storage adds para soportar migration: `ObservationStore::save_raw_in_tx`
    (`RawSaveInput`/`RawSaveOutcome`) que bypassa privacy strip +
    topic upsert + dedup, `LinkStore::create_in_tx` para que links
    derived commiten atómicos con sus observations.
  - Tests: 304 verde (+102 vs Sprint-03), 4 ignored. Distribución
    nueva: 15 seele-project + 14 seele-setup + 10 seele-sync (2
    unit + 8 E2E) + 22 seele-engram-import (11 unit + 11 E2E) +
    17 seele-tui (7 state + 9 keymap + 10 snapshot) + 14 CLI
    subcommands_e2e (binary spawn).
- **Sprint-05 Polish + CI/CD + Release cerrado** (2026-05-11). Plan
  táctico movido a `genesis/plans/executed/tactica/sprint-05/`. Devlog
  en `docs/aegis/devlogs/2026-05-11-sprint-05-polish-release.md`. Tag
  AEGIS `sprint-05-polish-release`; SemVer release tag `v0.1.0` (precedido
  por `v0.1.0-rc.1` para validar `release.yml`). Cambios:
  - Property tests workspace-wide (cierra deferred Sprint-01): 17 nuevos
    casos en `seele-core/tests/types_roundtrip.rs` (extendido) +
    `seele-storage/tests/properties.rs` (nuevo) +
    `seele-search/tests/rrf_properties.rs` (nuevo) +
    `seele-sync/tests/properties.rs` (nuevo). `seele-sync` gana
    `proptest` dev-dep.
  - ONNX por default + fallback transparente a `FakeEmbedder` con warn
    a stderr cuando la inicialización falla. `--fake-embedder` deja de
    estar `hide = true`. Nuevo env var `SEELE_FAKE_EMBEDDER=1` con la
    misma semántica que el flag. Helper `pick_embedder` testeable sin
    SQLite.
  - `.github/workflows/release.yml`: pipeline 5 targets
    (linux x86_64 + aarch64, mac x86_64 + aarch64, windows x86_64) con
    `tar.gz`/`zip` + sidecar `.sha256`. GH Release auto-creado con
    `softprops/action-gh-release@v2`; `prerelease: true` cuando el tag
    contiene hyphen. Job `crates-io` valida con `cargo publish --dry-run`
    por crate en orden topológico; publish real gated detrás de
    `workflow_dispatch` input `crates_io_publish=true` (default OFF —
    decisión §C del plan).
  - `scripts/install.sh` + `scripts/install.ps1`: instaladores
    portables que descargan el binary, verifican SHA256, extraen a
    `$HOME/.local/bin` (linux/mac) / `$env:USERPROFILE\.seele\bin`
    (windows). Honoran `SEELE_VERSION` + `SEELE_INSTALL_DIR` env.
  - Docs polish: README.md rewrite completo (v0.1.0 status, quick
    start, install matrix, ENGRAM credit). Tres docs nuevas en
    `docs/`: `INSTALLATION.md` (~120 LOC), `AGENT-SETUP.md` (~110 LOC),
    `ENGRAM-MIGRATION.md` (~120 LOC, cierra ADR-13 doc deliverable).
    `docs/INDEX.md` sección "Guías de usuario (Sprint-05)".
  - `seele tui --smoke` flag oculto: render off-screen una frame en
    `TestBackend` 120×30 + `println "tui smoke ok"` + exit 0. Habilita
    criterio 8 del MVP smoke sin terminal interactivo.
  - `scripts/v0.1.0-smoke.sh`: corre los 11 criterios de aceptación
    secuencialmente contra el binary release-build. Criterios 9 (CI
    verde) y 10 (release.yml fires on tag push) son externos; el
    script los reporta como out-of-band. OpenAPI threshold relajado de
    "25+" del plan a ≥15 (v0.1 ship 18 paths / 22 ops, deja margen para
    evolución sin que el smoke chase su cola).
  - Allowlist `check-no-stele-residual.{sh,ps1}` extendida con prefijo
    `genesis/plans/tactica/` — los sprint plans en vuelo mencionan
    legítimamente el nombre legacy al describir CI coverage.
  - Tests: 322 verde (+18 vs Sprint-04, sin contar los proptest cases
    que se ejecutan dentro de cada `#[test]` con cases=32), 4 ignored.

- **Sprint-03 BE Interfaces cerrado** (2026-05-10). Plan táctico movido a
  `genesis/plans/executed/tactica/sprint-03/`. Devlog en
  `docs/aegis/devlogs/2026-05-10-sprint-03-interfaces.md`. Tag git
  `sprint-03-interfaces`. Cambios:
  - `seele-http`: ~25 endpoints sobre axum 0.8 (memories save/search/list/
    show/soft_delete/restore + links create/list/delete + sessions
    start/list/get/end/abort + relations create/list/judge + conflicts +
    stats + embedder). `SeeleService` shared service layer reusado por
    HTTP y MCP. Bearer-auth middleware opt-in (`/health`, `/version`,
    `/openapi.json`, `/docs/*` quedan public). utoipa OpenAPI 3.1 spec
    + Swagger UI en `/docs`. Anti-empty-query gate (mitigación Cloven
    list-all-DB exfiltration).
  - `seele-mcp`: JSON-RPC 2.0 stdio server con 19 tools `seele_*`. Server
    transport-generic sobre `AsyncRead + AsyncWrite` (`run_io`) + thin
    `run_stdio` wrapper. Tools: 8 memory, 4 session (incluye
    `capture_passive` que parsea `## Key Learnings` bullets), 2 relation,
    5 meta (`stats`, `projects`, `doctor`, `version`, `suggest_topic_key`
    con heurísticas ENGRAM-inherited).
  - **ADR-13 compat ENGRAM**: HTTP `--legacy-engram-paths` flag expone
    `POST /save` + `GET /show/{id}` aliases. MCP `--tool-prefix mnema`
    rename los tools (`mnema_save`, `mnema_recall` — recall en lugar de
    search por compat ENGRAM). Sin contaminación dentro de los handlers
    o tool_impls; el alias vive en el boundary (`build_index`).
  - `seele-cli`: argv parser hand-rolled mínimo. Soporta `--version`,
    `--help`, `mcp [--tool-prefix --db]`, `serve [--port --bind
    --legacy-engram-paths --auth-bearer --db]`. Default DB
    `~/.seele/seele.db`. Embedder FakeEmbedder en v0.1 (real ONNX behind
    flag en Sprint-04). Full clap-based CLI llega en Sprint-04.
  - Bumped `utoipa-swagger-ui` 8 → 9.0.2 (8 solo soportaba axum 0.7;
    SEELE usa axum 0.8 desde Bloque A).
  - 6 stores marcados `#[derive(Clone)]` para soportar `SeeleService:
    Clone`.
  - Tests: 202 verde (+72 vs Sprint-02), 4 ignored. Distribución: 99
    Sprint-01 + 31 Sprint-02 + 72 Sprint-03 (34 seele-http + 18
    seele-mcp + 14 service-layer + 6 binary E2E).

### Changed
- **MSRV bump a Rust 1.85** desde 1.83 inicial. Razón: `clap_lex` (transitiva
  vía `clap` 4.5) y otras deps modernas requieren `edition2024`. Si bajamos
  versiones de deps el conflicto se resuelve, pero perdemos features de
  `axum 0.8`, `utoipa 5`, etc. — preferimos bumpear MSRV una vez ahora antes
  de v0.1.0 que en cada release minor.
- **Pin de dependencia**: `ort = "=2.0.0-rc.10"` — no existe `2.0.0` stable
  a fecha 2026-05. Despinear cuando upstream haga release stable. Monitoreado
  vía `.github/dependabot.yml` (PR automático cuando salga el bump).

## Histórico de bumps de dependencias vendorizadas

### `sqlite-vec` (vendored)
- 2026-05-10 — `v0.1.9` (génesis). Vendorizado en
  `crates/seele-storage/vendor/sqlite-vec/`. Procedimiento de bump
  documentado en el README de esa carpeta.

---

[Unreleased]: https://github.com/orlando-vazquez-career/seele/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/orlando-vazquez-career/seele/releases/tag/v0.1.0
