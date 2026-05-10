# Auditoría profunda ENGRAM vs plan SEELE

**Fecha**: 2026-05-09
**Fuente**: GitHub `Gentleman-Programming/engram` v1.15.10 (release 2026-05-07).
**Postura legal**: ENGRAM MIT — ver `02-reimplementacion-inspirada.md`.
**Método**: WebFetch del README, DOCS.md, ARCHITECTURE.md, CODEBASE-GUIDE.md, main.go y estructura de directorios.

## Estructura del proyecto upstream

```
engram/
├── cmd/engram/main.go           # CLI dispatcher (~1080 líneas)
├── internal/
│   ├── store/                   # SQLite + FTS5 + data operations
│   ├── server/                  # HTTP REST API (port 7437)
│   ├── mcp/                     # MCP stdio (19 tools)
│   ├── project/                 # DetectProject 5-case algorithm
│   ├── sync/                    # Git chunks + cloud transport
│   ├── cloud/
│   │   ├── cloudserver/         # /sync API + dashboard + auth
│   │   ├── cloudstore/          # Postgres-backed cloud storage
│   │   ├── dashboard/           # templ-rendered HTML
│   │   └── autosync/            # Background goroutine manager
│   ├── setup/                   # Agent integration installer
│   ├── tui/                     # bubbletea TUI
│   ├── llm/                     # LLM integration (semantic judging)
│   ├── obsidian/                # Obsidian export
│   ├── diagnostic/              # Doctor checks
│   └── version/                 # Version metadata
├── plugin/                      # Per-agent integrations
├── tools/                       # Repair scripts
├── docs/                        # User docs
├── docker/cloud/                # Cloud deployment
└── .goreleaser.yaml             # Release pipeline
```

## Tabla de feature parity — categoría A (Storage)

| Feature ENGRAM | SEELE v0.1 | SEELE v0.2+ | Nota |
|---|---|---|---|
| `sessions` table | ✅ | | Adoptado tal cual + ULID en lugar de TEXT id arbitrario |
| `observations` table | ✅ | | Adoptado con shape: id (ULID), session_id, type, title, content, tool_name, project, scope, topic_key, normalized_hash, revision_count, duplicate_count, last_seen_at, created_at, updated_at, deleted_at |
| `observations_fts` (FTS5 over title/content/tool_name/type/project) | ✅ | | Triggers idénticos en concepto, escritura nuestra |
| `user_prompts` table | ✅ | | Linked a sessions |
| `prompts_fts` | ✅ | | |
| `memory_relations` table | ✅ | | Schema: id, sync_id (UNIQUE), source_id, target_id, relation, judgment_status (pending/judged/orphaned/ignored), reason, evidence, confidence (0-1), marked_by_actor, marked_by_kind, marked_by_model, session_id |
| `sync_chunks` table | ✅ | | Para git sync deduplication |
| `sync_apply_deferred` | | ✅ v0.2 | Solo necesario con cloud sync |
| `schema_version` migrations table | ✅ | | refinery |
| Soft delete (`deleted_at`) | ✅ | | |
| Topic key upserts | ✅ | | revision_count incrementa en mismo project+scope+topic_key |
| Normalized hash dedup window | ✅ | | duplicate_count incrementa, last_seen_at actualiza |
| Privacy stripping `<private>...</private>` | ✅ | | En layer del store + opcionalmente en plugin layer |
| `embedding` column / vec0 virtual table | ✅ ⚡ | | **Diferenciador SEELE** — ENGRAM no tiene embeddings |
| Virtual generated columns over JSON metadata | ✅ ⚡ | | **Diferenciador SEELE** — `meta_kind`, `meta_domain`, etc para filtros sub-10ms |
| Partial B-tree indexes | ✅ ⚡ | | **Diferenciador SEELE** |
| `links` table (general purpose) | ✅ | | Coexiste con memory_relations (relations es para conflict, links es para graph general) |

## Categoría B — Search / retrieval

| Feature ENGRAM | SEELE v0.1 | SEELE v0.2+ | Nota |
|---|---|---|---|
| FTS5 query sanitization (wrap words in quotes) | ✅ | | |
| Search across title/content/tool_name/type/project | ✅ | | |
| Filter by type, project, scope | ✅ | | |
| Limit param (default 10) | ✅ | | |
| Annotation lines en search results (`supersedes:`, `conflicts:`) | ✅ | | |
| Vector similarity (cosine) | ✅ ⚡ | | **Diferenciador SEELE** — sqlite-vec |
| Hybrid search RRF | ✅ ⚡ | | **Diferenciador SEELE** |
| Boost por `score` metadata | ✅ | | Configurable via `--boost-score-multiplier` |
| Reranking por LLM | | ✅ v0.2 | Cross-encoder o LLM rerank post-RRF |
| Query expansion (sinónimos, traducciones) | | ✅ v0.3 | |

