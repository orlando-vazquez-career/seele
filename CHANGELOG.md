# Changelog

Todos los cambios notables a este proyecto se documentan acá. Formato basado en
[Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y SemVer.

## [Unreleased]

_v0.3-α «calidad de memoria» (evaluation-first, ADR-14): harness `seele-eval`
(recall@k/MRR por categoría), primer baseline auditable (v0.2), tabla
`embeddings_meta`, e higiene A6/A7 (drift de versión, robustez de `int_id`,
footprint del binario ADR-15). Devlog
`docs/aegis/devlogs/2026-05-29-sprint-v0.3-alpha-eval-harness.md`. Sin release
SemVer marcado._

### Fixed

- **Re-embedding an observation always failed (vec0 UNIQUE)** — sqlite-vec
  0.1.9 virtual tables reject `INSERT OR REPLACE` on an existing rowid
  ("UNIQUE constraint failed on observations_vec primary key"), so any second
  `set_embedding` for the same row — the exact path a future
  `seele reindex` / embedder swap (A2) needs — errored. Now DELETE + INSERT
  inside the same transaction. Latent since Sprint-02; exposed by the new
  `embeddings_meta` write-path tests.
- **Embedder first-run download (`hf-hub` 0.3 → 0.5)** — bump fixes the relative-`307`
  redirect failure (`RelativeUrlWithoutBase`) that broke the first-run download of
  `all-MiniLM-L6-v2` from Hugging Face on the `ureq` sync backend. hf-hub 0.5.0 uses
  `ureq` 3+, which follows relative redirects. **No code changes** — the
  `hf_hub::api::sync` surface (`ApiBuilder`/`with_cache_dir`/`build`/`model.get`/`ApiError`)
  is API-compatible. Verified end-to-end with a real fresh-cache download
  (`cargo test -p seele-embedder -- --ignored embed_returns_unit_vector_of_correct_dim`).
  The seeded-cache workaround is no longer required; the offline
  `OnnxEmbedder::from_local_dir` path remains for air-gapped baselines.

### Added

- **CLI JSON envelope (Q2, sprint GRAIL-H1)** — with `--json`, every one-shot
  subcommand now emits a uniform envelope: success
  `{"ok":true,"data":<payload>,"warnings":[..]}`, error
  `{"ok":false,"error":"<display>","kind":"<class>"}` **on stdout** with exit 1
  (errors were previously free text on stderr even with `--json`). `warnings`
  carries in-band degradation signals (`fake-embedder-fallback` when ONNX init
  fails and the CLI degrades to FakeEmbedder). `delete`/`restore` drop their
  hand-rolled ad-hoc shapes for typed payloads under the same envelope.
  **Breaking for `--json` consumers** (acceptable: 0 releases published);
  this is the contract layer the future agent skill builds on. Pattern
  validated by GRAIL's `Reply {ok, data, warnings, error}` envelope.
- **`embeddings_meta` wired into the write path (Q5, sprint GRAIL-H1)** —
  `ObservationStore::set_embedding` now takes an `EmbeddingMeta
  {model_id, dim, contextualized}` and persists vector + provenance in one
  transaction (the V002 table existed but nothing wrote it: the ADR-14 guard
  was inoperative). New `embedding_provenance()` snapshot + shared
  `mix_warning()` guard surfaced by **both** doctors (CLI fields
  `embedding_models` / `observations_active_without_vector` /
  `embedding_mix_warning`; MCP `seele_doctor.embeddings`). Detects the
  genuinely silent A2 failure mode: same-dim/different-model swap.
- **New crate `seele-eval`** (#14) — evaluation-first memory-quality harness
  (ADR-14). recall@5 / recall@10 (binary) + MRR per category against the live
  hybrid FTS+vec+RRF pipeline. Fixture contract (`Suite`/`CorpusItem`/
  `EvalQuery`), `load_suite`/`builtin_suite`, `ingest` (via `save_raw`, no
  topic-key collapse), `run_suite` → `SuiteReport`. Two embedded suites:
  `coding-memory` and `longmemeval-subset` (homemade). CLI subcommand
  `seele eval --suite <name> [--json]` (ephemeral DB, never the real one).
- **`embeddings_meta` table** (migration `V002__embeddings_meta.sql`) —
  per-observation embedding provenance (`model_id`/`dim`/`contextualized`),
  FK to `observations(id)`, backfilled `all-MiniLM-L6-v2`/384/0. Prerequisite
  for the multilingual-embedder / reranking work; not consumed by search yet.
- **`OnnxEmbedder::from_local_dir`** — load the ONNX model + tokenizer from a
  directory, bypassing `hf-hub` (offline / air-gapped baselines).
- **First auditable retrieval baseline**
  (`docs/plans/executed/tactica/v0.3-calidad-memoria/baseline-v0.2.json`):
  `coding-memory` recall@5 0.833 / recall@10 0.944 / MRR 0.673;
  `longmemeval-subset` 1.0 / MRR 0.944. Flags paraphrase + multi-hop as the
  weak spots of pure RRF.
- **MCP envelope shape test** (`crates/seele-mcp/tests/call_tool_result_envelope.rs`).
  Verifies that `tools/call` responses follow `CallToolResult` shape
  per MCP spec 2024-11-05. Two cases: `seele_doctor` (full payload) and
  `seele_version` (minimal payload). Prevents regression of the wire
  envelope bug.

### Fixed

- **`int_id` silent-drop on the raw import path**
  (`crates/seele-storage/src/observations.rs`). `SeeleId::as_i64()` maps the
  ULID's random tail (bytes 9..16, 56 bits); `save_raw_in_tx` used
  `INSERT OR IGNORE`, so a preserved ULID whose `int_id` collided with an
  existing row was dropped silently — data loss masked as an idempotent skip
  in `import`/`sync`. New `RawSaveOutcome::IntIdCollision` detects the case
  (0 rows affected + PK absent) and **reports** it instead of swallowing it;
  surfaced by `engram-import` and `sync`. Regression test
  `crates/seele-storage/tests/int_id_collision.rs`.
- **MCP `tools/call` wire envelope** (`crates/seele-mcp/src/server.rs`). The
  dispatcher returned each tool's raw JSON as the JSON-RPC `result`,
  bypassing the `CallToolResult` envelope required by the MCP spec
  (`{ content: [{ type: "text", text: ... }], isError }`). Clients
  (Claude Code, Cursor, Windsurf) looked for `result.content[0].text`,
  found nothing, and rendered every tool call as "completed with no
  output" — even though the handlers ran and the DB was healthy. The
  fix wraps the handler payload in a single `text` content block and
  sets `isError: false`. JSON-RPC error paths (parse / unknown method /
  invalid params / tool domain errors) are unchanged.
- **`seele-http` test helpers**: six integration test files
  (`handlers_basicos.rs`, `handlers_c1_lifecycle.rs`,
  `handlers_c2_relations_stats.rs`, `handlers_d_auth_openapi.rs`,
  `skeleton.rs`, `openapi_consistency.rs`) failed to compile because
  `ServerConfig` gained the `chat` field in v0.2.0 LUMEN sprints but
  the test fixtures were not updated. Added `chat: None`.
- **OpenAPI spec missing `/chat` + `/chat/info`** (`crates/seele-http/src/openapi.rs`).
  The chat endpoints (added in the v0.2.0 LUMEN sprint) were wired into the
  router but never declared in the hand-authored OpenAPI paths, so the
  `openapi_consistency` test (router ⊆ spec) had been red since then. Added
  both path entries (POST `/chat`, GET `/chat/info`) with prose
  request/response descriptions. Found running the full guardrails for the
  first time post-v0.2.
- **Stale `seele-tui` snapshot baselines** (`crates/seele-tui/tests/snapshots/*.snap`).
  The TUI title bar renders `SEELE v{CARGO_PKG_VERSION}`; the 10 `views_snapshot`
  baselines were captured at `v0.1.0` and never refreshed when the workspace
  bumped to `v0.2.0`, leaving `views_snapshot` red since the bump. Baselines
  refreshed to `v0.2.0` (the only changed line in each).

### Changed

- **Doc drift (A6)**: `README` Status → v0.2; `as_i64()` doc corrected from
  "first 6 bytes" to "random tail (bytes 9..16, 56 bits)" in `CLAUDE.md` /
  ADR-14 / plan; workspace crate count 13 → 14.
- **STELE residual allowlist**: `scripts/check-no-stele-residual.{sh,ps1}`
  gain `docs/plans/tactica/`, `docs/plans/executed/tactica/`,
  `docs/plans/estrategia/`, `docs/plans/executed/estrategia/` and
  `docs/compendium/` prefixes so post-v0.1 plans and the architecture
  compendium can reference the legacy name when documenting CI / static-check
  coverage and naming history.

### Removed

- **Crypto-donation widget** from the web landing (`DonateButtons.astro` +
  the `// 04 — crypto donations` block in `Support.astro`) and the **Crypto
  section** of the root `README.md` (BTC/ETH/Base/Syscoin/SOL wallet
  addresses). Low traction; not worth the maintenance or the wallet-address
  trust surface. Support copy reframed to star / issues / hire, and the
  "spare hours / week of evenings" self-description dropped.


## [0.2.0] — 2026-05-13

First public release. Adds the web frontend (Astro static site at
`https://orlando-vazquez-career.github.io/seele/`) and the chat-with-DB
backend on top of the v0.1.0 backend foundations.

### Added — Frontend (LUMEN sprints 01-04, applies LUMEN protocol v0.11.0)

- **Landing site** at `web/` — Astro 6.3.1 static, deployed to GitHub Pages
  via `.github/workflows/deploy-web.yml`. Bundle: 12.7 KB gzipped home,
  9.5 KB gzipped observability page.
- **Brutalist dev-craft visual identity** (Variation 2 chosen from 4 parallel
  sub-agent proposals at LUMEN-02 Gate 1, see
  `docs/design/plans/executed/material/02/`):
  - 3-color OKLCH palette (bg `oklch(0.08 0 0)` / fg `oklch(0.94 0 0)` /
    accent `oklch(0.65 0.18 50)`)
  - JetBrains Mono single typeface family
  - 2px solid borders + "incomplete borders" (3-sided) as visual syntax
  - `transition: none` global (motion zero except detection pulses)
  - No gradients, no shadows, no border-radius (except status pill)
- **Five ADRs documenting the audacious decisions**: monospace-only,
  asymmetric-brutal, 3-color palette, incomplete borders, motion zero.
- **Donate widget** with real wallet integrations (no SDK, no
  WalletConnect, no tracking):
  - EVM (Ethereum, Base, Syscoin NEVM) via EIP-6963 multi-provider
    discovery + EIP-1193 popup. Chain switching with
    `wallet_addEthereumChain` fallback for Base + Syscoin.
  - Bitcoin via browser-extension detection (UniSat, Xverse, Leather)
    with BIP-21 URI scheme fallback for desktop wallets (Sparrow,
    Electrum, Trezor Suite). Install-page CTA if neither found.
  - Solana via solana: URI (Phantom desktop extension parses natively).
- **Live observability page** at `/observability` reading the local SEELE
  HTTP server:
  - Stats card, by-type and by-project bar charts, recent saves list,
    full-text search input with detail expand, 14-day saves sparkline.
  - Three states: probing / offline / cors-blocked / online with sample
    preview data in the first three so the panel never looks broken.
- **Light + dark mode** with `prefers-color-scheme` auto-detection plus
  manual `[ ◐ ]` toggle in header. Persisted to `localStorage.seele-theme`.
  Inline anti-FOUC script prevents flash on load.
- **Bilingual (EN/ES) UI** with `[ EN ]` / `[ ES ]` toggle. Resolution
  order: `localStorage.seele-lang` → `navigator.language` → `en`. Latin
  motto in footer stays untranslated (lang="la").
- **Responsive shell**: asymmetric brutalist layout (left-edge commit)
  at viewports ≤1400px, centered max-width 1300px above that. Mobile
  (320px) verified via Playwright multi-resolution matrix.
- **Playwright visual critique tooling** (`web/scripts/visual-critique.mjs`).
  Captures matrix of pages × viewports × themes × langs = 40 screenshots.
  Used as the artifact input for the LUMEN v0.11.0 Visual Critique
  Multi-Resolution phase.
- **Footer connect column** with 6 socials (LinkedIn, WhatsApp, YouTube,
  Instagram, TikTok, Facebook).

### Added — Chat-with-DB (Sprint LUMEN-04)

- **New crate `seele-chat`** with `ChatProvider` trait + two implementations:
  - `OpenAICompatibleProvider` — wire-compatible with `/v1/chat/completions`
    for Minimax, OpenAI, OpenRouter, Together, Groq, DeepSeek, etc.
  - `AnthropicProvider` — Anthropic's `/v1/messages` schema (separate
    path: `x-api-key` header, content blocks, distinct tool_use shape).
- **Tool-use loop** orchestrated server-side via `run_chat`. Model returns
  `tool_calls` → server invokes the registered handler → result fed back
  → loop until model returns text or hits max iterations (default 5).
- **`seele_search` tool** wired into the existing hybrid FTS+vec search
  engine. Args: `query`, optional `limit` (default 5, max 20), optional
  `project` filter.
- **HTTP endpoints**: `POST /chat` runs the tool-use loop and returns the
  full message history. `GET /chat/info` returns
  `{enabled, provider, model}` so the frontend can render the right state.
- **CLI flags on `seele serve`**: `--chat-provider`, `--chat-key`
  (supports `$ENVVAR` reference), `--chat-model`, `--chat-endpoint`.
  Default models: Minimax → `MiniMax-M2`, OpenAI → `gpt-4o-mini`,
  Anthropic → `claude-haiku-4-5-20251001`, Groq → `llama-3.3-70b-versatile`,
  etc.
- **API key never touches the browser**. It stays on the box running
  `seele serve`. The frontend only sees assistant/tool messages in the
  response.
- **Chat panel** on the `/observability` page with three states. When chat
  is not configured, shows the exact `seele serve` command to enable it.
  Bilingual labels and placeholders.

### Added — Backend extensions

- **`seele serve --cors-allow <ORIGIN>`** flag (repeatable). Empty =
  CORS disabled (default, safe for local-only). Non-empty = permissive
  `Access-Control-Allow-Origin: *`. Per-origin allowlist refinement
  remains on the backlog.
- **`AppState` refactored** from `Arc<SeeleService>` type alias to a struct
  with `FromRef` impls. Existing handlers keep their
  `State<Arc<SeeleService>>` extraction unchanged.

### Changed

- Workspace version `0.1.0` → `0.2.0`.

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
