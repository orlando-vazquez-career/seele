## 2. System Architecture & Crate Topology

SEELE is a **Cargo workspace of 13 crates** (`resolver = "2"`) that build into a single binary, `seele`. The architecture is a clean dependency-acyclic layering: a leaf domain crate at the bottom, data and capability crates in the middle, transport crates above them, and the CLI binary at the top wiring everything together. This section is the map; the territory is detailed in §3–§18, and the dynamic behavior in §19.

### 2.1 The layered crate graph

```
                          ┌───────────────┐
   binary / orchestrator  │   seele-cli   │  (17 subcommands; depends on ALL crates)
                          └───────┬───────┘
                                  │
        ┌─────────────────────────┼───────────────────────────────┐
        │                         │                                │
   ┌────┴─────┐            ┌──────┴──────┐                   ┌──────┴──────┐
   │ seele-tui│            │  seele-mcp  │                   │ seele-sync  │
   │ (ratatui)│            │ (MCP stdio) │                   │ seele-setup │
   └────┬─────┘            └──────┬──────┘                   │ seele-      │
        │                         │                          │  engram-    │
        │      ┌──────────────────┴──────────┐               │  import     │
        │      │        seele-http           │               │ seele-      │
        │      │ (axum REST + service layer) │               │  project    │
        │      └──────┬───────────────┬──────┘               └──────┬──────┘
        │             │               │                             │
        │       ┌─────┴─────┐   ┌─────┴──────┐                       │
        └───────┤seele-search│  │ seele-chat │ (standalone provider) │
                └─────┬──────┘  └────────────┘                       │
                      │                                              │
            ┌─────────┴──────────┬──────────────┐                   │
            │                    │              │                   │
     ┌──────┴──────┐     ┌───────┴──────┐  ┌────┴─────────┐         │
     │seele-storage│     │seele-embedder│  │  (project,   │         │
     │ (SQLite +   │     │ (ONNX/ort)   │  │   setup ──────┼─────────┘
     │ FTS5 + vec0)│     └───────┬──────┘  └────┬─────────┘
     └──────┬──────┘             │              │
            └───────────┬────────┴──────────────┘
                        │
                 ┌──────┴──────┐
                 │ seele-core  │  (domain types, SeeleId/ULID, errors — NO internal deps)
                 └─────────────┘
```