## Categoría C — CLI commands

| Comando ENGRAM | SEELE v0.1 | SEELE v0.2+ | Nota |
|---|---|---|---|
| `engram serve [port]` | `seele serve [port]` | | port default 7437 (heredado para compat) |
| `engram mcp [--tools=PROFILE]` | `seele mcp [--tools=PROFILE]` | | stdio v0.1, HTTP v0.2 |
| `engram tui` | `seele tui` | | ratatui en lugar de bubbletea |
| `engram search <q> [--type --project --scope --limit]` | ✅ | | Same flags |
| `engram save <title> <msg> [--type --project --scope --topic]` | ✅ | | Same flags |
| `engram timeline <obs_id>` | ✅ | | |
| `engram context [project]` | ✅ | | |
| `engram stats` | ✅ | | |
| `engram export [file]` | ✅ | | JSON dump |
| `engram import <file>` | ✅ | | JSON ingest |
| `engram doctor [--json] [--project]` | ✅ | | |
| `engram setup [agent]` | ✅ parcial | ✅ completo v0.2 | v0.1: Claude Code + un genérico. v0.2: OpenCode, Gemini CLI, Codex, VS Code, Cursor, Windsurf |
| `engram sync` (git chunks export) | ✅ | | |
| `engram sync --import` | ✅ | | |
| `engram sync --status` | ✅ | | |
| `engram sync --cloud --project NAME` | | ✅ v0.2 | Cloud diferido |
| `engram cloud status\|enroll\|config\|serve` | | ✅ v0.2 | |
| `engram cloud upgrade {doctor,repair,bootstrap,status,rollback}` | | ✅ v0.2 | |
| `engram conflicts list\|show\|stats` | ✅ | | Read-only ops del schema |
| `engram conflicts scan [--semantic]` | | ✅ v0.2 | Semantic scan requiere LLM |
| `engram conflicts deferred [--replay]` | | ✅ v0.2 | |
| `engram projects list\|consolidate\|prune` | ✅ | | |
| `engram obsidian-export` | | ✅ v0.2 | |
| `engram version` | ✅ | | |
| `engram help` | ✅ | | clap genera por default |

## Categoría D — MCP tools (19 tools)

Renamed con prefijo `seele_*` en lugar de `mem_*` (diferencia de branding sin perder semántica).

| Tool ENGRAM | SEELE v0.1 | Notas |
|---|---|---|
| `mem_save` | `seele_save` | Input: `{session_id, type, title, content, project?, scope?, topic_key?, capture_prompt?}`. Types: `decision\|architecture\|bugfix\|pattern\|config\|discovery\|learning`. Scope: `project` (default) \| `personal`. Upserts en topic_key. |
| `mem_update` | `seele_update` | Partial updates de title/content/type/scope/topic_key. |
| `mem_delete` | `seele_delete` | Soft default; `hard_delete=true` opcional. |
| `mem_suggest_topic_key` | `seele_suggest_topic_key` | Family heuristics: `architecture/*`, `bug/*`, `decision/*`, etc. |
| `mem_search` | `seele_search` | FTS + filters + limit + annotation lines. |
| `mem_context` | `seele_context` | Recent sessions + prompts + observations del project/scope. |
| `mem_timeline` | `seele_timeline` | Chronological context around observation_id (before/after). |
| `mem_get_observation` | `seele_get_observation` | Full untruncated content. |
| `mem_save_prompt` | `seele_save_prompt` | Records what user asked; injected before next save. |
| `mem_stats` | `seele_stats` | Counts por sessions/observations/prompts/projects. |
| `mem_session_start` | `seele_session_start` | Input: `{id, project, directory?}`. |
| `mem_session_end` | `seele_session_end` | Input: `{id, summary?}`. |
| `mem_session_summary` | `seele_session_summary` | Structured: `## Goal`, `## Discoveries`, `## Accomplished`, `## Next Steps`, `## Relevant Files`. |
| `mem_capture_passive` | `seele_capture_passive` | Parsea `## Key Learnings:` sections, save bullets/numbered. |
| `mem_merge_projects` | `seele_merge_projects` | Admin: reasigna observations/sessions/prompts de varios `from` a un `to` canónico. |
| `mem_current_project` | `seele_current_project` | Detect from cwd. NUNCA error — devuelve project_source y warning si ambiguo. |
| `mem_doctor` | `seele_doctor` | Read-only health check + project detection report. |
| `mem_judge` | `seele_judge` | Record verdict para pending memory_relation. Relations: `related\|compatible\|scoped\|conflicts_with\|supersedes\|not_conflict`. |
| `mem_compare` | `seele_compare` | Persist semantic relation entre dos observation IDs. Idempotent per pair. |

## Categoría E — HTTP REST endpoints

Adopción 1-a-1 de la API de ENGRAM (ya documentada en sus DOCS.md):

| Endpoint ENGRAM | SEELE v0.1 |
|---|---|
| `POST /sessions` | ✅ |
| `POST /sessions/{id}/end` | ✅ |
| `GET /sessions/recent` | ✅ |
| `DELETE /sessions/{id}` | ✅ |
| `POST /observations` | ✅ |
| `GET /observations/recent` | ✅ |
| `GET /observations/{id}` | ✅ |
| `PATCH /observations/{id}` | ✅ |
| `DELETE /observations/{id}?hard=true` | ✅ |
| `GET /search` | ✅ |
| `GET /timeline` | ✅ |
| `GET /context` | ✅ |
| `POST /prompts` | ✅ |
| `GET /prompts/recent` | ✅ |
| `GET /prompts/search` | ✅ |
| `DELETE /prompts/{id}` | ✅ |
| `POST /observations/passive` | ✅ |
| `GET /export?project=` | ✅ |
| `POST /import` | ✅ |
| `GET /conflicts` (admin) | ✅ |
| `GET /conflicts/{relation_id}` | ✅ |
| `GET /conflicts/stats` | ✅ |
| `POST /conflicts/scan` | v0.2 |
| `GET /conflicts/deferred` | v0.2 |
| `POST /conflicts/deferred/replay` | v0.2 |
| `GET /sync/status` | ✅ |
| `POST /sync/mutations/push` (cloud) | v0.2 |
| `GET /sync/mutations/pull` (cloud) | v0.2 |

## Categoría F — Cloud (todo diferido a v0.2+)

ENGRAM tiene un cloud server completo (Postgres + dashboard templ + auth). SEELE v0.1 NO incluye cloud.

| Feature ENGRAM cloud | SEELE | Nota |
|---|---|---|
| Cloud server con Postgres | v0.2 | Diferido |
| Dashboard templ-rendered (HTMX) | nunca | SEELE no apunta a dashboard web propio |
| Bearer token + JWT auth | v0.2 | |
| AcquireSyncLease (lease-guarded) | v0.2 | |
| Mutation push/pull endpoints | v0.2 | |
| `ENGRAM_CLOUD_*` env vars | v0.2 | |
| `engram cloud upgrade *` ops | v0.2 | |
| Postgres audit log | v0.2 | |
| Insecure auth mode | v0.2 | |

## Categoría G — Algoritmos

| Algoritmo ENGRAM | SEELE v0.1 | Nota |
|---|---|---|
| `DetectProject` 5-case algorithm | ✅ | Adoptado: config.json → git remote → git root → git child scan → dir basename. Timeout 200ms, skip noise dirs (node_modules, vendor, .venv, etc). |
| Topic key family heuristics | ✅ | Prefixes: architecture/, bug/, decision/, pattern/, config/, discovery/, learning/. |
| Normalized hash dedup | ✅ | hash(content) + project + scope + type + title en ventana temporal. |
| Privacy stripping en 2 capas | ✅ | Plugin layer (TS) + store layer Rust (`stripPrivateTags`). |
| Capture passive parser | ✅ | Detecta `## Key Learnings:` + bullets/numbered, save c/u como observation. |
| Progressive disclosure 3-layer | ✅ | search (~100 tokens) → timeline → get_observation. |
| FTS5 query sanitization | ✅ | Wrap words en quotes para escape. |
| Autosync debounce 500ms + lease | v0.2 | Solo para cloud. |

## Categoría H — Configuration / Environment