**Exact dependency edges** (internal `seele-*` deps only, from each crate's `Cargo.toml`):

| Crate | Depends on (internal) | Role |
|---|---|---|
| `seele-core` | — | Domain types, `SeeleId` (ULID), metadata, relations, links, sessions, filters, errors. The foundation. |
| `seele-embedder` | `core` | ONNX `all-MiniLM-L6-v2` embeddings (+ `FakeEmbedder`). |
| `seele-storage` | `core` | SQLite + FTS5 + vec0; migrations, stores, dedup, privacy, pool. |
| `seele-project` | `core` | 5-case project-name detection. |
| `seele-setup` | `core` | MCP install wizard for agents. |
| `seele-search` | `core`, `storage`, `embedder` | Hybrid FTS+vec retrieval, RRF fusion. |
| `seele-sync` | `core`, `storage` | Git-friendly gzip JSON sync chunks. |
| `seele-engram-import` | `core`, `storage` | One-shot ENGRAM/MNEMA migration. |
| `seele-chat` | — (standalone) | Multi-provider LLM chat backend (OpenAI-compatible + Anthropic). |
| `seele-http` | `core`, `storage`, `search`, `embedder`, `chat` | axum REST API + the shared **service layer**. |
| `seele-mcp` | `core`, `storage`, `search`, `embedder`, `http` | MCP stdio server; **reuses the HTTP service layer**. |
| `seele-tui` | `core`, `storage`, `search`, `http`, `embedder` | ratatui terminal UI. |
| `seele-cli` | **all of the above** | The `seele` binary; routes subcommands to the right crate. |

Two topology facts worth internalizing:

- **`seele-core` has no internal dependencies** — it is the shared vocabulary every other crate speaks. Changes here ripple everywhere, so it is the most stability-sensitive crate.
- **`seele-mcp` depends on `seele-http`** rather than re-implementing business logic. The service layer that the REST handlers call is the same logic the MCP tools call, so the three transports stay behaviorally consistent by construction.

### 2.2 Two planes: control vs data

It helps to read SEELE as a thin **control plane** (how requests arrive) sitting on a shared **data plane** (how memory is stored and retrieved).

**Control plane (transports)** — each is a different doorway into the same core:
- `seele-cli` → in-process calls against the local DB (no network).
- `seele-mcp` → JSON-RPC 2.0 over stdio, the agent-facing default.
- `seele-http` → REST over TCP, for tooling, dashboards, and the web observability page.
- `seele-tui` → interactive local browsing.

**Data plane (the memory core):**
- `seele-storage` owns the SQLite file: the schema (observations, prompts, sessions, relations, links, sync chunks), the FTS5 shadow tables and their triggers, the vec0 virtual tables, the r2d2 connection pool, WAL mode, dedup hashing, privacy stripping, and soft-delete.
- `seele-embedder` turns text into 384-dim vectors (ONNX, with a deterministic fake fallback).
- `seele-search` runs the two retrieval paths (FTS5 `MATCH` and vec0 KNN) and fuses them with RRF.
- `seele-core` defines the types all of the above exchange, and the critical `SeeleId::as_i64()` bridge that maps a textual ULID to the INTEGER rowid vec0 requires.

The **service layer** in `seele-http` (`service.rs`) is the seam between the two planes: it composes storage + embedder + search into the verbs (`save`, `search`, `show`, `list`, `delete`, `restore`, `link`, `stats`, …) that every transport exposes.

### 2.3 The unifying identifier and the vec0 bridge

A single design decision threads through the whole data plane: **every record is keyed by `SeeleId`, a newtype around a ULID**, stored as text in SQL. But `sqlite-vec`'s vec0 virtual tables key vectors by an **INTEGER rowid**. SEELE bridges the two with `SeeleId::as_i64()`, which packs **bytes `9..16` of the ULID** (the last 7 bytes / low 56 bits of its random tail) into an i64 with the top byte zeroed to stay non-negative — this is the authoritative storage↔vector mapping. That value is written into a regular **stored** `int_id` column (the only *virtual generated* columns in the schema are the five `meta_*` columns). *Note: `CLAUDE.md` still describes this as "first 6 bytes" and an "approximate virtual `int_id`" — both are stale relative to the shipped source; the running code (`crates/seele-core/src/id.rs:35-41`) uses bytes `9..16`. See §3 and §4.* Understanding this bridge is the key to reading the save and search flows in §19, and its collision surface is flagged as an item in §23.

### 2.4 Repository layout

```
SEELE/
├── crates/               # the 13 workspace crates (see table above)
├── web/                  # Astro static landing + observability page (Brutalist design system)
├── docs/
│   ├── INSTALLATION.md, AGENT-SETUP.md, ENGRAM-MIGRATION.md, INDEX.md
│   ├── aegis/devlogs/    # sprint-by-sprint development logs + cost ledger
│   ├── design/           # web design devlogs + component specs (LUMEN)
│   └── compendium/       # ← this document
├── genesis/plans/        # strategy, 13 architecture ADRs, sprint/tactical plans
│   ├── estrategia/        # overview, scope/MVP, ENGRAM feature audit, naming
│   ├── arquitectura/      # ADR 01–13 (the design rationale, digested in §22)
│   └── tactica/           # sprint plans (executed/ holds closed cycles)
├── scripts/              # install.{sh,ps1}, STELE-residual checks, smoke tests
├── .github/workflows/    # ci.yml, release.yml, deploy-web.yml
├── Cargo.toml            # workspace manifest + shared deps + release profile
├── rust-toolchain.toml   # pins Rust 1.85
├── CLAUDE.md, DESIGN.md, README.md, CHANGELOG.md, CREDITS.md
```

### 2.5 Development protocol context (the "why" behind the structure)

SEELE was not built ad hoc; its structure reflects three named processes that recur throughout the docs and are digested in §22:

- **AEGIS** — the engineering protocol: plans live in `genesis/plans/`, each sprint closes with a devlog under `docs/aegis/devlogs/` and a git tag, with two mandatory human gates (plan approval, close approval). Five sprints produced v0.1.0.
- **Cloven** — an external-review persona invoked between cycles; its findings (e.g. "sync import must be transactional," "setup writes must be atomic," "git subprocess must have a timeout") are recorded in `CLAUDE.md` and were resolved in specific commits.
- **LUMEN** — the design protocol governing the `web/` landing, which produced the Brutalist dev-craft visual system documented in `DESIGN.md` (3-color OKLCH palette, single JetBrains Mono family, 2px borders as the hierarchy primitive, zero motion).

This discipline is why the codebase is small, layered, and well-tested — and it is the context any improvement proposal should respect.