| Var ENGRAM | SEELE | Nota |
|---|---|---|
| `ENGRAM_DATA_DIR` (default `~/.engram`) | `SEELE_DATA_DIR` (default `~/.seele`) | |
| `ENGRAM_PORT` (default 7437) | `SEELE_PORT` (default 7437) | port heredado para compat MCP configs |
| `ENGRAM_PROJECT` | `SEELE_PROJECT` | Default project para sync/status. |
| `ENGRAM_CLOUD_AUTOSYNC` | v0.2 | |
| `ENGRAM_CLOUD_TOKEN` | v0.2 | |
| `ENGRAM_CLOUD_SERVER` | v0.2 | |
| `ENGRAM_AGENT_CLI` | `SEELE_AGENT_CLI` v0.2 | claude / opencode para semantic scanning |
| `ENGRAM_DATABASE_URL` (cloud) | v0.2 | Postgres DSN |
| `ENGRAM_JWT_SECRET` (cloud) | v0.2 | |

## Categoría I — Operational tools

| Feature ENGRAM | SEELE v0.1 | Nota |
|---|---|---|
| Doctor command | ✅ | Health check + project detection report |
| Stats command | ✅ | Counts + DB size + last save |
| Export JSON | ✅ | Per-project filtering |
| Import JSON | ✅ | |
| Git sync workflow | ✅ | Compressed chunks en `.seele/` git-tracked |
| Setup wizard Claude Code | ✅ | |
| Setup wizard OpenCode | v0.2 | |
| Setup wizard Gemini CLI | v0.2 | |
| Setup wizard Codex | v0.2 | |
| Setup wizard VS Code | v0.2 | |
| Setup wizard Cursor | v0.2 | |
| Setup wizard Windsurf | v0.2 | |
| Setup wizard Antigravity | v0.2 | |
| Catppuccin Mocha theme | v0.2 (theme alternativo) | SEELE v0.1 tiene theme propio sobrio |
| Repair scripts (`tools/repair-*.sh`) | v0.2 | |
| Docker compose para cloud | v0.2 (con cloud) | |
| goreleaser config | nunca | SEELE usa GitHub Actions custom |

## Categoría J — Conflict detection (Phase 2-4 beta de ENGRAM)

| Feature ENGRAM | SEELE v0.1 | SEELE v0.2 | Nota |
|---|---|---|---|
| `memory_relations` table + judgment_status | ✅ | | Schema completo en v0.1 |
| Manual judgment via `seele_judge` MCP tool | ✅ | | Read/write básico |
| Manual semantic compare via `seele_compare` | ✅ | | |
| Read-only conflict views (`list`, `show`, `stats`) | ✅ | | |
| `conflicts scan --semantic` (LLM-based) | | ✅ | Requiere LLM CLI integration |
| `conflicts deferred` + `--replay` | | ✅ | Solo con cloud sync |
| Concurrency control (`--concurrency N`) | | ✅ | |
| `--max-insert N`, `--max-semantic N` | | ✅ | |
| `--dry-run` / `--apply` flags | | ✅ | |
| `--fix-exported` | | ✅ | |
| `--interactive` mode | | ✅ | |

## Resumen ejecutivo

**Funcionalidades de ENGRAM v0.1 adoptadas por SEELE v0.1**: ~85% (todo lo local, sin cloud).
**Funcionalidades diferidas a v0.2+**: cloud sync (todo), conflict scan semantic, setup wizards extra agentes, obsidian export, repair scripts, theme alternativo.
**Funcionalidades NO adoptadas**: dashboard templ web (nunca — SEELE local-first puro), goreleaser (sub-decisión técnica).
**Funcionalidades agregadas por SEELE que ENGRAM no tiene**: embeddings vectoriales, hybrid search RRF, virtual generated columns, ULID IDs.

## Implicancias para el plan

Plan original SEELE tenía 8 crates y ~7-10K LOC. **El plan actualizado tiene ~11 crates y ~10-15K LOC** dada la inclusión de:

- `seele-sync` (git chunks)
- `seele-setup` (agent wizard)
- `seele-project` (project detection 5-case)
- Mucho más en `seele-storage` (sessions/observations/relations vs solo memories)
- Mucho más en `seele-mcp` (19 tools vs 9)
- Mucho más en `seele-http` (~28 endpoints vs ~12)

**Sprints estimados** sube de 3-4 a **5-7 sprints AEGIS** para v0.1 funcional con todo lo descrito.

Detalle de bloques de táctica en `tactica/00-INDEX.md` (próxima fase).
