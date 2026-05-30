# SEELE — Technical Compendium

> *Super stellatum firmamentum iudicat Deus, sicut nos iudicamus.*
> *Over the starry firmament God judges, as we judge.*

| | |
|---|---|
| **Subject** | SEELE — a local-first memory engine for AI agents |
| **Version documented** | `v0.2.0` (workspace), shipped baseline `v0.1.0` |
| **Repository** | `C:/dev/tools/SEELE` — `https://github.com/orlando-vazquez-career/seele` |
| **Language / stack** | Rust 1.85 (edition 2021) · SQLite (FTS5 + sqlite-vec/vec0) · ONNX (`all-MiniLM-L6-v2`) · axum · ratatui · clap |
| **Scale** | ~19,500 lines of Rust across 13 workspace crates + an Astro static site |
| **License** | MIT © 2026 DevZen SpA (clean-room reimplementation inspired by ENGRAM) |
| **Document date** | 2026-05-29 |
| **Document purpose** | Exhaustive technical reference of the entire system, intended to be handed to another AI so it can propose improvements |

---

## How this document was produced

Every detailed section (§3–§23) was written by a dedicated analysis pass that read the actual source of that subsystem, and was then **adversarially fact-checked** by a second pass that re-read the source and corrected any claim it could not verify. The orientation sections (§1, §2, §24) and this front matter were authored from a whole-system reading of the repository's `README.md`, `DESIGN.md`, `CLAUDE.md`, `Cargo.toml`, `docs/INDEX.md`, the 13 architecture ADRs, and the inter-crate dependency graph.

Where the verification pass found and corrected inaccuracies, they are reflected directly in the prose; a consolidated note on verification confidence is recorded at the end of the assembly.

## How to read it

- **If you want the 60-second model**, read §1 (Executive Summary) and §2 (Architecture & Crate Topology).
- **If you are tracing behavior**, jump to §19 (End-to-End Data Flows), then dive into the specific subsystem section.
- **If you are proposing improvements** (the primary intent of this document), §23 (Known Limitations, Technical Debt & Improvement Surface) is the highest-signal section, backed by §22 (Design Rationale & ADR Digest) for the *why* behind each decision.
- **If you need a precise contract**, §20 (Database Schema Reference) and §21 (Interface Catalog) are the normative appendices.

## Document conventions

- Code references use `path:line` form relative to the repository root (`C:/dev/tools/SEELE`).
- "Observation" is SEELE's canonical name for a stored memory record; "engram" and "memory" appear as synonyms inherited from the ENGRAM lineage (see §24 Glossary).
- "vec0" refers to the virtual-table interface of the vendored `sqlite-vec` extension.
- Throughout, **MCP** = Model Context Protocol (Anthropic), **RRF** = Reciprocal Rank Fusion, **FTS5** = SQLite's full-text search module v5.

---

## Table of Contents

**Part I — Orientation**
- §1 — Executive Summary
- §2 — System Architecture & Crate Topology

**Part II — Foundation & Data Plane**
- §3 — Domain Core (`seele-core`)
- §4 — Storage Layer I — Schema, Migrations, Connection Pool & vec0 Loading
- §5 — Storage Layer II — Stores, Dedup, Privacy, Links, Relations & Chunks
- §6 — Embeddings (`seele-embedder`)
- §7 — Hybrid Search & Reciprocal Rank Fusion (`seele-search`)

**Part III — Transports & Interfaces**
- §8 — MCP Server (`seele-mcp`)
- §9 — HTTP REST API (`seele-http`)
- §10 — Multi-Provider Chat Backend (`seele-chat`)
- §11 — Command-Line Interface (`seele-cli`)
- §12 — Terminal UI (`seele-tui`)

**Part IV — Operations, Migration & Surface**
- §13 — Multi-Machine Sync (`seele-sync`)
- §14 — Agent Setup Wizard (`seele-setup`)
- §15 — Project Detection (`seele-project`)
- §16 — ENGRAM / MNEMA Import (`seele-engram-import`)
- §17 — Web Landing & Observability (`web/`)
- §18 — Build, CI/CD, Distribution & Release

**Part V — Cross-Cutting References**
- §19 — End-to-End Data Flows
- §20 — Database Schema Reference (Data Dictionary)
- §21 — Interface Catalog — MCP Tools × HTTP Endpoints × CLI Subcommands

**Part VI — Rationale & Improvement**
- §22 — Design Rationale & ADR Digest
- §23 — Known Limitations, Technical Debt & Improvement Surface

**Appendix**
- §24 — Glossary & References


---

## 1. Executive Summary

**SEELE is a local-first memory engine for AI agents.** It gives a coding agent (Claude Code, Cursor, Windsurf, and others) a durable, searchable, project-scoped memory that lives on the user's machine — not in a cloud service. An agent *saves* observations ("WAL is faster than DELETE for crash recovery") and later *searches* them back with a hybrid of keyword and semantic retrieval, so prior decisions, bugs, patterns, and discoveries survive across sessions and across context-window resets.

The entire system compiles to **one Rust binary, `seele`**, that exposes the same memory core through **three transports plus an interactive UI**:

1. **CLI** — 17 `clap` subcommands (`save`, `search`, `show`, `list`, `delete`, `restore`, `link`, `stats`, `projects`, `doctor`, `setup`, `mcp`, `serve`, `tui`, `sync`, `import`) running directly against the local SQLite database.
2. **MCP stdio server** — a JSON-RPC 2.0 server speaking the Model Context Protocol, exposing **19 `seele_*` tools** (with `mnema_*` aliases for ENGRAM/MNEMA drop-in compatibility). This is the primary way agents talk to SEELE.
3. **HTTP REST API** — an `axum` server (~22 operations over ~18 paths) with Bearer auth, OpenAPI 3.1, and a Swagger UI at `/docs`.
4. **TUI** — a `ratatui` terminal interface with five views (Home/Browse/Search/Detail/Stats) and a vi-style keymap.

### The core technical bet

SEELE's retrieval combines three primitives over a single SQLite file:

- **FTS5** for lexical/keyword matching,
- **`sqlite-vec` (vec0)** for vector K-nearest-neighbor search over **384-dimensional embeddings** produced by an **ONNX `all-MiniLM-L6-v2`** model run locally via the `ort` runtime,
- **Reciprocal Rank Fusion (RRF)** to merge the two ranked lists into one result set.

This "FTS + embeddings + RRF" design is SEELE's deliberate divergence from its inspiration, ENGRAM, which bets on "FTS + an LLM judge." SEELE's approach needs no network round-trip at query time and is fully deterministic given a fixed model.

### What makes it *local-first* and *honest infrastructure*

- **Zero required network calls at runtime.** The embedding model is downloaded once (auto-fetched from Hugging Face, SHA256-verified, cached on disk) and then runs offline. If the ONNX runtime cannot initialize, SEELE transparently falls back to a deterministic `FakeEmbedder` so the tool keeps working.
- **Privacy stripping.** Any text wrapped in `<private>...</private>` is removed before hashing, indexing, and storage.
- **Deduplication.** A normalized-hash check suppresses duplicate saves within a 24-hour window; same `(project, scope, topic_key)` upserts in place and bumps a `revision_count`.
- **Soft delete + restore.** Records are tombstoned (`deleted_at`), not destroyed, and can be restored.
- **Multi-machine sync** ships observations between machines as a single git-friendly gzipped JSON chunk, made idempotent by a SHA-256 chunk id and a per-target ledger.
- **Migration path** from ENGRAM/MNEMA via a one-shot importer that preserves ULIDs and promotes `linked_to[]` arrays to first-class link rows.

### Status & provenance

SEELE reached **v0.1.0** as its first stable release, delivered across five sprints under an internal development protocol called **AEGIS**, with external code review by an agent persona named **Cloven**; the web landing was built under a sibling protocol called **LUMEN**. The workspace currently carries **v0.2.0**, accumulating the next round of features (additional setup-agent integrations, a sync chunk splitter, in-place TUI editing, wider project-detection wiring). The codebase is notably disciplined: a clean layered crate graph, ~322 passing tests (plus a handful of `#[ignore]`d model-download and performance-smoke tests), enforced `clippy -D warnings` + `rustfmt`, and a static check guarding against residue of the legacy project name.

### Why this document exists

SEELE is small enough to understand fully and serious enough to be worth improving carefully. The sections that follow document **every subsystem in depth** — types, algorithms, SQL, control flow, and the rationale behind each decision — so that a downstream reasoning system can propose improvements that respect the existing architecture rather than fighting it. The single most actionable section for that purpose is **§23 (Known Limitations & Improvement Surface)**.


---

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


---

## 3. Domain Core — `seele-core`

`seele-core` is the foundation crate of the workspace: the canonical domain model, the typed identifier, the query/filter contracts, and the unified error enum that every other crate consumes. It is the only crate with **no internal `seele-*` dependencies** — its `Cargo.toml` lists only third-party crates. Other crates such as `seele-storage`, `seele-embedder`, `seele-search`, etc. depend on it directly; everything else transitively. The crate doc comment states the contract explicitly:

```rust
// crates/seele-core/src/lib.rs:1-6
//! SEELE core — types, errors, IDs, filters shared across all crates.
//!
//! This crate has no dependencies on storage or runtime concerns.
//! It only defines the data shapes and contracts.
//!
//! See ADR-02 (schema-sqlite) and ADR-10 (mapping-mnema-seele) for rationale.
```

There is no runtime logic here — no SQLite, no async, no I/O beyond what `serde_json` and `std::io::Error` require for the error enum. The crate is pure data shapes plus small pure functions (string parsing, the `as_i64` bit-twiddling). Concurrency, locking, and async are entirely absent; all types are plain `Send + Sync` value types.

### 3.1 Crate dependencies

`Cargo.toml` declares a deliberately minimal dependency set, all via `workspace = true`:

| Crate | Role |
|---|---|
| `serde` + `serde_json` | Derive `Serialize`/`Deserialize`; `serde_json::Value` backs `Metadata` |
| `thiserror` | Derives the `SeeleError` enum (convention: `<Area>Error` with `thiserror`) |
| `ulid` | Underlying ULID implementation wrapped by `SeeleId` |
| `chrono` | `DateTime<Utc>` timestamps on every entity |
| `proptest` (dev) | Property tests in `tests/types_roundtrip.rs` |

The non-dev dependency list is exactly `serde`, `serde_json`, `thiserror`, `ulid`, `chrono` (all `*.workspace = true`); `proptest` is the only dev-dependency. Notably there is **no `anyhow`** (per repo convention `anyhow` is CLI-only) and **no async runtime**.

### 3.2 Public API surface (re-exports)

`lib.rs:17-24` is the canonical export list — the public face of the crate:

```rust
pub use error::{Result, SeeleError};
pub use filter::{MetadataFilter, ObservationQuery};
pub use id::SeeleId;
pub use link::{link_types, Link};
pub use memory::{Observation, ObservationType, Scope};
pub use metadata::Metadata;
pub use relation::{JudgmentStatus, MemoryRelation, RelationKind};
pub use session::{Session, SessionStatus};
```

Every **data** type re-exported here derives at least `Debug + Clone`, and all of them derive `Serialize + Deserialize` — they all cross the serialization boundary (JSON over MCP/HTTP, JSON-gzip over sync, JSON columns in SQLite). This is intentional: the same struct shapes flow unchanged from CLI args to HTTP DTOs to the storage row mapper. The two exceptions are the error exports: `SeeleError` derives only `Debug + thiserror::Error` (no `Clone`, no `Serialize`/`Deserialize`), and `Result` is a plain `type` alias (`Result<T> = std::result::Result<T, SeeleError>`), not a struct/enum, so it derives nothing.

### 3.3 `id.rs` — `SeeleId` and the `as_i64()` vec0 bridge

`SeeleId(Ulid)` is a newtype wrapping `ulid::Ulid`, declared `#[serde(transparent)]` so it serializes as a bare ULID string (e.g. `"01HX..."`), not a tuple. Derives: `Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize` — fully value-semantic and usable as a map key. `Default` delegates to `new()` (fresh ULID), `Display` prints the inner ULID, and `FromStr` parses a ULID, mapping failures to `SeeleError::InvalidInput` (`id.rs:60-68`).

| Method | Signature | Purpose |
|---|---|---|
| `new()` | `fn() -> Self` | Fresh ULID (timestamp + random tail) |
| `from_ulid(u)` | `fn(Ulid) -> Self` | Wrap an existing ULID (used by ENGRAM import to preserve IDs) |
| `as_ulid()` | `fn(&self) -> Ulid` | Unwrap |
| `as_i64()` | `fn(&self) -> i64` | **The vec0 rowid bridge** |
| `timestamp_ms()` | `fn(&self) -> u64` | Extract the ULID's millisecond timestamp |

The single most important — and most subtly documented — method is `as_i64()`:

```rust
// crates/seele-core/src/id.rs:35-41
pub fn as_i64(&self) -> i64 {
    let bytes = self.0.to_bytes();
    let mut int_bytes = [0u8; 8];
    // Top byte zeroed to keep value positive (sign bit clear).
    int_bytes[1..8].copy_from_slice(&bytes[9..16]);
    i64::from_be_bytes(int_bytes)
}
```

**Why it exists.** SQLite's FTS5 and the vendored `sqlite-vec` (`vec0`) virtual tables key on an INTEGER `rowid`, but SEELE's primary key is a textual ULID. A stable, deterministic, mostly-collision-free ULID→`i64` mapping is needed to bridge the two. `as_i64()` is that mapping and, per the schema note below and the repo invariant, it is the **authoritative** one.

**The mechanism (precisely).** A ULID is 16 bytes: bytes `0..6` are the 48-bit timestamp, bytes `6..16` are the 80-bit random tail. `as_i64()` copies **bytes `9..16`** (the **last 7 bytes**, i.e. the low 56 bits of the random tail) into positions `1..8` of an 8-byte big-endian buffer, leaving byte 0 zero. Zeroing the top byte clears the sign bit so the result is always non-negative. So the value carries **56 bits of entropy** drawn from the random portion — it is **not** timestamp-derived and therefore not monotonic with creation time.

> **Documentation discrepancy (flag for the improvement pass).** The shared compendium context and `CLAUDE.md` both state `as_i64()` uses the "first 6 bytes" of the ULID. That is **wrong relative to the source**: the code uses `bytes[9..16]` — the *last 7 bytes* of the random tail. The doc comment at `id.rs:28-34` says "last 56 bits … bytes 9..16" (lines 28 and 31), which matches the code; the actual slice copy is on `id.rs:39` (`int_bytes[1..8].copy_from_slice(&bytes[9..16]);`), and the storage-side comment `observations.rs:23` ("bytes 9..16 of ULID") also matches. (Note that the in-method comment on `id.rs:38` is about zeroing the top byte for sign, not about the byte range.) The prose docs (`CLAUDE.md` §IDs, the section brief) are the outliers and should be corrected to "low 56 bits of the random tail (bytes 9..16)".

**Collision handling.** The doc comment (`id.rs:28-34`) reasons about the birthday bound, stating that "collision probability for 10^6 IDs is roughly 1 in 10^4 (birthday-bound)" and that for SEELE's expected DB size (~100K observations) this is acceptable. The test comment at `id.rs:107-108` adds the complementary figure: with ~56 random bits the 50% collision threshold is ~2^28 (≈268M) IDs, far above a 10K batch. The `int_id` column is declared `INTEGER NOT NULL UNIQUE`; on insert the storage layer (`crates/seele-storage/src/observations.rs:537-580`) loops up to `ID_COLLISION_RETRIES = 5` (`observations.rs:24`), regenerating a fresh `SeeleId` whenever a `ConstraintViolation` whose message contains `observations.int_id` fires (`observations.rs:565-573`), and only after exhausting the retries errors with `StorageError::Conflict("exhausted 5 retries on int_id collision")` (`observations.rs:577-579`). Six unit tests in `id.rs` (`new_ids_unique`, `roundtrip_string`, `invalid_string_errors`, `as_i64_is_positive`, `as_i64_unique_in_batch`, `as_i64_deterministic_for_same_ulid`) and three SeeleId proptests in `tests/types_roundtrip.rs` (`ulid_string_roundtrip_property`, `ulid_to_i64_is_non_negative`, `ulid_to_i64_is_deterministic`) pin the invariants: non-negativity, determinism, string round-trip, and uniqueness across a 10K batch.

**`int_id` (SQL) vs `as_i64()` (Rust).** The section brief frames `int_id` as an "approximate SQL int_id virtual column". The schema header (`V001__initial_schema.sql:7-13`) clarifies the real history: the architecture plan *originally* proposed `int_id` as a **virtual generated column** computing the value from `SUBSTR(id, …) AS INTEGER`, but because ULIDs are Crockford-base32 strings, `CAST(letter AS INTEGER)` yields `0` — that formula was broken. The shipped schema therefore stores `int_id` as a **regular INTEGER column populated at INSERT time from Rust's `as_i64()`** (`observations.rs:540-549`, `588-598`). So in the *current* code there is exactly one mapping — `as_i64()` — written into a plain column; the "approximate virtual column" is a historical/abandoned design, not live behavior. FTS5 binds to it via `content_rowid='int_id'`, and triggers (`observations_ai/ad/au`) propagate `new.int_id`/`old.int_id` into the FTS shadow table; the `observations_vec` (vec0) table stores embeddings at `rowid = int_id`.

### 3.4 `memory.rs` — `Observation`, `ObservationType`, `Scope`

`Observation` is *the* core entity — a single memory unit, **17 fields** in total. Its shape deliberately inherits ENGRAM's fields (sessions, type, scope, topic_key, normalized_hash, revision_count, duplicate_count) and adds vector embedding support downstream. Derives `Debug, Clone, Serialize, Deserialize` (note: **not** `PartialEq` — unlike `ObservationType`/`Scope`, the struct itself is not equatable).

| Field | Type | Meaning / invariant |
|---|---|---|
| `id` | `SeeleId` | ULID primary key |
| `session_id` | `Option<SeeleId>` | Owning session; `ON DELETE SET NULL` in SQL |
| `kind` | `ObservationType` | `#[serde(rename = "type")]` — the JSON/SQL field is `type` |
| `title` | `String` | Short label; FTS-indexed; privacy-stripped before hashing |
| `content` | `String` | Body; FTS-indexed; privacy-stripped before hashing |
| `tool_name` | `Option<String>` | Origin tool (e.g. `seele_save`) |
| `project` | `Option<String>` | Project scope key |
| `scope` | `Scope` | `project` or `personal` |
| `topic_key` | `Option<String>` | e.g. `decision/backend-stack`; drives upsert/revision semantics |
| `normalized_hash` | `Option<String>` | Dedup hash over normalized title+content |
| `revision_count` | `u32` | Incremented on same-`(project,scope,topic_key)` upsert |
| `duplicate_count` | `u32` | Incremented when an identical capture lands in the dedup window |
| `last_seen_at` | `DateTime<Utc>` | Last time the same content was observed |
| `created_at` / `updated_at` | `DateTime<Utc>` | Lifecycle timestamps |
| `deleted_at` | `Option<DateTime<Utc>>` | Soft-delete marker (`NULL` = live) |
| `metadata` | `Metadata` | Free-form JSON sidecar |

The field doc (`memory.rs:7-10`) names the inheritance and the `sqlite-vec` extension explicitly. The `#[serde(rename = "type")]` on `kind` is load-bearing: `type` is a Rust keyword, so the Rust field is `kind` but the wire/column name is `type` — every HTTP DTO and SQL column uses `type`.

**`ObservationType`** is an extensible enum stored as free-form `TEXT` (no SQL `CHECK`). `#[serde(rename_all = "snake_case")]` with a catch-all `Other(String)` marked `#[serde(untagged)]`:

```rust
// crates/seele-core/src/memory.rs:39-54  (variants)
Decision, Architecture, Bugfix, Pattern, Config, Discovery, Learning,   // 7 ENGRAM-inherited
Memory, Skill, AdvisorOutput, Review, Verdict,                          // 5 MNEMA-extended
#[serde(untagged)] Other(String)
```

Twelve named variants (first 7 are ENGRAM-inherited coding-agent types, next 5 are MNEMA extensions) plus `Other(String)` for anything else. `as_str()` (`memory.rs:57-73`) maps to the canonical snake_case string (note `AdvisorOutput → "advisor_output"`), and `from_str_relaxed()` (`memory.rs:76-92`) is the **inverse that never fails** — unknown strings become `Other(s)`. A no-panic proptest (`types_roundtrip.rs:155-159`) guarantees the relaxed parser accepts any printable ASCII.

**`Scope`** is a `Copy` enum, `#[serde(rename_all = "lowercase")]`, with `#[default] Project` (so a missing scope defaults to `project`). It exposes `as_str() -> &'static str` and `from_str_strict(s) -> Option<Self>` — note the asymmetry with `ObservationType`: `Scope` parsing is **strict** (`None` on unknown), reflecting that scope is a closed set while observation type is open.

### 3.5 `metadata.rs` — `Metadata`

`Metadata(pub Value)` is a `#[serde(transparent)]` newtype over `serde_json::Value`, intended to hold a JSON object. SEELE enforces **no schema** here — the doc says consumers (MNEMA, etc.) define their own (`metadata.rs:4-5`). API: `new()`/`Default` produce an empty `Value::Object`; `from_value`/`From<Value>` wrap arbitrary JSON; `get(key)`/`set(key, value)` operate on the object (`set` is a no-op if the inner value isn't an object); `as_str()` returns the JSON-serialized string; `as_value()` borrows the inner `Value`. This is where consumer-specific keys live (e.g. `axiomatic`, `context_mode`).

**Topic-key families — a clarification.** The section brief lists "topic-key families … architecture/bug/decision/pattern/config/discovery/learning" as belonging here. In the *code*, `seele-core` has **no topic-family registry**: `metadata.rs` is solely the JSON wrapper, and `topic_key` is just an `Option<String>` on `Observation`. The default heuristic families (`architecture/*`, `bug/*`, `decision/*`, `pattern/*`, `config/*`, `discovery/*`, `learning/*`, ENGRAM-inherited) and the per-consumer override file `~/.seele/topic-families.toml` are **consumer/config concerns documented in `CLAUDE.md`**, not types in this crate. The core's only structural support for topic keys is the `topic_key` field plus the `(project, scope, topic_key)` upsert invariant enforced in the storage layer.

### 3.6 `relation.rs` vs `link.rs` — two distinct edge types

SEELE has **two separate inter-observation edge concepts**, and keeping them straight matters:

**`MemoryRelation`** (`relation.rs`) is the *judgment-carrying* relation, inherited from ENGRAM, for conflict/invalidation lifecycles where auditability matters (e.g. "decision X is superseded/invalidated by decision Y"). Its lifecycle runs `pending → judged (with reason/evidence/confidence) → orphaned/ignored`. Fields: `id: SeeleId`, `sync_id: String` (sync dedup key, declared `NOT NULL UNIQUE` in SQL), `source_id`/`target_id: SeeleId`, `relation: RelationKind`, `judgment_status: JudgmentStatus`, `reason: Option<String>`, `evidence: Option<String>`, `confidence: Option<f64>`, the `marked_by_actor`/`marked_by_kind`/`marked_by_model: Option<String>` provenance triple, `session_id: Option<SeeleId>`, and `created_at: DateTime<Utc>`. Derives `Debug, Clone, Serialize, Deserialize`.

- `RelationKind` (`#[serde(rename_all="snake_case")]`): `Supersedes`, `ConflictsWith`, `Scoped`, `Related`, `Compatible`, `NotConflict`.
- `JudgmentStatus` (`#[serde(rename_all="lowercase")]`): `Pending`, `Judged`, `Orphaned`, `Ignored`.

Both enums are `Copy` and provide `as_str()`/`from_str_strict()` (strict — `None` on unknown).

**`Link`** (`link.rs`) is the *plain graph edge* — no judgment lifecycle, no provenance. Fields: `id`, `from_id`/`to_id: SeeleId`, `link_type: String` (free-form TEXT), `metadata: Metadata`, `created_at`. The doc comment (`link.rs:7-8`) is explicit about the distinction: *"For invalidation/conflict relationships with auditing, use `MemoryRelation` instead."* Recommended (but non-enforced) link types live in the `link_types` submodule as `&str` constants: `DERIVES_FROM`, `SUPERSEDES`, `RELATED_TO`, `CONTRADICTS`, `EVIDENCE_FOR`, `PART_OF_VERDICT`. Note `supersedes` appears in *both* taxonomies — a `RelationKind` (auditable) and a `link_type` constant (lightweight) — so callers must pick the right edge type for their intent.

| | `MemoryRelation` | `Link` |
|---|---|---|
| Kind field | `RelationKind` (closed enum) | `link_type: String` (free-form) |
| Judgment lifecycle | Yes (`JudgmentStatus`) | No |
| Provenance / evidence | `reason`/`evidence`/`confidence`/`marked_by_*` | None |
| Endpoints | `source_id` → `target_id` | `from_id` → `to_id` |
| Use case | Auditable conflict/invalidation | General graph edge |

### 3.7 `session.rs` — session lifecycle

`Session` groups observations under a working session. Fields: `id: SeeleId`, `project: String` (required, unlike `Observation.project`), optional `directory`, `started_at`, optional `ended_at`/`summary`, and `status: SessionStatus`. `SessionStatus` is a `Copy` enum, `#[serde(rename_all="lowercase")]`: `Active`, `Ended`, `Aborted`, with the usual `as_str()`/`from_str_strict()` pair. The serde test pins the wire form (`SessionStatus::Ended` → `"ended"`).

### 3.8 `filter.rs` — query contracts

`MetadataFilter` is the storage-translated WHERE-clause contract (`Default`-derived). Fields: `kind`, `project`, `scope: Option<Scope>`, `topic_key`, `axiomatic: Option<bool>`, and two `#[serde(default)]` booleans — `include_deleted` and `include_purist` — both defaulting to `false`. The `include_purist` default is behaviorally significant: per ADR-10 capa 4.5 (cited in the doc comment at `filter.rs:5-8`), observations whose `metadata.context_mode == 'purist'` are **excluded from Recall by default**. (The actual SQL exclusion lives in the storage layer, which keys off the `meta_context_mode` virtual generated column; `seele-core` only carries the flag.) `ObservationQuery` bundles an optional `query: Option<String>` (the FTS/semantic text), a `filter: MetadataFilter`, and pagination `limit`/`offset: u32`; its manual `Default` sets `limit = 10`, `offset = 0`, `query = None`.

### 3.9 `error.rs` — `SeeleError`

`SeeleError` is the workspace-wide typed error enum (`thiserror::Error`), with `type Result<T> = std::result::Result<T, SeeleError>`. Variants:

| Variant | `#[error]` prefix | Source / construction |
|---|---|---|
| `Storage(String)` | `storage error: {0}` | manual |
| `Embedder(String)` | `embedder error: {0}` | manual |
| `Search(String)` | `search error: {0}` | manual |
| `Mcp(String)` | `mcp error: {0}` | manual |
| `Http(String)` | `http error: {0}` | manual |
| `InvalidInput(String)` | `invalid input: {0}` | manual (e.g. bad ULID) |
| `NotFound(String)` | `not found: {0}` | manual |
| `Conflict(String)` | `conflict: {0}` | manual |
| `Io(std::io::Error)` | `io error: {0}` | `#[from]` |
| `Serde(serde_json::Error)` | `serialization error: {0}` | `#[from]` |

The five area variants (`Storage`/`Embedder`/`Search`/`Mcp`/`Http`) let each layer collapse its own typed error into a string and bubble it through the shared boundary, while `InvalidInput`/`NotFound`/`Conflict` carry semantic HTTP-mappable meaning. Only `Io` and `Serde` get `#[from]` conversions; the rest are constructed explicitly. `SeeleId::from_str` is the one in-crate site that produces a `SeeleError` (`InvalidInput`).

### 3.10 File-by-file map

- **`lib.rs`** — module declarations and the curated re-export list (§3.2); the crate's public contract surface.
- **`id.rs`** — `SeeleId` newtype, the `as_i64()` vec0 bridge (§3.3), `FromStr`/`Display`/`Default`, and 6 invariant unit tests.
- **`memory.rs`** — `Observation` (17 fields), the open `ObservationType` enum with relaxed parsing, and the closed `Scope` enum.
- **`metadata.rs`** — `Metadata` transparent newtype over `serde_json::Value`; schemaless consumer sidecar.
- **`relation.rs`** — `MemoryRelation` plus `RelationKind`/`JudgmentStatus`; the auditable, judgment-bearing edge.
- **`link.rs`** — `Link` plus the `link_types` constants; the lightweight graph edge.
- **`session.rs`** — `Session` and `SessionStatus`.
- **`filter.rs`** — `MetadataFilter` and `ObservationQuery`; the query DSL the storage layer compiles to SQL.
- **`error.rs`** — `SeeleError` enum and `Result<T>` alias.
- **`tests/types_roundtrip.rs`** — workspace-level serde round-trip tests for every public type plus proptests for `SeeleId`, `ObservationType`, and `Metadata`.

### 3.11 How it connects to the rest of SEELE

`seele-core` types are the lingua franca. `seele-storage` maps SQLite rows to/from `Observation`/`Session`/`Link`/`MemoryRelation`, calls `SeeleId::as_i64()` to populate `int_id`, and translates `MetadataFilter`/`ObservationQuery` into SQL. `seele-search` consumes `ObservationQuery` and returns `Observation`s. `seele-http`/`seele-mcp` expose these shapes as DTOs and JSON-RPC payloads (the `#[serde(rename="type")]` and `rename_all` attributes define those wire contracts). `seele-sync` serializes them to gzip-JSON chunks (`sync_id`/`normalized_hash` drive idempotency). `seele-engram-import` uses `SeeleId::from_ulid` to **preserve** ULIDs during migration. `seele-cli`/`seele-tui` render and construct these same types. Every error in the system can be funneled into `SeeleError`. The key cross-boundary invariant to carry forward: `as_i64()` (low 56 bits of the ULID random tail, bytes 9..16) is the **single authoritative** ULID→rowid mapping, written into the regular `int_id` column — the "virtual generated column" alternative was prototyped, found broken for base32 strings, and abandoned.


---

## 4. Storage Layer I — Schema, Migrations, Connection Pool & vec0 Loading

This section documents the SQLite persistence foundation of SEELE: the `seele-storage` crate's database schema and refinery migration set, the r2d2 connection pool and its canonical PRAGMAs, the runtime mechanism that locates and loads the vendored `sqlite-vec` (`vec0`) loadable extension, the DB open/init flow in `lib.rs`, and the `StorageError` enum. The *stores* layered on top of this foundation (observations, sessions, links, relations, chunks, prompts, dedup, privacy) are covered in Section 5; here we cover only schema, migrations, pool, and extension loading. Where store code is load-bearing for understanding the schema (notably the `int_id` bridge and embedding writes), it is referenced but not re-documented.

### 4.1 Crate placement and responsibilities

`seele-storage` is a leaf-adjacent crate: it depends only on `seele-core` (for `SeeleId`, `ObservationType`, `Scope`, `Metadata`) and on external crates. Everything above it — `seele-search`, `seele-http`, `seele-mcp`, `seele-tui`, `seele-sync`, `seele-engram-import`, `seele-cli` — reaches the database through this crate's `Pool` and store types. The crate's stated purpose in `Cargo.toml:8` is "SQLite + FTS5 + sqlite-vec storage layer for SEELE."

External dependencies (`crates/seele-storage/Cargo.toml`) all use `.workspace = true`; the concrete pins live in the root workspace manifest (`Cargo.toml:44-85`). Their roles:

| Crate | Workspace pin | Role |
| --- | --- | --- |
| `rusqlite` | `0.32`, features `bundled` + `load_extension` | SQLite driver; `bundled` statically compiles SQLite (no system dep), `load_extension` enables loading `vec0` |
| `r2d2` | `0.8` | Generic connection-pool abstraction |
| `r2d2_sqlite` | `0.25` | `SqliteConnectionManager` adapter for r2d2 |
| `refinery` | `0.8`, feature `rusqlite` | Compile-time-embedded SQL migrations |
| `sha2` + `hex` | `0.10` / `0.4` | SHA-256 integrity check + filename derivation for the cached `vec0` binary; also content-dedup hashing (Section 5) |
| `dirs` | `5` | Resolves the user cache directory for the `vec0` install path |
| `serde` / `serde_json` | `1` / `1` | Metadata JSON (de)serialization |
| `thiserror` | `2` | Typed `StorageError` |
| `tracing` | `0.1` | Structured logging |
| `chrono`, `ulid`, `regex`, `once_cell` | `0.4` / `1.1` / `1` / `1.20` | Timestamps, ID handling, privacy regex, lazy statics (used in the stores) |

Dev-dependencies are `tempfile` (throwaway DB dirs in tests) and `proptest` (property tests in `tests/properties.rs`).

### 4.2 File-by-file map

- **`lib.rs`** — Crate root. Declares every module (`chunks`, `error`, `hash`, `links`, `migrations`, `observations`, `pool`, `privacy`, `prompts`, `relations`, `sessions`, `vec0_install`, `vec0_loader`), re-exports the public store types and `Result`/`StorageError`/`Pool`/`PoolConfig`/`init_pool`, and defines the top-level `init_db()` open-and-migrate entry point.
- **`migrations/mod.rs`** — Wraps `refinery::embed_migrations!("./src/migrations")` so the SQL files are compiled into the binary, and exposes `run_pending(conn)`.
- **`migrations/V001__initial_schema.sql`** — The single migration (so far): all tables, indexes, FTS5 virtual tables + triggers, and vec0 virtual table. This is the entire schema.
- **`pool.rs`** — `PoolConfig`, `init_pool()`, the per-connection init closure that sets PRAGMAs and loads `vec0`, and the `load_vec0()` helper using `LoadExtensionGuard`.
- **`vec0_loader.rs`** — Build-time embedding of the per-target `vec0` binary via `include_bytes!`; exposes `vec0_bytes()` and `vec0_extension_suffix()`.
- **`vec0_install.rs`** — Runtime resolution of a usable `vec0` file on disk: env override, cache-dir write with atomic rename, SHA-256 integrity verification. Exposes `ensure_vec0_extension()`.
- **`error.rs`** — `StorageError` enum and the crate's `Result<T>` alias.
- **`vendor/sqlite-vec/`** — Five vendored loadable binaries (one per supported target), upstream checksums, both upstream license texts, and a README documenting provenance and the bump procedure.

### 4.3 The migration system

Migrations are driven by **refinery** with compile-time embedding. `migrations/mod.rs` is intentionally tiny:

```rust
// crates/seele-storage/src/migrations/mod.rs:7-17
use refinery::embed_migrations;

embed_migrations!("./src/migrations");

use crate::error::Result;

/// Run all pending migrations against `conn`.
pub fn run_pending(conn: &mut rusqlite::Connection) -> Result<()> {
    migrations::runner().run(conn)?;
    Ok(())
}
```

`embed_migrations!` scans `./src/migrations` **at compile time** and bakes each `V{N}__{description}.sql` file into the binary, so no SQL is read from disk at runtime — a deliberate property that lets `cargo install seele` produce a self-contained executable. Migration files follow refinery's versioned convention `V{N}__{snake_description}.sql`; the module doc-comment (`migrations/mod.rs:1-5`) instructs that new migrations go in `crates/seele-storage/src/migrations/` as `V{N}__{description}.sql` and are picked up automatically at compile time. Refinery tracks applied versions in its own bookkeeping table (`refinery_schema_history`) inside the same SQLite DB, so `run_pending` is idempotent: re-running it applies only versions not yet recorded. As of workspace version v0.2.0 there is exactly one migration, `V001__initial_schema.sql`, whose own header banner reads "SEELE schema v0.1.0 — initial migration"; the whole schema lives in that file, meaning there have been **no in-place schema evolutions** — the schema was frozen at genesis.

Failures surface as `StorageError::Migration` (a `#[from] refinery::Error`).

### 4.4 Schema reference — tables

`V001__initial_schema.sql` references ADR-02 (`schema-sqlite`), ADR-10 (MNEMA↔SEELE mapping / `context_mode`), and ADR-11 (vendored sqlite-vec). All base tables are declared `WITHOUT ROWID` and use a **TEXT ULID primary key** (`id`). The full table set:

| Table | Purpose | Key columns / constraints |
| --- | --- | --- |
| `sessions` | Agent work sessions | `id` PK; `project` NOT NULL; nullable `directory` and `summary`; `started_at` NOT NULL / `ended_at` nullable, epoch ms; `status` CHECK in (`active`,`ended`,`aborted`) default `active` |
| `observations` | Core memory entity | `id` TEXT PK + `int_id` INTEGER NOT NULL UNIQUE; `session_id` FK→sessions ON DELETE SET NULL; `type`, `title`, `content` NOT NULL; `scope` CHECK in (`project`,`personal`); dedup/soft-delete/timestamp columns; `metadata` JSON default `'{}'` |
| `observations_fts` | FTS5 over observations | external-content table, `content='observations'`, `content_rowid='int_id'` |
| `observations_vec` | vec0 KNN index | `embedding FLOAT[384]` |
| `user_prompts` | Raw user prompts | `id` PK; `session_id` FK→sessions ON DELETE SET NULL; `content` NOT NULL; `project`; `created_at` |
| `prompts_fts` | FTS5 over user_prompts | external-content, `content='user_prompts'` (no `content_rowid`, no sync triggers — see §4.6 gotcha) |
| `links` | General graph edges | `id` PK; `from_id`/`to_id` NOT NULL FK→observations ON DELETE CASCADE; `link_type` NOT NULL; `metadata` JSON default `'{}'`; `created_at`; UNIQUE(`from_id`,`to_id`,`link_type`) |
| `memory_relations` | Judgment-lifecycle relations (ENGRAM-inherited) | `id` PK + `sync_id` NOT NULL UNIQUE; `source_id`/`target_id` NOT NULL FK→observations CASCADE; `relation` NOT NULL; `judgment_status` NOT NULL CHECK in (`pending`,`judged`,`orphaned`,`ignored`) default `pending`; `reason`/`evidence`/`confidence` + `marked_by_actor`/`marked_by_kind`/`marked_by_model` metadata; `session_id` FK SET NULL |
| `sync_chunks` | Git-sync import dedup | composite PK (`target_key`,`chunk_id`); `imported_at` NOT NULL |

#### The `observations` table in detail

```sql
-- crates/seele-storage/src/migrations/V001__initial_schema.sql:33-53
CREATE TABLE observations (
    id                TEXT PRIMARY KEY,     -- ULID
    int_id            INTEGER NOT NULL UNIQUE,  -- ULID random tail mapped to i64
    session_id        TEXT REFERENCES sessions(id) ON DELETE SET NULL,
    type              TEXT NOT NULL,
    title             TEXT NOT NULL,
    content           TEXT NOT NULL,
    tool_name         TEXT,
    project           TEXT,
    scope             TEXT NOT NULL DEFAULT 'project'
                      CHECK (scope IN ('project', 'personal')),
    topic_key         TEXT,
    normalized_hash   TEXT,
    revision_count    INTEGER NOT NULL DEFAULT 0,
    duplicate_count   INTEGER NOT NULL DEFAULT 0,
    last_seen_at      INTEGER NOT NULL,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    deleted_at        INTEGER,
    metadata          TEXT NOT NULL DEFAULT '{}'  -- JSON
) WITHOUT ROWID;
```

Field semantics relevant to this layer:

- **`int_id`** — the authoritative INTEGER rowid surrogate. Because the base tables are `WITHOUT ROWID`, FTS5 and vec0 (which require an integer rowid) cannot reference SQLite's implicit rowid. `int_id` is computed in Rust from `SeeleId::as_i64()` and stored at INSERT time (see §4.6). `UNIQUE` enforces a 1:1 ULID↔int_id mapping per row.
- **`normalized_hash`** — content-dedup key; see §4.8 and Section 5.
- **`revision_count` / `duplicate_count`** — topic-upsert revision counter and within-window duplicate counter (defaults 0).
- **`last_seen_at`, `created_at`, `updated_at`, `deleted_at`** — epoch-ms timestamps; `deleted_at` NULL means *alive* (soft-delete model, §4.8).
- **`metadata`** — JSON blob, default `'{}'`.

##### Virtual generated columns over `metadata`

Five `VIRTUAL` generated columns are added by `ALTER TABLE` so JSON metadata fields become indexable without denormalizing:

```sql
-- crates/seele-storage/src/migrations/V001__initial_schema.sql:56-65
ALTER TABLE observations ADD COLUMN meta_kind TEXT
    GENERATED ALWAYS AS (json_extract(metadata, '$.kind')) VIRTUAL;
ALTER TABLE observations ADD COLUMN meta_domain TEXT
    GENERATED ALWAYS AS (json_extract(metadata, '$.domain')) VIRTUAL;
ALTER TABLE observations ADD COLUMN meta_axiomatic INTEGER
    GENERATED ALWAYS AS (json_extract(metadata, '$.axiomatic')) VIRTUAL;
ALTER TABLE observations ADD COLUMN meta_score REAL
    GENERATED ALWAYS AS (json_extract(metadata, '$.score')) VIRTUAL;
ALTER TABLE observations ADD COLUMN meta_context_mode TEXT
    GENERATED ALWAYS AS (json_extract(metadata, '$.context_mode')) VIRTUAL;
```

These are `VIRTUAL` (computed on read, not stored), so they cost nothing in row size but can back partial indexes. `meta_context_mode` is the ADR-10 MNEMA-bridge field; `meta_axiomatic` marks always-true facts; `meta_kind`/`meta_domain` are taxonomy keys; `meta_score` is a relevance/quality score. These five are the *only* `VIRTUAL` generated columns in the schema (relevant to the `int_id` caveat in §4.6).

### 4.5 Indexes (including partial indexes)

SEELE uses **partial indexes** keyed on `WHERE deleted_at IS NULL` extensively, so soft-deleted rows never bloat the indexes that serve live queries. Full index inventory:

| Index | Table | Columns | Partial predicate |
| --- | --- | --- | --- |
| `idx_sessions_project` | sessions | project | — |
| `idx_sessions_started_at` | sessions | started_at | — |
| `idx_sessions_status` | sessions | status | — |
| `idx_obs_session` | observations | session_id | `deleted_at IS NULL` |
| `idx_obs_project` | observations | project | `deleted_at IS NULL` |
| `idx_obs_topic_upsert` | observations | (project, scope, topic_key) | `topic_key IS NOT NULL AND deleted_at IS NULL` |
| `idx_obs_dedup` | observations | (normalized_hash, project, scope, type, title) | `deleted_at IS NULL` |
| `idx_obs_meta_kind` | observations | meta_kind | `meta_kind IS NOT NULL AND deleted_at IS NULL` |
| `idx_obs_meta_domain` | observations | meta_domain | `meta_domain IS NOT NULL AND deleted_at IS NULL` |
| `idx_obs_meta_kind_domain` | observations | (meta_kind, meta_domain) | `deleted_at IS NULL` |
| `idx_obs_meta_axiomatic` | observations | meta_axiomatic | `meta_axiomatic = 1` |
| `idx_obs_meta_score` | observations | meta_score | `deleted_at IS NULL` |
| `idx_obs_meta_context_mode` | observations | meta_context_mode | `meta_context_mode IS NOT NULL AND deleted_at IS NULL` |
| `idx_obs_created_at` | observations | created_at | `deleted_at IS NULL` |
| `idx_obs_deleted_at` | observations | deleted_at | `deleted_at IS NOT NULL` |
| `idx_prompts_session` | user_prompts | session_id | — |
| `idx_prompts_project` | user_prompts | project | — |
| `idx_prompts_created_at` | user_prompts | created_at | — |
| `idx_links_from` | links | from_id | — |
| `idx_links_to` | links | to_id | — |
| `idx_links_type` | links | link_type | — |
| `idx_relations_source` | memory_relations | source_id | — |
| `idx_relations_target` | memory_relations | target_id | — |
| `idx_relations_status` | memory_relations | judgment_status | — |
| `idx_relations_relation` | memory_relations | relation | — |

Two are noteworthy. `idx_obs_dedup` directly backs the content-dedup lookup (matching `normalized_hash` within a `(project, scope, type, title)` partition). `idx_obs_deleted_at` is the *complement* partial index — it indexes only soft-deleted rows (`deleted_at IS NOT NULL`), so listing/restoring trash is fast without touching the live working set. `idx_obs_meta_axiomatic` is the most narrowly scoped: it indexes only rows where `meta_axiomatic = 1`.

### 4.6 The `int_id` bridge and FTS5 wiring

The single most important schema decision is how a base table keyed by a TEXT ULID drives FTS5 and vec0, both of which require an integer rowid. The migration header records the history (an earlier ADR proposal was broken):

```
-- crates/seele-storage/src/migrations/V001__initial_schema.sql:7-13
-- NOTE on int_id:
--   The arquitectura plan originally proposed a virtual generated column
--   computing int_id from SUBSTR(id, ...) as INTEGER. ULIDs are base32
--   strings, so CAST(letter AS INTEGER) yields 0 — the formula was broken.
--   We instead store int_id as a regular INTEGER column populated at
--   INSERT time from Rust (SeeleId::as_i64). FTS5 and vec0 both reference
--   it for stable rowid mapping.
```

The authoritative mapping is `SeeleId::as_i64()` in `seele-core`, which takes **bytes 9..16 (the last 7 bytes / 56 bits) of the 16-byte ULID** and packs them into an i64 with the top byte zeroed to keep the value non-negative:

```rust
// crates/seele-core/src/id.rs:35-41
pub fn as_i64(&self) -> i64 {
    let bytes = self.0.to_bytes();
    let mut int_bytes = [0u8; 8];
    // Top byte zeroed to keep value positive (sign bit clear).
    int_bytes[1..8].copy_from_slice(&bytes[9..16]);
    i64::from_be_bytes(int_bytes)
}
```

This is a lossy projection — it uses 56 bits of the ULID's random tail, so collisions are theoretically possible (birthday bound ≈ 2^28 ≈ 268M IDs for 50% probability; the `id.rs:28-34` doc-comment frames the same risk as "~1 in 10^4 for 10^6 IDs"). The `observations` insert path defends against this with bounded retry (`ID_COLLISION_RETRIES = 5`, `observations.rs:24`): in the new-row insert loop (`observations.rs:537-580`), on an `Err(rusqlite::Error::SqliteFailure(..))` whose `code` is `ConstraintViolation` and whose message contains `observations.int_id`, it `continue`s — regenerating a fresh `SeeleId` and recomputing `int_id` — and returns `StorageError::Conflict("exhausted 5 retries on int_id collision")` only after exhausting all five attempts. Any other error returns immediately.

**Caveat for cross-section coherence.** The migration's `int_id` column is the authoritative INTEGER mapping used by FTS5/vec0. There is *no* SQL virtual generated `int_id` column in the shipped schema — `int_id INTEGER NOT NULL UNIQUE` is a plain stored column populated from Rust, and the broken `SUBSTR(...)` virtual-column proposal was abandoned (the only `VIRTUAL` generated columns that ship are the five `meta_*` columns). This **contradicts `CLAUDE.md`**, whose "IDs" section still describes a SQL virtual `int_id` column as "aproximada" and `as_i64()` as using the "primeros 6 bytes" — both are stale relative to source. The source of truth is `SeeleId::as_i64()` taking bytes 9..16 (the last 7 bytes / 56 bits), and `int_id` is an exact stored value, not a virtual or approximate one.

#### FTS5 external-content table + sync triggers

```sql
-- crates/seele-storage/src/migrations/V001__initial_schema.sql:90-99
CREATE VIRTUAL TABLE observations_fts USING fts5(
    title, content, tool_name, type, project,
    content='observations',
    content_rowid='int_id',
    tokenize='porter unicode61 remove_diacritics 2'
);
```

This is an **external-content** FTS5 table: it stores only the inverted index, not copies of the text, and uses `int_id` as `content_rowid`. The tokenizer chain is `porter unicode61 remove_diacritics 2` — Porter stemming over Unicode-aware tokenization with full diacritic folding. Because external-content tables are not auto-maintained, three triggers keep the index consistent:

| Trigger | Fires | Action |
| --- | --- | --- |
| `observations_ai` | AFTER INSERT | inserts the new row's searchable columns at `rowid = new.int_id` |
| `observations_ad` | AFTER DELETE | issues the FTS5 `'delete'` command for `old.int_id` (removes index entries) |
| `observations_au` | AFTER UPDATE | a `'delete'` of the old image followed by a re-insert of the new image |

```sql
-- crates/seele-storage/src/migrations/V001__initial_schema.sql:107-117
CREATE TRIGGER observations_ad AFTER DELETE ON observations BEGIN
    INSERT INTO observations_fts(observations_fts, rowid, title, content, tool_name, type, project)
    VALUES('delete', old.int_id, old.title, old.content, old.tool_name, old.type, old.project);
END;
CREATE TRIGGER observations_au AFTER UPDATE ON observations BEGIN
    INSERT INTO observations_fts(observations_fts, rowid, ...) VALUES('delete', old.int_id, ...);
    INSERT INTO observations_fts(rowid, ...) VALUES (new.int_id, ...);
END;
```

(The `observations_ai` trigger at `V001:102-105` is the simple insert: `INSERT INTO observations_fts(rowid, title, content, tool_name, type, project) VALUES (new.int_id, ...)`.)

**Gotcha — soft delete does not de-index FTS.** The triggers fire on physical `DELETE`/`UPDATE`. SEELE's normal "delete" is a soft delete (setting `deleted_at`), which is an `UPDATE`; the `_au` trigger re-inserts the row into FTS unchanged, so soft-deleted observations *remain in the FTS index*. Live-only filtering must therefore be enforced at query time (joining back to `observations` on `deleted_at IS NULL`) — the FTS index alone is not a "live rows" view. This is consistent with `idx_obs_deleted_at` existing to scan soft-deleted rows separately.

**Gotcha — `prompts_fts` has no triggers.** `prompts_fts` is declared as an external-content FTS5 table over `user_prompts` (`V001:137-142`, columns `content`, `project`) but the migration creates **no `_ai`/`_ad`/`_au` triggers for it**, and it omits `content_rowid` (so it would default to the base table's rowid, which does not exist on a `WITHOUT ROWID` table). As written, `prompts_fts` is effectively inert — nothing populates it. This is a latent defect in the schema worth flagging for the improvement pass (Section 23).

#### vec0 virtual table

```sql
-- crates/seele-storage/src/migrations/V001__initial_schema.sql:120-122
CREATE VIRTUAL TABLE observations_vec USING vec0(
    embedding FLOAT[384]
);
```

A single 384-dimension `FLOAT[384]` column matching the all-MiniLM-L6-v2 output. The `vec0` virtual table is keyed by an INTEGER `rowid` set equal to `int_id`. Embeddings are written by `ObservationStore::set_embedding` (`observations.rs:318-338`), which looks up the row's `int_id` (only for an active, `deleted_at IS NULL` row — otherwise `StorageError::NotFound`), packs the `&[f32]` into little-endian bytes, and runs `INSERT OR REPLACE INTO observations_vec(rowid, embedding)`. `hard_delete` clears the vec row too, explicitly to avoid a future `int_id` collision surfacing a stale embedding (`observations.rs:305-311`); a standalone `delete_embedding` (`observations.rs:340-349`) does the same on demand. KNN queries against this table are the vector half of hybrid search (Section 7).

### 4.7 Connection pool and PRAGMAs (`pool.rs`)

The pool type is a thin alias: `pub type Pool = r2d2::Pool<SqliteConnectionManager>` (`pool.rs:10`). `PoolConfig` carries the DB `path` and `max_size`; `PoolConfig::with_path` defaults `max_size` to **8** connections (`pool.rs:18-25`).

`init_pool` builds the manager with an init closure run on **every checked-out connection** (since each pooled SQLite connection is independent and PRAGMAs/loaded extensions are per-connection):

```rust
// crates/seele-storage/src/pool.rs:30-48
pub fn init_pool(cfg: PoolConfig) -> Result<Pool> {
    let vec0_path = vec0_install::ensure_vec0_extension()?;
    let manager = SqliteConnectionManager::file(&cfg.path).with_init(move |conn| {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             PRAGMA temp_store=MEMORY;",
        )?;
        load_vec0(conn, &vec0_path)
    });
    let pool = r2d2::Pool::builder().max_size(cfg.max_size).build(manager)
        .map_err(StorageError::Pool)?;
    Ok(pool)
}
```

PRAGMA rationale:

| PRAGMA | Value | Why |
| --- | --- | --- |
| `journal_mode` | `WAL` | Write-Ahead Logging lets readers proceed concurrently with a writer (multiple readers + one writer), critical because several transports (CLI/HTTP/MCP) and the pool's up-to-8 connections may touch the DB simultaneously. WAL is also faster for the small frequent writes SEELE does. (`journal_mode` is persistent per-DB, but is re-asserted on every connection harmlessly.) |
| `synchronous` | `NORMAL` | The recommended pairing with WAL: durable enough (no corruption on app crash) while avoiding an `fsync` on every commit, trading a small window of last-transaction loss on OS/power failure for throughput. |
| `foreign_keys` | `ON` | SQLite defaults FK enforcement *off* per-connection; SEELE relies on `ON DELETE CASCADE`/`SET NULL` (links, relations, sessions↔observations), so it must be explicitly enabled on every connection. |
| `temp_store` | `MEMORY` | Temp tables / sort spills go to RAM, speeding `ORDER BY` and FTS auxiliary work. |

Crucially, `ensure_vec0_extension()` is called **once** at pool construction (resolving/writing the binary to disk), but `load_vec0` runs **per connection** (inside the `with_init` closure) so every connection in the pool has the `vec0` functions and virtual-table module available. The init closure makes vec0 loading a hard requirement — a connection that fails to load the extension fails to enter the pool (doc comment `pool.rs:27-29`: "Failure to load the extension is fatal — SEELE depends on hybrid search"). A connection-level test, `pool_loads_vec0_and_pragmas` (`pool.rs:75-98`, cfg-gated to the five supported targets), asserts `vec_version()` responds and that `journal_mode`/`foreign_keys` are set.

`load_vec0` itself:

```rust
// crates/seele-storage/src/pool.rs:50-61
fn load_vec0(conn: &rusqlite::Connection, path: &Path) -> rusqlite::Result<()> {
    unsafe {
        let _guard = rusqlite::LoadExtensionGuard::new(conn)?;
        // Exported init function is `sqlite3_vec_init` (no `0` suffix).
        conn.load_extension(path, Some("sqlite3_vec_init"))?;
    }
    Ok(())
}
```

Two correctness points: (1) `LoadExtensionGuard` temporarily enables extension loading and disables it again when dropped (a security default, since arbitrary native code can be loaded — hence the `unsafe` block). (2) The entry-point symbol is passed **explicitly** as `Some("sqlite3_vec_init")`; the loadable *file* is named `vec0` but its exported init function has no `0` suffix, so passing `None` (which would derive the symbol from the filename → `sqlite3_vec0_init`) would fail to find the entry point.

### 4.8 WAL, dedup, and soft-delete invariants (summary)

- **WAL** is the journaling mode; combined with `synchronous=NORMAL` it provides reader/writer concurrency suitable for the multi-transport single-binary design, at the cost of WAL/SHM sidecar files (`-wal`, `-shm`) next to the DB.
- **Dedup** hinges on `normalized_hash` (computed in `hash.rs` / `privacy.rs`, Section 5), the partial index `idx_obs_dedup`, and the `DEDUP_WINDOW_MS = 24h` window (`observations.rs:22`, `24 * 60 * 60 * 1000`). Within the window, a re-save of the same normalized content under the same `(project, scope, type, title)` increments `duplicate_count` rather than inserting a new row.
- **Soft delete** is the `deleted_at` timestamp column. NULL = alive. Every live-query partial index carries `WHERE deleted_at IS NULL`; the complementary `idx_obs_deleted_at` serves trash listing/restore. As noted in §4.6, soft delete does not remove a row from FTS, so query-time filtering is mandatory.

### 4.9 Vendored `vec0` loading (build-time embed + runtime install)

SEELE ships the `sqlite-vec` loadable extension *inside the binary* rather than depending on a runtime download or a build script. This is split across two files.

**Build time — `vec0_loader.rs`.** Per host target, a `cfg`-gated `const VEC0_BYTES: Option<&[u8]>` is set with `include_bytes!` pointing at the matching vendored file. The five supported targets and their filenames:

| `cfg` target | Embedded file |
| --- | --- |
| linux + x86_64 | `vendor/sqlite-vec/linux-x86_64/vec0.so` |
| linux + aarch64 | `vendor/sqlite-vec/linux-aarch64/vec0.so` |
| macos + x86_64 | `vendor/sqlite-vec/macos-x86_64/vec0.dylib` |
| macos + aarch64 | `vendor/sqlite-vec/macos-aarch64/vec0.dylib` |
| windows + x86_64 | `vendor/sqlite-vec/windows-x86_64/vec0.dll` |

Any other target (`#[cfg(not(any(...)))]`) sets `VEC0_BYTES = None`. `vec0_bytes() -> Option<&'static [u8]>` returns those bytes; `vec0_extension_suffix() -> &'static str` returns `.so` / `.dylib` / `.dll` (empty string on unsupported OS) so the on-disk file gets the suffix the OS loader (`dlopen`/`LoadLibrary`) needs — SQLite itself infers nothing from the filename. Two tests enforce the contract: a soft size sanity check (`vec0_bytes_size_sanity`, `> 1024` bytes, ungated), and a hard `vec0_bytes_required_on_supported_targets` (cfg-gated to the five targets) that asserts `Some` and `> 100_000` bytes so a future bump that breaks all `include_bytes!` cannot pass silently (`vec0_loader.rs:89-104`). The five vendored binaries on disk total ~874 KB and range from ~128 KB (macos-x86_64) to ~289 KB (windows-x86_64), comfortably above both thresholds.

**Runtime — `vec0_install.rs`.** `ensure_vec0_extension() -> Result<PathBuf>` resolves a usable file on disk with this precedence:

1. **`$SEELE_VEC_PATH`** override (`VEC0_ENV_OVERRIDE`) — if set and pointing to an existing file (`p.is_file()`), use it verbatim (escape hatch for unsupported targets / debugging). If set but missing, return `StorageError::InvalidInput`.
2. **Vendored bytes** → write to `<user-cache>/seele/vec0-<sha><suffix>`. The SHA-256 prefix (first 16 hex chars, `SHA_PREFIX_LEN = 16`) of the embedded bytes is part of the filename, so a content change produces a new path and old cached copies are never silently reused. If the target file already exists, `verify_integrity` re-hashes it and returns it (or errors `Vec0IntegrityMismatch` on tamper); otherwise the bytes are written via `write_atomic`.
3. **No vendored bytes and no override** → `StorageError::Vec0NotSupportedTarget { os, arch }` (populated from `std::env::consts::OS` / `ARCH`).

The cache dir comes from `dirs::cache_dir()` joined with `seele`; if `cache_dir()` returns `None`, `StorageError::Vec0CacheUnresolvable`. `write_atomic` (`vec0_install.rs:72-100`) writes to a per-call unique temp name (`<name>.<pid>.<counter>.tmp`, with a process-static `AtomicU64` counter incremented `Relaxed`) then `fs::rename`s onto the target — atomic on POSIX. It explicitly handles the Windows case where rename fails if the target exists by treating "rename errored but target is now a file" as a benign post-race success and removing the temp, deferring to `verify_integrity` to catch genuine corruption. This makes concurrent installers (e.g. parallel test threads) safe.

**Vendoring strategy and provenance.** `vendor/sqlite-vec/README.md` documents that the binaries are upstream `sqlite-vec` **v0.1.9** (dual-licensed Apache-2.0 OR MIT, © 2024 Alex Garcia), redistributed with both license texts (`LICENSE-APACHE-upstream.txt`, `LICENSE-MIT-upstream.txt`) and the upstream `checksums.txt` (`CHECKSUMS-upstream.txt`) for manual re-verification. The README's rationale (mirroring ADR-11): a `build.rs` download would force network access at build time and break offline `cargo install`, and consolidated Rust bindings did not exist at genesis, so vendoring the loadable gives deterministic control of the exact binary in the final executable. The README states the accepted cost as ~880 KB across the five files (measured ~874 KB on disk). A six-step bump procedure is documented (bump README + CHANGELOG, download all `loadable-{target}.tar.gz`, extract each `vec0.{so,dylib,dll}`, replace `CHECKSUMS-upstream.txt`, run `cargo test -p seele-storage`, commit `vendor: bump sqlite-vec to vX.Y.Z`). CLAUDE.md reinforces that the vendored binaries must not be touched without updating this README.

### 4.10 Error model (`error.rs`)

`StorageError` is the crate's single error type; `pub type Result<T> = std::result::Result<T, StorageError>`. Variants:

| Variant | Source / trigger | Notes |
| --- | --- | --- |
| `Sqlite(rusqlite::Error)` | `#[from]` | Any SQLite-level failure |
| `Pool(r2d2::Error)` | `#[from]` (also mapped explicitly via `.map_err(StorageError::Pool)` in `init_pool`) | Pool build/checkout failure |
| `Migration(refinery::Error)` | `#[from]` | Migration apply failure |
| `Io(std::io::Error)` | `#[from]` | File I/O during vec0 install |
| `Vec0NotSupportedTarget { os, arch }` | `ensure_vec0_extension` | No vendored binary + no override (`os`/`arch` are `&'static str`) |
| `Vec0CacheUnresolvable` | `resolve_cache_dir` | `dirs::cache_dir()` returned `None` |
| `Vec0IntegrityMismatch { path, expected, actual }` | `verify_integrity` | Cached binary SHA prefix mismatch |
| `NotFound(String)` | stores | e.g. missing/inactive observation on `set_embedding`, soft-delete, hard-delete |
| `InvalidInput(String)` | stores / install | bad ULID, bad `$SEELE_VEC_PATH`, non-serializable metadata |
| `Conflict(String)` | observations insert | exhausted `int_id` collision retries |

Failure modes worth highlighting: pool init is **all-or-nothing** — if `ensure_vec0_extension` fails (unsupported target, unresolvable cache, integrity mismatch) the pool is never built, so the whole storage layer refuses to come up rather than silently degrading without vector search. There is no fallback to a non-vec0 mode at this layer (contrast the embedder, which per CLAUDE.md falls back transparently to a fake embedder with a warning when ONNX init fails).

### 4.11 Open/init flow and store wiring (`lib.rs`)

The single entry point combines all of the above:

```rust
// crates/seele-storage/src/lib.rs:34-39
pub fn init_db(path: impl AsRef<Path>) -> Result<Pool> {
    let pool = init_pool(PoolConfig::with_path(path))?;
    let mut conn = pool.get()?;
    migrations::run_pending(&mut conn)?;
    Ok(pool)
}
```

Order is significant: (1) `init_pool` resolves+caches the `vec0` binary and builds the pool, so the *first* connection checkout already has PRAGMAs set and `vec0` loaded — necessary because the migration creates a `vec0` virtual table (`CREATE VIRTUAL TABLE observations_vec USING vec0(...)`), which would fail if the extension were not loaded first. (2) A connection is checked out and `run_pending` applies any unapplied migrations (idempotent). (3) The `Pool` is returned. Every downstream store (`ObservationStore`, `SessionStore`, `LinkStore`, `RelationStore`, `PromptStore`, `ChunkStore` — the store types and their input/query types are re-exported from `lib.rs:17-25`) is constructed from a clone of this `Pool` and checks out connections as needed. The doc comment summarizes it: "Open a DB at `path`, applying canonical PRAGMAs, loading vec0, and running pending migrations. Idempotent." (`lib.rs:32-33`). This is the function every transport's bootstrap path ultimately calls to obtain a ready database.


---

## 5. Storage Layer II — Stores, Dedup, Privacy, Links, Relations & Chunks

This section documents the behavioral layer that sits on top of the schema, pool, and vec0 loading covered in Section 4. Where Section 4 explains *how the database exists*, this section explains *what the code does with it*: the CRUD stores (`ObservationStore`, `PromptStore`, `SessionStore`, `RelationStore`, `LinkStore`, `ChunkStore`), the dedup and privacy machinery (`hash.rs`, `privacy.rs`), the transaction helpers that let `seele-sync` and `seele-engram-import` drive multiple stores atomically, the vec0 embedding write path keyed by `SeeleId::as_i64()`, and the soft-delete / restore lifecycle. All of these live in the `seele-storage` crate, which depends on `seele-core` (domain types) plus the external crates `rusqlite`, `chrono`, `serde`/`serde_json`, `sha2`, `hex`, `regex`, `once_cell`, `r2d2`, `refinery`, and `thiserror`.

### 5.1 Crate position, module map, and public surface

`seele-storage` is the second layer from the bottom of the crate graph: it depends on `seele-core` and is depended on by `seele-search`, `seele-sync`, `seele-engram-import`, `seele-http`, `seele-mcp` (transitively), `seele-tui`, and `seele-cli`. Every higher-level service reaches the database exclusively through the store types defined here; no caller issues raw SQL except inside tests.

`crates/seele-storage/src/lib.rs` declares the module tree and the curated re-export surface. The modules are declared one per line (`lib.rs:3-15`):

```rust
// crates/seele-storage/src/lib.rs:3
pub mod chunks;       pub mod error;   pub mod hash;     pub mod links;
pub mod migrations;   pub mod observations; pub mod pool; pub mod privacy;
pub mod prompts;      pub mod relations; pub mod sessions;
pub mod vec0_install; pub mod vec0_loader;
```

The flat `pub use` block (`lib.rs:17-28`) hoists the store types and their input/query DTOs (`ChunkStore`, `SyncChunk`; `LinkInput`, `LinkQuery`, `LinkStore`; `ObservationPatch`, `ObservationQuery`, `ObservationStore`, `RawSaveInput`, `RawSaveOutcome`, `SaveInput`, `SaveOutcome`; `PromptInput`, `PromptQuery`, `PromptStore`, `UserPrompt`; `JudgmentInput`, `RelationInput`, `RelationQuery`, `RelationStore`; `SessionFilter`, `SessionInput`, `SessionStore`), plus `Result`, `StorageError`, and the pool primitives (`init_pool`, `Pool`, `PoolConfig`). The crate also exposes one free function that ties pool + migrations together:

```rust
// crates/seele-storage/src/lib.rs:34
pub fn init_db(path: impl AsRef<Path>) -> Result<Pool> {
    let pool = init_pool(PoolConfig::with_path(path))?;
    let mut conn = pool.get()?;
    migrations::run_pending(&mut conn)?;
    Ok(pool)
}
```

Every store is a thin `#[derive(Clone)]` wrapper around a single `pool: Pool` field and is constructed with `new(pool: Pool)`. They share no state beyond the pool, so cloning a store is cheap (it clones an `r2d2` handle), and the same `Pool` is handed to several stores at once by the service layer.

#### Error model

`crates/seele-storage/src/error.rs` defines `StorageError` (a `thiserror` enum) and `pub type Result<T> = std::result::Result<T, StorageError>`. The variants are: `Sqlite(#[from] rusqlite::Error)`, `Pool(#[from] r2d2::Error)`, `Migration(#[from] refinery::Error)`, `Io(#[from] std::io::Error)`, the three vec0 variants (`Vec0NotSupportedTarget { os, arch }`, `Vec0CacheUnresolvable`, `Vec0IntegrityMismatch { path, expected, actual }`), and three domain-shaped variants used heavily by the stores: `NotFound(String)`, `InvalidInput(String)`, and `Conflict(String)`. The store code maps `rusqlite::Error::QueryReturnedNoRows` to either `Ok(None)` (for `get`) or to `NotFound` (for mutating ops where the row was expected), turns serde failures into `InvalidInput`, and reserves `Conflict` for the int_id-collision retry exhaustion path.

### 5.2 Privacy stripping (`privacy.rs`)

Privacy stripping is the first transform applied on the write path. `crates/seele-storage/src/privacy.rs` compiles a single lazy global regex and exposes one function:

```rust
// crates/seele-storage/src/privacy.rs:11
static PRIVATE_TAG_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?si)<private>.*?</private>").expect("private tag regex must compile")
});

pub fn strip_private_tags(s: &str) -> String {
    PRIVATE_TAG_RE.replace_all(s, "").to_string()
}
```

The flags matter: `(?s)` makes `.` match newlines (blocks may span lines), `(?i)` makes the tag names case-insensitive (`<PRIVATE>` is stripped too), and the `.*?` is **lazy** — it stops at the first `</private>`. Documented consequences, all backed by unit tests in the same file (`privacy.rs:22-75`):

- **Unclosed tags are left intact** (`<private>oops never closed` is returned verbatim; test `unclosed_tag_left_intact`, `privacy.rs:49`). The module comment states this is deliberate: "Malformed blocks (no closing tag) are left untouched on purpose — silent … stripping would surprise the caller" (`privacy.rs:4-6`).
- **Nested tags**: `<private>outer<private>inner</private>tail</private>` strips only up to the first close, leaving `tail</private>` (test `nested_inner_tag_treated_as_text_until_first_close`, `privacy.rs:68`). This is a real gotcha — nested private blocks do not fully strip.
- **Multiple blocks** all strip (test `multiple_blocks_all_stripped`, `privacy.rs:62`); the text between/around them is preserved with the surrounding whitespace untouched (so `Public <private>secret</private> rest.` becomes `Public  rest.` with a double space — test `strips_single_line_block`, `privacy.rs:27`).

Per the repo conventions (CLAUDE.md "Privacy" section), stripping is applied to **both `title` and `content`** in `ObservationStore::save()`, and CLAUDE.md also lists `PromptStore::save()` as a strip site. **Note on the actual code:** the strip for prompts is *not* implemented inside `PromptStore::save` (`prompts.rs:44` writes `input.content` verbatim) — so in the current source the prompt strip is the caller/service-layer's responsibility, not the store's; treat the CLAUDE.md line as an intent/contract that the `prompts.rs` store does not itself enforce. For observations, stripping happens **before** `normalized_hash` and **before** the row is written, so the FTS5 index (populated by the V001 triggers off the stored `content`/`title`) never sees private text. The property test `strip_private_tags_is_idempotent` (`tests/properties.rs:69`) confirms applying twice equals once and that no tag literals survive.

### 5.3 Normalized hashing for dedup (`hash.rs`)

`crates/seele-storage/src/hash.rs` computes the dedup fingerprint over *content only* (not title):

```rust
// crates/seele-storage/src/hash.rs:17
pub fn normalized_hash(content: &str) -> String {
    let normalized = normalize(content);
    let digest = Sha256::digest(normalized.as_bytes());
    hex::encode(digest)
}

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
```

The exact algorithm, in order: (1) lowercase the whole string (Unicode-aware via `str::to_lowercase`), (2) `split_whitespace()` which both collapses any run of whitespace and drops leading/trailing whitespace, (3) re-join tokens with a single ASCII space, (4) SHA-256 the UTF-8 bytes, (5) lowercase hex-encode (always 64 chars). The module comment frames the intent: "The transform is intentionally aggressive: small reformat-only edits to the same content should hash identically and be merged as duplicates" (`hash.rs:15`). Note that `to_lowercase()` is full Unicode case-folding, while the property test `normalized_hash_is_ascii_case_insensitive` (`tests/properties.rs:54`) only asserts ASCII behavior — non-ASCII case behavior follows whatever Rust's `to_lowercase` does and is untested. The hash is a *component* of the dedup key, not the whole key; the time window, project, scope, type, and title comparisons live in the upsert layer (see §5.4), exactly as the module comment notes: "the time/scope check belongs to the upsert layer" (`hash.rs:5`).

### 5.4 `ObservationStore` — the core write path (`observations.rs`)

`ObservationStore` is the heart of SEELE's storage. Its module doc summarizes the `save()` pipeline: privacy strip → normalized hash → topic_key upsert → 24h normalized-hash dedup → new-row insert, with FTS5 kept in sync automatically by the V001 triggers (`observations_ai/ad/au`).

#### DTOs and outcome types

| Type | Role | Notable fields |
|------|------|----------------|
| `SaveInput` | Normal save request | `session_id: Option<SeeleId>`, `kind: ObservationType`, `title`, `content`, `tool_name: Option<String>`, `project: Option<String>`, `scope: Scope`, `topic_key: Option<String>`, `metadata: Metadata` |
| `RawSaveInput` | Migration/bulk insert; caller owns `id` + timestamps | same shape as `SaveInput` plus `id: SeeleId`, `created_at: DateTime<Utc>`, `updated_at: DateTime<Utc>`; no dedup/strip applied (no separate `last_seen_at` field — `save_raw_in_tx` reuses `updated_at` for `last_seen_at`) |
| `SaveOutcome` | Result of `save` | `Created(SeeleId)` / `UpsertedTopic { id, revision_count: u32 }` / `DuplicateMerged { id, duplicate_count: u32 }`; `.id()` extracts the id from any variant |
| `RawSaveOutcome` | Result of `save_raw_in_tx` | `Inserted` / `AlreadyExisted` (a `Copy` enum) |
| `ObservationQuery` | `list` filter (all `Option` except the flag) | `project`, `scope`, `kind`, `session_id`, `topic_key`, `include_deleted: bool`, `limit: Option<u32>` |
| `ObservationPatch` | `update` patch | all fields `Option`; `tool_name`/`topic_key` are `Option<Option<String>>` to distinguish "leave alone" from "set to NULL" |

Two module constants govern dedup and id generation:

```rust
// crates/seele-storage/src/observations.rs:19
const DEDUP_WINDOW_MS: i64 = 24 * 60 * 60 * 1000;   // 24h, inherited from ENGRAM
const ID_COLLISION_RETRIES: usize = 5;              // int_id tail collisions (bytes 9..16 of ULID)
```

(The `DEDUP_WINDOW_MS` declaration is at `observations.rs:22` with its doc comment beginning at line 19; `ID_COLLISION_RETRIES` at line 24.)

#### `save()` step by step

`save()` checks out a connection, opens a transaction, calls the free function `save_in_tx`, commits, and returns the outcome (`observations.rs:119`). All the logic is in `save_in_tx` (`observations.rs:439`). The control flow, in order:

**Step 0 — Normalize inputs.** Strip private tags from both title and content, hash the *stripped content*, snapshot `now_ms = Utc::now().timestamp_millis()`, and serialize `metadata` to JSON (a serde failure becomes `StorageError::InvalidInput`).

```rust
// crates/seele-storage/src/observations.rs:440
let stripped_title = strip_private_tags(&input.title);
let stripped_content = strip_private_tags(&input.content);
let hash = normalized_hash(&stripped_content);
```

**Step 1 — Topic-key upsert.** If `input.topic_key` is `Some`, query for an existing *active* row matching `WHERE project IS ?1 AND scope = ?2 AND topic_key = ?3 AND deleted_at IS NULL LIMIT 1` (`observations.rs:451-454`). The `project IS ?1` operator is null-safe, so two rows with `project = NULL` and the same topic_key collide correctly. If a row is found, it is **updated in place** — title, content, hash, `revision_count = old + 1`, `last_seen_at`/`updated_at = now`, metadata, type, and tool_name are all overwritten (`observations.rs:466-470`) — and the function returns `UpsertedTopic { id, revision_count: new_rev }`. This is the mechanism behind the documented invariant "same `(project, scope, topic_key)` active → update + revision_count++" (CLAUDE.md). The integration test `topic_key_upsert_increments_revision` (`observations_crud.rs:68`) confirms the second save returns `revision_count == 1` and the row reflects the latest title/content. The test `topic_key_isolated_per_scope_and_project` (`observations_crud.rs:120`) confirms the key is scoped per `(project, scope)`: the same `topic_key` under different projects, or under `Project` vs `Personal` scope, yields distinct rows. The schema backs this path with the partial index `idx_obs_topic_upsert ON observations(project, scope, topic_key) WHERE topic_key IS NOT NULL AND deleted_at IS NULL` (V001 schema).

**Step 2 — Normalized-hash dedup window.** If no topic_key matched (or none was supplied), look for a recent duplicate. The query is the full ENGRAM dedup key: `normalized_hash = ?` AND null-safe project equality AND `scope = ?` AND `type = ?` AND `title = ?` (the *stripped* title) AND `last_seen_at >= now - DEDUP_WINDOW_MS` AND `deleted_at IS NULL`.

```rust
// crates/seele-storage/src/observations.rs:494
let dedup_threshold = now_ms - DEDUP_WINDOW_MS;
// SELECT id, duplicate_count FROM observations
//  WHERE normalized_hash = ?1
//    AND ((project IS NULL AND ?2 IS NULL) OR project = ?2)
//    AND scope = ?3 AND type = ?4 AND title = ?5
//    AND last_seen_at >= ?6 AND deleted_at IS NULL LIMIT 1
```

On a hit the existing row's `duplicate_count` is incremented and `last_seen_at` is bumped to `now` (`observations.rs:523-527`), which slides the dedup window forward, and the function returns `DuplicateMerged { id, duplicate_count }`. The test `dedup_hash_window_increments_duplicate_count` (`observations_crud.rs:95`) shows that `"same content"` and `"  SAME content  "` collapse (because the normalized hash ignores case and whitespace) and report `duplicate_count == 1`. The test `different_title_with_same_content_is_new_row` (`observations_crud.rs:111`) shows that title is part of the key — same body, different title → new row. Two gotchas to flag for the improvement section: title is matched by **exact stored (stripped) string**, not by normalized hash, so case/whitespace differences in the title defeat dedup. As for indexing, **this path is index-assisted**: the schema declares the composite partial index `idx_obs_dedup ON observations(normalized_hash, project, scope, type, title) WHERE deleted_at IS NULL` (V001 schema, lines 72-73), which covers five of the six predicates in column order; `last_seen_at` is the only predicate not in the index, so it is applied as a residual filter after the index seek. (This corrects an earlier draft claim that "no composite index" exists — it does.)

**Step 3 — Insert new row with collision retry.** If neither step matched, mint a fresh `SeeleId::new()`, derive `int_id = id.as_i64()` (bytes `9..16` of the ULID — the last 7 bytes / low 56 bits, matching the `ID_COLLISION_RETRIES` comment above), and `INSERT` the full row with `revision_count = 0`, `duplicate_count = 0`, and `created_at = updated_at = last_seen_at = now` (the SQL binds the single `?12` placeholder to `now_ms` for all three timestamp columns):

```rust
// crates/seele-storage/src/observations.rs:538
for _ in 0..ID_COLLISION_RETRIES {
    let id = SeeleId::new();
    let int_id = id.as_i64();
    let res = tx.execute("INSERT INTO observations(... int_id ...) VALUES (...)", params![...]);
    match res {
        Ok(_) => return Ok(SaveOutcome::Created(id)),
        Err(rusqlite::Error::SqliteFailure(e, msg))
            if matches!(e.code, rusqlite::ErrorCode::ConstraintViolation)
                && msg.as_deref().is_some_and(|m| m.contains("observations.int_id")) =>
            { continue; }   // collision on the random tail mapping; regenerate
        Err(other) => return Err(StorageError::from(other)),
    }
}
Err(StorageError::Conflict(format!("exhausted {ID_COLLISION_RETRIES} retries on int_id collision")))
```

The retry exists because `int_id` is declared `INTEGER NOT NULL UNIQUE` in the schema (V001 `observations.int_id`) and is only a 6-byte projection of the 16-byte ULID, so collisions are possible (astronomically unlikely but handled). Only an `int_id` UNIQUE violation is retried (the error message must contain `"observations.int_id"`); any other constraint or error propagates immediately. After `ID_COLLISION_RETRIES` (5) failures it returns `Conflict`. Note: an FTS5 row is *not* written by `save_in_tx` directly — the `observations_ai` AFTER INSERT trigger from V001 does it, inserting `INTO observations_fts(rowid, title, content, tool_name, type, project) VALUES (new.int_id, new.title, …)`. The `observations_fts` virtual table declares the indexed columns `title, content, tool_name, type, project` and uses `content='observations'` with `content_rowid='int_id'`, so `int_id` is the FTS rowid rather than an FTS column.

#### Embedding / vec0 write path

The vector index is **not** written inside `save`. Embedding generation happens at the service layer and is pushed in via a separate call. In `crates/seele-http/src/service.rs`, `save_observation` (`service.rs:73`) saves the observation (`service.rs:94`), then calls `self.embedder.embed(&req.content)` and, on success, `self.observations.set_embedding(outcome.id(), &v)` as a *best-effort* follow-up — failures are logged (`tracing::warn!`) and do **not** surface as a 5xx; the comment states "the row is persisted and a reindex pass can fix it" (`service.rs:96-97`). An embedder failure is likewise swallowed with a warning (`service.rs:104-106`).

`set_embedding` is where the `SeeleId` → vec0 rowid bridge is realized:

```rust
// crates/seele-storage/src/observations.rs:318
pub fn set_embedding(&self, id: SeeleId, embedding: &[f32]) -> Result<()> {
    let conn = self.pool.get()?;
    let int_id: i64 = conn.query_row(
        "SELECT int_id FROM observations WHERE id = ?1 AND deleted_at IS NULL",
        [id.to_string()], |r| r.get(0),
    ).map_err(/* NoRows -> NotFound */)?;
    let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
    conn.execute("INSERT OR REPLACE INTO observations_vec(rowid, embedding) VALUES (?1, ?2)",
                 params![int_id, bytes])?;
    Ok(())
}
```

Two things worth emphasizing. First, the rowid used for the vec0 row is the **stored `int_id` column**, looked up by the textual ULID (and only for active rows — the lookup filters `deleted_at IS NULL`, so calling `set_embedding` on a soft-deleted observation yields `NotFound`). The schema's `int_id` was itself populated from `SeeleId::as_i64()` at insert time, so they agree, but the authoritative mapping the system relies on is `as_i64()`; per the global invariant the SQL `int_id` column is "an approximation used only for JOIN queries". Second, the vector is serialized as a little-endian `f32` byte blob; the doc comment notes the caller owns normalization and that the length must equal the column dim (currently 384, matching the V001 `observations_vec` declaration `embedding FLOAT[384]`). The companion `delete_embedding` (`observations.rs:341`) deletes the vec0 row by sub-selecting `int_id` for the ULID and is a no-op if absent. `hard_delete` (`observations.rs:299`) deletes the observation row first and then *also* clears the vec0 row "so a future ID collision doesn't surface a stale embedding" (`observations.rs:305-306`) — that secondary delete is best-effort (`let _ = …`).

#### Read, update, and lifecycle

`get(id)` runs `SQL_SELECT_BY_ID` (`observations.rs:627`) and maps `QueryReturnedNoRows` → `Ok(None)`. Note `get` does **not** filter `deleted_at`, so it returns soft-deleted rows (the test `soft_delete_then_restore_round_trip`, `observations_crud.rs:176`, relies on this to inspect `deleted_at`). `list(ObservationQuery)` builds SQL dynamically: it always starts from `SQL_SELECT_PREFIX` plus `WHERE 1=1`, appends `AND deleted_at IS NULL` unless `include_deleted` is set, then optional `project`/`scope`/`kind`/`session_id`/`topic_key` predicates, orders `created_at DESC`, and applies an optional `LIMIT`. Parameters are bound via boxed `dyn ToSql` and `params_from_iter`.

`update(id, ObservationPatch)` is transactional: it reads the current title/content (active rows only, via `WHERE id=?1 AND deleted_at IS NULL`), errors `NotFound` if missing, applies the patch (falling back to current values), **re-strips the resulting content** and **recomputes `normalized_hash`** (`observations.rs:226-227`), and always bumps `updated_at` and `last_seen_at`. Note the patch path strips and re-hashes *content* but does **not** re-strip the patched *title* before storing it. The test `update_recomputes_normalized_hash` (`observations_crud.rs:196`) confirms the hash changes after a content edit. The `Option<Option<String>>` shape of `tool_name`/`topic_key` is what allows explicitly nulling those columns.

Soft-delete / restore are mirror operations and both are guarded by status predicates so they are idempotency-safe:

```rust
// crates/seele-storage/src/observations.rs:267
// soft_delete: WHERE id = ?2 AND deleted_at IS NULL     -> 0 rows => NotFound
// restore:     WHERE id = ?2 AND deleted_at IS NOT NULL -> 0 rows => NotFound
```

`soft_delete` sets `deleted_at` and `updated_at` to now; `restore` clears `deleted_at` and bumps `updated_at`. Because they check the prior state in the WHERE clause, double-delete or restoring a live row both yield `NotFound` rather than silently succeeding. `hard_delete` physically removes the row (and its vec0 entry). The FTS5 `observations_ad` AFTER DELETE trigger only fires on `hard_delete`; **soft-deleted rows remain in the FTS index** — search must therefore re-check `deleted_at` itself (this is a cross-cutting fact relevant to Section 7).

#### Stats helpers

`ObservationStore` also exposes the aggregation queries consumed by `seele stats` / the doctor / HTTP stats: `count_active`, `count_deleted`, `count_by_type` (GROUP BY `type`, `(String,u64)` pairs ordered `n DESC, type ASC`), `count_by_scope` (ordered `n DESC, scope ASC`), `count_projects` (distinct non-null), and `list_projects` (distinct non-null, alphabetical `ASC`). All filter on `deleted_at IS NULL` except `count_deleted` (which selects `deleted_at IS NOT NULL`), and all clamp negative counts via `.max(0)`.

#### Transaction helpers and the raw insert path

Three associated functions exist specifically to let other crates compose multiple writes into one atomic boundary:

- `ObservationStore::save_in_tx(tx, input)` (`observations.rs:132`) — the same logic as `save` but on a caller-owned transaction; the caller does the commit/rollback. It delegates to the free function `save_in_tx` (`observations.rs:439`).
- `ObservationStore::pool()` (`observations.rs:138`) — borrows the underlying `&Pool` so a caller (notably `seele-sync`) can open one connection and drive several stores across it in a single transaction.
- `ObservationStore::save_raw_in_tx(tx, RawSaveInput)` (`observations.rs:146`) — wraps the free function `save_raw_in_tx` (`observations.rs:582`), an `INSERT OR IGNORE` that **bypasses privacy strip, topic-key upsert, and the dedup window** (it does still compute `normalized_hash` from the raw content for storage). The caller supplies the `id` and timestamps; idempotency comes from `INSERT OR IGNORE` on the `(id)` primary key. It returns `Inserted` (rowcount 1) or `AlreadyExisted` (rowcount 0).

`save_raw_in_tx` is the path used by both bulk importers, and the *reason* it exists rather than reusing `save_in_tx` is documented as a Cloven critical finding (the 2026-05-11 sync save-path finding). `seele-sync::import_from_file` (`crates/seele-sync/src/lib.rs:281`) opens one transaction via `observations.pool()`, calls `save_raw_in_tx` for every observation, then `ChunkStore::mark_imported_in_tx`, then commits once — so a crash mid-import rolls the entire chunk back and leaves the destination DB and ledger untouched. The doc comment explains the IDs-preserved rationale: source ULIDs ride through unchanged, so the same observation on two machines collapses on the shared id, and a re-import "does not overwrite a destination `topic_key` collision (which the legacy `save_in_tx` path would silently UPDATE, causing data loss)" (`sync/lib.rs:264-267`). The sync path also deliberately drops `session_id` (set to `None` at `sync/lib.rs:317`) to avoid tripping the `observations.session_id` foreign key (`REFERENCES sessions(id) ON DELETE SET NULL`) when the referenced session is unknown to the destination. `seele-engram-import` (`engram-import/src/lib.rs:151`) uses the same `save_raw_in_tx` plus `LinkStore::create_in_tx` to commit imported observations and their derived `links` atomically; before each link insert it probes with `link_exists_in_tx` (`engram-import/src/lib.rs:203`) to avoid hitting the `links` UNIQUE constraint and aborting the whole transaction.

#### Row parsing

`parse_observation` (`observations.rs:634`) maps an 18-column row into a `seele_core::memory::Observation`, converting epoch-ms integers to `DateTime<Utc>` via `ms_to_dt` (`observations.rs:691`, which falls back to `Utc::now()` if the timestamp is somehow out of range — a silent fallback worth noting). It parses the ULID text columns into `SeeleId` (errors → `InvalidInput`), uses `ObservationType::from_str_relaxed` (lenient) for the `type` column but `Scope::from_str_strict` (strict, errors on unknown) for `scope`, and `serde_json::from_str` for metadata. Note that `normalized_hash` is read as `Option<String>` (the column is nullable in the schema).

### 5.5 `PromptStore` (`prompts.rs`)

`PromptStore` persists short records of user input into the `user_prompts` table, which has its own FTS index (`prompts_fts`, over columns `content` and `project`) maintained by triggers. Its domain struct `UserPrompt { id: SeeleId, session_id: Option<SeeleId>, content: String, project: Option<String>, created_at: DateTime<Utc> }` is `Serialize + Deserialize` (it crosses the HTTP boundary). The API is small: `save(PromptInput) -> UserPrompt` (mints a `SeeleId`, stores `created_at` as epoch-ms, returns the full struct), `get(id) -> Option<UserPrompt>`, `list(PromptQuery)` (filter by `project`/`session_id`, order `created_at DESC`, optional `limit`), and `delete(id)` (hard delete; `NotFound` if the row didn't exist). **Important:** `PromptStore::save` (`prompts.rs:44`) does **not** strip private tags — it inserts `input.content` verbatim. CLAUDE.md lists `PromptStore::save()` as a privacy-strip site, but in the actual source the prompt strip is not present in the store, so any stripping for prompts must come from the caller. There is no dedup, no soft-delete, and no vec0 write for prompts. The test `prompts_save_get_list_delete_round_trip` (`secondary_crud.rs:50`) covers the full lifecycle.

### 5.6 `SessionStore` (`sessions.rs`)

`SessionStore` manages the `sessions` table whose `status` column is CHECK-constrained to `('active','ended','aborted')` (V001 schema). API:

| Method | Behavior |
|--------|----------|
| `start(SessionInput)` | INSERT with `status='active'`, `started_at=now`; returns a fully-built `Session` (`status: Active`, `ended_at: None`, `summary: None`) |
| `end(id, summary)` | `UPDATE ... status='ended', ended_at=?1, summary=?2 WHERE id=?3 AND status='active'`; 0 rows → `NotFound` |
| `abort(id)` | `UPDATE ... status='aborted', ended_at=?1 WHERE id=?2 AND status='active'`; 0 rows → `NotFound` (does not set `summary`) |
| `get(id)` | `Option<Session>` |
| `list(SessionFilter)` | filter `project`/`status`, order `started_at DESC`, optional `limit` |
| `count_total` / `count_by_status` | stats (`count_by_status` GROUP BY `status`, ordered `n DESC, status ASC`) |

`SessionInput` carries `project: String` (required) and `directory: Option<String>`. The `AND status='active'` guard on `end`/`abort` makes them state-machine-safe: ending an already-ended session returns `NotFound` rather than re-closing it. The test `end_idempotency_rejects_already_ended` (`sessions_crud.rs:47`) asserts exactly this. `row_to_session` (`sessions.rs:167`) uses an unusual nested-`Result` closure shape (`rusqlite::Result<rusqlite::Result<Session>>`) so that ULID/status parse failures map to `FromSqlConversionFailure` while still allowing `query_row`'s NoRows to be distinguished by the caller; `get` collapses with `.transpose()` and `list` collapses with `r??`. `SessionStatus::from_str_strict` rejects unknown status strings. Sessions are the FK target of `observations.session_id` (`ON DELETE SET NULL`), which is why `seele-sync` strips `session_id` on import.

### 5.7 `RelationStore` vs `LinkStore` — two distinct edge tables

SEELE has **two** separate graph-edge concepts, and they are *not* interchangeable.

`memory_relations` (via `RelationStore`, `relations.rs`) is the **judged**, provenance-rich relation table used by the `seele_judge` / `seele_compare` workflows. `RelationInput` carries a `sync_id: String` (a `NOT NULL UNIQUE` column for cross-machine identity), `source_id`/`target_id` (`SeeleId`), a typed `relation: RelationKind`, optional `reason`/`evidence`/`confidence: f64`, provenance triples `marked_by_actor`/`marked_by_kind`/`marked_by_model`, and an optional `session_id`. `create` always inserts with `judgment_status = 'pending'` (the literal `'pending'` is baked into the INSERT VALUES, `relations.rs:62`). The lifecycle method `judge(id, JudgmentInput)` sets the new status and `COALESCE`s reason/evidence/confidence so a `None` field leaves the existing value untouched:

```rust
// crates/seele-storage/src/relations.rs:100
"UPDATE memory_relations SET judgment_status = ?1,
    reason = COALESCE(?2, reason), evidence = COALESCE(?3, evidence),
    confidence = COALESCE(?4, confidence) WHERE id = ?5"
```

`judge` returns `NotFound` if 0 rows matched. `get`/`list` round-trip the full struct; `list` filters by `source_id`/`target_id`/`relation`/`status`, orders `created_at DESC`, optional `limit`. `RelationKind` and `JudgmentStatus` both parse with `from_str_strict` (unknown → `InvalidInput`); the schema CHECK-constrains `judgment_status` to `('pending', 'judged', 'orphaned', 'ignored')` (V001 schema). There is no `delete` and no soft-delete on relations. The test `relations_create_judge_lifecycle` (`secondary_crud.rs:139`) walks create (`Pending`) → judge (`Judged`, evidence "PR #42", confidence 0.95) → list-by-status.

`links` (via `LinkStore`, `links.rs`) is the **generic, unjudged** edge table the module doc describes as used by MNEMA for "the derivative graph (counsel→advisor→review→verdict→skill→decision)" (`links.rs:1-2`). `LinkInput` is minimal: `from_id`, `to_id`, `link_type: String` (free-form, not an enum), and `metadata`. `create` inserts a row with a fresh `SeeleId` and `created_at`; the schema enforces `UNIQUE(from_id, to_id, link_type)` (V001 schema, line 152), so creating the same edge twice errors — the test `links_unique_constraint_blocks_duplicate_edge` (`secondary_crud.rs:115`) asserts the error message contains "UNIQUE"/"unique". `create_in_tx` (`links.rs:69`) is the transactional variant used by the ENGRAM importer to commit `linked_to[]`-derived edges atomically with their observations. `list(LinkQuery)` filters by `from_id`/`to_id`/`link_type` (order `created_at DESC`, optional `limit`); `delete(id)` hard-deletes (`NotFound` if absent). Note there is no `get(id)` on `LinkStore`. (Caveat: the `seele-engram-import` source carries a stale comment claiming `links` has *no* UNIQUE constraint and that it probes manually for that reason; the V001 schema does declare the UNIQUE, and the manual probe in the importer is there to avoid aborting the surrounding transaction on a duplicate — not because the constraint is absent.)

The key distinction for the improvement reviewer: relations are typed + judged + carry a `sync_id` and provenance and have no uniqueness constraint on the `(source,target,relation)` edge (only `sync_id` is unique); links are free-typed, idempotent on `(from,to,type)`, carry only metadata, and have no judgment lifecycle. They write to different tables and serve different consumers.

### 5.8 `ChunkStore` — the sync import ledger (`chunks.rs`)

`ChunkStore` records which sync chunks have already been imported, keyed by composite PK `(target_key, chunk_id)` so re-imports are idempotent. `SyncChunk { target_key: String, chunk_id: String, imported_at: DateTime<Utc> }` is `Serialize + Deserialize`. The API: `mark_imported(target_key, chunk_id) -> bool` returns `true` if newly recorded, `false` if already present (it uses `INSERT OR IGNORE` and inspects the affected rowcount, returning `inserted == 1`); `mark_imported_in_tx(tx, ...)` is the transaction-scoped variant used inside `seele-sync::import_from_file` so the ledger entry commits or rolls back together with the chunk's observation saves; `was_imported(...) -> bool` is the cheap pre-check `seele-sync` runs before opening a transaction (`sync/lib.rs:291`, implemented as `SELECT count(*) … == 1`); and `list_for_target(target_key)` returns all recorded chunks for a target ordered `imported_at DESC`. The test `chunks_idempotent_mark_imported` (`secondary_crud.rs:187`) verifies the `true`/`false` semantics and that a second mark of the same `(target,chunk)` is a no-op. `target_key` is typically a git remote ref like `"origin/main"`; `chunk_id` is the chunk's SHA-256 (per CLAUDE.md, "Re-imports idempotent por SHA-256").

### 5.9 Concurrency, transactions, and failure modes

There is no async in this crate — every store method is synchronous and blocks on a pooled `rusqlite::Connection` checked out from the `r2d2` `Pool`. Higher layers (axum, MCP) wrap these calls in `tokio` blocking contexts. Concurrency control is delegated to SQLite/WAL plus the pool: each method that mutates either runs a single `conn.execute` (auto-commit) or explicitly opens `conn.transaction()` (`save`, `update`) for multi-statement atomicity. The "open one connection, drive many stores in one tx" pattern is only reachable through the `*_in_tx` associated functions plus `pool()`, and is exercised by `seele-sync` and `seele-engram-import`.

Failure-mode summary for the improvement section: (1) `int_id` collision is bounded-retried (5×) then surfaced as `Conflict`; (2) `QueryReturnedNoRows` is normalized to `Ok(None)` for reads and `NotFound` for guarded mutations; (3) bad ULID text, unknown enum strings (strict-parsed `scope`/`status`/`relation`), and unserializable metadata become `InvalidInput`; (4) the embedding write is intentionally best-effort and decoupled from the row write, so a row can exist without a vector (the system tolerates this and relies on a future reindex); (5) `ms_to_dt` silently substitutes `Utc::now()` for an out-of-range stored timestamp; (6) soft-deleted observations stay in the FTS5 index, so search must re-filter `deleted_at`. There are no `TODO`/`FIXME` markers in these store files. The two documented behavioral gotchas a reviewer should examine are: nested `<private>` tags only partially strip (`privacy.rs:68`), and title dedup is exact-string rather than normalized (so reformatting a title defeats the dedup window even when the content hash matches).

### 5.10 File-by-file map

- **`observations.rs`** — `ObservationStore` and the full write pipeline (`save`/`save_in_tx`), the raw migration insert (`save_raw_in_tx`), CRUD (`get`/`list`/`update`), lifecycle (`soft_delete`/`restore`/`hard_delete`), the vec0 embedding bridge (`set_embedding`/`delete_embedding`), stats helpers, and `parse_observation`. The single most important file in the layer.
- **`prompts.rs`** — `PromptStore` for `user_prompts`: save/get/list/delete, the `UserPrompt` serde struct; no dedup, no soft-delete, and no in-store private-tag strip.
- **`sessions.rs`** — `SessionStore` over `sessions` with the active→ended/aborted state machine guarded by `WHERE status='active'`, plus session stats (`count_total`/`count_by_status`).
- **`relations.rs`** — `RelationStore` for the judged `memory_relations` graph: create (always `pending`), `judge` with `COALESCE` merge, get/list, provenance + `sync_id`.
- **`links.rs`** — `LinkStore` for the generic `links` graph with `UNIQUE(from_id,to_id,link_type)`, including `create_in_tx` for atomic imports (create/list/delete; no `get`).
- **`chunks.rs`** — `ChunkStore` import ledger keyed by `(target_key, chunk_id)`, with `mark_imported`/`mark_imported_in_tx`/`was_imported`/`list_for_target`.
- **`hash.rs`** — `normalized_hash`: lowercase → collapse whitespace → SHA-256 → hex; the dedup fingerprint over content.
- **`privacy.rs`** — `strip_private_tags` via the lazy `(?si)<private>.*?</private>` regex; multiline, case-insensitive, lazy; leaves unclosed tags and nested-tail intact.
- **`lib.rs`** — module declarations, the curated re-export surface, and `init_db` (pool + `run_pending` migrations).
- **`error.rs`** — `StorageError` enum and crate `Result` alias.
- **`tests/`** — `observations_crud.rs` (save/upsert/dedup/privacy/lifecycle), `sessions_crud.rs` (state machine + filters), `secondary_crud.rs` (prompts/links/relations/chunks smoke + the UNIQUE-edge assertion), `foundation_smoke.rs` (schema/FTS/vec0 presence, idempotent `init_db`, raw vec0 + FTS round-trips), and `properties.rs` (proptest invariants for hash determinism/hex-length/whitespace/ASCII-case, privacy idempotence, and save→get / save→list round-trips — 32 cases for pure-function properties, 8 for the DB-touching ones).


---

## 6. Embeddings — `seele-embedder`

`seele-embedder` is the subsystem that turns text into the 384-dimensional, L2-normalized float vectors that SEELE stores in the `vec0` virtual table and uses for semantic KNN. It is a leaf crate: its only declared internal dependency is `seele-core`, and that dependency is not referenced anywhere in the crate's source at all — the crate defines its own error type and never touches `SeeleId` (the dependency is effectively a workspace-convention placeholder). Everything that needs a vector — `seele-search` (vector arm of the hybrid query), `seele-storage` (vec0 inserts at save time), and through them `seele-http`, `seele-mcp`, `seele-tui`, and `seele-cli` — depends on the single `Embedder` trait this crate exports. The crate's job is narrow and well-bounded: define the trait, ship a production ONNX implementation, ship a deterministic fake for tests/offline use, provide a process-global slot for long-running servers, and expose a typed error enum.

The public API surface (re-exported from `lib.rs`) is small:

| Export | Kind | Source | Purpose |
|---|---|---|---|
| `Embedder` | trait | `embedder.rs` | The abstraction all callers program against |
| `EmbedderError`, `Result<T>` | enum / alias | `error.rs` | Typed failure modes |
| `FakeEmbedder` | struct | `fake.rs` | Deterministic hash-based vectors for tests / `--fake-embedder` |
| `OnnxConfig`, `OnnxEmbedder` | struct / struct | `onnx.rs` | Production all-MiniLM-L6-v2 path |
| `resolve_cache_dir` | fn | `onnx.rs` | Resolves the on-disk model cache directory |
| `global`, `init_global`, `try_global` | fn | `singleton.rs` | Process-global embedder slot |

### The `Embedder` trait (`embedder.rs`)

The trait is the contract every embedder must satisfy. Its documented invariant is load-bearing: **all returned vectors MUST be L2-normalized (norm = 1) and of length `dim()`** — because the downstream `vec0` table is queried with cosine/L2 distance and that distance is only meaningful when both query and stored vectors are unit vectors.

```rust
// crates/seele-embedder/src/embedder.rs:8
pub trait Embedder: Send + Sync {
    fn embed(&self, text: &str) -> Result<Vec<f32>>;
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }
    fn dim(&self) -> usize;
    fn model_id(&self) -> &str;
    fn expected_sha256(&self) -> Option<&str> { None }
}
```

| Method | Signature | Notes |
|---|---|---|
| `embed` | `fn embed(&self, text: &str) -> Result<Vec<f32>>` | Single text → one unit vector of length `dim()`. |
| `embed_batch` | `fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>` | Default impl iterates `embed`; `OnnxEmbedder` overrides it to do one ONNX session call for the whole batch (N tokenizations + 1 inference vs. N inferences). |
| `dim` | `fn dim(&self) -> usize` | 384 for all-MiniLM-L6-v2 and for `FakeEmbedder`. |
| `model_id` | `fn model_id(&self) -> &str` | Human-readable identifier for logging/telemetry; the HF repo for ONNX, the literal `seele/fake-embedder` for the fake. |
| `expected_sha256` | `fn expected_sha256(&self) -> Option<&str>` | Hex SHA256 of the model weights when verified; `None` for fakes/unverified models. Default returns `None`. Intended for upstream tooling to detect a model swap (which would require re-embedding). |

The trait is `Send + Sync`, which is what lets callers store an `Arc<dyn Embedder>` and share it across tokio worker threads in the HTTP and MCP servers. The trait is object-safe; SEELE almost always uses it behind `Arc<dyn Embedder>` or `Box<dyn Embedder>` rather than as a generic bound.

### `OnnxEmbedder` (`onnx.rs`) — the production path

This is the real pipeline. Constants at the top of the file fix the model and shapes:

| Constant | Value | Meaning |
|---|---|---|
| `DEFAULT_MODEL_REPO` | `"sentence-transformers/all-MiniLM-L6-v2"` | HF repo |
| `ONNX_PATH_FULL` | `"onnx/model.onnx"` | Full-precision weights inside the repo |
| `ONNX_PATH_QUANTIZED` | `"onnx/model_quantized.onnx"` | INT8-quantized weights |
| `TOKENIZER_PATH_IN_REPO` | `"tokenizer.json"` | HuggingFace fast-tokenizer JSON |
| `DEFAULT_DIM` | `384` | Output dimension (`pub(crate)`) |
| `DEFAULT_MAX_LEN` | `256` | Max sequence length (token cap) |
| `CACHE_DIR_ENV` | `"SEELE_EMBEDDER_DIR"` | Override env var for the cache dir |
| `CACHE_DIR_SUFFIX` | `"seele/embedder"` | Suffix appended to `dirs::cache_dir()` |

**`OnnxConfig`** is the tunable configuration:

| Field | Type | Default | Meaning |
|---|---|---|---|
| `model_repo` | `String` | `DEFAULT_MODEL_REPO` | Which HF repo to pull |
| `max_seq_len` | `usize` | `256` (`DEFAULT_MAX_LEN`) | Tokens beyond this are truncated |
| `dim` | `usize` | `384` (`DEFAULT_DIM`) | Asserted against the model's actual output rank-3 last dim |
| `quantized` | `bool` | `true` | Prefer the INT8 `model_quantized.onnx` (~30% faster, ~2% quality drop per the doc comment); falls back to full precision with a warning if the quantized artifact is absent |

`OnnxConfig` derives `Debug, Clone`. A unit test (`quantized_default_is_true`) pins the quantized default.

**`OnnxEmbedder` struct fields:**

| Field | Type | Meaning / invariant |
|---|---|---|
| `session` | `Mutex<Session>` | The `ort` inference session. Wrapped in a `Mutex` because `ort::session::Session::run` takes `&mut self` and the embedder is shared `Send + Sync`; only one inference runs at a time. |
| `tokenizer` | `Tokenizer` | The HuggingFace tokenizer; immutable after load, no lock needed. |
| `config` | `OnnxConfig` | The effective config. |
| `loaded_model_file` | `String` | Which file actually loaded (`onnx/model.onnx` or `onnx/model_quantized.onnx`) — used for hash lookup and `expected_sha256`. |

**Construction.** `OnnxEmbedder::new()` delegates to `with_config(OnnxConfig::default())`. `with_config` runs this sequence (`onnx.rs:92`): `resolve_cache_dir` → `resolve_model_files` (HF download/cache of model + tokenizer) → `verify_hash_if_listed` for the model file and then for the tokenizer → `Tokenizer::from_file` → `Session::builder()?.commit_from_file(&model_path)`. The session build is the expensive part (~150–300 ms per the singleton doc).

**hf-hub auto-download and cache location.** `resolve_model_files` (`onnx.rs:255`) builds an `ApiBuilder::new().with_cache_dir(...)` sync HF API and calls `model.get(path)` for each artifact; `hf-hub` downloads on first run and serves from cache afterwards. The cache directory is decided by `resolve_cache_dir()` (`onnx.rs:239`), priority:
1. `SEELE_EMBEDDER_DIR` env var (verbatim, trimmed; empty/whitespace → `EmbedderError::CacheDirUnresolvable`).
2. `dirs::cache_dir().join("seele/embedder")` (the OS cache dir + suffix); if `dirs::cache_dir()` returns `None` → `EmbedderError::CacheDirUnresolvable`.

The directory is `create_dir_all`'d unconditionally before the path is returned (`onnx.rs:251`). Note a documentation/behavior nuance: `lib.rs` and the `onnx.rs` module doc say the cache is under `~/.seele/embedder/<model>/`, but the code actually uses `dirs::cache_dir()` unless `SEELE_EMBEDDER_DIR` is set. On Windows that base is `%LOCALAPPDATA%` (e.g. `C:\Users\<user>\AppData\Local`), and on Linux `$XDG_CACHE_HOME` or `~/.cache`; with the `seele/embedder` suffix joined the effective root becomes e.g. `%LOCALAPPDATA%\seele\embedder` or `~/.cache/seele/embedder`. The `~/.seele/embedder` phrasing in the doc comments is therefore aspirational/inaccurate unless `SEELE_EMBEDDER_DIR` is set. The HF API additionally lays out its own `models--<org>--<repo>/snapshots/...` tree underneath whatever cache dir is passed.

**INT8 quantization selection** (`resolve_model_files`, the `if config.quantized` branch at `onnx.rs:265`): if `config.quantized` (default), it tries `model.get(ONNX_PATH_QUANTIZED)`; on error it emits a `tracing::warn!` with `repo`/`quantized_path`/`err` fields and the message `"quantized model not available; falling back to full precision"`, then pulls `ONNX_PATH_FULL`. If `quantized` is false it goes straight to full precision. The returned `loaded_model_file` records the actual choice. The tokenizer (`tokenizer.json`) is fetched unconditionally afterward.

**SHA256 verification.** `TRUSTED_HASHES: &[(&str, &str, &str)]` is a const allowlist of `(repo, file_path, hex)` tuples — **empty by default at this version** (the comment at `onnx.rs:48` says "No hashes are pinned by default at v0.1 — populate per release"). The policy implemented by `verify_hash_if_listed` (`onnx.rs:295`) is *trust-on-first-use with optional pinning*: if a `(repo, file)` is **not** listed, it logs a `tracing::debug!` line ("no trusted hash listed for this artifact; skipping integrity check") and returns `Ok(())` (so custom models load without a forced allowlist); if it **is** listed and the computed `Sha256::digest` (hex-encoded via `hex::encode`, compared case-insensitively with `eq_ignore_ascii_case`) does not match, it returns `EmbedderError::HashMismatch { file, expected, got }` — the tamper signal (where `file` is formatted as `"{repo}/{file}"`). Both the model file and the tokenizer are checked. Because the table is empty today, verification is effectively a no-op in practice and `expected_sha256()` returns `None`, which is why `seele doctor` shows a null `embedder_expected_sha256`. The `trusted_hash_for(repo, file)` helper (`onnx.rs:288`) does the lookup and is exercised by `trusted_hash_lookup_returns_none_for_unlisted`.

**Inference, pooling, normalization** (`run_inference`, `onnx.rs:115`). Step by step:
1. Empty batch short-circuits to `Ok(Vec::new())`.
2. Allocate three `Array2::<i64>::zeros((batch, max_len))` for `input_ids`, `attention_mask`, `token_type_ids`.
3. For each text: `tokenizer.encode(*text, true)` (the `true` = add special tokens), copy up to `min(ids.len(), max_len)` ids/mask/type-ids into the row (the rest stay zero-padded). This is where `DEFAULT_MAX_LEN = 256` truncation happens (`onnx.rs:132`).
4. Lock the session mutex, run with `ort::inputs![...]` wrapping each array as a `TensorRef::from_array_view`, extract `outputs["last_hidden_state"]` as `f32` via `try_extract_tensor` (returns `(shape, hidden)`).
5. Validate the output: shape must be rank 3 (`shape.len() == 3`, else `EmbedderError::Ort` with a "expected last_hidden_state rank 3" message); `shape[2]` (hidden dim) must equal `config.dim` else `EmbedderError::DimensionMismatch { expected, actual }`. `seq` is taken from `shape[1]`. Copy the hidden buffer out with `hidden.to_vec()` so the **mutex is released before CPU-bound pooling** — an explicit concurrency optimization (comments at `onnx.rs:140`–`141` and `:164`–`166`). The copied buffer is then re-viewed as an `ArrayView3` of shape `(batch, seq, dim)`.
6. Mean pooling with attention-mask weighting: for each sequence position with mask `m != 0`, `pooled.scaled_add(m, &token_embed)` and accumulate `mask_sum`; divide pooled by `mask_sum` (floored at `1e-9` to avoid div-by-zero on an all-masked row).
7. `l2_normalize(&mut pooled)` — divide by `‖v‖` with the norm floored at `1e-12` so a zero vector stays zero instead of producing NaNs.

The doc comment (`onnx.rs:9`) states the *why*: this recipe matches the sentence-transformers reference, and "without that recipe the cosine similarity in vec0 would be miscalibrated."

`OnnxEmbedder`'s `Embedder` impl (`onnx.rs:196`): `embed` rejects empty input with `EmbedderError::EmptyInput("text")` (`onnx.rs:199`), calls `run_inference(&[text])`, and pops the single result (returning `EmbedderError::EmptyInput("empty inference result")` if the result vec is somehow empty). `embed_batch` short-circuits an empty slice to `Ok(Vec::new())`, rejects an empty-string member of the batch (`"at least one text in batch is empty"`), and otherwise calls `run_inference` once for the whole slice — the batching win. There are two `expected_sha256` methods: the inherent `OnnxEmbedder::expected_sha256(&self) -> Option<&'static str>` (`onnx.rs:111`, looks up `TRUSTED_HASHES` for the loaded file) and the trait method (`onnx.rs:226`) that forwards to it.

### `FakeEmbedder` (`fake.rs`)

A zero-sized (unit struct, derives `Default, Clone`), deterministic embedder used by tests across `seele-search`, `seele-storage`, `seele-http`, `seele-mcp`, and by the CLI when `--fake-embedder` / `SEELE_FAKE_EMBEDDER` is set or when ONNX init fails. Constants: `FAKE_DIM = 384`, `FAKE_MODEL_ID = "seele/fake-embedder"`. The literal substring `fake` in the model id is what `seele doctor` keys on to emit its warning. `FakeEmbedder` exposes a `new()` constructor in addition to being default-constructible.

`hash_to_vector` (`fake.rs:23`) builds the 384-element vector in 8-element blocks (48 blocks total). For block `block_idx`, it SHA256s `text_bytes || (block_idx as u32).to_le_bytes()` (the seed is the block index cast to `u32`, four little-endian bytes), then maps the first 16 bytes of the resulting digest into 8 floats: for element `j`, `lo = digest[j*2]`, `hi = digest[j*2 + 1]`, `raw = ((hi << 8) | lo)` interpreted as a `u16`, scaled to `[-1, 1)` via `raw / 32768.0 - 1.0`. (Only 16 of each digest's 32 bytes are consumed; the rest are discarded.) The full 384-vector is then L2-normalized inline (norm floored at `1e-12`). Properties (covered by its unit tests): correct dim, unit norm, identical text → identical vector, different text → different vector. Because it is a hash, two near-identical strings produce close-but-distinct vectors and unrelated strings decorrelate — enough structure for tests to assert ranking behavior without semantic meaning. (The `doctor.rs` warning text claims the fake "returns deterministic zeros," which is stale/inaccurate — it returns non-zero hash-derived unit vectors.)

### `singleton.rs` — the process-global slot

Long-running servers (HTTP `seele serve`, MCP `seele mcp`) want a single shared embedder because building the ONNX session costs ~150–300 ms and should not happen per request. The slot is a `static GLOBAL_EMBEDDER: OnceCell<Arc<dyn Embedder>>` (`once_cell::sync::OnceCell`).

| Function | Signature | Behavior |
|---|---|---|
| `init_global` | `fn init_global(embedder: Arc<dyn Embedder>) -> Result<()>` | One-shot install (via `OnceCell::set`); second call returns `EmbedderError::GlobalAlreadyInitialized`. |
| `global` | `fn global() -> Arc<dyn Embedder>` | Clones the `Arc`; **panics** with a descriptive message ("global embedder accessed before init_global() was called") if uninitialized. |
| `try_global` | `fn try_global() -> Option<Arc<dyn Embedder>>` | Non-panicking variant. |

The module doc is explicit that tests and one-shot CLI invocations should **not** use the global — they construct their own embedder and pass it explicitly to keep state isolated. In practice the SEELE service layer (`SeeleService::new(pool, embedder)`) takes the embedder by argument rather than reading the global, so the `init_global`/`global` API is available infrastructure that the current binaries largely sidestep via dependency injection. The one-shot, no-replace semantics are deliberate: callers wanting hot-swap must roll their own `Arc<RwLock<…>>` (the doc comment says exactly this). A single inline test exercises init + read via `global`/`try_global` + the double-init error.

### `error.rs`

`EmbedderError` (derive `Debug` + `thiserror::Error`) plus `pub type Result<T> = std::result::Result<T, EmbedderError>`:

| Variant | Trigger |
|---|---|
| `HfHub(String)` | hf-hub download/API failure (also via `From<hf_hub::api::sync::ApiError>`) |
| `Tokenizer(String)` | tokenizer load/encode failure (`From<tokenizers::tokenizer::Error>`) |
| `Ort(String)` | ONNX session/run/extract failure or bad output rank (`From<ort::Error>`) |
| `Io(std::io::Error)` | `#[from]` — filesystem (cache dir create, file read) |
| `DimensionMismatch { expected, actual }` | model hidden dim ≠ `config.dim` |
| `EmptyInput(&'static str)` | empty single text, empty batch member, or empty inference result |
| `CacheDirUnresolvable` | `SEELE_EMBEDDER_DIR` empty/whitespace, or unset and `dirs::cache_dir()` returns `None` |
| `HashMismatch { file, expected, got }` | listed artifact's SHA256 differs (tamper signal); message tells the user to delete the cache file and retry to redownload |
| `GlobalAlreadyInitialized` | second `init_global` |

Three manual `From` impls bridge external error types into `Ort`/`HfHub`/`Tokenizer` by stringifying — SEELE keeps the foreign error opaque rather than carrying its type. `ort::Error` is stringified to avoid leaking the release-candidate dependency's error type across the API. (Note `Io` is the only `#[from]`-derived bridge; the other three are hand-written `impl From`.)

### External crates and why

`ort = "=2.0.0-rc.10"` (exact pin — no 2.0 stable existed at the build date; Dependabot is configured to bump it) runs the ONNX session. `ndarray 0.16` provides the `Array2`/`ArrayView3`/`Array1` tensors for tokenization buffers and pooling math. `tokenizers 0.20` is the HuggingFace fast tokenizer. `hf-hub 0.3` auto-downloads and caches model artifacts. `sha2 0.10` + `hex 0.4` implement the integrity check and the fake's hashing. `dirs 5` resolves the OS cache dir. `once_cell 1.20` backs the global slot. `thiserror 2` derives the error enum. `tracing 0.1` logs the quantized-fallback warning and the hash-skip debug line. `serde`/`serde_json` are declared dependencies (workspace convention) though not central to the runtime path here. All runtime deps are pulled via `workspace = true`; the lone dev-dependency is `tempfile` (also `workspace = true`).

### The transparent ONNX→Fake fallback and how Fake is forced

The fallback policy is **not** in `seele-embedder` itself — the crate exposes both implementations and lets callers choose. The policy lives in the CLI's `app.rs::pick_embedder` (`crates/seele-cli/src/app.rs:144`), which is the single chokepoint all DB-touching subcommands use via `build_service` (`app.rs:128`):

```rust
// crates/seele-cli/src/app.rs:144
fn pick_embedder(fake_flag: bool) -> Arc<dyn Embedder> {
    if fake_flag || fake_env_set() {
        return Arc::new(FakeEmbedder);
    }
    match OnnxEmbedder::new() {
        Ok(emb) => Arc::new(emb),
        Err(e) => {
            eprintln!(
                "seele: warning — ONNX embedder unavailable ({e}); falling back \
                 to FakeEmbedder. Search quality is degraded (hash-based, not \
                 semantic). Re-run with network access on first call to \
                 download the model, or set SEELE_FAKE_EMBEDDER=1 to silence \
                 this message."
            );
            Arc::new(FakeEmbedder)
        }
    }
}
```

Selection priority (first match wins): (1) the global `--fake-embedder` clap flag (`Cli::fake_embedder`, `app.rs:33`–`34`) **or** a non-empty `SEELE_FAKE_EMBEDDER` env var (`fake_env_set` reads the `FAKE_EMBEDDER_ENV` constant, trims, and checks non-empty) forces `FakeEmbedder`; (2) otherwise `OnnxEmbedder::new()`; (3) if that errors (no network on first run, blocked HF, etc.), warn on **stderr** and fall back to `FakeEmbedder` so the tool keeps working with degraded (hash, not semantic) search. The env var exists so processes that cannot pass `argv` (containers, agents) can still force the fake. The CLI E2E test harnesses set `SEELE_FAKE_EMBEDDER=1` on every spawn to avoid downloads. A unit test (`pick_embedder_with_flag_returns_fake`) asserts the flag path returns `seele/fake-embedder`.

### Connections to the rest of SEELE

The embedder crosses the boundary as `Arc<dyn Embedder>`. `SeeleService::new(pool, embedder)` (`seele-http/src/service.rs:47`) stores it on the `embedder: Arc<dyn Embedder>` field; `SeeleService::embedder_info()` (`seele-http/src/service.rs:429`) projects `model_id`/`dim`/`expected_sha256` into the `EmbedderInfo` DTO (`seele-http/src/dto.rs:464`) surfaced by `GET /embedder` (handler `get_embedder_info`, `handlers.rs:194`; route registered at `server.rs:150`) and reused by `seele doctor`. Because `seele-search`'s `SearchEngine` currently wants an owned `Box<dyn Embedder>`, the HTTP service defines an `ArcEmbedder(Arc<dyn Embedder>)` newtype (`service.rs:458`) that re-implements `Embedder` by forwarding every method (`service.rs:460`–`476`); `SeeleService::new` wraps the shared `Arc` in a `Box::new(ArcEmbedder(embedder.clone()))` at `service.rs:54` so one shared instance serves both the engine and direct callers. `seele doctor` reads `model_id` and emits a warning whenever it contains `"fake"` (`doctor.rs:39`). The `expected_sha256` field threads through to detect model swaps for a future re-embed flow (the inherent method's doc at `onnx.rs:110` references `seele embedder reembed-all`, which does not yet exist).

### Edge cases, gotchas, invariants

- **Cache-dir documentation drift**: `lib.rs:5` and the `onnx.rs` module doc (`onnx.rs:5`) say `~/.seele/embedder/<model>/`, but the code uses `dirs::cache_dir().join("seele/embedder")` unless `SEELE_EMBEDDER_DIR` overrides. Resolution lives in `crates/seele-embedder/src/onnx.rs:239`.
- **`doctor` stale text**: `crates/seele-cli/src/commands/doctor.rs:41` says the fake "returns deterministic zeros" and the comment at `doctor.rs:35` says "v0.1 always uses FakeEmbedder (real ONNX in Sprint-05+)" — both are stale relative to the current code (real ONNX is the default selection; the fake returns non-zero unit vectors).
- **Empty `TRUSTED_HASHES`**: integrity verification is inert until populated (`onnx.rs:47`); a tampered cached model would load silently. The intended remedy is per-release hash pinning, documented in `CHANGELOG.md`.
- **Mutex poisoning**: `self.session.lock().expect("OnnxEmbedder mutex poisoned")` (`onnx.rs:143`) will panic if a prior inference panicked while holding the lock — there is no recovery path.
- **Truncation is silent**: inputs over 256 tokens are clipped with no warning (`onnx.rs:132`).
- **Real-model tests are `#[ignore]`** (`onnx.rs:466`, `:476`) because they download ~30–90 MB; run with `cargo test --package seele-embedder -- --ignored`.
- **No `tests/` directory**: the crate has no integration-test directory; all tests are inline `#[cfg(test)]` modules in `onnx.rs`, `fake.rs`, and `singleton.rs`. The only dev-dependency is `tempfile`, used by the cache-dir env tests (which serialize env mutation behind a module-local `ENV_LOCK` mutex and use `unsafe { std::env::set_var }`).

### File-by-file map

- **`lib.rs`** — crate doc + module declarations and the public re-exports (`Embedder`, `EmbedderError`/`Result`, `FakeEmbedder`, `OnnxConfig`/`OnnxEmbedder`/`resolve_cache_dir`, `global`/`init_global`/`try_global`). Documents the default model and the server-vs-CLI usage split.
- **`embedder.rs`** — the `Embedder` trait: five methods, the L2-norm/length invariant, default `embed_batch` and default `expected_sha256` returning `None`.
- **`onnx.rs`** — the production implementation: constants, `OnnxConfig`, `OnnxEmbedder`, HF download/cache (`resolve_model_files`), cache-dir resolution (`resolve_cache_dir`), INT8 fallback, SHA256 pinning helpers (`trusted_hash_for`, `verify_hash_if_listed`), tokenization + masked mean pooling + `l2_normalize` in `run_inference`, the `Embedder` impl, and unit + ignored-integration tests.
- **`fake.rs`** — `FakeEmbedder`, the 8-element-block SHA256-to-vector construction (`hash_to_vector`), its `Embedder` impl, and unit tests.
- **`singleton.rs`** — the `OnceCell<Arc<dyn Embedder>>` global with `init_global`/`global`/`try_global` and a single init+read+double-init test.
- **`error.rs`** — `EmbedderError` enum, `Result<T>` alias, and `From` bridges for hf-hub/tokenizers/ort errors (plus the `#[from]` io bridge).
- **`Cargo.toml`** — declares the dependency set (all runtime deps via `workspace = true`, plus the `seele-core` path dep) and `tempfile` as the lone dev-dependency.


---

## 7. Hybrid Search & Reciprocal Rank Fusion — `seele-search`

`seele-search` is SEELE's retrieval brain. It takes a free-text query plus filters and returns ranked `SearchHit`s by running two independent retrieval methods against the same SQLite database — FTS5 lexical search (`observations_fts`) and vec0 vector KNN (`observations_vec`) — and fusing their ranked lists with Reciprocal Rank Fusion (RRF). It optionally re-weights by a per-document `meta_score` boost and annotates each hit with `memory_relations` edges (supersedes / conflicts).

In the crate graph it is a mid-layer crate: it depends on `seele-core` (domain types: `Observation`, `SeeleId`, `Scope`, `Metadata`), `seele-storage` (the `Pool`, `StorageError`, the schema it queries directly via raw SQL), and `seele-embedder` (the `Embedder` trait, to turn the query string into an embedding vector — 384-dim under the production `all-MiniLM-L6-v2` model). It is consumed by `seele-http` (which constructs one `SearchEngine` and wraps `SearchQuery`/`SearchHit` in its DTO layer — see `crates/seele-http/src/service.rs:55` where `SearchEngine::new` is called and `:141` where `SearchQuery` is assembled), and transitively by `seele-mcp`, `seele-tui`, and `seele-cli` through the HTTP service core. The crate is fully synchronous (blocking rusqlite); async lives in the transport crates above it.

### 7.1 File-by-file map

- **`lib.rs`** (10 lines) — the crate root. Declares the three modules (`engine`, `error`, `rrf`) and re-exports the public surface: `AnnotationKind, RelationAnnotation, SearchEngine, SearchHit, SearchQuery` from `engine`; `Result, SearchError` from `error`; `RrfHit, DEFAULT_K` from `rrf`. Crate doc (`lib.rs:1`): "SEELE search — hybrid FTS5 + vec0 search via Reciprocal Rank Fusion."
- **`rrf.rs`** (146 lines) — a small, fully generic, storage-agnostic RRF combiner: `pub fn combine<I>(...) -> Vec<RrfHit<I>>` with bound `where I: Eq + Hash + Clone`, the `RrfHit<I>` struct, and `pub const DEFAULT_K: usize = 60`. Pure function, no I/O, heavily unit- and property-tested.
- **`engine.rs`** (629 lines) — the orchestration layer: `SearchEngine`, `SearchQuery`, `SearchHit`, `RelationAnnotation`, `AnnotationKind`, plus all the SQL (FTS path, vec path, empty-query list path, meta-score boost fetch, annotation fetch, row hydration) and helpers (`escape_fts`, `parse_observation`, `unpack_per_source`, `annotation_for_source/target`). Also defines two private constants: `DEFAULT_PER_METHOD_LIMIT: u32 = 50` and `DEFAULT_FINAL_LIMIT: u32 = 10` (`engine.rs:14-15`).
- **`error.rs`** (22 lines) — the `SearchError` enum (`thiserror`) and the crate `Result<T>` alias.
- **`Cargo.toml`** — declares deps `seele-core`, `seele-storage`, `seele-embedder`, plus `serde`, `serde_json`, `thiserror`, `tracing`, `rusqlite`, `chrono` (all via `workspace = true`). Dev-deps: `tempfile`, `serde_json`, `proptest`.

### 7.2 Public API surface

| Item | Kind | Purpose |
|------|------|---------|
| `SearchEngine` | struct | Holds the `Pool`, a boxed `Embedder`, and `rrf_k`; entry point for all retrieval. |
| `SearchEngine::new(pool, embedder)` | fn | Build an engine with `rrf_k = DEFAULT_K` (60). |
| `SearchEngine::with_rrf_k(self, k)` | fn | Builder override of the RRF constant (consumes and returns `self`). |
| `SearchEngine::search(&self, SearchQuery) -> Result<Vec<SearchHit>>` | fn | The one public retrieval method. |
| `SearchQuery` | struct | All inputs/knobs (see §7.3). Derives `Debug, Clone, Default`. |
| `SearchHit` | struct | One result: `observation`, `score`, `fts_rank`, `vec_rank`, `annotations`. Derives `Debug, Clone`. |
| `RelationAnnotation` | struct | A relation edge attached to a hit (`kind`, `other_id`, `other_title`, `reason`). Derives `Debug, Clone`. |
| `AnnotationKind` | enum | `Supersedes` / `SupersededBy` / `ConflictsWith` / `ContestedBy`. Derives `Debug, Clone, Copy, PartialEq, Eq`. |
| `rrf::combine<I>(rankings, k)` | fn | Generic RRF fuser over named ranked lists. |
| `rrf::RrfHit<I>` | struct | `{ id: I, score: f64, per_source: Vec<(&'static str, usize)> }`. |
| `rrf::DEFAULT_K` | const | `60`. |
| `SearchError` | enum | Typed errors (see §7.11). |

`SearchEngine`'s only public method is `search`; everything else (`fts_query`, `vec_query`, `list_by_filters`, `apply_score_boost`, `fetch_meta_scores`, `attach_annotations`, `fetch_annotations`, `hydrate`) is a private inherent method. The struct itself is opaque (no public fields): `pool: Pool`, `embedder: Box<dyn Embedder>`, `rrf_k: usize`.

### 7.3 `SearchQuery` field-by-field

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `text` | `String` | `""` | Query text. Empty/whitespace switches to the list-by-filters path. |
| `project` | `Option<String>` | `None` | Restrict to `observations.project = ?`. |
| `scope` | `Option<Scope>` | `None` | Restrict to `observations.scope = ?` (bound via `Scope::as_str()`). |
| `kind` | `Option<String>` | `None` | Restrict to `observations.type = ?`. |
| `per_method_limit` | `Option<u32>` | `50` (`DEFAULT_PER_METHOD_LIMIT`) | Candidates pulled from FTS and from vec *each* before RRF. |
| `limit` | `Option<u32>` | `10` (`DEFAULT_FINAL_LIMIT`) | Final count returned after fusion. |
| `include_purist` | `bool` | `false` | When false, excludes rows whose `meta_context_mode = 'purist'` (Counsel "purists do not see prior counsels" rule). The SQL guard is `(meta_context_mode IS NULL OR meta_context_mode != 'purist')`. |
| `score_boost_multiplier` | `f64` | `0.0` | When non-zero, post-RRF score becomes `rrf_score * (1.0 + multiplier * meta_score.unwrap_or(1.0))` (ADR-03 §"Capa 5"). `0.0` disables (Sprint-01 behavior). |
| `max_vec_distance` | `Option<f64>` | `None` | Drop vec hits whose cosine distance exceeds the threshold (`AND vec.distance <= ?`). `None` keeps all. |
| `include_annotations` | `bool` | `false` | Attach `memory_relations` annotations (one extra query; off by default to preserve the sub-300ms target). |

Note: `SearchQuery` derives `Default`, so the documented defaults for `bool`/`f64`/`Option` fields are Rust's zero-values (`false`, `0.0`, `None`). The `50`/`10` defaults are *not* in the `Default` impl — they are applied inside `search()`/`list_by_filters()` via `unwrap_or(DEFAULT_PER_METHOD_LIMIT)` / `unwrap_or(DEFAULT_FINAL_LIMIT)` when the `Option` is `None`.

### 7.4 The `search()` control flow (engine.rs:96)

The orchestration is short and deterministic. Step by step:

1. **Empty-query gate** (`engine.rs:97`). If `query.text.trim().is_empty()`, delegate to `list_by_filters` — no embedder call, no FTS, no vec. This is why whitespace-only queries no longer error (a Sprint-02 change replacing an older `InvalidInput`).
2. **Resolve limits** (`:101-102`). `per_method = per_method_limit.unwrap_or(50)`; `final_limit = limit.unwrap_or(10) as usize`.
3. **FTS path** (`:104`). `fts_query` returns `Vec<SeeleId>` in `rank` order (best first).
4. **Vec path** (`:105`). `vec_query` embeds the query and returns `Vec<SeeleId>` in ascending `distance` order.
5. **Fuse** (`:107-110`). Call `rrf::combine(&[("fts", fts_rank.clone()), ("vec", vec_rank.clone())], self.rrf_k)`. The source names `"fts"` and `"vec"` are `&'static str` and become the per-source provenance tags. (Both rank vectors are `.clone()`d into the call.)
6. **Boost + truncate** (`:113-114`). `apply_score_boost` multiplies each fused score by `(1 + multiplier * meta_score)` and re-sorts (no-op when multiplier is 0.0). Then `.take(final_limit)`.
7. **Hydrate** (`:116`). One `SELECT ... WHERE id IN (...)` fetches the full `Observation` rows for the surviving IDs, collected into a `HashMap<SeeleId, Observation>`.
8. **Assemble `SearchHit`s** (`:120`). For each fused hit, look up its observation, unpack its per-source ranks into `fts_rank`/`vec_rank` via `unpack_per_source`, and carry through `hit.score`. `filter_map` silently drops any fused ID that failed to hydrate (e.g., a stale vec0/FTS rowid pointing at a row that no longer satisfies the hydrate query). Annotations are initialized to `Vec::new()` here.
9. **Annotations** (`:135`). If `include_annotations` and there are hits, `attach_annotations` enriches them in place.

Notice ordering: the final order is the RRF (or boosted-RRF) order, and the post-hydrate step does **not** re-sort, so hydration only maps IDs to data — it never changes ranking.

### 7.5 FTS5 path (`fts_query`, engine.rs:191)

```sql
-- engine.rs:192
SELECT o.id FROM observations_fts fts
JOIN observations o ON o.int_id = fts.rowid
WHERE observations_fts MATCH ?1
  AND o.deleted_at IS NULL
-- + project/scope/type/purist filters
ORDER BY rank LIMIT ?
```

Key points:

- The query text is passed through `escape_fts` (`engine.rs:501`), which wraps it in double-quotes and doubles internal quotes: `escape_fts("hello world") == "\"hello world\""` (see the unit test `escape_fts_wraps_in_quotes_and_escapes_internal_quotes` at `engine.rs:578`). This deliberately treats the whole input as a quoted FTS5 phrase so punctuation can't break the MATCH parser. The doc-comment (`engine.rs:497-500`) flags this as a v0.1 simplification — general-text power-users cannot pass raw FTS5 boolean syntax (NEAR, OR, prefix `*`) because it would be quoted literally.
- `ORDER BY rank` uses FTS5's built-in relevance ordering (bm25-based); the more-negative rank is the stronger match, so `ORDER BY rank` ascending puts the strongest lexical match first.
- The join `o.int_id = fts.rowid` is the FTS↔observations bridge. `int_id` is a **regular stored INTEGER column** on `observations` (`int_id INTEGER NOT NULL UNIQUE`), populated at INSERT time from `SeeleId::as_i64()` — *not* a virtual/generated column. The FTS5 contentless-external mirror uses `content_rowid='int_id'`, and vec0 rows are written under the same `int_id` value (see §7.12). The arquitectura plan originally proposed a *virtual generated* `int_id` computed via `SUBSTR(id, ...) AS INTEGER`, but that formula was broken for base32 ULID strings (`CAST` of letters yields 0), so the schema NOTE in `V001__initial_schema.sql:7-13` documents the switch to a Rust-populated regular column.
- Soft-deleted rows (`deleted_at IS NOT NULL`) and purist rows are filtered in SQL, not post-hoc.
- Each returned `id` string is parsed back into a `SeeleId`; a parse failure becomes `SearchError::InvalidInput` (message `bad ULID '...'`).

### 7.6 vec0 KNN path (`vec_query`, engine.rs:232)

```sql
-- engine.rs:242
SELECT o.id, vec.distance FROM observations_vec vec
JOIN observations o ON o.int_id = vec.rowid
WHERE vec.embedding MATCH ?1
  AND vec.k = ?2
  AND o.deleted_at IS NULL
-- + project/scope/type/purist filters, + optional vec.distance <= ?
ORDER BY vec.distance
```

Steps:

1. **Embed** (`:233`). `self.embedder.embed(&query.text)` produces a `Vec<f32>`; an `EmbedderError` is converted into `SearchError::Embedder` via `#[from]` (the `?` operator).
2. **Dimension check** (`:234-239`). If `embedding.len() != self.embedder.dim()`, fail with `SearchError::DimensionMismatch { query, db }`. (This compares the produced vector length against the embedder's *own* declared `dim()`, guarding against a malformed embedder; despite the `db` field name it does not read the DB column dim here.)
3. **Serialize** (`:240`). The vector is flattened to little-endian bytes (`f.to_le_bytes()`), the byte layout sqlite-vec's `MATCH` expects.
4. **KNN** (`:245-246`). vec0's `MATCH` (line 245) plus the `vec.k = ?2` constraint (line 246), bound to `per_method`, asks for the top-`per_method` nearest neighbors. `ORDER BY vec.distance` makes the result ascending by distance (nearest first).
5. **Max-distance cutoff** (`:266-269`). If `max_vec_distance` is set, an `AND vec.distance <= ?` clause prunes far hits. This matters because KNN returns top-K *regardless of how dissimilar* — without the cutoff, an unrelated document can still occupy a vec rank and thus contribute RRF score (verified by `max_distance_drops_far_hits`, `hybrid_search.rs:454`). The `distance` column is selected but, notably, only used for the SQL filter — the engine does **not** carry the raw distance into `SearchHit` (only the integer vec *rank* survives, via RRF).

The vec path is the only path that calls the embedder. The empty-query path skips it entirely (a deliberate cost saving documented at `engine.rs:141-142`).

### 7.7 Reciprocal Rank Fusion (`rrf.rs`)

The fusion is a textbook RRF. For each ranked list and each document at 1-based `rank`, the contribution is `1 / (k + rank)`; a document's final score is the sum of its contributions across all lists it appears in:

```
score(d) = Σ over methods  1 / (k + rank_in_method(d))
```

```rust
// rrf.rs:39
for (source_name, ids) in rankings {
    for (rank0, id) in ids.iter().enumerate() {
        let rank = rank0 + 1;
        let increment = 1.0 / (k as f64 + rank as f64);
        let entry = score_map.entry(id.clone()).or_insert_with(|| {
            order.push(id.clone());
            0.0
        });
        *entry += increment;
        sources_map.entry(id.clone()).or_default().push((*source_name, rank));
    }
}
```

- **`k`** is `DEFAULT_K = 60`, "the standard from the original RRF paper (Cormack et al. 2009) and what ENGRAM/sqlite-vec docs default to" (`rrf.rs:5-6`). Larger `k` flattens the curve (rank 1 vs rank 50 differ less); smaller `k` amplifies the top-rank advantage — pinned by the unit test `smaller_k_amplifies_top_rank_advantage` (`rrf.rs:122`). The engine uses 60 unless overridden via `with_rrf_k`.
- **Combining the two lists.** `combine` is invariant to the number of input lists — `engine.rs` always passes exactly two (`"fts"`, `"vec"`), but the function loops over any slice (and returns empty on an empty slice, per `empty_rankings_produce_empty_result`, `rrf.rs:116`). A document present in *both* lists accumulates two increments and therefore outranks a document present in only one — the core hybrid-search payoff, asserted directly in `doc_in_both_sources_outranks_doc_in_one` (`rrf.rs:90`), `rank_1_in_both_sources_beats_rank_1_in_one` (`rrf.rs:103`), and the property test `doc_in_both_sources_outscores_doc_in_one` (`rrf_properties.rs:100`).
- **Missing documents contribute 0** from that method — RRF "gracefully handles partial recall" (`rrf.rs:8-9`). No imputation, no penalty.
- **Tie-breaking.** Output is sorted descending by score with `partial_cmp(...).unwrap_or(Ordering::Equal)` (`rrf.rs:69-73`). Because Rust's `sort_by` is stable and entries are appended to `order` in first-seen insertion order (the `"fts"` list is iterated first, then `"vec"`), ties resolve to first-seen order, giving "reproducible results" (`rrf.rs:29-30`). Practically, a tie favors the document that appeared earlier — and FTS entries are seen before vec entries.
- **`per_source`** preserves provenance: `Vec<(&'static str, usize)>` of `(method name, rank)` for every appearance, which `engine.rs` later unpacks into `fts_rank`/`vec_rank` (via `unpack_per_source`, `engine.rs:458`) so a caller can see *why* a hit surfaced. Provenance fidelity is property-tested by `per_source_counts_match_appearances` (`rrf_properties.rs:86`).
- **NaN safety.** Scores are sums of positive reciprocals, so always finite and `>= 0` (property-tested by `scores_are_non_negative`, `rrf_properties.rs:33`); the `unwrap_or(Equal)` is defensive rather than reachable.

### 7.8 Meta-score boost (`apply_score_boost`, engine.rs:290)

When `score_boost_multiplier != 0.0`, the engine fetches `meta_score` (the `metadata.score` virtual generated column on `observations`) for the surviving IDs via `fetch_meta_scores` (`engine.rs:316`, a single `SELECT id, meta_score FROM observations WHERE id IN (...)`), then sets `hit.score *= 1.0 + multiplier * ms`, treating a missing/null `meta_score` as `1.0` (ADR-03), and re-sorts descending. With multiplier `0.0` or no hits it returns early (`engine.rs:295`) — a true no-op, validated by `boost_zero_preserves_rrf_order` (`hybrid_search.rs:411`). `boost_promotes_high_meta_score_doc` (`hybrid_search.rs:365`) confirms a doc with `{"score": 10.0}` and multiplier `0.1` is pushed ahead of an otherwise-tied peer. The boost is applied *before* truncation (the `.take(final_limit)` in `search()` runs on the already-boosted, re-sorted list), so it can change which documents make the final `limit`. Note `fetch_meta_scores` only inserts rows whose `meta_score` is non-null into its map, so null scores fall through to the `unwrap_or(1.0)` default in `apply_score_boost`.

### 7.9 Empty-query / list-by-filters path (`list_by_filters`, engine.rs:143)

When the query text is blank, search degenerates to a recency listing: `SELECT ... FROM observations WHERE deleted_at IS NULL` plus the same project/scope/type/purist filters, `ORDER BY created_at DESC LIMIT ?`. These hits carry `score = 0.0`, `fts_rank = None`, `vec_rank = None` (so callers can distinguish "browse" hits from "ranked" hits — checked in `whitespace_only_query_treated_as_empty_query`, `hybrid_search.rs:238`, which also asserts the score-0/None-rank shape). Annotations can still be attached (`engine.rs:185-187`). The HTTP/MCP transport layer applies its *own* anti-empty-query gate (documented at `service.rs:134-135`), so this path is mostly reachable from internal callers and tests; `empty_query_with_filters_lists_recent_in_project` (`hybrid_search.rs:268`) and `empty_query_excludes_purist_by_default_and_deleted` (`hybrid_search.rs:309`) exercise it directly.

### 7.10 Relation annotations (`attach_annotations` / `fetch_annotations`, engine.rs:343)

When opted in, `attach_annotations` (`engine.rs:343`) calls `fetch_annotations` (`engine.rs:354`), which runs one query against `memory_relations` joined to `observations` twice (aliased `src` and `tgt`, for both endpoint titles), matching rows where `r.source_id` *or* `r.target_id` is in the hit set (the ID list is bound twice — once for each `IN` clause, `engine.rs:377-383`). For each row it maps the relation to an `AnnotationKind` from the perspective of whichever endpoints are in the result set:

| relation | judgment_status | source POV (`annotation_for_source`) | target POV (`annotation_for_target`) |
|----------|-----------------|-----------|-----------|
| `supersedes` | any | `Supersedes` | `SupersededBy` |
| `conflicts_with` | not `judged` | `ConflictsWith` | `ConflictsWith` |
| `conflicts_with` | `judged` | `ContestedBy` | `ContestedBy` |
| anything else | — | `None` (no annotation) | `None` (no annotation) |

So a superseding winner sees `Supersedes(other)`, a superseded loser sees `SupersededBy(other)`, and both sides of a judged conflict see `ContestedBy`. The fetch also returns the other endpoint's `title` (`other_title`) and the relation `reason`, so a UI can render "supersedes: <title> — <reason>" without a second round-trip. This is verified end-to-end in `annotations_supersedes_attaches_to_winner_and_loser` (`hybrid_search.rs:499`) and `annotations_judged_conflict_yields_contested_by` (`hybrid_search.rs:609`), with `annotations_off_by_default` (`hybrid_search.rs:558`) confirming opt-out. The unit tests at `engine.rs:583-627` pin the mapping table directly (`annotation_for_source`/`annotation_for_target`, defined at `engine.rs:474` and `:486`).

### 7.11 Error handling (`error.rs`)

`SearchError` is a `thiserror` enum with five variants:

| Variant | Source | When |
|---------|--------|------|
| `Storage(StorageError)` | `#[from]` | Pool acquisition failures (`pool.get()` is mapped via `StorageError::Pool`). |
| `Embedder(EmbedderError)` | `#[from]` | The embedder fails to produce a vector. |
| `Sqlite(rusqlite::Error)` | `#[from]` | Any `prepare`/`query`/`get` failure on raw SQL. |
| `InvalidInput(String)` | manual | Malformed ULID parse (observation/session/source/target IDs), unknown scope, or invalid metadata JSON during row hydration. |
| `DimensionMismatch { query, db }` | manual | Query embedding length ≠ embedder's declared dim. |

`type Result<T> = std::result::Result<T, SearchError>` (`error.rs:21`). There is no panicking fallback in the hot path; the only `unwrap_or`-style patterns are the defensive float comparison in RRF/boost sorting and `Utc::now()` as a fallback for an out-of-range epoch-ms timestamp in `parse_observation` (the `to_dt` closure, `engine.rs:546-550`, `unwrap_or_else(Utc::now)` at `:549`).

### 7.12 Concurrency, dependencies, gotchas

- **Concurrency.** The crate is synchronous. Every DB touch acquires a connection from `Pool` (r2d2) per call — `search()` therefore acquires *several* connections in sequence (FTS, vec, optional boost fetch, hydrate, optional annotations). The engine holds no locks itself; correctness under concurrent searches rests on SQLite/WAL configured in `seele-storage`.
- **External crates.** `rusqlite` (raw SQL + `params_from_iter` for dynamic IN-lists), `chrono` (epoch-ms → `DateTime<Utc>`), `serde_json` (parse the `metadata` column into `Metadata`), `thiserror` (error enum), `tracing` (declared in `Cargo.toml`; no logging calls appear in this crate's source). Internal: `seele-core`, `seele-storage`, `seele-embedder`.
- **The `int_id` rowid bridge.** Both FTS and vec paths JOIN on `o.int_id = <virtual>.rowid`. Contrary to a common misreading, `int_id` is **not** an approximate virtual column and `as_i64()` is **not** a separate authoritative mapping — they are the *same value*. `int_id` is a regular stored `INTEGER NOT NULL UNIQUE` column (`V001__initial_schema.sql:35`), written once at INSERT time from `SeeleId::as_i64()` (`observations.rs:540`, and again on the migration insert path at `:588`); the matching vec0 rowid is written from that same `int_id` in `set_embedding` (`observations.rs:318-336`, `INSERT OR REPLACE INTO observations_vec(rowid, embedding)`). `as_i64()` takes the ULID's random tail (bytes 9..16 per the collision comment at `observations.rs:23`), so `int_id` can in principle collide; storage retries up to `ID_COLLISION_RETRIES = 5` on the `UNIQUE` violation (`observations.rs:537-579`). The engine relies on FTS/vec/observations staying consistent on this single `int_id` — a residual mismatch would cause the `filter_map` at `engine.rs:122` to discard the orphan hit. (Note: `CLAUDE.md` describes `int_id` as an "approximate virtual column"; the schema source is authoritative and contradicts that note — `int_id` is a stored column, while the *virtual generated* columns are `meta_kind`/`meta_domain`/`meta_axiomatic`/`meta_score`/`meta_context_mode` at `V001__initial_schema.sql:56-65`.)
- **Quoted-phrase FTS limitation** (`engine.rs:497-500`) — the `// in v0.1 we're OK with the simpler quoted-phrase model for general text` doc-comment on `escape_fts` is the clearest standing limitation: no exposed FTS5 operator syntax.
- **Distance discarded.** The raw cosine/vec distance is filtered on (`max_vec_distance`) but not surfaced; only integer ranks reach the caller. A consumer wanting a similarity score must derive it from rank or re-query.

### 7.13 Property tests as invariant evidence

The crate ships two property-test files (32 `proptest` cases each).

`tests/rrf_properties.rs` pins the combiner's algebra: scores are non-negative (`scores_are_non_negative`, `:33`); output IDs equal the deduplicated union of inputs (`output_ids_equal_union_of_inputs`, `:44`); output length equals the union size — no dups, no drops (`output_length_equals_union_size`, `:55`); output is sorted descending by score (`output_is_sorted_descending_by_score`, `:70`); `per_source.len()` equals the number of source lists a doc appears in (`per_source_counts_match_appearances`, `:86`); and a doc in both sources strictly outscores a doc in one (`doc_in_both_sources_outscores_doc_in_one`, `:100`). The generators draw IDs from a small universe (`0..32`) so two independent rankings overlap often (`ranking_strategy`, `:18`).

`tests/property_tests.rs` pins the engine end-to-end with the `FakeEmbedder` fixture (via the `common` module): save→embed→search is a roundtrip when the keyword is in the content (`save_then_search_roundtrip_matches_by_keyword`, `:42`); `limit` always caps the result count (`limit_caps_result_count`, `:63`); and the empty-query path returns ≤ limit hits, all from the requested project (`empty_query_respects_project_and_limit`, `:81`). Together with the example-based `hybrid_search.rs`/`end_to_end.rs` suites and the `#[ignore]`d `perf_smoke.rs` (sub-300ms @ 10K rows in `search_under_300ms_with_10k_observations`, `perf_smoke.rs:25`, plus a sub-100ms @ 1K variant at `:55`), these encode the subsystem's guarantees.

### 7.14 The retrieval bet: FTS + embeddings + RRF vs ENGRAM's FTS + LLM-judge

SEELE's wager is that **lexical recall + semantic recall, fused by rank, is enough** to beat either alone — without an LLM in the retrieval loop. FTS5 catches exact terms, identifiers, and rare tokens (where embeddings are weak); vec0 catches paraphrase and conceptual similarity (where keyword search misses). RRF is the cheap, parameter-light glue: a single constant `k = 60`, no score normalization across heterogeneous scales (FTS5 rank vs vec distance never get compared directly — only their *ranks* do), and graceful behavior when one method has no hits.

ENGRAM (the Go inspiration) instead routes candidates through an LLM judge to re-rank/select. The trade-offs are direct: SEELE's RRF path is **local, deterministic, offline, and fast** (sub-300ms target, no API latency, no token cost, reproducible ordering for tests), at the price of no semantic *reasoning* about relevance — it cannot understand that a result is on-topic-but-wrong the way an LLM judge can. SEELE recovers some of that lost nuance not in the ranker but in *adjacent* signals: the `meta_score` boost (ADR-03 "Capa 5") lets an upstream actor weight important memories, and the relation annotations (`Supersedes`/`ContestedBy`) surface contradiction/staleness so the *consumer's* LLM can judge with that context, rather than baking judgment into retrieval. This keeps the engine a pure, testable function and pushes any LLM-grade reasoning to the agent calling SEELE — consistent with SEELE's local-first, library-not-service posture.


---

## 8. MCP Server — `seele-mcp`

`seele-mcp` is SEELE's Model Context Protocol transport: a JSON-RPC 2.0 server spoken over stdio so that MCP host clients (Claude Code, Cursor, Windsurf, OpenCode) can read and write the local memory store as a tool surface. It is one of the three transports over the same service/storage core; unlike the CLI (which links the storage stores directly) the MCP server reuses the HTTP crate's service layer. In the crate graph it depends on `seele-core`, `seele-storage`, `seele-search`, `seele-embedder`, and crucially `seele-http` (for `SeeleService`, `ApiError`, and the `dto::*` request/response types). The binary `seele` mounts it via the `seele mcp [--tool-prefix <p>] [--db <path>]` subcommand. The crate is small and synchronous at its core: only the I/O loop is async (tokio); every tool handler is a plain blocking `fn`.

The crate-level doc comment states the design intent (`crates/seele-mcp/src/lib.rs:1`):

```rust
// crates/seele-mcp/src/lib.rs:1
//! SEELE MCP server — stdio transport for Claude Code, Cursor, OpenCode, etc.
//! The crate exposes a JSON-RPC 2.0 server over a generic
//! `AsyncRead + AsyncWrite` pair (stdio by default). 19 canonical SEELE
//! tools are registered; consumers can rename them at the boundary via
//! `McpServerConfig.tool_prefix` (ADR-13: `mnema` is a first-class
//! ENGRAM-compat alias set).
```

The public API surface is re-exported at the crate root (`lib.rs:14`): `McpServer`, `McpServerConfig` (from `server`), and `all_tools`, `build_index`, `Tool`, `ToolError` (from `tools`). The `jsonrpc` module is public but not re-exported.

### 8.1 File-by-file map

| File | Responsibility |
|------|----------------|
| `src/lib.rs` | Module declarations and public re-exports. |
| `src/jsonrpc.rs` | JSON-RPC 2.0 wire types (`Request`, `Response`, `ErrorObject`) and the error-code constants. |
| `src/server.rs` | `McpServer`, `McpServerConfig`, the stdio/IO read loop, line dispatch, `initialize`/`tools/list`/`tools/call` handling, and the `CallToolResult` envelope wrapping. |
| `src/tools.rs` | `Tool`/`ToolDescriptor`/`ToolHandler` types, `ToolError` enum + conversions, `all_tools()` registry of all 19 tools, `build_index()` + `translate_name()` (ADR-13 prefix mapping), and all `schema_*()` input-schema builders. |
| `src/tool_impls/mod.rs` | Declares the four handler modules: `memories`, `meta`, `relations`, `sessions`. |
| `src/tool_impls/memories.rs` | Handlers: `save`, `search`, `show`, `list`, `update_metadata`, `soft_delete`, `restore`, `link`. |
| `src/tool_impls/sessions.rs` | Handlers: `start`, `end`, `summary`, `capture_passive` + the `extract_key_learnings` / `truncate_title` helpers. |
| `src/tool_impls/relations.rs` | Handlers: `judge`, `compare`. |
| `src/tool_impls/meta.rs` | Handlers: `stats`, `projects`, `doctor`, `version`, `suggest_topic_key`. |
| `tests/stdio_e2e.rs` | 12 E2E tests over a `tokio::io::duplex` transport (protocol + tools/call). |
| `tests/call_tool_result_envelope.rs` | 2 narrow tests asserting the `CallToolResult` wire shape (added by the 2026-05-20 patch). |

### 8.2 `jsonrpc.rs` — wire framing and error codes

MCP rides a subset of JSON-RPC 2.0. The module's doc note (`jsonrpc.rs:1`) summarizes it: request side carries only `id`, `method`, `params`; response side carries `result` xor `error`; notifications (no `id`) get logged and produce no response.

`Request` (`jsonrpc.rs:11`) is `Deserialize`-only:

| Field | Type | Notes |
|-------|------|-------|
| `jsonrpc` | `String` | `#[allow(dead_code)]` — accepted but never validated against `"2.0"`. |
| `id` | `Option<Value>` | `#[serde(default)]`; `None` means notification. The id is an opaque `Value` (number or string), echoed verbatim. |
| `method` | `String` | The RPC method name. |
| `params` | `Value` | `#[serde(default)]` → `Value::Null` when absent. |

`Response` (`jsonrpc.rs:23`) is `Serialize`-only with `jsonrpc: &'static str` pinned to `"2.0"`, an `id: Value`, and mutually exclusive `result`/`error` (both `#[serde(skip_serializing_if = "Option::is_none")]`). Two constructors enforce the xor: `Response::ok(id, result)` and `Response::err(id, error)`.

`ErrorObject` (`jsonrpc.rs:53`) carries `code: i32`, `message: String`, optional `data: Option<Value>` (skipped when `None`), with a `new(code, message)` constructor that leaves `data: None`.

The `codes` submodule (`jsonrpc.rs:72`) defines the standard codes plus one SEELE extension:

| Constant | Value | Meaning |
|----------|-------|---------|
| `PARSE_ERROR` | `-32700` | Line was not valid JSON. |
| `INVALID_REQUEST` | `-32600` | Declared but currently unused in dispatch. |
| `METHOD_NOT_FOUND` | `-32601` | Unknown JSON-RPC method. |
| `INVALID_PARAMS` | `-32602` | Missing/invalid params, unknown tool, or `ToolError::BadParams`. |
| `INTERNAL_ERROR` | `-32603` | `ToolError::Internal`. |
| `TOOL_ERROR` | `1001` | SEELE-specific: tool exists but handler returned a domain error (`NotFound`/`Conflict`). |

Note `TOOL_ERROR = 1001` is a positive, non-standard code (the reserved JSON-RPC server-error band is `-32099..-32000`); it is a deliberate SEELE convention to distinguish protocol failures from domain failures while still travelling through the JSON-RPC `error` channel.

### 8.3 `server.rs` — lifecycle, dispatch, and the CallToolResult patch

`McpServerConfig` (`server.rs:17`) is `#[derive(Clone, Default)]` and holds a single field, `tool_prefix: Option<String>`. `None` keeps canonical `seele_*` names; `Some("mnema")` triggers the ADR-13 alias set.

`McpServer` (`server.rs:25`) holds two fields: `service: SeeleService` (the reused HTTP service) and `tools: HashMap<String, Tool>` (the exposed-name → handler index). The constructor `McpServer::new(service, config)` (`server.rs:31`) builds the index eagerly via `build_index(config.tool_prefix.as_deref())` (`server.rs:32`), so prefix translation happens once at startup, not per request. The map is never mutated afterward.

**Lifecycle / transport.** `run_stdio(&self)` (`server.rs:38`) wires `tokio::io::stdin()`/`stdout()` into the generic `run_io`. `run_io<R, W>(&self, reader, mut writer)` (`server.rs:46`) is the heart of the loop. It is generic over `AsyncRead + Unpin` / `AsyncWrite + Unpin` precisely so tests can substitute `tokio::io::duplex` (the doc comment at `server.rs:44` says exactly this). The loop is line-delimited:

```rust
// crates/seele-mcp/src/server.rs:51
let mut lines = BufReader::new(reader).lines();
while let Some(line) = lines.next_line().await? {
    if line.trim().is_empty() { continue; }
    if let Some(resp) = self.handle_line(&line) {
        let s = serde_json::to_string(&resp)?;
        writer.write_all(s.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
    }
}
Ok(())
```

Each response is serialized, written, terminated with `\n`, and explicitly `flush()`ed (so a streaming client sees output immediately). Blank lines are skipped. The loop is single-threaded and strictly serial — there is no concurrency between requests, no per-request task spawn, and no batching. **Shutdown** is implicit: `next_line()` returns `None` at EOF on the reader (the host closing stdin), the `while` exits, and `run_io` returns `Ok(())`. There is no explicit MCP `shutdown` method handler and no graceful-drain step; the process simply ends when stdin closes.

**`handle_line` dispatch.** `handle_line(&self, line: &str) -> Option<Response>` (`server.rs:68`) is the pure (non-async) parse+route function. It returns `Option` so notifications can yield no response:

1. Parse the line as a `Request`. On `serde_json` failure it returns `Some(Response::err(Value::Null, PARSE_ERROR, "parse error: …"))` — the id is `Null` because the request could not even be read (matches spec). This is the only branch that returns a response with a null id.
2. `let id = req.id.clone()?;` — the `?` on the `Option<Value>` short-circuits to `None` for notifications, so no response is emitted. This is the documented "no id → no response" behavior (verified by `notification_no_id_produces_no_response`).
3. Match on `req.method`:
   - `"initialize"` → the handshake (see below).
   - `"tools/list"` → enumerate tools.
   - `"tools/call"` → delegate to `dispatch_tool_call`.
   - anything else → `METHOD_NOT_FOUND` error.

**The initialize handshake** (`server.rs:81`) returns a fixed object:

```rust
// crates/seele-mcp/src/server.rs:83
serde_json::json!({
    "serverInfo": { "name": "seele", "version": env!("CARGO_PKG_VERSION") },
    "protocolVersion": "2024-11-05",
    "capabilities": { "tools": {} },
})
```

The advertised `protocolVersion` is pinned to `"2024-11-05"`. Capabilities advertise only `tools` (an empty object meaning "tools are supported"); no `resources`, `prompts`, `logging`, or `completions` capabilities are declared. The server does not negotiate or read the client's requested `protocolVersion` from `initialize` params — it always answers `2024-11-05`. The `notifications/initialized` notification a client sends after the handshake is handled by the generic notification path (no id → no response); there is no dedicated handler for it.

**`tools/list`** (`server.rs:89`) maps the `tools` HashMap into a `Vec<ToolDescriptor>`, **sorts by name** (`entries.sort_by(|a, b| a.name.cmp(&b.name))`) for deterministic ordering, and returns `{ "tools": [...] }`. Each descriptor serializes `name`, `description`, and `inputSchema` (the `input_schema` field is renamed to `inputSchema` via `#[serde(rename)]`).

**`tools/call`** (`server.rs:113`, `dispatch_tool_call`) is the most important path and the site of the 2026-05-20 patch:

1. Read `params.name` as a string; missing → `INVALID_PARAMS "params.name required"`.
2. Read `params.arguments`, defaulting to an empty object when absent (`unwrap_or(Value::Object(Default::default()))`).
3. Look up the tool by exposed name in `self.tools`; missing → `INVALID_PARAMS "unknown tool: {name}"` (note: unknown tool maps to `-32602`, not `METHOD_NOT_FOUND`, since the method `tools/call` exists; verified by `tools_call_unknown_tool_returns_invalid_params`).
4. Invoke the handler `(tool.handler)(&self.service, arguments)`.
5. On `Ok(v)`, wrap in the `CallToolResult` envelope; on `Err(e)`, return `Response::err(id, e.to_jsonrpc())`.

The success-path envelope is the patch's core change (`server.rs:138`):

```rust
// crates/seele-mcp/src/server.rs:145
let text = serde_json::to_string(&v)
    .unwrap_or_else(|_| "<unserializable tool output>".into());
Response::ok(
    id,
    serde_json::json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false,
    }),
)
```

**The 2026-05-20 patch (`docs/aegis/devlogs/2026-05-20-patch-mcp-call-tool-result.md`).** Before this patch (a bug present since Sprint-03, 2026-05-10, surviving v0.1.0 and v0.2.0), `dispatch_tool_call` returned the handler's raw JSON directly as the JSON-RPC `result`. MCP clients look for `result.content[0].text`; finding `undefined`, every tool call rendered as **"completed with no output"** even though the DB and handlers worked. The bug was caught by Orlando on 2026-05-20 during the CivicSys Syscoin hackathon when three calls (`seele_doctor`, `seele_version`, `seele_stats`) from Claude Code 4.7 produced no visible output; a `printf | seele mcp` wire probe confirmed `result` lacked `content[]`. The fix wraps every successful handler output as a single `text` content block holding the JSON-stringified value, with `isError: false`. The devlog records several deliberate decisions:

- **One `text` block, JSON-stringified**: idiomatic for structured-JSON tools pre-2025-06-18; the spec also allows `image`/`audio`/`embeddedResource` blocks but a single stringified `text` block is the ecosystem convention.
- **`unwrap_or_else` not `?`/`unwrap`**: although `Value` is always round-trippable to a string, a panic here would kill the entire MCP session. The visible fallback `"<unserializable tool output>"` is preferred over a process crash.
- **Errors stay on the JSON-RPC `error` channel** (unchanged). The spec also permits domain errors via `isError: true` in the envelope; migrating to that is a deferred follow-up.
- **`structuredContent` (MCP 2025-06-18) deferred**: it would require bumping the advertised `protocolVersion`, which older clients (some Cursor versions) might reject at handshake. Deferred to v0.3.

The patch shipped a new test file (`call_tool_result_envelope.rs`) verifying the envelope shape with `seele_doctor` and `seele_version`, and migrated five assertions in `stdio_e2e.rs` to a `tool_result()` helper that unwraps `r["result"]["content"][0]["text"]` and re-parses it. The crate's test count went to 20/20 green. The verification round also surfaced (but did not cause) pre-existing failures: OpenAPI drift for `/chat` + `/chat/info` and 10 drifted `seele-tui` insta snapshots.

### 8.4 `tools.rs` — the registry, ToolError, and the ADR-13 prefix mapping

`ToolHandler` (`tools.rs:60`) is a function-pointer type: `fn(&SeeleService, Value) -> Result<Value, ToolError>`. Using `fn` (not a boxed closure) keeps `Tool` `Clone` and the registry zero-allocation per entry. `Tool` (`tools.rs:62`) bundles `name: &'static str`, `description: &'static str`, `input_schema: Value`, and `handler: ToolHandler`. `ToolDescriptor<'a>` (`tools.rs:70`) is the serialized form sent on the wire, with `input_schema` renamed to `inputSchema`.

`ToolError` (`tools.rs:18`) is the crate's `thiserror` enum:

| Variant | `to_jsonrpc()` code | Source mapping |
|---------|---------------------|----------------|
| `BadParams(String)` | `INVALID_PARAMS` (-32602) | `ApiError::BadRequest`, `serde_json::Error`, local validation. |
| `NotFound(String)` | `TOOL_ERROR` (1001) | `ApiError::NotFound`. |
| `Conflict(String)` | `TOOL_ERROR` (1001) | `ApiError::Conflict`. |
| `Internal(String)` | `INTERNAL_ERROR` (-32603) | `ApiError::Internal`, `ApiError::Unauthorized`. |

Two `From` impls bridge the boundary. `From<seele_http::ApiError>` (`tools.rs:41`) maps the service-layer error enum: `BadRequest→BadParams`, `NotFound→NotFound`, `Conflict→Conflict`, `Internal→Internal`, and — notably — `Unauthorized→Internal("unauthorized")` (MCP has no auth concept, so the HTTP-only `Unauthorized` is collapsed to an internal error). `From<serde_json::Error>` (`tools.rs:54`) maps deserialization failures to `BadParams("json error: …")`, which is what lets handlers use `serde_json::from_value(params)?` and have a malformed payload surface as `-32602`.

`all_tools()` (`tools.rs:79`) is the static registry — a `Vec<Tool>` of exactly 19 entries, "never mutated at runtime." `build_index(prefix)` (`tools.rs:205`) converts it into the `HashMap<String, Tool>` used by the server, applying the prefix policy:

```rust
// crates/seele-mcp/src/tools.rs:208
let name = match prefix {
    None => tool.name.to_string(),
    Some("seele") => tool.name.to_string(),
    Some(p) => translate_name(tool.name, p),
};
```

So both `None` and the explicit `"seele"` prefix yield canonical names. **ADR-13 mapping** lives in `RENAMES` + `translate_name` (`tools.rs:223`):

```rust
// crates/seele-mcp/src/tools.rs:223
const RENAMES: &[(&str, &[(&str, &str)])] = &[("mnema", &[("search", "recall")])];
```

`translate_name(canonical, prefix)` strips the `seele_` prefix from the canonical name to get the suffix, looks for a `(prefix, [(from_suffix, to_suffix)])` row, applies any suffix rename, and formats `"{prefix}_{renamed}"`. For `prefix = "mnema"`, `seele_save → mnema_save`, but `seele_search → mnema_recall` because ENGRAM used "recall" rather than "search". Every other tool simply gets the `mnema_` prefix with its suffix unchanged. This is a table-driven design explicitly so that future consumers add a row instead of an `if` chain. The mapping is verified by `tools_list_with_mnema_prefix_aliases_recall` (expects both `mnema_save` and `mnema_recall`, and no `seele_*` names) and by `tools_call_under_mnema_prefix_routes_to_canonical_handler` (a `mnema_save` call routes to the canonical `memories::save` handler). The CLI flag that drives this is `--tool-prefix` (`crates/seele-cli/src/commands/mcp.rs:11`), passed straight into `McpServerConfig.tool_prefix`.

The remainder of `tools.rs` is the `schema_*()` helpers (`tools.rs:239`–415), each returning a `serde_json::Value` JSON Schema object. Shared helpers: `empty_object_schema()` (`{type:object, properties:{}, additionalProperties:false}`) and `schema_id_only(field)` (one required string ULID field). The rest are per-tool.

### 8.5 The 19 tools — schemas, handlers, effects

All handlers reach storage/search through `SeeleService` methods (e.g. `save_observation`, `search_observations`, `get_observation`, `create_link`, `judge_relation`, `stats`, `embedder_info`). They convert JSON params into the same `seele_http::dto` request structs the HTTP handlers use, call the service, and serialize the response DTO back to `Value`. Several handlers parse a ULID via a local `parse_id` (duplicated in `memories.rs`, `sessions.rs`, `relations.rs`) that fails with `BadParams("invalid id: …")`.

| # | Tool | Required params | Optional params | Handler | Effect / returns |
|---|------|-----------------|-----------------|---------|------------------|
| 1 | `seele_save` | `title`, `content` | `type`(default memory), `project`, `scope`(project\|personal), `topic_key`, `session_id`, `tool_name`, `metadata` | `memories::save` | Save observation; computes embedding, strips `<private>`, topic-key upsert + dedup. Returns `SaveResponse` (`id`, `outcome`, ...). |
| 2 | `seele_search` | — | `query`, `project`, `scope`, `type`, `limit`, `include_purist`, `include_annotations`, `score_boost_multiplier`, `max_vec_distance` | `memories::search` | Calls `enforce_search_query_or_filter` then hybrid FTS+vec+RRF. Empty query w/o filter → `BadParams` (-32602). |
| 3 | `seele_show` | `id` | — | `memories::show` | Fetch one observation; absent → `NotFound("observation {id}")` (1001). |
| 4 | `seele_list` | — | `project`, `scope`, `type`, `topic_key`, `session_id`, `limit`, `include_deleted` | `memories::list` | List observations (`Vec<ObservationDto>`). |
| 5 | `seele_update_metadata` | `id`, `metadata_patch`(object) | — | `memories::update_metadata` | Merge patch into metadata JSON. Rejects non-object patch. Returns `{ok, id}`. |
| 6 | `seele_delete` | `id` | — | `memories::soft_delete` | Soft-delete (sets `deleted_at`). Returns `{ok, id}`. |
| 7 | `seele_restore` | `id` | — | `memories::restore` | Clears `deleted_at`. Returns `{ok, id}`. |
| 8 | `seele_link` | `from_id`, `to_id`, `link_type` | `metadata` | `memories::link` | Create a typed link. Returns `LinkDto`. |
| 9 | `seele_stats` | — | — | `meta::stats` | Aggregate stats (`StatsResponse`). |
| 10 | `seele_session_start` | `project` | `directory` | `sessions::start` | Start a session. Returns `SessionDto`. |
| 11 | `seele_session_end` | `id` | `summary` | `sessions::end` | End active session. Returns `{ok, id}`. |
| 12 | `seele_session_summary` | `session_id`, `title`, `summary` | `project` | `sessions::summary` | Saves a `type=memory` observation with `topic_key="session/<id>"` (upserts in-place on repeat) + `tool_name="seele_session_summary"`. Returns `SaveResponse`. |
| 13 | `seele_capture_passive` | `transcript` | `project`, `session_id` | `sessions::capture_passive` | Parses `## Key Learnings` block, saves each bullet as `type=learning`. Returns `{saved:[ids], count}`. |
| 14 | `seele_judge` | `relation_id`, `status`(pending\|judged\|orphaned\|ignored) | `reason`, `evidence`, `confidence` | `relations::judge` | Apply judgment to a `memory_relation`. Returns `{ok, id}`. |
| 15 | `seele_compare` | `sync_id`, `source_id`, `target_id` | `reason` | `relations::compare` | Create a `conflicts_with` relation (pending), `marked_by_actor="seele_compare"`, `marked_by_kind="tool"`. Returns `RelationDto`. |
| 16 | `seele_suggest_topic_key` | `title` | `content` | `meta::suggest_topic_key` | Keyword-scores against 7 hard-coded families; returns `{family, score, suggestion:"family/auto"}` or `{family:null, suggestion:null}`. Pure — ignores the service. |
| 17 | `seele_projects` | — | — | `meta::projects` | Distinct active projects → `{projects:[...]}`. |
| 18 | `seele_doctor` | — | — | `meta::doctor` | `{status:"ok", embedder:{model_id,dim,expected_sha256}, observations_active, sessions_total, schema_version}` (schema_version is `CARGO_PKG_VERSION`). |
| 19 | `seele_version` | — | — | `meta::version` | `{name:"seele", version:CARGO_PKG_VERSION}`. Pure — ignores the service. |

Three handlers carry non-trivial logic worth detailing:

**`sessions::capture_passive`** (`sessions.rs:68`) extracts learnings via `extract_key_learnings` (`sessions.rs:111`): it scans lines, toggles `in_section` when it hits a `## ` heading whose lowercased text equals `"key learnings"` or `"key learnings:"`, treats `- ` / `* ` prefixed lines as new bullets, and folds non-bullet continuation lines into the current bullet (joined by a space). It stops at the next `## ` heading or EOF. Empty transcript → `{saved:[], count:0}` early-return. Each learning becomes a `SaveRequest` with `r#type:"learning"`, `topic_key:None`, `tool_name:"seele_capture_passive"`, and a `truncate_title` (120-char cap with an ellipsis). Unit tests at `sessions.rs:175` pin the parsing behavior.

**`relations::compare`** (`relations.rs:46`) hard-codes `relation:"conflicts_with"` and the actor/kind provenance fields, leaving `evidence`/`confidence`/`session_id` as `None` — it is a thin "these two conflict, pending judgment" marker.

**`meta::suggest_topic_key`** (`meta.rs:44`) is the only handler with embedded domain heuristics: a `&[(&str, &[&str])]` of 7 ENGRAM-inherited families (architecture, bug, decision, pattern, config, discovery, learning), each scored by counting keyword substring hits in the lowercased `title + content`. The highest non-zero score wins; below the noise floor it returns nulls. It takes `&SeeleService` but never uses it (the test constructs a dummy in-memory service).

### 8.6 Boundary, concurrency, and gotchas

**How handlers reach storage/search.** `seele-mcp` does not own a pool or stores; it holds a `SeeleService` (constructed by the CLI's `build_service`, `crates/seele-cli/src/commands/mcp.rs:15`). `SeeleService::new(pool, embedder)` (`crates/seele-http/src/service.rs:47`) assembles the `ObservationStore`, `SessionStore`, `LinkStore`, `RelationStore`, `PromptStore`, `ChunkStore`, and a `SearchEngine`. So the MCP server, HTTP server, and (indirectly) the CLI all funnel through the *same* service code — this is why the 2026-05-20 patch correctly notes the DB and handlers always worked; only the wire envelope was wrong. The anti-exfiltration gate `enforce_search_query_or_filter` (`service.rs:442`, flagged by Cloven 2026-05-10) is shared between HTTP `/search` and MCP `seele_search`; `memories::search` calls it explicitly before invoking the service.

**Concurrency / async / locking.** The only async surface is `run_io`'s read/write loop; everything inside `handle_line` and every handler is synchronous and blocking. Requests are processed strictly one at a time, in order, on a single task. There is no `Mutex`/`RwLock` in this crate — connection pooling and any locking live in `seele-storage`'s `Pool`. A slow handler (e.g. an embedding computation in `seele_save`) blocks the loop until it completes; there is no per-request timeout. Because handlers run on the tokio runtime thread without `spawn_blocking`, a long synchronous DB/embed call could stall the reactor, though in practice the MCP server is single-client and serial so this is acceptable.

**Error handling and failure modes.** Protocol-level failures (bad JSON, unknown method, missing `params.name`, unknown tool) become JSON-RPC `error` objects with the codes in §8.2. Domain failures bubble as `ToolError` → `to_jsonrpc()` → `error`. The success envelope's `serde_json::to_string` uses a defensive `unwrap_or_else` fallback to avoid panicking the whole session. There is no `isError: true` path yet — all domain errors travel as JSON-RPC `error` (the devlog lists "domain errors via `isError: true`" as a deferred follow-up, since clients render an in-chat message better than a bare error code).

**Edge cases / invariants / known limitations:**

- **`protocolVersion` is hard-coded `"2024-11-05"`** and never negotiated against the client's requested version. Bumping to `2025-06-18` (for `structuredContent`) is explicitly deferred to v0.3 (devlog §"Por qué NO migrar").
- **`jsonrpc` version is not validated** (`jsonrpc.rs:13` `#[allow(dead_code)]`) — a request claiming `"jsonrpc":"1.0"` is accepted.
- **`INVALID_REQUEST` (-32600) is defined but never emitted.**
- **Unknown tool yields `-32602` (INVALID_PARAMS), not `-32601`** — a subtle but intentional distinction (the `tools/call` method exists; the *tool* doesn't).
- **`tools/list` is sorted by name** for determinism; tests rely on the count being exactly 19.
- **No explicit `shutdown` method**; the server stops only on stdin EOF.
- **No batching**: JSON-RPC batch arrays are not supported — each line must be a single object (a batch array would fail to deserialize into `Request` and return a parse error).
- **`parse_id` is duplicated** across three `tool_impls` modules — a minor DRY wart, not a bug.
- The crate `Cargo.toml` lists `tracing` as a dependency, but the dispatch path emits no tracing spans/events itself (tracing surfaces in the underlying `SeeleService`, e.g. the best-effort embedding warning at `service.rs:101`).

**Tests.** `tests/stdio_e2e.rs` drives the full server over `tokio::io::duplex(64*1024)` and a `round_trip` helper; it covers `initialize`, the 19-tool list, the mnema alias list, method-not-found, parse-error-with-null-id, the notification no-response invariant, a save→search round trip, the empty-query tool error, unknown-tool, mnema routing, doctor, and capture_passive. `tests/call_tool_result_envelope.rs` adds the two envelope-shape guards from the patch. The devlog's own follow-up flags that all these tests use an in-memory duplex pipe, not a real OS pipe to a subprocess, so a true end-to-end test against a real MCP client remains a deferred ticket.


---

## 9. HTTP REST API — `seele-http`

`seele-http` is the axum-based REST transport for SEELE. It is also, structurally, the **home of the shared application service** (`SeeleService`) that every other transport reuses: the MCP server (`seele-mcp`), the TUI (`seele-tui`), and the CLI. The crate therefore plays a dual role — it is both a leaf-ish HTTP adapter and the application-layer core that sits between the storage/search/embedder crates and the outward-facing interfaces.

### 9.1 Purpose and place in the crate graph

Per the workspace layering, `seele-http` depends on `seele-core`, `seele-storage`, `seele-search`, `seele-embedder`, and `seele-chat` (see `crates/seele-http/Cargo.toml`, lines 11–15). It is depended upon by `seele-mcp` (which imports `SeeleService`, the DTOs, the `enforce_search_query_or_filter` gate, and `ApiError` — `crates/seele-mcp/Cargo.toml`, line 15), by `seele-tui` (which uses `SeeleService` and the DTOs **directly, in-process** — `crates/seele-tui/Cargo.toml`, line 14), and by `seele-cli` (the binary, whose `serve` subcommand builds and runs the `Server`). The crate doc spells out the three public surfaces (`crates/seele-http/src/lib.rs`, lines 1–21):

- `SeeleService` — application-layer service used by both HTTP handlers and MCP tools.
- `Server` / `ServerConfig` / `AppState` — the router builder and binder (re-exported from `server`; `ChatProviderConfig` is **not** re-exported at crate root and must be reached via `seele_http::server::ChatProviderConfig`).
- `ApiError` / `ErrorBody` / `Result` — the uniform JSON error envelope.

External crates and their roles: `axum` (HTTP framework + router + extractors), `tower`/`tower-http` (CORS, trace, gzip compression middleware), `utoipa` + `utoipa-swagger-ui` (OpenAPI doc + Swagger UI), `serde`/`serde_json` (DTO (de)serialization), `thiserror` (typed `ApiError`), `tracing` (structured logs), `chrono` (timestamp conversion in DTOs), `tokio` (async runtime + `TcpListener`), `anyhow` (only on `Server::run`'s return type). Dev-deps (`Cargo.toml`, lines 30–32) add `tempfile` (per-test temp DB dir), `reqwest` (in-process HTTP client for E2E tests) and `regex` (the anti-drift OpenAPI/router consistency test).

### 9.2 File-by-file map

- **`lib.rs`** — module declarations (`auth`, `dto`, `error`, `handlers`, `openapi`, `server`, `service`) and the re-export surface. Nothing else.
- **`server.rs`** — `AppState`, `ServerConfig`, `ChatProviderConfig`, the `Server` struct, `Server::router()` (router + middleware assembly), `Server::run()` (bind + serve), and the two public handlers `health` / `version`.
- **`service.rs`** — `SeeleService` (the application service holding all stores + search + embedder + pool), all of its business methods, the free function `enforce_search_query_or_filter`, and the private `ArcEmbedder` adapter.
- **`handlers.rs`** — one async axum handler per endpoint, plus the chat handler (`chat`, `chat_info`), the `ChatRequest`/`ChatResponse`/`ChatInfoResponse`/`ConflictsQuery` local types, and the `default_model_for`/`default_endpoint_for` helpers.
- **`dto.rs`** — every request/response DTO, their `From` conversions from core types, and the parsing helpers (`parse_id`, `parse_scope`, `parse_type`, `parse_metadata`, `parse_session_status`, `parse_relation_kind`, `parse_judgment_status`).
- **`auth.rs`** — `check_bearer` and the `require_bearer` middleware.
- **`error.rs`** — `ApiError` enum, `ErrorBody`, `IntoResponse` mapping, and `From` impls from each inner error tree.
- **`openapi.rs`** — `SchemaRegistry` (utoipa schema derive), `build_openapi`, hand-authored `build_paths`, the path-builder DSL helpers, and `routes()` (Swagger UI mount).
- **`tests/`** — six integration test files: `skeleton.rs` (health/version/404), `handlers_basicos.rs`, `handlers_c1_lifecycle.rs`, `handlers_c2_relations_stats.rs`, `handlers_d_auth_openapi.rs` (auth + OpenAPI + legacy paths), `openapi_consistency.rs` (router↔spec anti-drift).

### 9.3 `service.rs` — the shared application service

`SeeleService` (`service.rs`, lines 33–44) is `#[derive(Clone)]` and aggregates one store per domain plus the search engine, embedder, and pool:

```rust
// service.rs:33
#[derive(Clone)]
pub struct SeeleService {
    pub observations: ObservationStore,
    pub sessions: SessionStore,
    pub links: LinkStore,
    pub relations: RelationStore,
    pub prompts: PromptStore,
    pub chunks: ChunkStore,
    pub search: Arc<SearchEngine>,
    pub embedder: Arc<dyn Embedder>,
    pub pool: Pool,
}
```

`SeeleService::new(pool, embedder)` (lines 47–67) constructs each store over a clone of the `Pool` (cheap — the pool is itself a clone-shareable handle), wraps the search engine in `Arc`, and bridges the shared `Arc<dyn Embedder>` into the `Box<dyn Embedder>` that `SearchEngine::new` requires via the private `ArcEmbedder(Arc<dyn Embedder>)` adapter (struct at line 458, `impl Embedder` at lines 460–476). This adapter exists because `SearchEngine` currently demands an *owned* `Box<dyn Embedder>` while callers want to share one embedder instance; the adapter delegates `embed`, `embed_batch`, `dim`, `model_id`, and `expected_sha256` straight through to the inner `Arc`. The net effect is that one model instance serves both the post-save embedding write and the search-time query embedding. Note: `new` clones the supplied `embedder` once into the `ArcEmbedder` (line 54) and also stores the original `Arc<dyn Embedder>` on the struct (line 64), so the same instance is reachable both directly (for `save_observation`/`embedder_info`) and through the search engine.

The service is the **single point of truth for business logic** so that the HTTP handlers and the MCP tools stay DRY (`service.rs`, lines 1–7). Notable methods:

| Method | Behavior / notes |
|---|---|
| `save_observation(SaveRequest) -> Result<SaveResponse>` (lines 73–132) | Parses `session_id`/`scope`/`type`/`metadata` at the boundary, calls `observations.save(SaveInput)`, then **best-effort** embeds the content and writes the vector (lines 96–107). Embedding failure (or a failed embedding write) logs a `tracing::warn!` but does NOT 5xx — the row is persisted and a reindex can fix the vec branch later. The `SaveOutcome` enum maps to `outcome: "created" \| "upserted_topic" \| "duplicate_merged"` with the optional `revision_count`/`duplicate_count`. |
| `search_observations(SearchRequest) -> Result<SearchResponse>` (lines 136–157) | Builds a `SearchQuery` (mapping `type`→`kind`, parsing `scope`, `per_method_limit: None`, passing through `include_purist`, `score_boost_multiplier`, `max_vec_distance`, `include_annotations`), runs the engine, converts hits to `SearchHitDto`. **Does not** enforce the empty-query gate — that is the transport's job. |
| `get_observation` / `list_observations` / `soft_delete_observation` / `restore_observation` | Thin pass-throughs to `ObservationStore`. `list_observations` maps the query DTO to `ObservationQuery` including `include_deleted` and parses `scope`/`session_id`/`type` at the boundary. |
| `merge_observation_metadata(id, patch)` (lines 200–226) | Read-modify-write: fetches the row (404 via `ApiError::NotFound` if absent), shallow-merges the patch object into existing metadata (new keys overwrite; nested objects are NOT recursively merged), writes back via `ObservationPatch`. Errors `Internal("metadata is not an object")` if the stored metadata is non-object. (Note: this method has no dedicated HTTP route in `server.rs` — it is exposed via the MCP `seele_update_metadata` tool, not the REST router.) |
| `list_projects()` (lines 229–231) | Distinct active project names. (Likewise has no dedicated HTTP route; reached via the MCP `seele_projects` tool / CLI.) |
| `start_session` / `end_session` / `abort_session` / `get_session` / `list_sessions` | Session lifecycle over `SessionStore`. `list_sessions` parses the `status` filter strictly. |
| `create_link` / `list_links_for_observation` / `delete_link` | Links. `create_link` rejects empty `link_type` (after `trim`) with a 400 (lines 273–275). `list_links_for_observation` queries both `from_id` and `to_id` sides, then **dedups by link id** through a `BTreeMap<String, Link>` so a self-link is not returned twice (lines 286–302). |
| `create_relation` / `list_relations` / `judge_relation` / `list_pending_conflicts` | Relations. `create_relation` validates non-empty `sync_id` (after `trim`), parses both endpoint IDs, and rejects `source_id == target_id` with a 400 (lines 311–321). `list_pending_conflicts` is convenience sugar for `relation=conflicts_with & status=pending` (lines 384–392). |
| `stats()` (lines 396–427) | Aggregates `ObservationStats` (active/deleted/projects/by_type/by_scope) and `SessionStats` (total/by_status) into `StatsResponse`. |
| `embedder_info()` (lines 429–435) | Returns model id, dim, expected sha256 — infallible, returns `EmbedderInfo` directly (not `Result`). |

`enforce_search_query_or_filter(&SearchRequest)` (lines 442–453) is the **anti-exfiltration gate** shared by HTTP `/search` and MCP `seele_search`. It returns `Err(ApiError::BadRequest)` when the query text is empty (after `trim`) **and** none of `project`/`scope`/`type` is present, mitigating a list-all-DB vector. The doc comment attributes this to Cloven's 2026-05-10 review (lines 438–441). The HTTP handler calls it before delegating (`handlers.rs`, line 35); the MCP search tool calls it identically (`crates/seele-mcp/src/tool_impls/memories.rs`, line 28); the TUI also imports it (`crates/seele-tui/src/app.rs`, line 8).

### 9.4 `server.rs` — router, state, middleware, binding

**`AppState`** (lines 27–31) is the `#[derive(Clone)]` state every handler sees. It holds `service: Arc<SeeleService>` and `chat: Option<Arc<ChatProviderConfig>>`. Two `FromRef<AppState>` impls (lines 33–43) let handlers extract either `State<Arc<SeeleService>>` (the common case) or `State<Option<Arc<ChatProviderConfig>>>` (chat handlers) from the single composite state — the `chat` handler pulls both via two `State(...)` extractors.

**`ServerConfig`** (lines 45–57): `addr: SocketAddr`, `cors_origins: Vec<String>`, `auth_bearer: Option<String>`, `legacy_engram_paths: bool`, `chat: Option<ChatProviderConfig>`. The doc comment states the precise auth carve-out: when `auth_bearer` is set, *every route except `/health`, `/version`, `/docs/*`, `/openapi.json`* requires the bearer token (lines 49–50). `ServerConfig::loopback(port)` (lines 76–85) is a convenience constructor (127.0.0.1, no CORS, no auth, no legacy, no chat).

**`ChatProviderConfig`** (lines 61–73): `provider`, `api_key`, `model`, `endpoint: Option<String>`. The comment emphasizes the secret never leaves the box (lines 59–60) and that any provider label other than `"anthropic"` is treated as OpenAI-compatible using `endpoint` as the `/v1/chat/completions` URL (lines 63–66, 70–72).

**`Server::router()`** (lines 99–182) is the heart of the file and is `pub` so tests can mount it against an in-process listener. Control flow, in order:

1. Build `AppState` from the service (`Arc::new(self.service.clone())`) and the optional chat config mapped into `Arc` (lines 100–103).
2. Construct the CORS layer (lines 105–117): if `cors_origins` is empty, a bare `CorsLayer::new()` (effectively CORS-off for browsers); otherwise a **permissive** layer with `allow_origin(Any).allow_methods(Any).allow_headers(Any)`. The comment explains the explicit methods/headers are required so a browser JSON `POST` with `Content-Type` clears preflight — without them it fails with "failed to fetch" (lines 108–116). A per-origin allowlist is explicitly deferred ("Block D refines... today we open up permissively").
3. Build the **protected** sub-router (lines 122–152) — 22 operations across 16 canonical (non-legacy) paths. This is built first so the auth middleware wraps *only* these routes.
4. If `legacy_engram_paths` is set, append two ENGRAM aliases — `POST /save` (→ `save_memory`) and `GET /show/{id}` (→ `get_memory`) — to the protected router (lines 154–161). The comment (ADR-13) notes the new SEELE endpoints (sessions, relations, etc.) are deliberately NOT exposed under legacy paths because they don't exist in ENGRAM.
5. Attach state with `.with_state(state)` (line 163).
6. Conditionally wrap the protected router in the bearer middleware (lines 165–172): when `auth_bearer` is `Some(token)`, a `middleware::from_fn` closure clones the token per-request and calls `require_bearer(token, req, next)`; otherwise the protected router is unchanged.
7. Assemble the **outer** router (lines 174–182): public `/health` + `/version`, then `.merge(protected)`, then `.merge(openapi::routes())` (Swagger UI + `/openapi.json`), then the global layers in order: `TraceLayer::new_for_http()`, the CORS layer, `CompressionLayer::new()` (gzip). Because `/health`, `/version`, the docs, and the spec are merged *outside* the protected block, they are never wrapped by the auth middleware — the structural reason they stay public.

The route table inside the protected router uses axum 0.8 path syntax (`{id}` curly braces, not `:id`):

| Path | Methods → handlers |
|---|---|
| `/memories` | `POST save_memory`, `GET list_memories` |
| `/memories/{id}` | `GET get_memory`, `DELETE soft_delete_memory` |
| `/memories/{id}/restore` | `POST restore_memory` |
| `/memories/{id}/links` | `GET list_links_for_memory` |
| `/search` | `POST search_memories` |
| `/sessions` | `POST start_session`, `GET list_sessions` |
| `/sessions/{id}` | `GET get_session` |
| `/sessions/{id}/end` | `PUT end_session` |
| `/sessions/{id}/abort` | `PUT abort_session` |
| `/links` | `POST create_link` |
| `/links/{id}` | `DELETE delete_link` |
| `/relations` | `POST create_relation`, `GET list_relations` |
| `/relations/{id}/judge` | `PUT judge_relation` |
| `/conflicts` | `GET list_pending_conflicts` |
| `/stats` | `GET get_stats` |
| `/embedder` | `GET get_embedder_info` |
| `/chat` | `POST chat` |
| `/chat/info` | `GET chat_info` |
| `/save` (legacy) | `POST save_memory` |
| `/show/{id}` (legacy) | `GET get_memory` |

Counting operations: **22 protected operations across 18 protected paths** (16 paths × their methods sum to 22 operations including the two `/chat*` routes), plus 2 public (`/health`, `/version`), plus the docs/spec routes, plus 2 legacy aliases when enabled. (`/chat` and `/chat/info` are always *routed* but the `chat` handler errors with a 400 if no provider config is resolvable.)

**`Server::run()`** (lines 190–198) binds a `tokio::net::TcpListener` to `config.addr`, reads back `local_addr()`, and emits a **single grep-able line to stderr**: `seele http listening on http://<addr>` via `eprintln!` (line 194). The doc comment (lines 184–189) explains this exists so `--port 0` callers (tests, scripts) can discover the OS-chosen port without a listener-then-drop race. It then logs the same via `tracing::info!` (line 195) and calls `axum::serve(listener, app).await` (line 196). There is **no explicit graceful-shutdown wiring** (no `with_graceful_shutdown`) — the future runs until the process is killed; `run` returns `anyhow::Result<()>`.

The two public handlers are trivial: `health` returns `Json({"status":"ok"})` (lines 201–203); `version` returns `Json({"name":"seele","version": env!("CARGO_PKG_VERSION")})` (lines 205–210).

### 9.5 `auth.rs` — bearer token scheme

Two functions implement the opt-in scheme. `check_bearer(headers, expected_token)` (lines 17–32) looks up the `authorization` header (it calls `.get("authorization").or_else(|| .get("Authorization"))`; note that `http::HeaderMap` keys are already case-insensitive, so the second lookup is effectively redundant), reads it as a str, strips the **mandatory** `"Bearer "` prefix, trims surrounding whitespace from the remaining token via `str::trim`, and compares it for exact equality to the expected token. Any missing header, non-`to_str`-able header value, missing `Bearer ` prefix, or mismatch yields `Err(StatusCode::UNAUTHORIZED)` (401). The token comparison is a plain `==` on `&str` — **not constant-time**, a potential timing side-channel (no `subtle`/constant-time crate is used).

```rust
// auth.rs:23
let provided = header
    .strip_prefix("Bearer ")
    .map(str::trim)
    .ok_or(StatusCode::UNAUTHORIZED)?;
```

`require_bearer(expected_token: String, req, next) -> Result<Response, StatusCode>` (lines 37–44) is the middleware: it calls `check_bearer` on `req.headers()`, and on success runs `next.run(req).await`. On failure axum turns the bare `StatusCode::UNAUTHORIZED` into a 401 response with an empty body. Note: this 401 path does **not** go through the `ErrorBody` envelope — auth-rejected requests get a plain 401 with no JSON body, unlike domain errors. The module doc (lines 1–6) restates the carve-out, but only mentions `/health` and `/version` as public (the full four-route carve-out including `/docs/*` and `/openapi.json` is documented on `ServerConfig.auth_bearer` in `server.rs`).

Tests confirm the contract (`handlers_d_auth_openapi.rs`): auth disabled by default (200 without token), missing token → 401, wrong token → 401, valid token → 200, and `/health`, `/version`, `/openapi.json` remain public under auth. Legacy `/save` also respects auth (it lives in the protected block) — see `legacy_paths_respect_auth_bearer`. The CLI surfaces this as `--auth-bearer <token>` (`crates/seele-cli/src/commands/serve.rs`, line 21).

### 9.6 `dto.rs` — request/response shapes

DTOs are deliberately simpler than core types (`dto.rs`, lines 1–5): IDs are `String` (parsed/validated at the boundary), metadata is `serde_json::Value`, and timestamps are Unix-epoch **milliseconds as `i64`** (via `chrono`'s `timestamp_millis()`), avoiding chrono types on the wire. Every DTO derives `utoipa::ToSchema` for OpenAPI registration.

Key request DTOs:

| DTO | Notable fields / defaults |
|---|---|
| `SaveRequest` | `title`, `content` (required); `type` defaults to `"memory"` via `default_type()`; `project`, `scope` (`"project"`/`"personal"`, default project), `topic_key`, `session_id`, `tool_name`, `metadata` all optional. The doc comment enumerates the 12 canonical types (decision, architecture, bugfix, pattern, config, discovery, learning, memory, skill, advisor_output, review, verdict) or any custom string. |
| `SearchRequest` | `#[derive(Default)]`; `query` defaults to `""`; optional `project`/`scope`/`type`/`limit`/`max_vec_distance`; bools `include_purist`, `include_annotations`; `score_boost_multiplier: f64` (defaults to `0.0` — see gotcha below). |
| `ListRequest` | All-optional filters; `include_deleted: bool`. Used via `Query<ListRequest>`. |
| `SessionStartRequest` | `project` required, `directory` optional. |
| `SessionEndRequest` | `summary` optional. |
| `SessionListQuery` | `project`, `status`, `limit` — used via `Query`. |
| `LinkCreateRequest` | `from_id`, `to_id`, `link_type` required; `metadata` optional. |
| `RelationCreateRequest` | `sync_id`, `source_id`, `target_id`, `relation` required; plus optional `reason`/`evidence`/`confidence`/`marked_by_actor`/`marked_by_kind`/`marked_by_model`/`session_id`. |
| `RelationListQuery` | `source_id`/`target_id`/`relation`/`status`/`limit` — `Query`. |
| `JudgeRequest` | `status` required; `reason`/`evidence`/`confidence` optional. |

Key response DTOs: `SaveResponse` (`id`, `outcome: &'static str`, optional `revision_count`/`duplicate_count`), `SearchResponse` (`hits: Vec<SearchHitDto>`, `count: usize`), `SearchHitDto` (id/title/content/project/scope/type/score, optional `fts_rank`/`vec_rank`, `created_at` ms, `metadata`, `annotations: Vec<AnnotationDto>`), `AnnotationDto` (`kind` mapped from `AnnotationKind` to one of `supersedes`/`superseded_by`/`conflicts_with`/`contested_by`, lines 118–123; plus `other_id`, optional `other_title`/`reason`), `ObservationDto`, `SessionDto`, `LinkDto`, `RelationDto`, `StatsResponse`/`ObservationStats`/`SessionStats`/`CountBucket`, and `EmbedderInfo`. Each has an idiomatic `From<CoreType>` impl converting timestamps and `as_str()` enum labels (note `SearchHitDto` converts from `&SearchHit` by reference, lines 111–144, whereas `ObservationDto`/`SessionDto`/`LinkDto`/`RelationDto` consume an owned core value).

The parsing helpers centralize boundary validation:
- `parse_id(s, label) -> Result<SeeleId, ApiError>` — `s.parse::<SeeleId>()`, mapping failure to `BadRequest("invalid {label}: {e}")` (lines 475–478).
- `parse_scope(Option<&str>)` — `None`→default (`Scope::Project`), `"project"`/`"personal"` accepted, anything else 400 (lines 481–490).
- `parse_type(&str) -> ObservationType` — **infallible**, unknown strings become `ObservationType::Other(...)` via `from_str_relaxed` (lines 494–496).
- `parse_metadata(Value) -> Metadata` — null→empty `Metadata`, otherwise `Metadata::from_value` (lines 499–505).
- `parse_session_status` / `parse_relation_kind` / `parse_judgment_status` (lines 267–275, 410–416, 418–426) — strict parses via the core `from_str_strict` constructors, with descriptive 400 messages enumerating valid values.

The asymmetry is deliberate: observation *type* is relaxed (custom types allowed), but *scope*, *session status*, *relation kind*, and *judgment status* are strict closed sets.

### 9.7 `handlers.rs` — endpoint handlers

Each handler is the thinnest possible wrapper: extract from path/query/body, call a `SeeleService` method, serialize (`handlers.rs`, lines 1–4). They return `Result<Json<T>>` for bodies, `Result<StatusCode>` for no-content actions, or bare `Json<T>` for infallible ones (`get_embedder_info` at lines 194–196, `chat_info` at lines 237–252). Status-code conventions:

- **Mutations that return a body** (`save_memory`, `create_link`, `create_relation`, `start_session`) → **200 OK** with the DTO (not 201 — a deliberate simplification).
- **Lifecycle/no-content actions** (`soft_delete_memory`, `restore_memory`, `end_session`, `abort_session`, `delete_link`, `judge_relation`) → **204 No Content** (`StatusCode::NO_CONTENT`).
- **Reads** → 200 with body, or **404** via `ApiError::NotFound` when `get_memory`/`get_session` find nothing (lines 47, 100).
- **Validation failures** → 400 via the parse helpers / service guards.

`search_memories` (lines 31–38) is the only handler that runs the `enforce_search_query_or_filter` gate before delegating. `list_pending_conflicts` uses a small local `ConflictsQuery { limit: Option<u32> }` query type (struct lines 175–179, handler lines 181–186).

#### Chat endpoint (wiring `seele-chat`)

`/chat` and `/chat/info` integrate the standalone `seele-chat` crate (imports at `handlers.rs` lines 200–205). `chat_info` (lines 237–252) reports `{enabled, provider, model}` from the optional `ChatProviderConfig`, returning `enabled: false` with `provider`/`model` `None` when chat is disabled. The `chat` handler (lines 254–376) is substantial:

1. Pulls both `State<Arc<SeeleService>>` and `State<Option<Arc<ChatProviderConfig>>>`.
2. Resolves provider config in priority order **per-request fields > CLI config** for `provider`, `api_key`, `model`, `endpoint`. Missing `provider` or `api_key` after both sources → `BadRequest` with a panel-oriented message (lines 261–289).
3. Defines a single tool, `seele_search`, with a JSON-schema (`query` required, `limit` default 5/max 20, optional `project`) (lines 290–302).
4. Picks the provider: `"anthropic"` (case-insensitive via `eq_ignore_ascii_case`) → `AnthropicProvider`; anything else → `OpenAICompatibleProvider` using `endpoint_override` or `default_endpoint_for(provider)` (lines 304–315).
5. Builds an async `ToolHandler` closure capturing a cloned `Arc<SeeleService>`. The closure deserializes the model's JSON args, clamps `limit` to ≤20 (`args.limit.unwrap_or(5).min(20)`), constructs a `SearchRequest` (no scope/type filters, `include_purist=false`, `score_boost_multiplier=1.0`), calls `svc.search_observations`, and returns a compact JSON summary truncating each hit's content to 280 chars (lines 317–360).
6. Builds `ChatConfig::default()` (the baked SEELE system prompt) and overrides `config.system_prompt` if `req.system_prompt` is present (lines 362–365).
7. Calls `seele_chat::run_chat(provider.as_ref(), &tool_handler, req.messages, &config).await`, mapping failure to `ApiError::Internal("chat failed: ...")`, and returns `ChatResponse { messages, provider, model }` (lines 367–375).

`default_model_for` / `default_endpoint_for` (lines 378–401) hardcode per-provider defaults for `minimax`, `openai`, `openrouter`, `together`, `groq`, `deepseek`, `anthropic`, falling back to OpenAI's `gpt-4o-mini` / `https://api.openai.com/v1/chat/completions` for any other label. Note `default_model_for` is duplicated verbatim in the CLI's `serve.rs` (lines 97–108 there) — a small DRY gotcha. The `ChatRequest` re-uses `seele_chat::Message` directly as its `messages` field (`handlers.rs` line 209), so the wire format mirrors the chat crate's message model. The `system_prompt` override is one knob exposed beyond per-request provider/key/model/endpoint. Importantly, `/chat` and `/chat/info` are **not** in the OpenAPI spec (see §9.9) — they are routed but undocumented, which currently breaks the consistency test (see §9.9 / §9.11).

### 9.8 `error.rs` — domain-to-HTTP mapping

`ApiError` (lines 14–30) is a `thiserror` enum with five variants: `BadRequest(String)`, `Unauthorized`, `NotFound(String)`, `Conflict(String)`, `Internal(String)`. `ErrorBody` (lines 32–36) is the stable JSON envelope `{ code: &'static str, message: String }` and itself derives `ToSchema` (so it is registered in the OpenAPI components). The `IntoResponse` impl (lines 38–56) maps each variant to a `(StatusCode, code)` pair and serializes the envelope (the `message` is `self.to_string()`, i.e. the `thiserror` `#[error("...")]` text such as `"bad request: ..."`):

| Variant | Status | `code` |
|---|---|---|
| `BadRequest` | 400 | `BAD_REQUEST` |
| `Unauthorized` | 401 | `UNAUTHORIZED` |
| `NotFound` | 404 | `NOT_FOUND` |
| `Conflict` | 409 | `CONFLICT` |
| `Internal` | 500 | `INTERNAL` |

`From` conversions translate each inner error tree (lines 58–96): `StorageError::{NotFound→404, InvalidInput→400, Conflict→409, _→500}`, `SearchError::{InvalidInput→400, _→500}`, `EmbedderError→500` (always `Internal`, prefixed `embedder: `), and `SeeleError::{InvalidInput→400, NotFound→404, Conflict→409, _→500}`. The `?` operator in service/handlers thus auto-maps any inner failure to the right HTTP status. Gotcha: the `ApiError::Unauthorized` variant exists and maps to 401 with an `ErrorBody`, **but the auth middleware never produces it** — `require_bearer` returns a bare `StatusCode::UNAUTHORIZED`, so auth 401s have an empty body while a (hypothetical) handler-returned `Unauthorized` would carry the envelope. (`Unauthorized` is, however, consumed by `seele-mcp`'s `From<ApiError> for ToolError` impl, which maps it to an internal error — `crates/seele-mcp/src/tools.rs`, line 48.) `Result<T>` is the crate alias `std::result::Result<T, ApiError>` (line 98).

### 9.9 `openapi.rs` — OpenAPI + Swagger UI

The strategy (lines 1–7) is hybrid: use the `#[derive(OpenApi)]` macro only to **register component schemas** (the `SchemaRegistry` ZST lists all DTOs — the `schemas(...)` list spans lines 28–50, struct declared at line 51), but author the **paths by hand** in `build_paths()` rather than decorating each handler with `#[utoipa::path]`, to keep the router clean and avoid duplicating signature info.

`build_openapi()` (lines 55–70) calls `SchemaRegistry::openapi()`, sets `info.title = "SEELE HTTP API"`, `info.version = CARGO_PKG_VERSION`, a description noting the `{code, message}` error envelope, a single server `http://127.0.0.1:7777` ("Default loopback"), and replaces `doc.paths` with `build_paths()`.

`build_paths()` (lines 72–346) enumerates every documented path/method with summary, parameters, request body, and response schema using a small DSL of helpers: `json_body::<T>` (required `application/json` request body referencing a schema by **explicit `&str` name argument**), `json_response::<T>(status, desc)` (single object response), `json_array::<T>` (array response), `no_content` (204), `json_ok` (loose 200 for health/version), `path_param`, `query_param`. The schema name in `json_response`/`json_array` is derived at runtime from `std::any::type_name::<T>()` via `rsplit("::").next()` (lines 368–371, 389–392) — a slightly fragile reflection trick. Note that `json_body` does NOT use this reflection; it takes the schema name as a literal `&str` argument (e.g. `json_body::<SaveRequest>("SaveRequest")`), and `schema_ref::<T>` actually discards its `type_name` call (line 351) and uses the passed-in `name`. So a renamed DTO would need its `json_body("Name")` literal updated to avoid a dangling `$ref`, while `json_response`/`json_array` derive the name from the type and would self-correct. The spec documents `/health`, `/version`, and all the canonical data paths, but deliberately **omits** the legacy ENGRAM aliases (ADR-13) and **omits** `/chat`/`/chat/info`. The spec ends up with 20 path keys (`/health`, `/version`, plus 18 canonical data path keys).

`routes()` (lines 454–459) mounts `SwaggerUi::new("/docs").url("/openapi.json", build_openapi())` into an `axum::Router`. The comment (lines 451–453) explains it is merged outside the auth block so no middleware fires for docs — the spec describes schemas only, no data leak. Tests verify Swagger HTML at `/docs/` and JSON at `/openapi.json` (`handlers_d_auth_openapi.rs`).

The **anti-drift** test `openapi_consistency.rs` is a notable safeguard: because axum 0.8 exposes no route introspection, the test regex-parses `src/server.rs` for `.route("…")` literals **up to the `if self.config.legacy_engram_paths` cutoff** (so the two legacy aliases are excluded), adds `/health` and `/version`, and asserts the set is exactly equal to the keys of `/openapi.json`. Adding a route without a matching `paths.path(...)` (or vice versa) fails the test with a `MISSING`/`EXTRA` diff and a remediation hint (lines 91–100). It also guards against vacuous passes by asserting ≥15 paths (lines 106–110).

**This test is currently FAILING.** The `/chat` and `/chat/info` routes are wired at `server.rs` lines 151–152 — *before* the `legacy_engram_paths` cutoff at line 154 — so the regex captures them into the router-side set, but they are absent from `build_paths()`. Running `cargo test -p seele-http --test openapi_consistency` panics with:

```
router has routes not declared in OpenAPI spec: ["/chat/info", "/chat"].
Add a `paths.path("<path>", ...)` entry in crates/seele-http/src/openapi.rs.
```

In other words, the chat routes' deliberate absence from the spec and the consistency test's exact-match contract are in genuine conflict, and the test does not pass as the code stands today. (This is a concrete defect for the improvement pass, not a hypothetical "tension": either add the two `/chat*` paths to `build_paths()`, or move the chat routes below the legacy cutoff / teach the regex to skip them.)

### 9.10 How it connects to the rest of SEELE

- **CLI (`seele serve`)** builds a `SeeleService` via `crate::app::build_service` and constructs `ServerConfig` from flags `--port`, `--bind`, `--legacy-engram-paths`, `--auth-bearer`, repeatable `--cors-allow`, and the `--chat-*` family, then calls `Server::run()` (`crates/seele-cli/src/commands/serve.rs`). The CLI resolves `--chat-key $ENVVAR` to an env lookup via `resolve_key` (the value is read from the named env var only when it starts with `$`, else used literally — lines 88–95) and enforces that `--chat-provider` and `--chat-key` are set together (else `anyhow::bail!`, lines 53–72).
- **MCP (`seele-mcp`)** is the biggest consumer of this crate's *library* surface: it holds a `SeeleService`, reuses the request DTOs (`SaveRequest`, `SearchRequest`, `ListRequest`, `LinkCreateRequest`, etc.), reuses `enforce_search_query_or_filter` (`crates/seele-mcp/src/tool_impls/memories.rs`, lines 5–6, 28), and converts `seele_http::ApiError` into its own `ToolError` (`crates/seele-mcp/src/tools.rs`, lines 41–52). This is the concrete realization of the "shared service layer" promise — MCP tools are thin shims over the same methods the HTTP handlers call. (`seele-mcp`'s tool handlers are *synchronous* `fn(&SeeleService, Value) -> Result<Value, ToolError>` — `tools.rs` line 60 — unlike the async axum handlers.)
- **TUI (`seele-tui`)** also uses the library surface directly **in-process**: it imports `SeeleService`, the DTOs (`ObservationDto`, `SearchHitDto`, `StatsResponse`), and `enforce_search_query_or_filter` (`crates/seele-tui/src/app.rs`, lines 7–9; `lib.rs` line 21 and its module doc explicitly note "no HTTP round trip, no MCP"). The TUI does NOT talk to a running HTTP server.
- **Data crossing the boundary**: JSON DTOs in/out for HTTP; the same Rust DTO structs in-process for MCP and TUI. Inner domain types (`Observation`, `Session`, `Link`, `MemoryRelation`, `SearchHit`) never cross the wire directly — they are always projected through the `From` impls into DTOs.

### 9.11 Edge cases, gotchas, and limitations

- **`score_boost_multiplier` default is `0.0`.** `SearchRequest` derives `Default` and uses `#[serde(default)]` on `score_boost_multiplier: f64` (`dto.rs`, lines 71–72), so an omitted field yields `0.0`, not `1.0`. Whether the search engine treats `0.0` as "no boost" or as "zero out boosted scores" depends on `seele-search` semantics; callers who want a neutral multiplier must send `1.0` explicitly (the chat tool handler does, `handlers.rs` line 339). This is a likely foot-gun.
- **Embedding is best-effort and post-save.** A failing embedder (or a failing embedding write) yields a persisted row with no vector and only a `tracing::warn!` (`service.rs`, lines 96–107); the row is invisible to the vec branch of search until reindexed. No reindex endpoint exists in this crate.
- **No 201/Location on create.** All creates return 200 with the resource body, not 201 Created.
- **Auth is not constant-time** and emits empty-body 401s (no `ErrorBody`), diverging from the documented envelope contract.
- **CORS is all-or-nothing.** Any non-empty `cors_origins` opens `Access-Control-Allow-Origin: *` — the per-origin allowlist is explicitly on the backlog (`server.rs` comment lines 108–116; CLI flag doc lines 22–25).
- **No graceful shutdown** is wired into `Server::run`.
- **Schema-name handling** in `openapi.rs` is split: `json_body` takes an explicit `&str` literal (renaming a DTO without updating the literal silently produces a dangling `$ref`), while `json_response`/`json_array` derive the name via `type_name::<T>()` string-splitting (self-correcting on rename, but fragile if a type's path-tail collides).
- **`/chat` and `/chat/info` are undocumented in OpenAPI**, and because they are routed *before* the legacy cutoff the `openapi_consistency` test currently **fails** with `router has routes not declared in OpenAPI spec: ["/chat/info", "/chat"]` (verified via `cargo test -p seele-http --test openapi_consistency`). This is a live defect, not a latent one.
- **`merge_observation_metadata` and `list_projects` have no REST routes.** Both service methods exist but are exposed only through the MCP tool surface (and `list_projects` via the CLI); there is no `PATCH /memories/{id}/metadata` or `GET /projects` in the router.
- **No rate limiting, request-size cap, or body-size limit** is configured beyond axum/tower defaults.


---

## 10. Multi-Provider Chat Backend — `seele-chat`

`seele-chat` is the smallest non-trivial crate in the workspace and the only one that talks to *remote* LLM APIs. Its single job is the **chat-with-DB** feature surfaced on the web observability page: a user asks a natural-language question, a remote model decides when to call a search tool, the server runs the search against the local SQLite memory, feeds the results back to the model, and the model produces a grounded answer. The crate owns the provider wire formats and the tool-use loop; it owns nothing about *what* the tools do. Per its own doc comment, "The crate is intentionally storage-agnostic: tool execution is supplied by the caller via a [`ToolHandler`] closure, so `seele-chat` can be reused by other binaries that bind their own tools" (`crates/seele-chat/src/lib.rs:7-9`).

### Position in the crate graph

`seele-chat` is a **leaf with no internal dependencies** — its `Cargo.toml` lists only `tokio`, `async-trait`, `reqwest`, `serde`, `serde_json`, `thiserror`, and `tracing` (`crates/seele-chat/Cargo.toml:11-18`). It does not depend on `seele-core`, `seele-storage`, or `seele-search`; its message and tool types are defined locally rather than reusing domain types, which is what keeps it storage-agnostic. The single Rust consumer is **`seele-http`** (`crates/seele-http/Cargo.toml:15` → `seele-chat = { path = "../seele-chat" }`), which wires the search tool to its own `SeeleService`. The whole crate lives in one file, `lib.rs` (476 lines). Note that `seele-mcp` also defines a type literally named `ToolHandler` (`crates/seele-mcp/src/tools.rs:60`, a `fn(&SeeleService, Value) -> Result<Value, ToolError>` pointer), but that is an unrelated, synchronous tool dispatcher and `seele-mcp` does **not** depend on `seele-chat`.

### Public API surface

| Item | Kind | Purpose |
|------|------|---------|
| `Message` | struct | One conversation turn (role/content/tool_calls/tool_call_id/name) |
| `ToolCall` | struct | A structured tool invocation requested by the assistant |
| `ToolCallFunction` | struct | `{ name, arguments }` — arguments are a JSON-encoded *string* |
| `ChatProvider` | `#[async_trait]` trait | Provider abstraction: `complete`, `name`, `model` |
| `ToolSpec` | struct | A tool advertised to the model (`name`, `description`, JSON-schema `parameters`) |
| `ToolHandler` | type alias | Boxed async closure `Fn(String) -> Future<Result<String, String>>` |
| `ChatConfig` | struct | `system_prompt` + `max_iterations` for one run |
| `OpenAICompatibleProvider` | struct + impl | `/v1/chat/completions` provider |
| `AnthropicProvider` | struct + impl | `/v1/messages` provider |
| `run_chat` | async fn | The tool-use orchestrator loop |
| `ChatError` | enum | All failure modes |

#### `Message` and its serde shape

`Message` (`crates/seele-chat/src/lib.rs:39-50`) is the canonical wire type and is **shared between request and response** — the same struct is sent to the provider and deserialized back from it. Field-by-field:

| Field | Type | serde attribute | Meaning |
|-------|------|-----------------|---------|
| `role` | `String` | (required) | `"system"`, `"user"`, `"assistant"`, or `"tool"` |
| `content` | `Option<String>` | `default`, `skip_serializing_if = "Option::is_none"` | text body; absent on pure tool-call turns |
| `tool_calls` | `Vec<ToolCall>` | `default`, `skip_serializing_if = "Vec::is_empty"` | calls requested by an assistant turn |
| `tool_call_id` | `Option<String>` | `default`, skip-if-none | on a `"tool"` turn, the call it answers |
| `name` | `Option<String>` | `default`, skip-if-none | tool name on a `"tool"` turn |

The `skip_serializing_if` attributes are load-bearing: they keep the OpenAI request body clean (e.g. a plain user message serializes to just `{"role":"user","content":"…"}`) so strict providers do not reject unexpected `null` fields. `role` is a plain `String`, not an enum — the crate never validates it, trusting both the caller and the provider to use legal values.

`ToolCall` (`:52-58`) holds `id: String`, `r#type: String` (defaulting to `"function"` via the `default_tool_type` fn at `:60-62`, so a provider that omits `type` still deserializes), and `function: ToolCallFunction`. `ToolCallFunction` (`:64-69`) is `{ name: String, arguments: String }` where `arguments` is, per the comment, a "JSON-encoded arguments string (OpenAI convention)" — i.e. the model's structured input arrives as an opaque string the caller must parse.

#### `ChatProvider`, `ToolSpec`, `ToolHandler`, `ChatConfig`

The trait (`:72-78`) is the polymorphism seam:

```rust
// crates/seele-chat/src/lib.rs:72-78
#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn complete(&self, messages: &[Message]) -> Result<Message, ChatError>;
    fn name(&self) -> &str;
    fn model(&self) -> &str;
}
```

`#[async_trait]` (the `async-trait 0.1` crate) is required because Rust's native `async fn` in traits cannot be made into a `dyn` trait object, and `run_chat` takes `provider: &dyn ChatProvider`. The bound `Send + Sync` lets the provider be shared across tokio tasks.

`ToolSpec` (`:83-88`) carries `name`, `description`, and a `serde_json::Value` `parameters` holding the input JSON-schema. `ToolHandler` (`:91-93`) is the storage-agnostic hook:

```rust
// crates/seele-chat/src/lib.rs:91-93
pub type ToolHandler = Box<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>> + Send + Sync,
>;
```

This hand-rolled boxed-future signature (rather than `async_trait`) is what decouples the crate from any storage: the loop hands the handler the raw `arguments` JSON string and gets back either an `Ok(result_string)` to feed the model or an `Err(message)` that the loop turns into a tool-error string (it never aborts the loop). Importantly, the handler receives **only** `arguments` — not the tool name — so a multi-tool caller must dispatch on the parsed args itself. In practice the HTTP layer binds exactly one tool (`seele_search`), sidestepping that limitation.

`ChatConfig` (`:96-108`) holds a `system_prompt` and a `max_iterations: u32`. Its `Default` (`:101-108`) sets `max_iterations: 5` (line `:105`) and a baked English prompt instructing the assistant to use `seele_search`, "Cite retrieved memories by `id` and `project`," and "Respond in the language of the user's question." (full string at `:104`).

#### `ChatError` variants

| Variant | `#[from]` | When |
|---------|-----------|------|
| `Http(reqwest::Error)` | yes | transport/connection failure |
| `ProviderStatus { status: u16, body: String }` | no | non-2xx HTTP status; body captured for diagnostics |
| `Parse(serde_json::Error)` | yes | response body failed to deserialize |
| `Tool(String)` | no | tool-layer error; also reused for "provider returned zero choices" |
| `LoopBudget { max_iterations: u32 }` | no | tool-use loop hit the iteration cap |
| `UnsupportedProvider(String)` | no | declared but **never constructed** anywhere in the crate |

Defined at `:19-33`. `UnsupportedProvider` is dead surface today — provider selection happens in the caller (`seele-http`), which treats any non-`anthropic` label as OpenAI-compatible and so never needs to reject one.

### OpenAICompatibleProvider

`OpenAICompatibleProvider` (`:114-121`) holds `provider_name`, `endpoint`, `api_key`, `model`, `tools: Vec<ToolSpec>`, and a private `reqwest::Client` (one client per provider instance — providers are short-lived, built per request by the HTTP handler). `new` (`:123-139`) takes `impl Into<String>` for ergonomics. The crate-level doc comment lists Minimax, OpenAI, OpenRouter, Together, Groq, DeepSeek as targets (`:3-4`); the section banner comment above the struct (`:110-112`) repeats the same list but truncates it with "…" (it does not spell out DeepSeek).

`complete` (`:170-207`):
1. Builds `{ "model", "messages", "stream": false }`. `messages` serializes the `&[Message]` slice directly — the OpenAI schema *is* the internal schema, so no translation step exists.
2. If `tools` is non-empty, adds `tools` (via `tools_json`, `:141-157`, which wraps each `ToolSpec` as `{"type":"function","function":{name,description,parameters}}`) and `"tool_choice":"auto"`.
3. POSTs with `bearer_auth(&self.api_key)` (i.e. `Authorization: Bearer …`).
4. On non-success status, reads the body text (`unwrap_or_default()`) and returns `ProviderStatus`.
5. Deserializes into the private `OpenAIResponse { choices: Vec<OpenAIChoice> }` (`:210-218`), takes the first choice's `message`, or errors with `ChatError::Tool("provider returned zero choices")` if `choices` is empty. Only the first choice is ever used.

### AnthropicProvider

`AnthropicProvider` (struct `:224-230`, `new` `:232-242`) hard-codes `endpoint = "https://api.anthropic.com/v1/messages"` in `new` (`:237`), so unlike the OpenAI path there is no endpoint override. Its `complete` (`:254-302`) performs a real **schema translation** because `/v1/messages` differs from `/v1/chat/completions`:

- `split_system` (`:304-320`) extracts all `"system"` messages, concatenates their `content` with newlines into a top-level `system` string (Anthropic carries system text out of the message array).
- `anthropic_translate` (`:322-361`) maps each remaining message:
  - a `"tool"` message becomes a `"user"` message containing a `tool_result` block keyed by `tool_use_id` (from `tool_call_id`);
  - an assistant turn with `tool_calls` becomes content **blocks**: an optional `text` block plus one `tool_use` block per call, where `arguments` is parsed from string back into a JSON object (`serde_json::from_str(...).unwrap_or(json!({}))` at `:343-344` — malformed args silently degrade to `{}`);
  - anything else becomes `{ role, content }`.
- The request adds `"max_tokens": 2048` (hard-coded, `:272`) and tools as `{name, description, input_schema}` (note the key is `input_schema`, vs OpenAI's nested `parameters`). Auth uses headers `x-api-key`, `anthropic-version: 2023-06-01`, and `content-type: application/json` (`:283-285`) — **not** bearer auth.
- The response (`AnthropicResponse { content: Vec<AnthropicContent> }`, `:363-379`) is an internally-tagged enum (`#[serde(tag = "type", rename_all = "snake_case")]`) over `Text` and `ToolUse`. `anthropic_to_internal` (`:381-410`) folds it back to a single internal assistant `Message`: text parts are joined with `\n` (or `None` if empty), and each `tool_use` becomes a `ToolCall` whose `arguments` is `input.to_string()` (re-serialized JSON), restoring the OpenAI string convention.

This round-trip is the crux of multi-provider support: both providers ingest and emit the *same* internal `Message`, so `run_chat` is provider-agnostic.

### The tool-use loop: `run_chat` and `LoopBudget`

```rust
// crates/seele-chat/src/lib.rs:416-421
pub async fn run_chat(
    provider: &dyn ChatProvider,
    tool_handler: &ToolHandler,
    user_messages: Vec<Message>,
    config: &ChatConfig,
) -> Result<Vec<Message>, ChatError>
```

Control flow (`:416-476`):
1. Seed `history` with a `system` message from `config.system_prompt`, then append the caller's `user_messages` (`:422-430`).
2. Loop `for iteration in 0..config.max_iterations` (`:439`):
   - call `provider.complete(&history)` and push the assistant reply into `history`;
   - if the reply has **no** `tool_calls`, the model is done — return the full `history` (success, `:443-446`);
   - otherwise, for each tool call, invoke `tool_handler(tc.function.arguments.clone()).await`. On `Ok`, use the result; on `Err(e)`, synthesize `format!("(tool error) {}", e)` (`:457`) so the model can recover rather than the run aborting. Push each result as a `"tool"` message carrying `tool_call_id` and `name` (`:459-465`), then loop again.
3. If the loop exits without an answer, log a `warn!` (`:469-472`) and return `Err(ChatError::LoopBudget { max_iterations })`.

`LoopBudget` (default 5 iterations) is the safety cap that prevents an infinite request→tool→request cycle from a model that keeps calling tools and never settles, bounding both cost and latency. Tool calls within one iteration are executed **sequentially** (a `for` loop with `.await`, `:453-466`), not concurrently. The function returns the *entire* `history` including the prepended system turn and all intermediate tool turns — the caller is responsible for trimming for display.

### Concurrency, async, error handling

Everything is `async` over tokio + `reqwest`. There is no shared mutable state and no locking inside the crate; concurrency safety comes from the `Send + Sync` bounds on `ChatProvider` and `ToolHandler`. Errors are typed via `thiserror`; the two `#[from]` conversions (`reqwest::Error`, `serde_json::Error`) let `?` propagate transport and parse failures, while status and budget errors are constructed explicitly. The crate logs via `tracing` at `debug`/`info`/`warn` (e.g. `info!` at loop start `:432-437`, `debug!` per-iteration `:444`/`:448-452`, `warn!` on budget exhaustion `:469-472`) but never `error!`.

### How it connects to the rest of SEELE

The sole binding lives in `crates/seele-http/src/handlers.rs`, which imports the chat types as `use seele_chat::{ AnthropicProvider, ChatConfig, ChatProvider, Message as ChatMessage, OpenAICompatibleProvider, ToolHandler, ToolSpec }` (`handlers.rs:200-203`) — note `Message` is locally aliased to `ChatMessage`. `ChatProviderConfig` (`crates/seele-http/src/server.rs:61-73`) carries `provider`, `api_key`, `model`, and optional `endpoint`, and is stored in `AppState.chat: Option<Arc<ChatProviderConfig>>` (`server.rs:30`), exposed to handlers through a `FromRef` impl (`server.rs:39-43`). Two routes are registered: `POST /chat` and `GET /chat/info` (`server.rs:151-152`), both inside the auth-protected router group. The CLI `seele serve` populates the config from `--chat-provider`/`--chat-key`/`--chat-model`/`--chat-endpoint` (clap flags at `crates/seele-cli/src/commands/serve.rs:28-46`; config assembled at `serve.rs:53-72`). `--chat-key` is resolved by `resolve_key` (`serve.rs:88-95`): if the value starts with `$`, the remainder is read from that env var, otherwise the value is used literally. `--chat-provider` and `--chat-key` must be supplied together or both omitted, else `serve` bails.

The `chat` handler (`handlers.rs:254-376`) resolves provider/key/model/endpoint in priority order **per-request override > CLI config** (`handlers.rs:261-288`), then:
- builds a single `ToolSpec` named `seele_search` with a query/limit/project JSON-schema (`required: ["query"]`) (`handlers.rs:290-302`);
- selects the provider — `AnthropicProvider` when the name equals `anthropic` (case-insensitive via `eq_ignore_ascii_case`), else `OpenAICompatibleProvider` with a `default_endpoint_for` fallback when no endpoint override is present (`handlers.rs:304-315`);
- constructs the `ToolHandler` closure (`handlers.rs:318-360`) that parses `{query, limit, project}`, clamps `limit` to `unwrap_or(5).min(20)` (default 5, max 20), builds a `SearchRequest`, calls `svc.search_observations(...)`, and returns a JSON summary `{query, count, results:[{id,type,title,project,score,snippet}]}` where `snippet` is the hit content truncated to the first 280 chars (`handlers.rs:355`) — this is the line where storage re-enters; the closure captures an `Arc<SeeleService>` clone (`handlers.rs:317-319`);
- applies an optional per-request `system_prompt` override onto `ChatConfig::default()` (`handlers.rs:362-365`);
- runs `seele_chat::run_chat(...)` and returns `ChatResponse { messages, provider, model }` (`messages` is `Vec<ChatMessage>`, i.e. `Vec<seele_chat::Message>`), mapping any `ChatError` to `ApiError::Internal` (`handlers.rs:367-375`).

Default models and endpoints for each provider family are tabulated in `default_model_for`/`default_endpoint_for` (`handlers.rs:378-401`; e.g. minimax → `MiniMax-M2` at `https://api.minimax.io/v1/chat/completions`, anthropic → `claude-haiku-4-5-20251001`, with `gpt-4o-mini` / `https://api.openai.com/v1/chat/completions` as the catch-all defaults). An identical `default_model_for` table is duplicated in the CLI at `serve.rs:97-108` (so the CLI can compute a default model before constructing `ChatProviderConfig`).

The browser side is `web/src/components/ChatPanel.astro`, embedded on `web/src/pages/observability.astro` (imported at `observability.astro:5`, rendered at `:35`). It probes `GET /chat/info` (`ChatPanel.astro:250`) to learn whether chat is enabled and which provider/model is set, stores the user's provider/key/model/endpoint in `localStorage` under the key `seele-chat-settings` (`ChatPanel.astro:164`; forwarded only to the local SEELE server, never a third party — per the doc-comment privacy note at `ChatPanel.astro:8-10`), and POSTs the running `history` to `/chat`. On reply it slices `data.messages.slice(history.length + 1)` to skip the server-prepended system prompt and renders the new turns (`ChatPanel.astro:385-393`). The panel also strips reasoning traces (`<think>`, `<thinking>`, `<|thinking|>`) in `stripThinking` (`ChatPanel.astro:476-487`) and applies light client-side markdown — bold and inline code — in `renderAssistantBody` (`ChatPanel.astro:489-498`), and shows a compact `N results` summary for tool turns (`ChatPanel.astro:452-466`).

### Edge cases, gotchas, invariants

- **`arguments` is always a string.** Both providers normalize tool input to a JSON-encoded string; callers must `serde_json::from_str` it. Anthropic's translation parses it back to an object and the response re-serializes it, so a malformed arguments string silently becomes `{}` (`lib.rs:343-344`) — a lossy edge.
- **Single tool name is invisible to the handler.** `ToolHandler` receives only `arguments`, so the design assumes one tool (or caller-side disambiguation). With the HTTP layer's single `seele_search` tool this is fine.
- **`UnsupportedProvider` is dead** — declared but never constructed (`lib.rs:31-32`).
- **No streaming.** OpenAI requests pin `"stream": false` (`lib.rs:174`); Anthropic hard-codes `"max_tokens": 2048` (`lib.rs:272`). Both are non-configurable from the public API.
- **Only the first choice** of an OpenAI response is read; `n`-completions are unsupported.
- **API keys never persist server-side** — `ChatProviderConfig`'s doc note (`server.rs:59-60`) states none of its fields ever leave the machine and "the API key stays on the box running `seele serve`"; per-request keys are documented on `ChatRequest.api_key` as used "once and discards it — never persisted" (`handlers.rs:214-215`).
- No `TODO`/`FIXME`/`XXX`/`HACK` markers exist in `crates/seele-chat/src/lib.rs`.


---

## 11. Command-Line Interface — `seele-cli`

`seele-cli` is the crate that produces the single distributable binary, `seele`. It is the root of the dependency graph: its `Cargo.toml` declares paths to **every** internal crate (`seele-core`, `seele-storage`, `seele-embedder`, `seele-search`, `seele-mcp`, `seele-http`, `seele-tui`, `seele-sync`, `seele-setup`, `seele-project`, `seele-engram-import`) plus the external crates `clap`, `tokio`, `tracing`, `tracing-subscriber`, `anyhow`, `dirs = "5"`, `serde`, and `serde_json`. The `[[bin]]` table binds the name `seele` to `src/main.rs`. There is no `lib` target; the crate is purely an application shell. (Dev-dependencies add `tempfile`, `reqwest`, `serde_json`, `rusqlite`, and `ulid` for the E2E tests.)

Its responsibility is thin by design: parse arguments with a clap-derive command tree, build a `SeeleService` against the **local SQLite file directly** (never over HTTP, even though it reuses the HTTP crate's service/DTO layer), dispatch to one handler per subcommand, and render output as either human text or JSON. All real work — embeddings, storage CRUD, hybrid search, sync, import, setup, MCP, HTTP, TUI — is delegated to the lower crates. The CLI is the place where the embedder is chosen and where the ONNX-to-Fake fallback is decided.

### 11.1 File-by-file map

| File | Role |
| --- | --- |
| `src/main.rs` | Entry point. Builds a tokio runtime, parses `Cli`, runs `app::run`, maps `anyhow::Result<()>` to an `ExitCode`. Also defines `default_db_path()`. |
| `src/app.rs` | The clap-derive command tree (`Cli`, `Command`), the async `run` dispatcher, and the shared `build_service` / `pick_embedder` embedder-selection logic. |
| `src/output.rs` | Two output helpers (`emit_split`, `status`) that every command routes through so `--json` is honored uniformly. |
| `src/commands/mod.rs` | Declares the 16 per-subcommand modules. |
| `src/commands/<name>.rs` | One module per subcommand, each owning its clap `Args`/`Subcommand` struct and an async `run(...)`. |
| `tests/subcommands_e2e.rs` | E2E tests that spawn the real binary for save/list/show/search/delete/restore/link/stats/doctor/projects/sync/setup/import. |
| `tests/binary_e2e.rs` | E2E tests for `--version`/`--help`/unknown-command, `mcp` over piped stdio, and `serve` over a real TCP port. |

The 16 modules declared in `commands/mod.rs` (alphabetical in the source) are: `delete`, `doctor`, `import`, `link`, `list`, `mcp`, `projects`, `restore`, `save`, `search`, `serve`, `setup`, `show`, `stats`, `sync`, `tui`.

#### `main.rs`

The process entry is `fn main() -> ExitCode` (main.rs:23). It parses arguments via `Cli::parse()` (clap), then constructs a `tokio::runtime::Runtime` via `tokio::runtime::Runtime::new()` (which defaults to the multi-thread scheduler). Runtime construction failure prints `failed to start tokio runtime: {e}` to stderr and returns `ExitCode::FAILURE`. On success it calls `rt.block_on(app::run(cli))`; `Ok(())` becomes `ExitCode::SUCCESS`, and any `anyhow::Error` is printed as `seele error: {e:#}` (the `{:#}` alternate format prints the full anyhow cause chain) before returning `ExitCode::FAILURE`. The whole CLI is async only because some leaf operations (`mcp`, `serve`, `tui`) are async; the DB-touching commands are synchronous inside their `async fn`.

```rust
// crates/seele-cli/src/main.rs:23-39
fn main() -> ExitCode {
    let cli = Cli::parse();
    let rt = match tokio::runtime::Runtime::new() { /* ... */ };
    match rt.block_on(app::run(cli)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => { eprintln!("seele error: {e:#}"); ExitCode::FAILURE }
    }
}
```

`default_db_path()` (main.rs:42) returns `~/.seele/seele.db` via `dirs::home_dir()`, falling back to the relative path `.seele/seele.db` if the home directory cannot be resolved. This is the authoritative default used both by `build_service` and directly by `doctor`/`import` when `--db` is absent.

### 11.2 The clap command tree (`app.rs`)

The root parser is `struct Cli` (app.rs:23-42) with `#[command(name = "seele", version, about, long_about = None)]`. The bare `version` attribute makes clap derive `--version` and `-V` from `CARGO_PKG_VERSION`; `binary_e2e.rs::version_prints_seele_and_pkg_version` asserts the output starts with `seele ` (clap prints `seele <version>`). There is no dedicated `version` subcommand — version handling is entirely clap's built-in flag, which is why "version handling" is described as a built-in rather than a 17th `Command` variant.

Three flags are declared `global = true`, so they are accepted before or after any subcommand:

| Flag | Type | Meaning |
| --- | --- | --- |
| `--db <PATH>` | `Option<PathBuf>` | Database file; defaults to `~/.seele/seele.db`. |
| `--fake-embedder` | `bool` | Force the deterministic `FakeEmbedder`. |
| `--json` | `bool` | Emit JSON instead of human text. |

```rust
// crates/seele-cli/src/app.rs:25-42
pub struct Cli {
    #[arg(long, global = true)] pub db: Option<PathBuf>,
    #[arg(long, global = true)] pub fake_embedder: bool,
    #[arg(long, global = true)] pub json: bool,
    #[command(subcommand)] pub command: Command,
}
```

The `Command` enum (app.rs:44-80) has **16 variants**, one per subcommand module. Two of them (`Sync`, `Import`) are themselves `#[command(subcommand)]` groups, giving the tree a second level. `OutputOpts { json: bool }` (app.rs:82-85, deriving `Args, Debug, Clone`) is a tiny carrier struct threaded into every handler so they don't need the full `Cli`.

`run(cli)` (app.rs:87) builds `OutputOpts` from `cli.json` and `match`es `cli.command`, calling each handler with `(args, &cli.db, cli.fake_embedder, &out)` — except `Setup` (no DB, so just `args` + `out`) and the three long-running servers `Mcp`/`Serve`/`Tui` (no `out`, since they take over stdio/stdout themselves). `Stats`/`Doctor`/`Projects` are unit variants, so their handlers take `(&cli.db, cli.fake_embedder, &out)` with no `args`.

### 11.3 Service construction against the local DB

`build_service(db_override: &Option<PathBuf>, fake_embedder_flag: bool) -> anyhow::Result<SeeleService>` is the single chokepoint every DB-touching command uses (app.rs:128). It:

1. Resolves the path: `db_override.clone().unwrap_or_else(crate::default_db_path)`.
2. Creates the parent directory with `std::fs::create_dir_all` (so a first run on a fresh machine just works).
3. Calls `seele_storage::init_db(&path)` to get a connection `Pool`. Per `seele-storage/src/lib.rs:34`, `init_db` opens the connection, applies canonical PRAGMAs, loads the vendored vec0 extension, and runs pending refinery migrations (idempotent).
4. Picks an embedder via `pick_embedder(fake_embedder_flag)`.
5. Returns `SeeleService::new(pool, embedder)`.

This is the architectural crux of the CLI: it reuses `seele_http::SeeleService` — the exact same service object the HTTP server and MCP server wrap — but constructs it **in-process** against the local file. The CLI never opens a socket or speaks HTTP to itself; it calls `svc.save_observation(...)`, `svc.search_observations(...)`, etc. directly. `SeeleService` (defined at `seele-http/src/service.rs:34-44`) exposes the underlying stores as public fields — `observations`, `sessions`, `links`, `relations`, `prompts`, `chunks`, `search` (`Arc<SearchEngine>`), `embedder` (`Arc<dyn Embedder>`), and `pool` (`Pool`) — which the sync command uses to reach `svc.observations` and `svc.chunks` without going through a request DTO. Commands also import request/response DTOs straight from `seele_http::dto` (`SaveRequest`, `SearchRequest`, `ListRequest`, `LinkCreateRequest`), so the CLI's argument-to-DTO mapping is the same boundary the REST handlers use. Because the service is built fresh per invocation and the process exits afterward, there is no long-lived locking or concurrency to manage for the data commands; the `Pool` is the only shared resource and it lives for one command.

### 11.4 Embedder selection and the ONNX→Fake fallback

`pick_embedder(fake_flag) -> Arc<dyn Embedder>` (app.rs:144) implements a documented priority, first match wins:

1. **Forced fake.** If `fake_flag` is set (`--fake-embedder`) **or** `fake_env_set()` is true, return `Arc::new(FakeEmbedder)`. `fake_env_set()` (app.rs:163) reads `SEELE_FAKE_EMBEDDER` (the `FAKE_EMBEDDER_ENV` constant, app.rs:20) and treats any value that is non-empty after `trim()` as enabled. This is what the test harnesses set (`SEELE_FAKE_EMBEDDER=1`) so CI never downloads the ONNX model.
2. **ONNX default.** Otherwise call `OnnxEmbedder::new()`. On `Ok`, use the ONNX model. Per `seele-embedder/src/onnx.rs`, the default model is `sentence-transformers/all-MiniLM-L6-v2` (384-dim; `DEFAULT_DIM = 384`). On first run it downloads/caches the model. **Cache location:** `resolve_cache_dir()` (onnx.rs:239) uses `SEELE_EMBEDDER_DIR` (the `CACHE_DIR_ENV` constant) when set, otherwise `dirs::cache_dir().join("seele/embedder")` — i.e. the **OS cache directory** (e.g. `%LOCALAPPDATA%\…\seele\embedder` on Windows, `~/.cache/seele/embedder` on Linux), **not** `~/.seele/embedder/`. (The `build_service` doc comment at app.rs:121-127 still says `~/.seele/embedder/`, which is a stale comment — the authoritative behavior is `resolve_cache_dir`.) The model download is in the ~30–90 MB range (the app.rs doc comment says "~30-90 MB"; the test-harness comments say "~90 MB").
3. **Transparent fallback.** If `OnnxEmbedder::new()` returns `Err`, print a warning to **stderr** and fall back to `FakeEmbedder`:

```rust
// crates/seele-cli/src/app.rs:148-161
match OnnxEmbedder::new() {
    Ok(emb) => Arc::new(emb),
    Err(e) => {
        eprintln!(
            "seele: warning — ONNX embedder unavailable ({e}); falling back \
             to FakeEmbedder. Search quality is degraded (hash-based, not \
             semantic). Re-run with network access on first call to \
             download the model, or set SEELE_FAKE_EMBEDDER=1 to silence \
             this message."
        );
        Arc::new(FakeEmbedder)
    }
}
```

The warning goes to stderr precisely so it does not corrupt a `--json` payload on stdout. This is the "transparent ONNX-to-Fake fallback at the CLI boundary": the binary keeps working with degraded (hash-deterministic, non-semantic) vector hits rather than failing. The only unit test in the crate, `pick_embedder_with_flag_returns_fake` (app.rs:173-177), asserts that the flag path yields a model whose `model_id()` is `seele/fake-embedder` (the `FAKE_MODEL_ID` constant in `seele-embedder/src/fake.rs`).

Note a documentation/behavior drift worth flagging for the improvement pass: `app.rs` (the `build_service` priority doc, lines 118-127) and `commands/doctor.rs` correctly describe ONNX as the default with Fake as fallback, but several in-code comments still read as if v0.1 always used Fake — `doctor.rs:35-38` says *"v0.1 always uses FakeEmbedder (real ONNX in Sprint-05+)"*, `import.rs:27-29` and `import.rs:64-69` call `--re-embed` *"a no-op until Sprint-05"* / *"ONNX backend lands in Sprint-05"*, and the `app.rs:123` cache-dir comment is wrong. Per CLAUDE.md, ONNX is the v0.1+ default, so these comments are stale relative to `pick_embedder`.

### 11.5 Output rendering (`output.rs`)

All stdout rendering flows through two functions:

- `emit_split<T: Serialize>(value: &T, human: impl FnOnce() -> String, json: bool) -> anyhow::Result<()>` — if `json`, prints `serde_json::to_string_pretty(value)`; else prints `human()`. The `human` closure is lazy (`FnOnce`), so the human formatting cost is only paid when not in JSON mode. Each command crafts its own readable plain-text form here while the same `value` serializes to machine-readable JSON.
- `status(line: &str, json: bool)` — prints a confirmation line **only when not** `--json`. Used by `delete`/`restore` so the JSON mode emits a clean single object and the text mode emits a friendly status line.

This split is why `--json` output is reliably parseable: status chatter and the ONNX fallback warning are kept off stdout (status via `status()`, warning via stderr), leaving stdout as pure JSON. The E2E tests rely on this by piping stdout straight into `serde_json::from_str`. (Note: `delete`/`restore` do not use `emit_split` for their JSON object — they call `status()` for the text line and then `println!` a raw `serde_json::json!({"ok": true, "id": ...})` only when `out.json` is set.)

### 11.6 Subcommand catalog

There are **16 `Command` variants** (one per module in `commands/mod.rs`). If you also count clap's built-in `--version` flag as a pseudo-subcommand — and count the two-level `sync`/`import` groups by their roots — you arrive at the "17 subcommands" figure used in CLAUDE.md. Each data `run` first calls `build_service` (except `setup`, which needs no DB, and `import`, which builds the DB stores itself — see 11.8) and ends with `emit_split`/`status`.

| Subcommand | Positional args | Flags | Service call(s) | Behavior |
| --- | --- | --- | --- | --- |
| `save <title> <content>` | `title`, `content` | `--type` (default `memory`), `--project`, `--scope`, `--topic-key`, `--metadata <JSON>` | `save_observation(SaveRequest)` | Builds a `SaveRequest` with `session_id: None`, `tool_name: Some("seele-cli")`; `--metadata` is parsed as JSON (invalid JSON → `invalid --metadata JSON: {e}` error), defaulting to `Value::Null`. Human form: `saved {id} ({outcome})`, where `outcome` is the `&'static str` field of `SaveResponse`. |
| `search [query]` | optional `query` | `--project`, `--scope`, `--type`, `--limit` (default 20), `--include-purist`, `--include-annotations` | `search_observations(SearchRequest)` | Empty query allowed only with a filter; enforced by `enforce_search_query_or_filter` (defined in `seele_http::service`, service.rs:442) — empty query + no project/scope/type filter returns a `BadRequest`. `score_boost_multiplier` is hardcoded `0.0`, `max_vec_distance: None`. Human form prints `{count} hit(s):` then `  {id} [{type}] {score:.3}  {title}` per hit, or `no hits`. |
| `show <id>` | `id` (ULID) | — | `get_observation(SeeleId)` | Parses the id (`invalid id: {e}` on failure); `None` → `observation {id} not found`. Human form prints id, type, scope, project (`-` if none), title, and an indented content block (`indent(content, "    ")`). |
| `list` | — | `--project`, `--scope`, `--type`, `--topic-key`, `--session-id`, `--limit` (default 50), `--include-deleted` | `list_observations(ListRequest)` | Human form: `{len} observation(s):` then `  {id} [{type}] {title}`, or `no observations`. |
| `delete <id>` | `id` | — | `soft_delete_observation(SeeleId)` | Soft delete (sets `deleted_at`). Text: `soft-deleted {id}` via `status`; JSON: `{"ok": true, "id": ...}`. |
| `restore <id>` | `id` | — | `restore_observation(SeeleId)` | Clears `deleted_at`. Text: `restored {id}`; JSON: `{"ok": true, "id": ...}`. |
| `link <from_id> <to_id> <link_type>` | three positionals | `--metadata <JSON>` | `create_link(LinkCreateRequest)` | Free-form `link_type` (e.g. `derives_from`, `related_to`, `supersedes`, `contests`). Human form: `linked {from} -[{type}]-> {to} (id={id})`. |
| `stats` | — | — | `stats()` | Aggregate counters. Human form prints observations (`active`, `deleted`, `projects`, `by_type`, `by_scope`) and sessions (`total`, `by_status`), iterating the `{key}: {count}` buckets. |
| `doctor` | — | — | `stats()` + `embedder_info()` | Health snapshot (see 11.7). |
| `projects` | — | — | `list_projects()` | Distinct project names. Human form: `{len} project(s):` then `  {p}`, or `no projects yet`. |
| `sync export <dir>` | `dir` | `--project` | `seele_sync::export_to_dir(&svc.observations, &dir, ExportFilter)` | Writes a gzipped JSON chunk; human form reports `exported {observation_count} observation(s) → {path} ({bytes_on_disk} bytes)`. |
| `sync import <path>` | `path` | `--target-key <KEY>` (required) | `seele_sync::import_from_file(&svc.observations, &svc.chunks, &target_key, &path)` | Idempotent per `target_key + chunk_id`. Human form: `import {outcome:?}: saved={observation_count_saved} already_present={observation_count_already_present} chunk_skipped_rows={observation_count_skipped_chunk_level} chunk_id={chunk_id}`. |
| `import from-engram <path>` | `path` | `--re-embed`, `--dry-run` | `EngramImporter::new(&observations, &links).import_from(&path, dry_run)` | One-shot ENGRAM migration (ADR-13); see 11.8. |
| `setup` | — | `--agent <NAME>`, `--all`, `--list`, `--dry-run`, `--no-backup`, `--seele-binary <PATH>` | `seele_setup::{install, all_agent_names, implemented_agent_names}` | MCP installer wizard; see 11.9. |
| `mcp` | — | `--tool-prefix <P>` (+ global `--db`) | `McpServer::new(svc, McpServerConfig).run_stdio()` | Long-running JSON-RPC 2.0 stdio server. `--tool-prefix mnema` activates ENGRAM-compat aliases (`mnema_save`, `mnema_recall`, ...). |
| `serve` | — | `--port` (7777), `--bind` (`127.0.0.1`), `--legacy-engram-paths`, `--auth-bearer`, `--cors-allow` (repeatable), `--chat-provider`, `--chat-key`, `--chat-model`, `--chat-endpoint` | `Server::new(svc, ServerConfig).run()` | Long-running axum HTTP server; see 11.6.1. |
| `tui` | — | `--smoke` (hidden) | `seele_tui::run_tui(svc)` / `run_tui_smoke(svc)` | Interactive ratatui UI talking to the in-process `SeeleService`; `--smoke` renders one off-screen frame and prints `tui smoke ok`. |

#### 11.6.1 `serve` argument plumbing

`serve` is the richest argument surface. It assembles a `SocketAddr` by parsing `format!("{bind}:{port}")` (`--port 0` lets the OS pick). The chosen address is announced on **stderr** by the HTTP server itself: `seele_http::server` (server.rs:194) prints `seele http listening on http://{addr}` — `binary_e2e.rs::serve_health_returns_200_over_real_tcp` parses exactly that prefix (`strip_prefix("seele http listening on ")`). Chat configuration is paired: `--chat-provider` and `--chat-key` must both be present or both absent (`anyhow::bail!("--chat-provider and --chat-key must be set together (or both omitted)")` otherwise). The key is resolved by `resolve_key` (serve.rs:88): a value starting with `$` is treated as an env-var name (`--chat-key $MINIMAX_API_KEY` reads `MINIMAX_API_KEY`, erroring if unset), otherwise used literally. `default_model_for` (serve.rs:97) lowercases the provider and maps known providers to default models (`minimax`→`MiniMax-M2`, `openai`→`gpt-4o-mini`, `openrouter`→`openai/gpt-4o-mini`, `together`→`meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo`, `groq`→`llama-3.3-70b-versatile`, `deepseek`→`deepseek-chat`, `anthropic`→`claude-haiku-4-5-20251001`, anything else→`gpt-4o-mini`). The resulting `Option<ChatProviderConfig>` and the CORS/auth/legacy flags go into `ServerConfig` (`addr`, `cors_origins`, `auth_bearer`, `legacy_engram_paths`, `chat`). A documented gotcha lives in the `--cors-allow` help (serve.rs:23-27): empty = CORS disabled (default, safe for local-only use); any non-empty value enables permissive `Access-Control-Allow-Origin: *`; per-origin allowlist refinement is on the backlog.

### 11.7 The `doctor` diagnostics

`doctor` (commands/doctor.rs) builds a private `DoctorReport` struct (doctor.rs:12-23) and emits it via `emit_split`. Fields: `status` (`&'static str`, always the literal `"ok"`), `seele_version` (`env!("CARGO_PKG_VERSION")`), `db_path` (resolved `--db` or default), `embedder_model_id` (`String`), `embedder_dim` (`usize`), `embedder_expected_sha256` (`Option<String>` from `EmbedderInfo`), `fake_embedder_warning` (`Option<String>`), `observations_active` (`u64`), and `sessions_total` (`u64`). It gathers data from `svc.stats()` and `svc.embedder_info()` (which returns `seele_http::dto::EmbedderInfo { model_id, dim, expected_sha256 }`).

The load-bearing diagnostic is the fake-embedder warning. The check is `info.model_id.contains("fake")` (doctor.rs:39) — i.e. whenever the **active** model identifies as fake (the `seele/fake-embedder` literal returned by `FakeEmbedder::model_id`), whether forced by flag/env or reached via the ONNX fallback — it sets `fake_embedder_warning` to a message beginning `FakeEmbedder is active — vector search returns deterministic zeros...`. This is the "Cloven sight" requirement called out in the module doc (doctor.rs:1-3, tagged "Sprint-03"): the report must "grit-shout" so users don't silently lose vector search quality. The human form lists status, seele version, db path, `embedder: {model_id} (dim={dim})`, observations (active), sessions (total), and — if present — a `WARNING: {w}` line. `subcommands_e2e.rs::doctor_emits_fake_embedder_warning` asserts the JSON `status == "ok"` and that `fake_embedder_warning` is a string containing `FakeEmbedder is active` (run under `SEELE_FAKE_EMBEDDER=1`).

### 11.8 `import from-engram`

`import` is a subcommand group (`ImportCmd`) with one variant, `FromEngram(FromEngramArgs)`. Notably it does **not** use `build_service`; instead `run_from_engram` (import.rs:48) opens the DB itself: resolve path (`db.clone().unwrap_or_else(default_db_path)`) → `create_dir_all` → `init_db` → construct `ObservationStore` and `LinkStore` directly from the pool → `EngramImporter::new(&observations, &links).import_from(&path, dry_run)`. This bypasses the embedder entirely (the `_fake_embedder` parameter is ignored, prefixed `_`), which is consistent with import being a pure SQLite→SQLite row migration. `--re-embed` is recognized but only logs a `tracing::warn!` no-op (with the stale "ONNX backend lands in Sprint-05" message noted in 11.4). The report is an `ImportReport` with `source_path`, `dry_run`, `rows_seen`, `rows_inserted`, `rows_skipped_existing`, `rows_invalid`, `links_created`, `links_dangling`, and `errors`. The human form prints `source: {path} (dry-run|applied)`, then the row/link counters, then an `errors (N):` block if non-empty. The E2E tests confirm idempotency (second import yields `rows_inserted: 0`, `rows_skipped_existing: 1`) and that a DB lacking a `memories` table fails with an error mentioning `memories` on stderr.

### 11.9 `setup`

`setup` (commands/setup.rs) wraps `seele_setup`. Three modes, checked in order:

1. `--list` — prints `all_agent_names()` (seele-setup/src/lib.rs:134), tagging each with `[implemented]` or `[skeleton (v0.2)]` based on membership in `implemented_agent_names()` (lib.rs:140). The human form header is `{len} known agent(s):` and the per-line format is `  {name}  [{tag}]`. Returns early.
2. `--all` — builds `InstallOptions { dry_run, backup: !no_backup, home_override: None, seele_binary }` and iterates **only** `implemented_agent_names()` (skeletons are skipped to avoid `NotImplemented` noise — the fix Cloven flagged in the Sprint-04 mid-review). Successes collect into `reports`, failures into `errors` (formatted `{name}: {e}`); both are rendered, with each report line `  {agent}: {outcome:?} → {config_path}`.
3. Single agent — requires `--agent <name>` (else the error `--agent <name> or --all required`), calls `install(&agent, &opts)`, prints `{agent}: {outcome:?} → {config_path}`.

`--no-backup` inverts into `backup: !args.no_backup` (backups are on by default). `--seele-binary` overrides the binary path written into agent configs. `setup` is the only command in `run` whose handler signature is `run(args, out)` (no DB, no `fake_embedder`).

### 11.10 Concurrency, error handling, and edge cases

- **Async surface.** Only `mcp`, `serve`, and `tui` genuinely use async (`run_stdio().await`, `Server::run().await`, `run_tui(svc).await`). The data commands are synchronous bodies inside `async fn`; `main` drives them all on a tokio runtime built with `Runtime::new()` (multi-thread scheduler by default).
- **Error model.** The CLI uses `anyhow` exclusively (per CLAUDE.md, `thiserror` is for libraries, `anyhow` only in the CLI). Domain errors from the stores/service bubble up through `?` and are mapped to anyhow with context at the boundary (e.g. `invalid --metadata JSON: {e}`, `invalid id: {e}`, `observation {id} not found`). Everything funnels to `main`'s `eprintln!("seele error: {e:#}")` + `ExitCode::FAILURE`. clap parse errors (unknown subcommand, missing required arg) exit nonzero via clap itself, before `run` (verified by `binary_e2e.rs::unknown_command_exits_nonzero`).
- **Output cleanliness.** `--json` mode keeps stdout JSON-only: confirmations route through `status()` (suppressed under JSON), and the ONNX fallback warning and the `serve` listen line both go to stderr. This is an invariant the test suite depends on.
- **First-run UX.** `build_service` (and `import`'s `run_from_engram`) create the parent directory with `create_dir_all`, so a clean machine with no `~/.seele` directory works on the first `seele save`.
- **`save` stdin TODO.** save.rs:14-15 documents `--content -` for stdin ("Sprint-05 wires the stdin path"), but the code reads `content` as a plain positional `String`; the stdin path is not implemented in the read source.
- **`save` project default.** save.rs:20-22 carries a comment that project detection (`seele-project`) "wires in Sprint-04 Bloque D.2" — but in the current source `--project` is a plain `Option<String>`, so omitting it stores `None` rather than auto-detecting the repo. (Per CLAUDE.md, "project-detection wired in `seele save`" is still a v0.2 candidate, so that source comment is itself stale.)
- **`search` knobs hardcoded.** The CLI does not expose `score_boost_multiplier` or `max_vec_distance`; they are fixed at `0.0` / `None` (search.rs:44-45), so RRF tuning is HTTP/MCP-only.

### 11.11 How it connects to the rest of SEELE

The CLI is the convergence point of the crate graph. For data operations it constructs `seele_http::SeeleService` over a local `seele_storage` pool with a `seele_embedder` embedder, and calls service methods directly (no network). `sync` reaches into `svc.observations`/`svc.chunks` to call `seele_sync`. `import` builds `seele_storage` stores (`ObservationStore`/`LinkStore`) directly and calls `seele_engram_import`. `setup` calls `seele_setup`. `mcp` hands the same `SeeleService` to `seele_mcp::McpServer` (which itself layers over the HTTP service core). `serve` hands it to `seele_http::Server`. `tui` hands it to `seele_tui`. In every case the in-process `SeeleService` is the shared spine, and the three transports (CLI direct, MCP stdio, HTTP REST) all operate over identical service/DTO types — the CLI simply chooses the "direct" path. This makes the CLI the single binary that can act as client (data subcommands), MCP server, HTTP server, or TUI depending on the subcommand, all reading and writing the same local `seele.db`.


---

## 12. Terminal UI — `seele-tui`

`seele-tui` is the interactive [ratatui](https://ratatui.rs) terminal front-end launched by `seele tui` (ADR-07). It is a thin, read-only presentation layer over the same in-process service core the CLI uses: it holds a `seele_http::SeeleService`, calls its synchronous query methods directly (no HTTP round trip, no MCP envelope), caches the results in an `AppState`, and re-renders five views in response to vi-style keystrokes. In the crate graph it sits high — its `Cargo.toml` declares path deps on `seele-core`, `seele-storage`, `seele-search`, `seele-http`, and `seele-embedder` — but in practice every data access goes through `seele-http::SeeleService`; the other path deps are pulled in transitively through `seele-http`. Only `seele-cli` depends on `seele-tui` (`crates/seele-cli/Cargo.toml:21`).

### 12.1 Crate dependencies and why

The full `[dependencies]` list in `Cargo.toml` is: `seele-core`, `seele-storage`, `seele-search`, `seele-http`, `seele-embedder` (path deps), `ratatui`, `crossterm`, `tokio`, `tempfile`, `serde`, `serde_json`, `thiserror`, `tracing`, `futures`, `chrono`. The dev-dependency is `insta`. The most load-bearing ones:

| Dependency | Role in the TUI |
|---|---|
| `ratatui` (workspace) | Widget toolkit: `Frame`, `Layout`, `Block`, `List`, `Paragraph`, `Style`, `TestBackend`. |
| `crossterm = "0.28"` (feature `event-stream`) | Raw-mode/alt-screen control and the async `EventStream` keystroke source. The `event-stream` feature is what enables the `futures::Stream` adapter used in the event loop. |
| `tokio` (workspace) | The entrypoint and event loop are `async`; `run_tui` is awaited from the CLI's tokio runtime. |
| `futures` | `StreamExt::next()` on the crossterm `EventStream`. |
| `chrono` | `detail.rs` renders epoch-ms timestamps as `%Y-%m-%d %H:%M:%S UTC` via `ms_to_human`. |
| `serde_json` | Pretty-prints `ObservationDto.metadata` in the Detail pane. |
| `thiserror` | The crate's `TuiError` enum. |
| `seele-http` | `SeeleService` and the DTO/request types (`ObservationDto`, `SearchHitDto`, `StatsResponse`, `ListRequest`, `SearchRequest`, plus the free function `enforce_search_query_or_filter`). |
| `insta` (dev) | Snapshot assertions in `tests/views_snapshot.rs`. |

Note: `tempfile`, `serde`, and `tracing` are declared as regular (non-dev) dependencies as well, though they carry no load-bearing role in the rendering or event-loop code read for this section.

### 12.2 File-by-file map

- **`lib.rs`** — Crate root. Declares the four private modules (`app`, `event`, `theme`, `views`), re-exports the public surface (`AppState`, `Pane`, `handle_key`, `Action`), defines the `views_render` and `run_tui_smoke` helpers, defines the `TuiError` enum + `Result` alias, exposes `apply_action`, and owns the terminal lifecycle: `enter_tui`, the `TerminalGuard` RAII restorer, the async `run_event_loop`, and the public `run_tui` entrypoint.
- **`app.rs`** — The `Pane` enum and the `AppState` struct plus all of its side-effecting service calls (`refresh`, `refresh_for_current_pane`, `refresh_stats`, `refresh_browse`, `run_search`, `open_detail_for_selection`, `back`, `search_escape`) and pure navigation helpers (`switch_pane`, `move_selection`, `jump_to`, `current_list_len`). Contains the bulk of the crate's unit tests.
- **`event.rs`** — The `Action` enum and the pure `handle_key` keymap function, broken into `global_key`, `list_key`, `search_key`, `detail_key`. Unit-tests the keymap in isolation.
- **`theme.rs`** — Hardcoded dark palette: three color constants and three `Style` constructors.
- **`views/mod.rs`** — Top-level `render` that lays out header/body/footer and dispatches to one of the five sub-renderers; `render_header` (pane chooser) and `render_footer` (context hint + error).
- **`views/{home,browse,search,detail,stats}.rs`** — One `render(f, area, state)` per pane.
- **`tests/views_snapshot.rs`** + `tests/snapshots/*.snap` — `insta` + `TestBackend` snapshot suite (10 tests).

### 12.3 Public API surface

The crate exposes via `lib.rs`: re-exports `AppState`, `Pane` (from `app`), and `handle_key`, `Action` (from `event`); plus the locally-defined `pub fn views_render`, `pub fn run_tui_smoke`, `pub fn apply_action`, `pub async fn run_tui`, the `TuiError` enum, and the `Result<T>` alias.

| Item | Signature | Purpose |
|---|---|---|
| `run_tui` | `pub async fn run_tui(service: SeeleService) -> Result<()>` | Interactive entrypoint; the only function the CLI calls for normal use. |
| `run_tui_smoke` | `pub fn run_tui_smoke(service: SeeleService) -> Result<()>` | Headless one-frame render on a 120×30 `TestBackend`; backs `seele tui --smoke`. |
| `views_render` | `pub fn views_render(f: &mut ratatui::Frame, state: &AppState)` | Re-exports `views::render` so snapshot tests can drive rendering without the event loop. |
| `apply_action` | `pub fn apply_action(state: &mut AppState, service: &SeeleService, action: Action)` | Dispatch table mapping an `Action` to state mutations and service calls. |
| `handle_key` | `pub fn handle_key(state: &AppState, key: KeyEvent) -> Option<Action>` | Pure keymap; `None` means "ignore this key". |
| `AppState`, `Pane`, `Action` | enums/struct (below) | The state model, view enum, and intent enum. |
| `TuiError` / `Result<T>` | error enum / alias | Crate error type. |

`TuiError` (`lib.rs:60-68`, with the `Result<T>` alias at `lib.rs:70`) has three variants: `Io(#[from] io::Error)`, `Storage(#[from] seele_storage::StorageError)`, and `Service(String)`. In practice only `Io` is ever produced — `enter_tui`, `Terminal::new`, and `terminal.draw` all return `io::Error`. `Storage` and `Service` exist for completeness but are never constructed in this crate, because all service-call errors are caught and routed to `AppState.last_error` rather than propagated (see §12.7).

### 12.4 State model — `AppState` and `Pane`

`Pane` (enum at `app.rs:15-22`, impl at `app.rs:24-44`) is a `Copy` enum of the five top-level views: `Home`, `Browse`, `Search`, `Detail`, `Stats`. It carries two helpers: `label()` returns the display name and `hotkey()` returns the digit `'1'..'5'` (with Detail mapped to `'4'`, even though `'4'` is not a global keymap entry — see §12.5). Both the global header and the keymap derive their behavior from these.

`AppState` (`app.rs:46-68`) is the single mutable owner of all UI state:

| Field | Type | Meaning / invariant |
|---|---|---|
| `current` | `Pane` | Active pane; starts `Home`. |
| `prev_pane` | `Option<Pane>` | Pane Detail was opened from. `back()` returns here, then `take()`s it. Lets Search→Detail→Esc land back on Search. |
| `should_quit` | `bool` | Set by `Action::Quit`; breaks the event loop. |
| `stats` | `Option<StatsResponse>` | Cached stats; `None` until first refresh. |
| `browse` | `Vec<ObservationDto>` | Cached recent-observation list (≤ `BROWSE_LIMIT`). |
| `selected` | `usize` | Highlighted index in the *active* list pane (Browse OR Search). Shared between them; reset to 0 on pane switch. |
| `search_query` | `String` | Free-text query buffer in Search. |
| `search_results` | `Vec<SearchHitDto>` | Live search hits. |
| `detail` | `Option<ObservationDto>` | Observation shown in Detail. |
| `last_error` | `Option<String>` | Last service/parse error, surfaced red in the footer. |

`BROWSE_LIMIT` (`app.rs:271`) is `50` (typed `u32`) and is used as the limit for both the browse list (`refresh_browse`) and the search-result limit (`run_search`).

**Navigation helpers (pure):**
- `switch_pane(p)` (`app.rs:86-93`): resets `selected` to 0 *only if* `p != self.current`, so re-entering the same pane (e.g. `2` while on Browse) preserves the cursor.
- `move_selection(delta: i32)` (`app.rs:95-105`): clamps within `[0, len-1]` of `current_list_len()`; on an empty list it pins `selected = 0`.
- `jump_to(idx)` (`app.rs:107-114`): `g`/`G` use this with `0` / `usize::MAX`; clamps to `len-1` (and to 0 on an empty list).
- `current_list_len()` (`app.rs:116-122`): returns `browse.len()` on Browse, `search_results.len()` on Search, else `0` — which is why `j/k` are no-ops on Home/Stats/Detail.

### 12.5 Keymap — `event.rs`

`Action` (`event.rs:11-29`) is the side-effect-free intent enum: `Quit`, `SwitchPane(Pane)`, `Up`, `Down`, `Top`, `Bottom`, `Refresh`, `SearchEditAppend(char)`, `SearchEditBackspace`, `SearchSubmit`, `SearchEscape`, `OpenDetail`, `Back`.

`handle_key(state, key)` is a pure function (no mutation, no service) precisely so the keymap is unit-testable, and it resolves keys in a deliberate precedence order (`event.rs:31-56`):

1. **Ctrl-C always wins** → `Quit`, regardless of pane (`event.rs:33-35`).
2. **If on Search**, `search_key` runs first so printable input (digits, `/`, `q`) is captured as query text and never leaks to global hotkeys (`event.rs:38-44`).
3. **`global_key`** — `1/2/3/5` switch panes, `q` quits, bare `r` (no modifiers) refreshes (`event.rs:45-47`).
4. **Pane-specific fallthrough** — `detail_key` on Detail, `list_key` on Browse; Search and Home/Stats decline everything else (returning `None`) (`event.rs:48-55`).

`global_key` (`event.rs:58-68`) maps the digit hotkeys at `event.rs:60-63`:

```rust
KeyCode::Char('1') => Some(Action::SwitchPane(Pane::Home)),
KeyCode::Char('2') => Some(Action::SwitchPane(Pane::Browse)),
KeyCode::Char('3') => Some(Action::SwitchPane(Pane::Search)),
KeyCode::Char('5') => Some(Action::SwitchPane(Pane::Stats)),
```

Note that `'4'` is intentionally absent from `global_key`: Detail is reached only via Enter on a list row, never by a direct digit hotkey. (This is an inference from the code — the source `global_key` carries no comment to that effect; the only commentary near it is the `event.rs:52-53` comment explaining why Home/Stats decline non-global keys.)

- `list_key` (`event.rs:70-80`): `j`/`Down`→`Down`, `k`/`Up`→`Up`, `g`→`Top`, `G`→`Bottom`, `Enter`→`OpenDetail`, `/`→switch to Search.
- `search_key` (`event.rs:82-94`): `Enter`→`SearchSubmit`, `Esc`→`SearchEscape`, `Backspace`→`SearchEditBackspace`, `Up`/`Down` move the result cursor, and any non-Ctrl `Char(c)`→`SearchEditAppend(c)`. The Ctrl guard (`event.rs:89`) means Ctrl-C still reaches the quit check above.
- `detail_key` (`event.rs:96-101`): `Esc`, `h`, or `Backspace`→`Back`.

Two fixes attributed in source comments to external review (Cloven, 2026-05-11) are encoded here. The test `jk_on_home_or_stats_return_none` (`event.rs:228-239`) covers a NIT where `j/k` fired no-op moves on listless panes — Home/Stats now return `None`. `SearchEscape` (variant + doc comment at `event.rs:23-26`; implemented in `AppState::search_escape` at `app.rs:255-262`) was added so Orlando is never trapped in Search without Ctrl-C: the first Esc clears a non-empty query/results, a second (empty) Esc exits to Browse.

### 12.6 Rendering flow — `views/`

`views::render` (`views/mod.rs:20-40`) splits `f.area()` vertically into a 1-row header (`Constraint::Length(1)`), a flexible body (`Constraint::Min(0)`), and a 1-row footer (`Constraint::Length(1)`), then matches `state.current` to dispatch the body.

- **Header** (`render_header`, `views/mod.rs:42-66`): `SEELE v{CARGO_PKG_VERSION}` (rendered with `env!("CARGO_PKG_VERSION")` and `theme::header()`, bold cyan) followed by `[1] Home [2] Browse … [5] Stats`; the active pane uses `theme::selected()` (bold cyan), the rest `Style::default().fg(theme::DIM)`.
- **Footer** (`render_footer`, `views/mod.rs:68-85`): a per-pane hint string styled `theme::footer_hint()` (dim), and if `last_error.is_some()`, an appended red `error: {err}`.

| Pane | Layout & content |
|---|---|
| **Home** (`home.rs`) | Vertical split: an 8-row stats card (`Block` titled `home`, leading line `📊 Stats`) showing Total memories (= `observations.active + observations.deleted`), Active, Soft-deleted, Projects; below it a horizontal 50/50 "by kind" / "by scope" breakdown from `stats.observations.by_type` / `by_scope`. When `stats == None`, the card shows `(no stats yet — press 'r' to refresh)` and each breakdown shows `(no data)`. |
| **Browse** (`browse.rs`) | A bordered `List` titled `browse (N active)`. Empty → hint `` (no observations — save one with `seele save <title> <content>` and press 'r') ``. Each row: dimmed `short_id`, `[project|-]`, `truncate(title, 60)`. Uses a fresh `ListState` each frame with `select(Some(selected.min(browse.len().saturating_sub(1))))`. |
| **Search** (`search.rs`) | 3-row "query" box rendering `search_query` + a cyan block cursor `█`, then a "results (N)" list. Each hit: `short_id`, a `★{score:.2}` colored by `score_color` (green ≥0.7, yellow ≥0.4, else DarkGray), `[project|-]`, `truncate(title, 60)`. Empty → `(type a query and press Enter)`. |
| **Detail** (`detail.rs`) | If `detail == None`, a single hint in a `detail`-titled block. Otherwise a 7-row header card (`Block` titled `memory {id}`, lines: Created/Updated via `ms_to_human`, Project, Scope, Topic key), a flexible "body" Paragraph (`Constraint::Min(4)`; bold title, blank line, then `content.lines()`, `Wrap { trim: false }`), and a 5-row "metadata" box with `serde_json::to_string_pretty(&o.metadata)` (falling back to `"{}"` on serialization failure). |
| **Stats** (`stats.rs`) | Fuller than Home: a 7-row "observations" totals box (active/soft-deleted/projects), a 6-row "sessions" box (`sessions.total` only), and a 50/50 "by type"/"by scope" breakdown. Empty state (`stats == None`) shows a single `(no stats yet — press 'r' to refresh)` hint in a `stats`-titled block. Note: `sessions.by_status` is populated in the DTO but not rendered. |

Both Browse and Search reimplement identical private `short_id`/`truncate` helpers (duplicated code, a candidate for extraction). `short_id` returns the id unchanged when `id.len() <= 10`, otherwise `format!("{}…", &id[..10])` — i.e. the first 10 **bytes** plus an ellipsis. The byte slice is safe here only because ULIDs are ASCII; `truncate` correctly counts `chars()` and, when over `max`, keeps `max - 1` chars and appends `…`.

### 12.7 Control flow: event loop, actions, concurrency, error handling

`run_tui` (`lib.rs:75-87`) calls `enter_tui` (raw mode + `EnterAlternateScreen` + `CrosstermBackend` over `io::stdout`, `lib.rs:89-96`), installs a `TerminalGuard`, builds `AppState::new()`, warms it with `state.refresh(&service)`, and enters `run_event_loop`.

The loop (`run_event_loop`, `lib.rs:110-139`) is a single `async` task — there is **no spawned task, no shared-state locking, no `Mutex`**. (The `lib.rs` module docstring at lines 8-11 describes it as a "tokio `select!` loop", but the actual implementation is a plain `loop` that awaits `EventStream::next()`, not a `tokio::select!`.) It draws, then awaits the next crossterm event:

```rust
// lib.rs:119-137
loop {
    terminal.draw(|f| views::render(f, state))?;

    let Some(event_res) = events.next().await else {
        break;
    };
    let evt = match event_res {
        Ok(e) => e,
        Err(_) => continue,
    };
    if let crossterm::event::Event::Key(key) = evt {
        if let Some(action) = handle_key(state, key) {
            apply_action(state, service, action);
        }
    }
    if state.should_quit {
        break;
    }
}
```

Non-`Key` events (resize, mouse, focus) fall through and trigger a redraw on the next iteration; transport errors from the stream are skipped with `continue`. The service calls inside `apply_action` are **synchronous and blocking** (rusqlite is sync), so a slow query blocks the loop — acceptable for a single-user local DB but a known characteristic.

`apply_action` (`lib.rs:145-168`) is the dispatch table. Key behaviors: `SwitchPane(p)` calls `switch_pane` then `refresh_for_current_pane` so Home/Browse/Stats numbers stay current on entry without a dedicated refresh key; `Up/Down/Top/Bottom` mutate `selected` (via `move_selection`/`jump_to`); `Refresh` re-runs both stats and browse; `SearchEditAppend`/`SearchEditBackspace` edit the query buffer in place; `SearchSubmit`→`run_search`, `SearchEscape`→`search_escape`; `OpenDetail` and `Back` move between panes.

**Error philosophy — never crash the loop.** Every service error is converted to a string in `last_error` instead of propagating:
- `refresh_stats` (`app.rs:147-157`) / `refresh_browse` (`app.rs:159-179`) set `last_error` on `Err` and clear it on success.
- `run_search` (`app.rs:181-205`) first applies the same anti-empty-query gate as the HTTP/MCP transports via `enforce_search_query_or_filter(&req)` (`seele-http/src/service.rs:442`), which rejects a blank query when `project`, `scope`, and `type` are all `None`; the TUI passes no filters, so an empty query always yields a `search: BadRequest(...)` string in the footer rather than a service call.
- `open_detail_for_selection` (`app.rs:207-240`) parses the selected row's `id` string into a `SeeleId`; a parse failure becomes a footer error (`bad id '{id_str}': {e}`). The comment at `app.rs:220-223` records this as a Cloven 2026-05-11 [MEDIO] fix: the bad-id path used to propagate through `apply_action` and terminate the loop. A `get_observation` returning `Ok(None)` produces `observation {id_str} not found`.

Terminal restoration is via the `TerminalGuard` `Drop` impl (`lib.rs:103-108`; the unit struct is declared at `lib.rs:101`), which best-effort `disable_raw_mode()` + `LeaveAlternateScreen` with results ignored (`let _ = ...`) — so even a panic inside the loop hands the terminal back cleanly (the doc comment notes a panicking guard inside `Drop` would be worse than a silent restore).

### 12.8 How it crosses the SEELE boundary

The TUI reads exclusively through four `SeeleService` methods (all in `seele-http/src/service.rs`): `stats() -> Result<StatsResponse>` (`service.rs:396`), `list_observations(ListRequest) -> Result<Vec<ObservationDto>>` (`service.rs:163`), `search_observations(SearchRequest) -> Result<SearchResponse>` (`service.rs:136`), and `get_observation(SeeleId) -> Result<Option<ObservationDto>>` (`service.rs:159`). (Each returns the service's `Result`, not a bare value; the TUI handles the `Err`/`None` branches as described in §12.7.) It performs **no writes** — there is no save/delete/link path in the current code; CLAUDE.md lists "TUI editing in-place" as a candidate v0.2 feature, confirming the read-only scope today. Data crossing the boundary is the HTTP DTO layer (`ObservationDto`, `SearchHitDto`, `StatsResponse`, `CountBucket`, `ObservationStats`, `SessionStats`, and the `SearchResponse { hits, count }` wrapper from which only `hits` is consumed), so the TUI shares serialization shapes with the REST API even though it never serializes them. `ListRequest` is built with all filters `None`, `include_deleted: false`, and `limit = Some(50)`; `SearchRequest` is built with all filters `None`, `limit = Some(50)`, `include_purist: false`, `include_annotations: false`, `score_boost_multiplier: 0.0`, `max_vec_distance: None`, and `query` from `search_query`.

### 12.9 Snapshot testing (insta + TestBackend)

`tests/views_snapshot.rs` (10 tests) builds an `AppState` by hand (no service), renders one frame onto a fixed **120×30** `TestBackend`, dumps the cell buffer to a plain `String` via `buffer_dump` (which walks `buf[(x,y)].symbol()` row by row, dropping styling), and asserts with `insta::assert_snapshot!`. The width is pinned and the module docstring explains the rationale so "CI on Linux/Mac/Windows produces identical frame buffers" — auto-resizing terminals would otherwise make snapshots noisy. Stub builders `stub_stats`, `stub_observation` plus inline `SearchHitDto` fixtures cover every pane and its empty state, the colored-score path, and the footer error line. The smoke entrypoint `run_tui_smoke` (`lib.rs:46-57`) reuses `TestBackend` for an end-to-end (service-backed) one-frame render used by `seele tui --smoke` (the hidden CLI flag at `crates/seele-cli/src/commands/tui.rs:14`).

### 12.10 Edge cases, gotchas, and known limitations

- **Stale snapshots / version drift (active).** All 10 committed `.snap` files hardcode the header line `SEELE v0.1.0`, but the workspace version (`Cargo.toml:20`, inherited as `CARGO_PKG_VERSION`) is now `0.2.0`. The tree contains 10 corresponding `.snap.new` files (e.g. `views_snapshot__browse_renders_list_with_selection_at_zero.snap.new:6` shows `SEELE v0.2.0`) — pending, unaccepted insta regenerations. This means the snapshot suite currently **fails** until someone runs `cargo insta accept`; embedding `env!("CARGO_PKG_VERSION")` in the header makes these tests version-coupled by design (a fragility worth flagging for §23).
- **`'4'` has no global hotkey** (`event.rs:60-63` lists only `1/2/3/5`): Detail is reachable only via `Enter` on a list row, which is a slight asymmetry versus the `[4] Detail` shown in the header (the header iterates all five `Pane` values, including Detail, and labels each with its `hotkey()`). This appears intentional but the source carries no comment stating so.
- **Shared `selected` index**: Browse and Search share one cursor field. Switching between them resets to 0 (via `switch_pane`), but `/` from Browse → Search and `Esc`-back paths rely on that reset to avoid an out-of-range index; the renderers also defensively `.min(len.saturating_sub(1))`.
- **`short_id` byte-slices `&id[..10]`** — safe only because ULIDs are ASCII; a non-ASCII id would panic on a non-char-boundary slice. Not reachable with real data, but a latent assumption.
- **Search has no live debounce.** The `search.rs:1-4` module doc records that ADR-07 originally specified live-debounced search and that it was deferred to "ship alongside the real ONNX embedder in Sprint-05" (attributed to a Cloven 2026-05-11 [MEDIO] spec/code mismatch); the current code requires an explicit `Enter` (`SearchSubmit`) to run a query.
- **Blocking queries on the UI thread**: synchronous rusqlite calls run inside the async loop; a heavy query briefly freezes input.
- **`sessions.by_status` is fetched but never rendered** in Stats (the only session metric shown is `sessions.total`), a minor unused-data gap.
- **No config-driven theme yet**: `theme.rs:1` notes "v0.2 may load from config"; the palette (`ACCENT = Cyan`, `DIM = DarkGray`, `ERR = Red`) is hardcoded as the only theme.
- **No mouse support**: only `Event::Key` is handled in the loop; other crossterm events are silently ignored and only cause a redraw on the next iteration.


---

## 13. Multi-Machine Sync — `seele-sync`

`seele-sync` is SEELE's mechanism for moving observations between machines without a server: an export writes a single self-describing, content-addressed, gzipped JSON file ("chunk") that can be committed to a git repository, and an import folds that chunk into another machine's local SQLite database. The design goal is *git-friendly, conflict-free, idempotent* sync: re-running an import is a no-op, two machines that hold the same observation collapse to one row, and the whole import is atomic so a crash cannot leave a half-populated database.

### 13.1 Place in the crate graph

`seele-sync` is a near-leaf crate. Per the workspace layering it depends only on `seele-core` (for the `Observation` domain type, `SeeleId`/ULID, `Scope`, `ObservationType`, `Metadata`) and `seele-storage` (for `ObservationStore`, `ChunkStore`, the raw-save path via `RawSaveInput`/`RawSaveOutcome`, and `ObservationQuery`). It has no dependency on `seele-search`, `seele-embedder`, or any transport crate. Notably the sync feature is **CLI-only**: the only caller in the whole workspace is `crates/seele-cli/src/commands/sync.rs` (`seele sync export|import`); there is no HTTP endpoint and no MCP tool for sync. Confirmed by grepping `crates/` for `export_to_dir`/`import_from_file` — exactly three files reference them: the CLI command, the crate's own `src/lib.rs`, and `tests/roundtrip.rs` (the property suite `tests/properties.rs` only exercises `compute_chunk_id`, so it does not appear in that grep). Embeddings are **not** transported: a chunk carries only the observation rows, so the destination must re-embed (or rely on FTS) after import. The crate is `Cargo.toml`-declared as `description = "Git-friendly compressed-chunk sync between machines."`.

External crates pulled in (`crates/seele-sync/Cargo.toml`): `flate2` (gzip encode/decode), `sha2` + `hex` (SHA-256 chunk id), `serde` + `serde_json` (payload (de)serialization), `chrono` (timestamps), `thiserror` (the error enum), `rusqlite` + `r2d2` (only to convert their error types into `SyncError`), and `tracing`. All are pulled in via `*.workspace = true`. Dev-deps: `tempfile` and `proptest`.

### 13.2 Public API surface

| Item | Signature (abridged) | Purpose |
|------|----------------------|---------|
| `ChunkPayload` | `struct { format_version: u32, seele_version: String, exported_at: DateTime<Utc>, project: Option<String>, observations: Vec<Observation> }` | The on-disk payload, serialized to JSON then gzipped. |
| `ChunkPayload::CURRENT_FORMAT_VERSION` | `const u32 = 1` | The format version this build writes and the max it will read. |
| `ExportFilter` | `struct { project: Option<String> }` (derives `Default`) | Selects which observations to export. v0.1 filters by project only. |
| `ExportReport` | `struct { chunk_id, path, observation_count, bytes_on_disk }` | Returned by `export_to_dir`. |
| `ImportReport` | `struct { chunk_id, path, outcome, observation_count_saved, observation_count_already_present, observation_count_skipped_chunk_level }` | Returned by `import_from_file`. |
| `ImportOutcome` | `enum { Imported, AlreadyImported }` (serde `snake_case`) | Whether the chunk was newly applied or skipped via the ledger. |
| `SyncError` | `enum { Io, Json, Storage, InvalidChunk(String) }` (`thiserror`) | Crate error type. `pub type Result<T> = std::result::Result<T, SyncError>`. |
| `build_chunk` | `fn(&ObservationStore, ExportFilter) -> Result<ChunkPayload>` | Query observations into a payload (no I/O). |
| `compute_chunk_id` | `fn(&ChunkPayload) -> Result<String>` | The canonical SHA-256 content hash (64 hex chars). |
| `export_to_dir` | `fn(&ObservationStore, &Path, ExportFilter) -> Result<ExportReport>` | Build + hash + gzip + write `<chunk_id>.json.gz`. |
| `read_chunk_file` | `fn(&Path) -> Result<(String, ChunkPayload)>` | Decode a chunk file, validate version + filename integrity. |
| `import_from_file` | `fn(&ObservationStore, &ChunkStore, target_key: &str, &Path) -> Result<ImportReport>` | Idempotent, atomic import into the local DB. |

`SyncError` carries `#[from]` conversions for `std::io::Error` (`Io`), `serde_json::Error` (`Json`), and `seele_storage::StorageError` (`Storage`); additional manual `From<rusqlite::Error>` and `From<r2d2::Error>` impls funnel those into `SyncError::Storage` (by routing through `StorageError::from`) so the import's connection/transaction plumbing can use `?` directly (`crates/seele-sync/src/lib.rs:49-59`). Note that `write_chunk_file` is a private `fn` (not part of the public surface); the public export entry point is `export_to_dir`.

### 13.3 The chunk format and its versioning

`ChunkPayload` is the wire format. `format_version` is an explicit, forward-checked integer (`CURRENT_FORMAT_VERSION = 1` for v0.1); `seele_version` (set in `build_chunk` from `env!("CARGO_PKG_VERSION")`) and `exported_at` (set to `Utc::now()`) are *informational only*. The split matters: only `format_version`, `project`, and the observations participate in the content hash, while `seele_version` and `exported_at` do not — this is what makes the chunk id stable across runs and across SEELE versions.

A chunk file is named `<chunk_id>.json.gz` and is the gzip of `serde_json::to_vec(&payload)`. The format is *git-friendly* for several converging reasons: (1) it is a single regular file per export, so a `git add` + commit + push is the entire transport; (2) the filename is content-addressed, so a logically-identical export from a second machine produces the *same filename*, and git deduplicates / shows no diff churn; (3) two diverging exports never overwrite each other — distinct content means distinct chunk id means distinct file, so there is no merge conflict at the git layer; and (4) gzip keeps the committed artifact small. The format being binary (gzipped) means git stores it as a blob rather than diffing line-by-line, which is the intended trade-off.

### 13.4 The content-addressed chunk id

`compute_chunk_id` defines the equivalence relation "same observation set" (`crates/seele-sync/src/lib.rs:162-174`):

```rust
// crates/seele-sync/src/lib.rs:162
pub fn compute_chunk_id(payload: &ChunkPayload) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(payload.format_version.to_le_bytes());
    if let Some(p) = &payload.project {
        hasher.update(p.as_bytes());
    }
    hasher.update([0u8]);                       // null terminator after project (unconditional)
    let mut obs = payload.observations.clone();
    obs.sort_by_key(|o| o.id.to_string());      // canonical ordering by ULID
    let bytes = serde_json::to_vec(&obs)?;
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}
```

Key properties, each pinned by a property test in `crates/seele-sync/tests/properties.rs`:

- **Order-independent** — observations are sorted by stringified ULID before hashing, so in-memory ordering does not change the id (`chunk_id_ignores_observation_order`).
- **Ignores informational fields** — `exported_at` and `seele_version` are not hashed (`chunk_id_ignores_exported_at_and_version`).
- **Content-sensitive** — any change to an observation flips the id (`chunk_id_changes_when_observation_content_changes`).
- **Filter-sensitive** — the `project` filter is mixed in (with a single unconditional `0u8` terminator written after the optional project bytes), so `None` vs `Some("p")` yield different ids even over the same rows (`chunk_id_changes_when_project_filter_changes`). The terminator is unconditional, so it disambiguates `Some("p")` (`p\0`) from `None` (`\0`); it does **not** distinguish `Some("")` from `None` (both reduce to the lone `\0`).

Because the full `Observation` is serialized into the hash, the chunk id is sensitive to mutable fields like `revision_count`, `last_seen_at`, and `metadata` — two machines must hold *byte-identical* observation rows (not just the same id) to produce an identical chunk. That is a deliberate "same snapshot" semantics, not "same id" semantics. (Note this is a property of the *export hash* only; on *import* the raw-save path does not carry those mutable fields — see 13.7.)

### 13.5 Export flow

`export_to_dir` (`lib.rs:181-199`) is the top-level export: it calls `build_chunk`, computes the id, `create_dir_all`s the target directory, joins `<chunk_id>.json.gz`, captures `observation_count` from `payload.observations.len()`, writes via `write_chunk_file`, then `stat`s the file (`std::fs::metadata(&path)?.len()`) for `bytes_on_disk`. `build_chunk` (`lib.rs:136-150`) constructs an `ObservationQuery { project: filter.project.clone(), include_deleted: false, ..Default::default() }` and calls `ObservationStore::list` — so soft-deleted rows (`deleted_at` set) are excluded from export. `write_chunk_file` (`lib.rs:201-208`, private) gzips with `Compression::default()` over a `BufWriter<File>`, and explicitly `encoder.finish()?.flush()?` to guarantee the stream is fully drained before the report's `metadata().len()` reads the size. Writing is idempotent at the filesystem layer: a same-id file is simply overwritten (via `File::create`) with content equal by construction.

### 13.6 Import flow and the Cloven CRITICO-1 fix

`import_from_file` (`lib.rs:281-345`) is the heart of the crate and embodies several review findings from Cloven (the external review agent). The ordered control flow:

1. **Decode + validate**: `read_chunk_file` (`lib.rs:216-244`) opens the file, `GzDecoder`s it over a `BufReader`, `read_to_end` + `serde_json::from_slice` to deserialize the `ChunkPayload`, rejects any `format_version` greater than `CURRENT_FORMAT_VERSION` with `SyncError::InvalidChunk`, recomputes the id, and — *only* when the filename stem (the part before `.json.gz`) is exactly 64 ASCII hex chars — verifies it matches the recomputed id, raising `chunk_id mismatch` otherwise. This integrity check catches silent corruption / tampering but tolerates arbitrarily-named files. It returns `(recomputed_id, payload)`, so the recomputed id (not the filename) is what flows downstream. Tests `read_chunk_file_rejects_unknown_future_format_version` (version `999`) and `read_chunk_file_detects_filename_chunk_id_mismatch` (file renamed to 64 zeros) cover both guards.
2. **Cheap ledger pre-check**: `chunks.was_imported(target_key, &chunk_id)?` — if the `(target_key, chunk_id)` pair is already in the `sync_chunks` ledger, return early with `outcome = AlreadyImported`, `observation_count_skipped_chunk_level = payload.observations.len()`, and zero saves / zero already-present. This avoids even opening a transaction for an already-applied chunk.
3. **Atomic apply**: otherwise, grab one pooled connection (`observations.pool().get()?`) and open a single `Transaction`. Loop over every observation, building a `RawSaveInput` and calling `ObservationStore::save_raw_in_tx`, then `ChunkStore::mark_imported_in_tx`, then `tx.commit()`.

The single-transaction wrapping is **the Cloven CRITICO-1 fix** (Sprint-04 mid-review, 2026-05-10, commit `d152842`). Before it, the import ran each save on its own connection/transaction, so a crash mid-loop left the destination partially populated *and* with no ledger mark — re-running would then double-apply. The doc comment states the contract explicitly:

```text
// crates/seele-sync/src/lib.rs:250
// **Atomic per chunk**: every observation save + the `sync_chunks`
// ledger write run inside a single SQLite transaction. A crash
// mid-loop rolls the entire chunk back ...
```

The atomicity is verified two ways: `import_atomicity_corrupt_chunk_leaves_db_untouched` overwrites the chunk file with `b"corrupt"` so gzip decode fails before the tx opens (DB and ledger both stay empty), and `import_rolls_back_when_save_in_tx_fails` asserts the *positive* invariant that on a healthy import the saves and the ledger row commit together (5 rows + exactly 1 ledger entry whose `chunk_id` matches the report). (The latter test's own comment notes it settles for the positive assertion because portably forcing a mid-tx commit failure is hard; the negative path is covered by the corrupt-chunk test.)

```rust
// crates/seele-sync/src/lib.rs:305 — one connection across the whole import
let mut conn = observations.pool().get()?;
let tx = conn.transaction()?;
// ... loop of save_raw_in_tx(&tx, input) ...
ChunkStore::mark_imported_in_tx(&tx, target_key, &chunk_id)?;
tx.commit()?;
```

### 13.7 Why the raw-save path (the other two Cloven findings)

Each observation is applied through `ObservationStore::save_raw_in_tx`. The public associated method is declared at `crates/seele-storage/src/observations.rs:146` and delegates to the free function `save_raw_in_tx` at `crates/seele-storage/src/observations.rs:582`, **not** the normal `save`/`save_in_tx` pipeline. The raw path emits `INSERT OR IGNORE INTO observations(...)` keyed on the ULID `id` (column list: `id, int_id, session_id, type, title, content, tool_name, project, scope, topic_key, normalized_hash, revision_count, duplicate_count, last_seen_at, created_at, updated_at, metadata`), bypassing privacy-strip, the topic-key upsert, and the dedup window. Notably it hardcodes `revision_count = 0` and `duplicate_count = 0`, recomputes `normalized_hash` from the content, and sets `last_seen_at = updated_at` — so the export-time values of those mutable fields are **not** carried across; only the fields present on `RawSaveInput` (`id`, `session_id`, `kind`, `title`, `content`, `tool_name`, `project`, `scope`, `topic_key`, `created_at`, `updated_at`, `metadata`) ride through. It returns `RawSaveOutcome::{Inserted, AlreadyExisted}`, which the import counts into `observation_count_saved` vs `observation_count_already_present`. This choice produces three guarantees:

- **Source ULIDs ride through unchanged.** The pre-fix import minted *new* ULIDs via the normal save pipeline, which broke cross-machine dedup. With raw save the id is preserved (test `import_preserves_source_observation_ids`).
- **Cross-machine dedup is automatic.** Two machines holding the same observation export it under the same ULID; imported into a common destination, the second one hits `INSERT OR IGNORE` on the shared `id` and is silently skipped — no duplicate row. This is the **Cloven CRITICO** dedup fix (test `re_import_under_different_target_keys_does_not_duplicate`: first import saves 2 / already-present 0, second under a different `target_key` is still `outcome = Imported` but saves 0 / already-present 2, and the destination row count stays 2).
- **No destination data loss on topic-key collision.** The legacy `save_in_tx` topic-key upsert would `UPDATE` a destination row that collides on `(project, scope, topic_key)`, silently overwriting its title/content/metadata with the chunk's. Raw save inserts under the source's own ULID and `INSERT OR IGNORE` leaves the colliding destination row untouched, so both coexist (test `import_does_not_overwrite_destination_topic_key_collision`: both `FROM_SOURCE` and `ALREADY_IN_DEST` survive, count = 2).

A separate **Cloven MEDIO-1** finding is handled inline: `session_id` is *dropped* on import (`session_id: None` in the `RawSaveInput`). Destinations enforce `PRAGMA foreign_keys = ON` and `observations.session_id REFERENCES sessions(id)`, so carrying an unknown session id would abort the whole transaction. Sessions are not part of the v0.1 chunk payload; the crate doc notes the session breadcrumb is also stamped into `metadata` by upstream consumers (MNEMA), making the strip safe, and flags that Sprint-05 may bump `format_version` to 2 to carry thread-level history. Test `import_strips_unknown_session_id_so_fk_constraint_does_not_fire` confirms the import succeeds and the destination row's `session_id` is `NULL` (the source-side test first creates a real session via `SessionStore::start` so the source FK is satisfied locally).

### 13.8 The per-target ledger (`sync_chunks`) — persistence side

Idempotency is anchored by `ChunkStore` in `crates/seele-storage/src/chunks.rs`, backed by table `sync_chunks` (`crates/seele-storage/src/migrations/V001__initial_schema.sql:184-189`). The schema as written (the `-- unix millis` annotation below is editorial; the actual SQL has no inline comments, but `imported_at` is in fact stored via `Utc::now().timestamp_millis()`):

```sql
CREATE TABLE sync_chunks (
    target_key  TEXT NOT NULL,
    chunk_id    TEXT NOT NULL,
    imported_at INTEGER NOT NULL,           -- (editorial) unix millis
    PRIMARY KEY (target_key, chunk_id)
) WITHOUT ROWID;
```

The composite primary key `(target_key, chunk_id)` is what makes re-import a no-op *per node*. `target_key` is a free-form node identifier supplied by the operator via `seele sync import --target-key <hostname|UUID>` (a required `#[arg(long)]` field on `ImportArgs` in `crates/seele-cli/src/commands/sync.rs:31-32`); there is no auto-derivation. `ChunkStore` exposes: `was_imported` (the cheap pre-check, a `SELECT count(*)` that returns `count == 1`), `mark_imported` (standalone `INSERT OR IGNORE`, returns `inserted == 1` i.e. whether newly recorded), `mark_imported_in_tx` (an *associated function* taking a `&rusqlite::Transaction` — the tx-scoped variant the import actually uses so the ledger row commits atomically with the saves), and `list_for_target` (used by tests and any tooling to read a node's import history, ordered `imported_at DESC`). Both insert paths use `INSERT OR IGNORE`, so even a concurrent double-import degrades gracefully to one ledger row. Note there are **two independent idempotency layers**: the chunk-level ledger (skip the whole chunk for a known `(target_key, chunk_id)`) and the row-level `INSERT OR IGNORE` on observation `id` (collapse individual rows even across different target keys). The `ImportReport` counters are deliberately split across these two abstraction levels — `observation_count_skipped_chunk_level` is non-zero only when the *ledger* short-circuits, while `observation_count_already_present` reflects per-row collisions during an actually-applied chunk; a Cloven NIT (`lib.rs:109-111`) documents exactly this distinction.

### 13.9 Edge cases, gotchas, invariants, and known limitations

- **The ~1 MB chunk-splitter is a v0.2 candidate, not shipped.** The crate doc states it plainly (`crates/seele-sync/src/lib.rs:19-20`): *"v0.1 ships a single chunk per export. Sprint-05 may add a size-bound splitter (~1 MB per chunk) if exports grow large enough to matter."* CLAUDE.md lists "sync chunk splitter (~1 MB cap)" among v0.2 features. Today a large export is one large file, loaded entirely into memory on both export and import.
- **Whole-payload-in-memory.** Both `serde_json::to_vec(payload)` (export) and `read_to_end` + `from_slice` (import) materialize the full observation set in RAM; there is no streaming. This is the practical motivation for the future splitter.
- **No deletes / tombstones travel.** Export skips `deleted_at` rows and import only ever inserts (`INSERT OR IGNORE`); a deletion on machine A is never propagated to machine B. Sync is additive-only.
- **Mutable bookkeeping fields are not carried.** `revision_count`, `duplicate_count`, `normalized_hash`, `last_seen_at`, and `deleted_at` are absent from `RawSaveInput`; on import they are reset/recomputed at the destination (`revision_count`/`duplicate_count` to 0, `last_seen_at` to the row's `updated_at`, `normalized_hash` from content). Only when the source row hits an existing `id` is it skipped wholesale (no merge).
- **Filename integrity check is conditional.** `read_chunk_file` only enforces the id-vs-filename match when the stem is exactly 64 ASCII hex chars; a deliberately mis-named file (e.g., `mychunk.json.gz`) bypasses that check but is still validated by version and successfully decoded — the recomputed id is used for the ledger regardless.
- **`format_version` is strictly forward-checked, not range-checked.** Any value `> 1` is rejected; there is no per-version migration path yet, which is acceptable while only v1 exists.
- **No concurrency model beyond SQLite.** The import takes a single pooled connection and one transaction; SQLite's own locking is the concurrency boundary. Two simultaneous imports of the same chunk on the same node either both succeed idempotently or one waits on the DB lock.

### 13.10 File-by-file map

- **`crates/seele-sync/src/lib.rs`** — the entire crate: error enum + conversions (`lib.rs:37-59`), `ChunkPayload`/reports/`ImportOutcome`, `ExportFilter`, the export functions (`build_chunk`, `compute_chunk_id`, `export_to_dir`, private `write_chunk_file`), and the import functions (`read_chunk_file`, `import_from_file`). The module doc comment (`lib.rs:1-20`) is the authoritative narrative of the design and explicitly names the Cloven CRITICO/MEDIO fixes. Inline `#[cfg(test)]` unit tests cover chunk-id determinism (`chunk_id_is_deterministic_for_same_observations`, `chunk_id_differs_when_observations_differ`).
- **`crates/seele-sync/tests/properties.rs`** — proptest suite (`cases: 32` each) pinning the four `compute_chunk_id` invariants: order-independence, ignoring `exported_at`/`seele_version`, content-sensitivity, and project-filter-sensitivity. It imports only `compute_chunk_id` and `ChunkPayload` — no store / no I/O.
- **`crates/seele-sync/tests/roundtrip.rs`** — end-to-end integration tests on real temp SQLite DBs (source + destination): gzip magic-byte check (`export_writes_a_gzipped_file_with_chunk_id_as_name`), clean import, ledger no-op re-import, atomicity on corrupt chunk, the positive atomicity assertion, project-filter exclusion, source-id preservation, cross-target dedup, topic-key non-overwrite, session-id stripping, future-version rejection, and filename mismatch.
- **`crates/seele-sync/Cargo.toml`** — declares deps (flate2, sha2/hex, serde/serde_json, chrono, thiserror, tracing, rusqlite/r2d2 for error conversion) and dev-deps (tempfile, proptest), all via `*.workspace = true`.
- **`crates/seele-storage/src/chunks.rs`** (persistence side) — `SyncChunk` row type + `ChunkStore` with `mark_imported`, `mark_imported_in_tx`, `was_imported`, `list_for_target` over the `sync_chunks` table.
- **`crates/seele-storage/src/observations.rs`** (raw-save side) — `RawSaveInput`/`RawSaveOutcome` (`:49-70`), the public `ObservationStore::save_raw_in_tx` (`:146`) delegating to the free `save_raw_in_tx` (`:582`) that runs the `INSERT OR IGNORE`.
- **`crates/seele-cli/src/commands/sync.rs`** (sole caller) — clap `SyncCmd { Export, Import }`, wires `--project` (on `ExportArgs`) / `--target-key` (on `ImportArgs`), and emits the report via `output::emit_split` (text or `--json`).


---

## 14. Agent Setup Wizard — `seele-setup`

`seele-setup` is the crate behind the `seele setup` subcommand: a one-shot wizard that wires SEELE's MCP stdio server (`seele mcp`) into a coding agent's MCP configuration file. Its job is narrow and deliberately offline — it never spawns the MCP server, never talks to the network, and never touches SQLite. It only locates the per-agent config path, computes the JSON entry SEELE needs, merges it idempotently into whatever already exists, optionally backs up the old file, and writes the result atomically.

### 14.1 Position in the crate graph

`seele-setup` is a near-leaf crate. Its only internal dependency is `seele-core` (declared in `Cargo.toml`), and even that is essentially nominal — nothing in `lib.rs` or `agents.rs` imports a `seele_core::` symbol; the dependency exists for workspace coherence (version/edition inheritance) rather than for code reuse. Its sole consumer inside SEELE is the `seele-cli` binary, specifically `crates/seele-cli/src/commands/setup.rs`, which is dispatched from `crates/seele-cli/src/app.rs` (`Command::Setup(args) => commands::setup::run(args, &out).await`, app.rs:108). The wizard is not invoked by `seele-mcp`, `seele-http`, the TUI, or any service-layer code; it is pure filesystem-config plumbing reachable only through the CLI.

External crates and why (`Cargo.toml`):

| Crate | Pin | Used for |
|---|---|---|
| `serde` | workspace | `#[derive(Serialize)]` on `InstallReport` / `Outcome` so `--json` can emit reports. |
| `serde_json` | workspace | Parse and re-serialize the agent config (`Value`, `Map`, `json!`, `to_string_pretty`). |
| `thiserror` | workspace | The `SetupError` enum. |
| `tracing` | workspace | Declared but unused in the current source (no `tracing::` call sites in this crate). |
| `chrono` | workspace | Timestamps for backup filenames (`timestamp_millis`) and tmp filenames (`timestamp_nanos_opt`). |
| `dirs` | `"5"` | `dirs::home_dir()` to resolve the canonical home directory per OS. |
| `tempfile` | workspace (dev) | E2E tests create a throwaway home so no real config is touched. |

Note the `Cargo.toml` `description` field over-promises ("Claude Code, Cursor, VS Code, OpenCode, Gemini CLI, Codex, Windsurf, Antigravity") relative to what is actually wired; the authoritative agent list lives in `AgentKind::all()`.

### 14.2 File-by-file map

- **`src/lib.rs`** — public API surface and the shared filesystem primitives. Declares the `SetupError` enum, the `Result<T>` alias, the `InstallReport` / `Outcome` / `InstallOptions` data types, the top-level `install()` entry point, the listing helpers (`all_agent_names`, `implemented_agent_names`), and the two crate-internal write primitives `write_atomic` and `backup_file` (plus the private helper `sidecar_tmp_path`). The module doc comment is also the canonical spec of which agents are implemented vs. skeleton.
- **`src/agents.rs`** — the per-agent dispatch and the JSON-merge installer. Defines `AgentKind` (the 8-variant enum), its `as_str` / `from_name` / `all` / `is_implemented` methods, the public `install_agent` dispatcher, the shared `install_mcp_json` merge routine, and the JSON-loading helper `load_json_object` plus a `type_name` formatter. Contains two unit tests covering `AgentKind` name round-tripping.
- **`tests/install_e2e.rs`** — 12 end-to-end tests that drive `seele_setup::install()` against a `tempfile::TempDir` used as `home_override`. They assert outcomes (Created/Added/Unchanged/Updated/DryRun), the exact config paths, the non-clobbering merge, idempotency, backup presence/absence, dry-run leaving disk untouched, skeleton `NotImplemented`, unknown-agent error, invalid-JSON and non-object-root errors, and the `all_agent_names` listing.
- **`docs/AGENT-SETUP.md`** — user-facing documentation: the implemented/skeleton table, per-agent invocation examples, the manual config snippet (including the `--tool-prefix mnema` ENGRAM-compat variant), `--dry-run` usage, and a `tools/list` stress-test recipe.

### 14.3 Public API surface

**`pub fn install(agent_name: &str, opts: &InstallOptions) -> Result<InstallReport>`** (lib.rs:126) — the single top-level entry point. It resolves `agent_name` via `AgentKind::from_name`; an unrecognized name yields `SetupError::UnknownAgent`, then delegates to `install_agent(kind, opts)`. A declared-but-skeleton agent will pass name resolution but fail inside `install_agent` with `SetupError::NotImplemented`.

**`pub fn all_agent_names() -> Vec<&'static str>`** (lib.rs:134) — every recognized name, implemented or skeleton (drives `--list`).

**`pub fn implemented_agent_names() -> Vec<&'static str>`** (lib.rs:140) — only the names with a real installer; filters via `AgentKind::is_implemented()`. This is what `--all` iterates over so skeletons are never reported as errors.

**`pub fn install_agent(kind: AgentKind, opts: &InstallOptions) -> Result<InstallReport>`** (agents.rs:74) — re-exported from `agents`; the dispatcher mapping `AgentKind` to a concrete config path and installer.

**`pub enum AgentKind`** (agents.rs:22) — the canonical identifier enum, re-exported at crate root.

Crate-internal (`pub(crate)`) primitives: `write_atomic` (lib.rs:168) and `backup_file` (lib.rs:195).

#### `InstallOptions` (lib.rs:83)

| Field | Type | Meaning |
|---|---|---|
| `dry_run` | `bool` | When `true`, compute the would-be result but write nothing. |
| `backup` | `bool` | When `true` (default), copy the existing config to `<path>.<ext>.bak.<unix_ms>` before writing. |
| `home_override` | `Option<PathBuf>` | Test seam: overrides `dirs::home_dir()`. `None` uses the real home. |
| `seele_binary` | `Option<PathBuf>` | The binary path written into the config's `command`. `None` falls back to the literal `"seele"` (assumes on PATH). |

`Default` (lib.rs:96) sets `dry_run: false, backup: true, home_override: None, seele_binary: None` — note the default is **backup-on**. Two helpers: `home()` (lib.rs:108) returns the override or `dirs::home_dir().ok_or(SetupError::NoHome)`; `seele_binary_str()` (lib.rs:115) lossily stringifies the override path or returns `"seele"`.

#### `InstallReport` (lib.rs:53) and `Outcome` (lib.rs:65)

`InstallReport` is `Serialize` so `--json` emits it directly. Fields: `agent: String`, `config_path: PathBuf`, `outcome: Outcome`, `preview: Option<String>` (the would-be JSON, populated only on dry-run), `backup_path: Option<PathBuf>` (set only when a backup was actually written).

`Outcome` (`#[serde(rename_all = "snake_case")]`) is the five-way classification of what happened:

| Variant | Meaning |
|---|---|
| `Created` | Config file did not exist; SEELE added as sole/initial entry. |
| `Added` | File existed without a `seele` entry; entry added, other content preserved. |
| `Unchanged` | File already had an identical `seele` entry; nothing written (idempotent). |
| `Updated` | File had a `seele` entry that differed; replaced in place. |
| `DryRun` | `dry_run = true`; nothing written, `preview` populated. |

#### `SetupError` (lib.rs:34)

| Variant | Trigger |
|---|---|
| `UnknownAgent(String)` | `agent_name` not in `AgentKind::all()`. |
| `NotImplemented(String)` | A skeleton agent was requested; message reads `agent {0} not implemented in v0.1 (planned for v0.2)`. |
| `NoHome` | `dirs::home_dir()` returned `None` and no override given. |
| `Io(std::io::Error)` | Any filesystem error (`#[from]`). |
| `InvalidExistingConfig { path, detail }` | Existing config is not valid JSON, root is not an object, or the top-level key is not an object. |
| `Json(serde_json::Error)` | Serialization failure (`#[from]`); practically unreachable since we build the JSON ourselves. |

### 14.4 `AgentKind`: the dispatch table

`AgentKind` (agents.rs:22) has 8 `Copy` variants. The three implemented MCP consumers — `ClaudeCode`, `Cursor`, `Windsurf` — come first; the five skeletons — `OpenCode`, `Aider`, `Cody`, `Continue`, `Zed` — follow, declared (per the inline comment) "so `--agent <name>` gives a useful error instead of 'unknown agent'." The `as_str` mapping is the CLI-accepted spelling: `claude-code`, `cursor`, `windsurf`, `opencode`, `aider`, `cody`, `continue`, `zed`. `from_name` is the inverse (linear scan over `all()`), `all()` returns the canonical ordered slice, and `is_implemented()` (agents.rs:69) is the single predicate dividing real installers from skeletons:

```rust
// agents.rs:69
pub fn is_implemented(&self) -> bool {
    matches!(self, Self::ClaudeCode | Self::Cursor | Self::Windsurf)
}
```

This predicate is the *hide/filter* mechanism the ALTO Cloven finding (see §14.7) demanded. Skeletons are never *removed* from `all()` — they remain visible in `--list` tagged `[skeleton (v0.2)]` — but `--all` iterates only `implemented_agent_names()`, so a bulk install never emits five `NotImplemented` errors.

`install_agent` (agents.rs:74) maps each implemented kind to `install_mcp_json` with its config path and the top-level key `"mcpServers"`; the catch-all arm `skeleton => Err(SetupError::NotImplemented(...))` handles all five skeletons uniformly.

### 14.5 Per-agent config locations

All three implemented agents share the same JSON shape (a top-level `"mcpServers"` object mapping server name → `{command, args}`), so they share `install_mcp_json`; only the path differs. Paths are built relative to `opts.home()`:

| Agent | `--agent` value | Config path (relative to home) | Top-level key |
|---|---|---|---|
| Claude Code | `claude-code` | `.claude.json` | `mcpServers` |
| Cursor | `cursor` | `.cursor/mcp.json` | `mcpServers` |
| Windsurf | `windsurf` | `.codeium/windsurf/mcp_config.json` | `mcpServers` |

The entry SEELE injects under `mcpServers.seele` is constructed at agents.rs:111:

```rust
// agents.rs:111
let seele_entry = json!({
    "command": opts.seele_binary_str(),
    "args": ["mcp"],
});
```

So a fresh Claude Code config becomes `{"mcpServers":{"seele":{"command":"seele","args":["mcp"]}}}` (or with an absolute path when `--seele-binary` / `seele_binary` is set). The wizard never adds `--tool-prefix mnema`; that ENGRAM-compat variant is documented as a manual edit only (`docs/AGENT-SETUP.md`).

### 14.6 `install_mcp_json`: control flow

`install_mcp_json` (agents.rs:105) is the heart of the wizard. Step by step:

1. **Build the desired entry** (`seele_entry`, agents.rs:111).
2. **Load existing config** via `load_json_object(&config_path)` → `(root: Map, existed: bool)`. A missing file returns `(empty map, false)`; an empty/whitespace-only file returns `(empty map, true)`; a file that parses but is not a JSON object errors with `InvalidExistingConfig { detail: "root is not an object (got <type>)" }`; unparseable content errors with `detail: "not valid JSON: <e>"`. This is the explicit guard against clobbering arbitrary JSON.
3. **Get or create the `mcpServers` object** via `root.entry(key).or_insert_with(|| Value::Object(...))`, then `as_object_mut()`; if the key exists but is not an object, error `InvalidExistingConfig { detail: "`mcpServers` is not an object" }`.
4. **Classify the outcome** by comparing the *current* `seele` entry to `seele_entry` (agents.rs:130):

```rust
// agents.rs:130
let outcome = match servers.get("seele") {
    Some(existing) if existing == &seele_entry => Outcome::Unchanged,
    Some(_) => Outcome::Updated,
    None if existed => Outcome::Added,
    None => Outcome::Created,
};
```

5. **Insert** the entry unconditionally (`servers.insert("seele", seele_entry)`) and pretty-print the whole root (`to_string_pretty`). The insert is harmless in the `Unchanged` case because the value is byte-identical.
6. **Dry-run short-circuit**: if `opts.dry_run`, return immediately with `outcome: Outcome::DryRun` (the classified `Created/Added/...` is discarded), `preview: Some(new_content)`, `backup_path: None`. Nothing is written.
7. **Unchanged short-circuit**: if `outcome == Unchanged`, return without writing and without backing up (`backup_path: None`) — this is the idempotency guarantee. Re-running is a no-op that never produces backup churn.
8. **Backup then write**: if `opts.backup`, call `backup_file(&config_path)` (which is a no-op returning `None` if the file doesn't exist, e.g. the `Created` path), then `write_atomic(&config_path, &new_content)`. Return the report with `backup_path` from the backup step.

The ordering — classify before insert, short-circuit Unchanged before backup, backup before write — is what makes the wizard simultaneously idempotent, non-clobbering, and recoverable.

### 14.7 Atomic write (Cloven CRITICO-2 fix)

`write_atomic` (lib.rs:168) is the closure of Cloven's Sprint-04 CRITICO-2 finding: the original implementation used a direct `std::fs::write`, which can leave `~/.claude.json` half-written if the process dies mid-write — and a reader (Claude Code) observing a truncated config is the real corruption bug. The fix is the classic write-to-sidecar-then-rename:

```rust
// lib.rs:168
pub(crate) fn write_atomic(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = sidecar_tmp_path(path);
    let _ = std::fs::remove_file(&tmp_path); // best-effort stale cleanup
    std::fs::write(&tmp_path, content)?;
    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e.into());
    }
    Ok(())
}
```

Key details: parent directories are created first (this is what materializes `~/.cursor/` or `~/.codeium/windsurf/` on a clean machine). The tmp path is a *sibling* of the destination — `sidecar_tmp_path` (lib.rs:184) appends `.seele-tmp-<pid>-<nanos>` to the full path string (e.g. `…/.claude.json.seele-tmp-1234-1700000000000000000`). Using a sibling (not the system temp dir) keeps the rename on the same volume, which is the precondition for atomicity: on POSIX `rename(2)` is atomic for same-volume renames; on Windows `std::fs::rename` issues `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`, atomic for same-volume files. The `<pid>-<nanos>` suffix avoids collisions between concurrent runs, and the best-effort `remove_file` clears any stale tmp from a prior crash so `std::fs::write` always starts fresh. On rename failure, the tmp is cleaned up before returning the error.

The doc comment (lib.rs:148–167) is candid about the **residual race**: if Claude Code is running, it may write `~/.claude.json` between SEELE's in-memory load and the rename, and the rename then clobbers Claude Code's change (last-write-wins). The atomic rename eliminates *partial-write corruption* but not this lost-update window. It is mitigated by the pre-write backup and by `seele setup` being a re-runnable one-shot. The documented v0.2 plan (lib.rs:166–167, echoed in `CLAUDE.md`) is to delegate to `claude mcp add` when the `claude` CLI is present, closing the gap.

### 14.8 Backup behavior

`backup_file` (lib.rs:195) returns `Ok(None)` when `path` does not exist (so a `Created` install never backs up), otherwise copies to a timestamped sibling and returns `Some(backup)`. The filename rule (lib.rs:200–206): take the existing extension; if empty, the backup extension is `bak.<unix_ms>`; otherwise it is `<ext>.bak.<unix_ms>`. Because `~/.claude.json`'s "extension" (per `Path::extension`) is `json`, the backup is `~/.claude.json.bak.<ms>` — matching what `docs/AGENT-SETUP.md` advertises. The timestamp is `chrono::Utc::now().timestamp_millis()`, so repeated runs within the same millisecond could in principle overwrite a backup, but in practice each run produces a distinct file. Backups are only made on the `Added`/`Updated` paths (the `Created` path has no file to copy; the `Unchanged` path short-circuits before backup; `dry_run` never reaches backup).

### 14.9 Idempotency and edge cases

Idempotency is exact and value-based: the `seele` entry is compared structurally (`existing == &seele_entry`) against the freshly built entry, so a second identical run yields `Unchanged` with no write and no backup (asserted by `claude_code_re_running_is_idempotent_unchanged`, which checks `second.backup_path.is_none()`). A stale entry (e.g. an old binary path) yields `Updated` *and* a backup (`claude_code_updates_when_seele_entry_differs`). Non-`seele` keys and unrelated top-level keys are preserved verbatim (`claude_code_adds_to_existing_config_without_clobbering` checks `other-tool` and `user_settings.theme` survive). Notable edge cases and gotchas:

- **Non-clobbering safety**: any non-object root or non-object `mcpServers` aborts with `InvalidExistingConfig` and leaves the file byte-for-byte intact (`invalid_existing_config_errors_loudly_without_clobbering` re-reads and asserts equality).
- **Empty file** is treated as `existed = true` with an empty map (`load_json_object` returns `(Map::new(), true)` for whitespace-only content, agents.rs:189). Because there is no `seele` key but `existed` is true, the match arm `None if existed => Outcome::Added` fires, so a touched-but-empty config reports `Added`, not `Created`. Emptiness counts as existence.
- **Outcome on dry-run is always `DryRun`**, discarding the underlying Created/Added/Updated/Unchanged classification — the preview JSON is the only signal of what *would* change.
- **`seele_binary` default `"seele"`** assumes the binary is on PATH; agents launching MCP servers without the user's PATH may fail to find it, which is why `--seele-binary /abs/path` exists and the docs recommend an absolute path.

### 14.10 CLI wiring (`seele setup`)

`crates/seele-cli/src/commands/setup.rs` translates flags into `InstallOptions` and renders reports. The `Args` struct exposes `--agent <name>`, `--all`, `--list`, `--dry-run`, `--no-backup`, and `--seele-binary <path>`. Flag handling:

- `--list` (handled first): prints all `all_agent_names()`, tagging each `[implemented]` or `[skeleton (v0.2)]` by membership in the `implemented_agent_names()` set; returns early. Honors `--json` via `output::emit_split`.
- Otherwise build `InstallOptions { dry_run: args.dry_run, backup: !args.no_backup, home_override: None, seele_binary: args.seele_binary }`. Note `home_override` is hard-coded `None` in the CLI — the override is purely a test seam. `--no-backup` inverts to `backup: false`.
- `--all`: iterates `implemented_agent_names()` only, collecting `Ok` reports and `Err` strings separately; prints per-agent `agent: Outcome → path` lines and an `errors:` block if any. This is the skeleton-aware path the ALTO Cloven finding produced.
- Single-agent: requires `--agent` (else `anyhow!("--agent <name> or --all required")`), runs `install`, prints one report line.

`--agent` and `--all` are documented as mutually exclusive (the `Args` doc comment), though the code checks `--all` first and would silently ignore a simultaneously-passed `--agent`. The CLI does not validate exclusivity beyond ordering.

### 14.11 Connection to the rest of SEELE

The data crossing the boundary is minimal and outbound only: the wizard writes a `command`/`args` pair that, when the agent later launches it, runs `seele mcp` — handing control to the `seele-mcp` crate's stdio JSON-RPC server (19 `seele_*` tools, §8). There is no runtime coupling; `seele-setup` produces a config file and exits, and (verified by grep) nothing in `seele-mcp` references `seele-setup` or `seele_setup`. Note that `docs/AGENT-SETUP.md` lists a `seele_setup_status` tool among its (also stale) "19 tools" enumeration, but **no such tool exists in source**: the authoritative tool table in `crates/seele-mcp/src/tools.rs` registers exactly 19 tools (`seele_save`, `seele_search`, `seele_show`, `seele_list`, `seele_update_metadata`, `seele_delete`, `seele_restore`, `seele_link`, `seele_stats`, `seele_session_start`, `seele_session_end`, `seele_session_summary`, `seele_capture_passive`, `seele_judge`, `seele_compare`, `seele_suggest_topic_key`, `seele_projects`, `seele_doctor`, `seele_version`) and `seele_setup_status` is not one of them. The count "19" is correct; the specific names in `AGENT-SETUP.md` are not, so do not rely on that doc for the tool inventory — see §8. The two unit tests in `agents.rs` (`agent_kind_round_trips`, `unknown_agent_name_returns_none`) plus the 12 E2E tests in `tests/install_e2e.rs` constitute the crate's verification, all running against `tempfile` homes so no developer's real `~/.claude.json` is ever mutated.


---

## 15. Project Detection — `seele-project`

`seele-project` is a leaf crate whose single job is to answer one question: *given the current working directory, what is the name of the project this memory belongs to?* SEELE partitions observations by `project` (the column that scopes searches, `seele list`, `seele projects`, and topic-key upserts keyed on `(project, scope, topic_key)`), so a good project name matters for retrieval quality. When an AI agent calls `seele save` without an explicit `--project`, the intent is that SEELE infers one. This crate implements that inference as a deterministic 5-case cascade ported from ENGRAM (`crates/seele-project/src/lib.rs:1`).

In the crate graph it sits near the bottom. Its `Cargo.toml` declares `seele-core` as its only internal dependency (and it does not actually use any core symbol in `lib.rs` — the dependency is structural, keeping the crate inside the workspace's core-anchored layering), plus four external crates: `serde`/`serde_json` (config parsing + `Serialize` on the result), `thiserror` (typed errors), and `tracing` (debug logging on timeout paths). Dev-dependency `tempfile` backs the E2E tests. The entire crate is a single 355-line `lib.rs` plus one integration test file.

### Public API surface

| Item | Signature | Purpose |
|------|-----------|---------|
| `detect` | `pub fn detect(cwd: &Path) -> Result<ProjectName>` | Runs the full 5-case cascade against `cwd`. |
| `parse_remote_basename` | `pub fn parse_remote_basename(url: &str) -> Option<String>` | Pure URL→basename parser (case 2 helper); public so it can be unit-tested and reused. |
| `ProjectName` | `pub struct { name: String, source: DetectSource }` | The result: the inferred name plus which case produced it. Derives `Debug, Clone, PartialEq, Eq, Serialize`. |
| `DetectSource` | `pub enum { Config, GitRemote, GitRoot, GitChild, DirBasename }` | Which of the five cases fired. `Copy`; `#[serde(rename_all = "snake_case")]`. |
| `ProjectError` | `pub enum { NoBasename, Io(io::Error), InvalidConfig(String) }` | Typed errors via `thiserror`. |
| `Result<T>` | `pub type = std::result::Result<T, ProjectError>` | Crate-local result alias. |

`ProjectName.source` is deliberately carried alongside the name: the crate doc notes it is "util para debugging (`seele doctor` lo expone) y para tests" (`lib.rs:39-40`). The `Serialize` derive plus `snake_case` rename means `DetectSource::GitRemote` serializes as `"git_remote"`, ready to surface in JSON diagnostics.

`ProjectError` has only three variants, and only two are practically reachable: `InvalidConfig` (malformed `.seele/config.json`) and `Io` (a non-`NotFound` filesystem error reading that file). `NoBasename` is the pathological case where even `cwd.file_name()` yields nothing — effectively only a filesystem root. `detect` is documented to "always return `Ok` because case 5 (basename) is reachable" except for that one degenerate path (`lib.rs:76`).

### The 5-case detection cascade

The ordering is load-bearing. The crate doc states the rationale plainly: "El orden importa: cada caso solo corre si el anterior fallo. Esto evita pisar el `project` correcto con un fallback" (`lib.rs:4`). Each case is tried in turn; the first to yield `Some`/a value wins and short-circuits.

| # | `DetectSource` | Mechanism | Helper |
|---|---------------|-----------|--------|
| 1 | `Config` | Read `<cwd>/.seele/config.json`, parse `{"project": "..."}`, accept if non-blank | `read_config_override` |
| 2 | `GitRemote` | `git -C <cwd> remote get-url origin` → basename of URL | `git_remote_name` → `parse_remote_basename` |
| 3 | `GitRoot` | `git -C <cwd> rev-parse --show-toplevel` → basename of path | `git_root_basename` |
| 4 | `GitChild` | Scan depth-1 subdirs of `cwd` for one containing `.git/` | `git_child_scan` |
| 5 | `DirBasename` | `cwd.file_name()` (final fallback) | inline in `detect` |

The control flow in `detect` is a flat chain of `if let Some(name) = … { return … }` blocks, ending with the basename fallback (`lib.rs:79-122`):

```rust
// crates/seele-project/src/lib.rs:79-110 (condensed)
pub fn detect(cwd: &Path) -> Result<ProjectName> {
    if let Some(name) = read_config_override(cwd)? { return Ok(ProjectName { name, source: DetectSource::Config }); }
    if let Some(name) = git_remote_name(cwd)       { return Ok(ProjectName { name, source: DetectSource::GitRemote }); }
    if let Some(name) = git_root_basename(cwd)     { return Ok(ProjectName { name, source: DetectSource::GitRoot }); }
    if let Some(name) = git_child_scan(cwd)        { return Ok(ProjectName { name, source: DetectSource::GitChild }); }
    let name = cwd.file_name().and_then(|s| s.to_str()).map(String::from).ok_or(ProjectError::NoBasename)?;
    Ok(ProjectName { name, source: DetectSource::DirBasename })
}
```

Note that only case 1 can propagate an error (the `?` on `read_config_override`); cases 2–4 return `Option` and silently fall through on any failure, which is why an unreadable git or a flaky filesystem degrades gracefully to the basename.

#### Case 1 — `.seele/config.json` override

`read_config_override` (`lib.rs:132`) joins `cwd/.seele/config.json` and reads it. A `NotFound` error is mapped to `Ok(None)` (no config is normal); any other IO error becomes `ProjectError::Io`. The file is parsed into `struct ConfigFile { #[serde(default)] project: Option<String> }`. Parse failure becomes `ProjectError::InvalidConfig(format!("{path:?}: {e}"))` — a *loud* error rather than a silent fall-through, so a typo in the config surfaces immediately (the test `case1_invalid_json_errors_loudly` asserts the message contains `"invalid config.json"`). The extracted project is run through `.filter(|s| !s.trim().is_empty())`, so a blank or whitespace-only value (`"   "`) is treated as absent and falls through to later cases — confirmed by `case1_empty_project_falls_through`.

#### Cases 2 & 3 — git-derived signals

Both shell out to `git` via the shared `run_git` helper. Case 2 (`git_remote_name`, `lib.rs:146`) runs `remote get-url origin` and feeds the URL into `parse_remote_basename`. Case 3 (`git_root_basename`, `lib.rs:227`) runs `rev-parse --show-toplevel` and takes `PathBuf::file_name()` of the result. Remote wins over toplevel because the remote name is the more canonical, machine-stable identity of a repo (two clones of the same repo into differently-named directories should map to the same project).

`parse_remote_basename` (`lib.rs:199`) is a careful, allocation-light URL normalizer that handles every common git URL shape. Its steps, in order: trim; bail on empty; strip query/fragment (`split(['?','#'])`); strip a trailing `/`; strip a trailing `.git`; take the segment after the last `:` (handles `git@host:org/repo`), then the segment after the last `/`; trim and reject if empty. The documented coverage (`lib.rs:191`):

```rust
// crates/seele-project/src/lib.rs:191-198
// - `git@github.com:org/repo.git`      → `repo`
// - `https://github.com/org/repo.git`  → `repo`
// - `ssh://git@host:22/group/repo`     → `repo`
// - `file:///tmp/repo`                 → `repo`
// - `bare/path/repo`                   → `repo`
```

Seven unit tests in `lib.rs:301-353` lock these down (https-with-suffix, ssh-short, no-suffix, ssh-with-port, file URL, trailing slash, empty→`None`).

##### The git subprocess timeout (Cloven MEDIO fix)

`run_git` (`lib.rs:160`) is the concurrency-sensitive heart of the crate, added to close a MEDIO finding from the Sprint-04 Cloven review: "`seele-project` git subprocess sin timeout (cerrado, 1500ms thread+mpsc cap)" (CLAUDE.md). The mechanism: spawn `git -C <cwd> <args>` on a worker `thread`, hand the `Command::output()` result back over an `mpsc::channel`, and block the caller only via `rx.recv_timeout(GIT_SUBPROCESS_TIMEOUT)`.

```rust
// crates/seele-project/src/lib.rs:160-189 (condensed)
fn run_git(cwd: &Path, args: &[&str], timeout: Duration) -> Option<String> {
    let (tx, rx) = mpsc::channel();
    // ... clone cwd + args into the worker ...
    thread::spawn(move || { let _ = tx.send(Command::new("git").arg("-C").arg(&cwd).args(&worker_args).output()); });
    match rx.recv_timeout(timeout) {
        Ok(Ok(output)) if output.status.success() => { /* trim stdout, None if empty else Some */ }
        Ok(_) => None,                                            // git ran but failed / non-zero exit
        Err(mpsc::RecvTimeoutError::Timeout)      => { tracing::debug!(args = ?owned_args, "..."); None }
        Err(mpsc::RecvTimeoutError::Disconnected) => None,        // worker panicked before sending
    }
}
```

`GIT_SUBPROCESS_TIMEOUT = Duration::from_millis(1500)` (`lib.rs:37`). The constant's rationale is documented at length: it is sized for "git is unresponsive" not "git is slow today" — a healthy local git on Windows pays ~50–300ms in process startup alone, so 1500ms never trips on a normal repo, while a stuck git (credential prompt, NFS hang) cannot block `detect` past ~3 seconds total across cases 2 and 3 combined (`lib.rs:28-36`).

The critical design decision: **a timed-out subprocess is abandoned, not killed.** The comment justifies this — git on this read path "has no externally visible side effects," and the orphaned worker thread "cleans itself up when git eventually exits" (`lib.rs:155-159`, `28-36`). Inputs are cloned into owned `PathBuf`/`Vec<String>` before the `move` closure so the worker outlives the caller's borrow safely. Success requires both `output.status.success()` *and* non-empty trimmed stdout; an empty result returns `None` (so a repo with no `origin` remote correctly falls through from case 2 to case 3).

#### Case 4 — git child scan

`git_child_scan` (`lib.rs:260`) handles the "monorepo parent" / "I'm one level above the actual repo" case: it reads the immediate children of `cwd` and returns the name of the first subdirectory that contains a `.git/` entry. It is bounded three ways to keep it cheap:

| Bound | Constant | Value | Effect |
|-------|----------|-------|--------|
| Time budget | `SCAN_TIMEOUT` | `200ms` | Aborts the scan if elapsed > budget. |
| Visit cap | `SCAN_MAX_DIRS` | `20` | Aborts after 20 *non-noise* dirs visited. |
| Noise filter | `NOISE_DIRS` | 11 names | Skips dirs that never hold project roots. |

`NOISE_DIRS` (`lib.rs:246`): `node_modules`, `target`, `.git`, `vendor`, `.venv`, `venv`, `__pycache__`, `dist`, `build`, `.next`, `.nuxt`. Each loop iteration first checks the timeout and visit cap (both log `tracing::debug!` and return `None` when hit), skips non-directories, skips noise dirs (without incrementing `visited`), then increments `visited` and tests `path.join(".git").exists()`. Iteration order follows `read_dir`, which is filesystem-defined and *not* sorted — so when multiple sibling repos exist, "first found" is non-deterministic across platforms. `case4_skips_noise_dirs` proves a `node_modules/foo/.git` is ignored in favor of a sibling `legit/.git`; `case4_child_scan_finds_first_subdir_with_dot_git` proves the basic find. Note this case detects only `.git` presence, not language manifests — despite a sprint planning note suggesting language-agnostic manifest detection, the shipped code keys solely on `.git/`.

#### Case 5 — directory basename

The final fallback: `cwd.file_name()` → `&str` → `String`, source `DirBasename`. Reachable for any directory that isn't a filesystem root. Only here can `detect` return `Err(ProjectError::NoBasename)`.

### Normalization, concurrency, and error handling

"Normalization" in this crate is light and per-case rather than a single shared pass: each case trims and basenames its own way. Cases 2/3/4/5 all reduce to a last-path-segment basename; case 2 additionally strips `.git`, query strings, and trailing slashes. There is **no** lowercasing, no whitespace-to-dash slugification, and no length cap — the detected name is used verbatim as the `project` value. The only emptiness guards are the case-1 blank filter and the empty-string rejections inside `parse_remote_basename`/`run_git`.

Concurrency is confined to `run_git`: one detached `std::thread` per git invocation, communicating over a single-shot `mpsc` channel, with the timeout enforced on the receive. There is no `tokio`/async here — `detect` is a blocking synchronous call, intended to run before/around the async save path in the CLI. Error handling is bimodal: case 1 is fail-loud (typed errors propagate), cases 2–4 are fail-soft (any failure → `Option::None` → next case), guaranteeing the cascade always reaches a usable answer.

### Wiring into SEELE — and the known gap

This is the crate's most important caveat for an improver. **`detect` is not called from production code.** A repo-wide search shows the only call sites of `detect`, `parse_remote_basename`, `DetectSource`, etc. are inside the crate's own `tests/detect_e2e.rs`. Notably, `seele-cli/Cargo.toml:24` *does* declare `seele-project = { path = "../seele-project" }` as a dependency, yet nothing under `crates/seele-cli/src/` imports the `seele_project` module — the link is present but unused. The CLI `save` command exposes `--project` as a plain `Option<String>` and, when omitted, passes that `None` straight through to `SaveRequest.project` (`save.rs:52`) — it never invokes `seele-project`. The code comment is explicit (`crates/seele-cli/src/commands/save.rs:20-24`):

```rust
// crates/seele-cli/src/commands/save.rs:20-24
/// Project name. If omitted, the caller is expected to set
/// `--project ""` explicitly when this matters; project detection
/// (`seele-project`) wires in Sprint-04 Bloque D.2.
#[arg(long)]
pub project: Option<String>,
```

That wiring "nunca shipeó." The Sprint-05 devlog records it as a known v0.2 gap: "Project detection auto-wired en `seele save` — la crate `seele-project` está testeada end-to-end pero no se llama desde el CLI… esa wiring nunca shipeó. Gap conocido, v0.2." (`docs/aegis/devlogs/2026-05-11-sprint-05-polish-release.md:151`). CLAUDE.md lists "project-detection wired in `seele save`" among v0.2 candidate features. So at v0.2.0 the crate is a complete, tested, dead-code island: correct and ready, but the data it would produce never crosses into the storage layer.

When wired, the intended boundary is straightforward: `detect(cwd)` would supply the `project` field of `SaveRequest` (`crates/seele-cli/src/commands/save.rs:48-58`) whenever `--project` is absent, partitioning the resulting observation in storage. The `DetectSource` is also a natural fit for `seele doctor` diagnostics (per the doc comment), though no `doctor` call site exists today either.

### File-by-file map

- **`crates/seele-project/src/lib.rs`** — the entire implementation: crate doc describing the cascade and noise list, the timeout/scan constants, `ProjectName`/`DetectSource`/`ProjectError`/`Result` public types, the `detect` cascade, the five case helpers (`read_config_override`, `git_remote_name`+`parse_remote_basename`, `git_root_basename`, `git_child_scan`), the `run_git` thread+mpsc wrapper, and a `#[cfg(test)]` module of seven `parse_remote_basename` unit tests.
- **`crates/seele-project/Cargo.toml`** — package metadata (description: "5-case project detection algorithm (config / git remote / git root / child scan / dir basename)"); deps `seele-core`, `serde`, `serde_json`, `thiserror`, `tracing`; dev-dep `tempfile`.
- **`crates/seele-project/tests/detect_e2e.rs`** — eight integration tests exercising all five cases against real `TempDir`s and real `git` subprocesses, with `git_available()` self-skip guards for minimal CI. Covers config-wins-over-git, blank-falls-through, invalid-JSON-errors, ssh remote basename, no-remote toplevel, child scan find, noise-dir skip, and the pure basename fallback. This file is also run as smoke criterion 6 in `scripts/v0.1.0-smoke.sh:116`.

### Edge cases, gotchas, and invariants

- **Dead code at v0.2.0** — fully tested, never invoked outside tests (the headline gotcha above).
- **No name normalization** — the detected name is used raw; mixed case, spaces, and unusual characters pass through unchanged.
- **Non-deterministic child scan order** — `read_dir` order is unsorted; with multiple sibling repos, the chosen project is platform/filesystem-dependent.
- **Orphaned git processes on timeout** — by design (`lib.rs:155-159`); safe only because this is a side-effect-free read path. A hung git leaks a thread + process until git itself exits.
- **Worst-case latency ~3.2s** — two 1500ms git timeouts (cases 2+3) plus a 200ms scan (case 4) before the basename fallback, if git hangs on every call.
- **Case-1 is the only fail-loud case** — a malformed `.seele/config.json` aborts detection rather than degrading; a blank `project` value, however, is treated as absent and falls through.
- **Case 4 detects `.git/` only**, not language manifests, contrary to one sprint planning note (`genesis/plans/executed/tactica/sprint-04/00-INDEX.md:56`, which states "`seele-project` no asume Rust — es lang-agnostic. Detecta `package.json`, `Cargo.toml`, `pyproject.toml`, `go.mod`, `pom.xml`, etc.") — a divergence between plan and shipped code worth noting for anyone extending it.


---

## 16. ENGRAM / MNEMA Import — `seele-engram-import`

`seele-engram-import` is the one-shot migration subsystem that ports an existing ENGRAM/MNEMA SQLite database into a SEELE database. It implements the first decision of ADR-13 ("ENGRAM compatibility"): give consumers that already run on ENGRAM (notably MNEMA, whose persistence layer is `~/.mnema/mnema.db`) a way to move to SEELE without discarding the observations they have accumulated. The second ADR-13 decision — the runtime `--tool-prefix mnema` / `--legacy-engram-paths` aliasing shim — is handled in the MCP and HTTP crates and is out of scope for this section. (ADR-13 itself lives at `genesis/plans/arquitectura/13-engram-compatibility.md` per the migration guide's "Related" links; it is not present in this checkout, so claims attributed to it below are flagged where they go beyond what the code or `docs/ENGRAM-MIGRATION.md` directly state.)

### 16.1 Place in the crate graph

The crate sits in the upper-middle of the dependency layering. Per `crates/seele-engram-import/Cargo.toml`, it depends only on two internal crates — `seele-core` (for `SeeleId`, `ObservationType`, `Scope`, `Metadata`, imported via three `use` lines) and `seele-storage` (for `LinkInput`, `LinkStore`, `ObservationStore`, `RawSaveInput`, `RawSaveOutcome`, `StorageError`) — plus external crates `serde`, `serde_json`, `thiserror`, `tracing`, `chrono`, `rusqlite`, `r2d2`, and `ulid` (pinned `version = "1.1"`, with the `serde` feature). It deliberately does *not* depend on `seele-embedder`, `seele-search`, or any transport crate. That isolation is an observable fact from the manifest; the rationale that it is meant to let future importers (JSON, CSV) drop in side-by-side is plausible but is not stated in the crate's own source (the module docs do not mention JSON/CSV importers) and the sprint-04 devlog that would record it is not present in this checkout, so treat that rationale as unverified. Its only caller is `seele-cli` (`crates/seele-cli/src/commands/import.rs`), which wires the `seele import from-engram` subcommand to it.

### 16.2 Public API surface

The public surface is intentionally small: one struct, one constructor, one method, one report struct, and one error enum.

| Item | Signature | Purpose |
| --- | --- | --- |
| `EngramImporter<'a>` | `struct` holding `observations: &'a ObservationStore`, `_links: &'a LinkStore` | The importer, borrowing both destination stores (struct decl `lib.rs:90`, after the `#[derive]`-free doc-commented definition). |
| `EngramImporter::new` | `fn new(observations: &'a ObservationStore, links: &'a LinkStore) -> Self` | Wire the importer to a destination. **Both stores must point at the same SEELE DB** (`lib.rs:98`). |
| `EngramImporter::import_from` | `fn import_from(&self, source: &Path, dry_run: bool) -> Result<ImportReport>` | Run the migration; `dry_run=true` opens and maps but writes nothing (`lib.rs:107`). |
| `ImportReport` | `struct` (Serialize) with eight counters + `dry_run` + `errors` | Result of both real and dry-run runs (`#[derive]` at `lib.rs:60`, `struct` at `lib.rs:61`). |
| `EngramImportError` | `enum` (thiserror) | Typed hard errors (`#[derive]` at `lib.rs:40`, `enum` at `lib.rs:41`). |
| `Result<T>` | `type Result<T> = std::result::Result<T, EngramImportError>` | Crate-local result alias (`lib.rs:56`). |

Note the `_links` field is underscore-prefixed: `LinkStore` is borrowed only so the constructor signature documents the contract that links and observations share a pool, but the actual link writes go through the associated function `LinkStore::create_in_tx(&tx, ...)`, driven on the importer's own transaction rather than through the borrowed store instance.

#### `ImportReport` fields

| Field | Type | Meaning |
| --- | --- | --- |
| `source_path` | `PathBuf` | The source DB path passed in. |
| `rows_seen` | `usize` | Total rows the source `memories` table exposed. |
| `rows_inserted` | `usize` | Rows newly inserted into the destination. |
| `rows_skipped_existing` | `usize` | Rows whose preserved `id` already existed (idempotent skip). |
| `rows_invalid` | `usize` | Rows that could not be mapped (bad metadata); listed in `errors`. |
| `links_created` | `usize` | `linked_to` references resolved and inserted as `links` rows. |
| `links_already_present` | `usize` | `linked_to` tuples already present, skipped to keep re-runs idempotent. |
| `links_dangling` | `usize` | `linked_to` references whose target was not found — dropped, not fatal. |
| `dry_run` | `bool` | `true` when produced by a dry-run call. |
| `errors` | `Vec<String>` | Soft errors (per-row mapping failures). Hard errors short-circuit to `Err`. |

The struct carries **no session counter** despite the operator-guide example output mentioning sessions (see 16.10 / 16.11 — that is documentation drift, since the importer never touches sessions: `map_one` always sets `session_id: None`).

#### `EngramImportError` variants

| Variant | Trigger |
| --- | --- |
| `OpenSource { path, detail }` | `Connection::open_with_flags` on the source failed (`lib.rs:270`). |
| `BadSchema { detail }` | No `memories` table, or a required column is missing (raised in `verify_schema`, `lib.rs:278-302`). |
| `Storage(StorageError)` | `#[from]` propagation from `save_raw_in_tx` / `create_in_tx`. |
| `Sqlite(rusqlite::Error)` | `#[from]` propagation from any raw SQL call. |
| `Pool(r2d2::Error)` | `#[from]`; failure to check out a connection from the destination pool. |
| `InvalidMetadata { id, detail }` | Declared (`lib.rs:52-53`) but **never constructed** — per-row metadata failure is surfaced as a *soft* error string in `report.errors`, not this hard variant. |

### 16.3 Source schema and the schema contract

The importer opens the source database **read-only** — `Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)` inside `open_source` (`lib.rs:269-276`) — so the migration physically cannot mutate the ENGRAM file. The migration guide's rollback story rests on this: "the import itself never touched it."

`verify_schema` (`lib.rs:278-302`) enforces two preconditions. First, `sqlite_master` must contain a table named `memories`; otherwise `BadSchema { detail: "table \`memories\` not found" }`. Second, `PRAGMA table_info(memories)` (via `pragma_columns`, `lib.rs:304-313`, which reads the column name from PRAGMA result index 1) must expose the four mandatory columns `id`, `body`, `metadata`, `created_at`; any missing one yields `BadSchema { detail: "\`memories\` is missing required column \`{required}\`" }`. The optional ENGRAM/MNEMA columns the module docs name as "picked up when present" — `title`, `updated_at`, `project`, `scope`, `tool_name`, `session_id`, `type`, `topic_key` — are ignored at the SQL level on this read path; everything SEELE needs beyond the four mandatory columns is read out of the `metadata` JSON blob. (The module docs do not mention any `mnema_*` virtual columns; that claim from an earlier draft was unsupported and has been dropped.) The actual read is a flat `SELECT id, body, metadata, created_at FROM memories` (`lib.rs:329`, inside `read_and_map_rows`).

### 16.4 Field mapping (ENGRAM → SEELE)

`map_one` (`lib.rs:365-414`) converts one source row into a `RawSaveInput`. The mapping is:

| Source | Destination | Notes |
| --- | --- | --- |
| `memories.id` | `RawSaveInput.id` | Preserved if a valid ULID, else minted (see 16.5). |
| `memories.body` | `RawSaveInput.content` | Verbatim. |
| `memories.created_at` | `created_at` **and** `updated_at` | Both set to the same value; the source carries no separate updated time on this path. |
| `metadata.kind` | `RawSaveInput.kind` | Parsed via `ObservationType::from_str_relaxed` (`seele-core` `memory.rs:76`; unknown ⇒ `Memory`); also re-stored in metadata as the canonical `as_str()` form. |
| `metadata.project` | `RawSaveInput.project` | |
| `metadata.scope` | `RawSaveInput.scope` | Parsed via `Scope::from_str_strict` (`seele-core` `memory.rs:111`); on failure defaults to `Scope::Project`. |
| `metadata.topic_key` | `RawSaveInput.topic_key` | |
| `metadata.tool_name` / `metadata.tool` | `RawSaveInput.tool_name` | Either key accepted; the original key (`tool_name` or `tool`) is preserved in metadata. |
| `metadata.linked_to[]` | `links` rows (pass 2) | See 16.6. |
| any other metadata key | preserved in `RawSaveInput.metadata` | Whole blob passes through (`domain`, `axiomatic`, `verdict_id`, `earn_score`, etc.). |

Note `RawSaveInput.session_id` is hard-coded to `None` (`lib.rs:396` region): no session is ever derived from the source on this path.

`created_at` parsing is defensive (`read_created_at`, `lib.rs:343-357`): it first tries `i64` epoch-milliseconds, then an RFC-3339 string, then epoch-ms-encoded-as-text, and finally falls back to `Utc::now()`. `ms_to_dt` (`lib.rs:359-363`) guards against out-of-range millisecond values by also falling back to `Utc::now()`.

The title is synthesized by `synthesize_title` (`lib.rs:529-545`) because ENGRAM's `memories` has no first-class title on this read path: it prefers `metadata.title` if non-blank, otherwise takes the first non-empty line of the body (trimmed), truncating to 77 chars + `"..."` when the first line exceeds 80 characters (counted in `chars`, not bytes), and returns `"(untitled)"` for an empty body.

`parse_metadata` (`lib.rs:445-516`) returns a `ParsedMetadata` struct (introduced specifically to avoid a 7-tuple return type that clippy's `type_complexity` lint rejected — doc comment + struct at `lib.rs:416-428`). It treats `None`, the empty/whitespace string, and the literal `"null"` as an empty (default) metadata. A non-JSON blob returns `Err("metadata not JSON: …")`; a JSON value that is not an object returns `Err("metadata not an object: …")`. Crucially these are returned as `String` errors, which `map_one` converts into a **soft** per-row failure (`MappedRow.mapped = None`, `map_error = Some(...)`), not a hard `Err`.

### 16.5 ULID preservation and the `engram_id` breadcrumb

`resolve_dest_id` (`lib.rs:522-527`) is the heart of id handling:

```rust
// crates/seele-engram-import/src/lib.rs:522
fn resolve_dest_id(source: &str) -> (SeeleId, bool) {
    if let Ok(id) = source.parse::<SeeleId>() {
        return (id, true);
    }
    (SeeleId::new(), false)
}
```

If the source id parses as a ULID it is preserved verbatim (`preserved = true`). Otherwise — e.g. an ENGRAM cuid-style id like `mem_abc123` — a fresh `SeeleId` is minted and `map_one` stashes the original under `metadata.engram_id` so it remains recoverable post-migration (`lib.rs:384-389`). The E2E tests `import_preserves_ulid_id_when_source_id_is_a_ulid` and `import_stashes_engram_id_in_metadata_when_source_is_not_a_ulid` (`tests/import_e2e.rs:85,107`) pin both branches.

Preserving the ULID is what makes re-runs idempotent: `save_raw_in_tx` issues `INSERT OR IGNORE INTO observations(...)` keyed on the `(id)` primary key (`crates/seele-storage/src/observations.rs:590-595`). A second import of the same source therefore inserts nothing for that row (rowcount 0) and returns `RawSaveOutcome::AlreadyExisted`, counted in `rows_skipped_existing`. Minted (non-ULID-source) ids are *not* idempotent across runs by themselves — each run mints a new ULID — but this is acceptable because real ENGRAM/MNEMA data uses ULIDs; the cuid path is a fallback.

### 16.6 `linked_to[]` → first-class `links` rows

`import_from` runs in two passes over the same in-memory `Vec<MappedRow>`, inside one transaction.

**Pass 1** (`lib.rs:140-166`): insert every mappable observation via `ObservationStore::save_raw_in_tx(&tx, input.clone())`. As each row lands (whether `Inserted` or `AlreadyExisted`), the importer records `engram_id (text) → dest SeeleId` in an `id_map: HashMap<String, SeeleId>`. Rows that failed to map (`mapped == None`) increment `rows_invalid` and push their `map_error` into `errors`.

**Pass 2** (`lib.rs:189-223`): for every source row that itself mapped, resolve each entry of its `linked_to` array to a destination `SeeleId`, then insert a `links` row of type `"related_to"`. The link type is the literal string `"related_to"` in the code (`lib.rs:203,211`); the migration guide attributes the `linked_to[]` decision to ADR-13, but ADR-13 is not in this checkout, so any framing of `related_to` as a deliberate choice "over `derives_from`" is not corroborated by available sources — what is verifiable is only that the code always uses `"related_to"`. Resolution order for each target `to_engram` (`lib.rs:194-200`):

1. The current import's `id_map` (this run's rows).
2. Failing that, try `to_engram.parse::<SeeleId>()` and probe whether that ULID already exists in the destination `observations` table via `observation_exists_in_tx`. This closes a Cloven 2026-05-11 [MEDIO] finding (cited in the source comment, `lib.rs:173-176`): link targets that live in a *previous* import pass should resolve, not be counted dangling. The test `import_resolves_linked_to_against_rows_from_a_previous_pass` (`tests/import_e2e.rs:160`) guards it.
3. Anything else → **dangling**: counted in `links_dangling`, dropped, not fatal. The code notes a deliberately deferred case (`lib.rs:177-180`): a cuid-style id whose new ULID lives behind `metadata.engram_id` would require a JSON scan to resolve and is "deferred to v0.2 if real migrations need it."

Link idempotency is handled explicitly because, as the comment notes (`lib.rs:182-185`), the `links` table has **no UNIQUE constraint** on `(from_id, to_id, link_type)`. Before each insert the importer probes via `link_exists_in_tx` (`lib.rs:253-267`); a hit increments `links_already_present` and skips the insert. This closes Cloven 2026-05-11 [MEDIO 2] and is pinned by `re_running_import_does_not_duplicate_links` (`tests/import_e2e.rs:226`).

```rust
// crates/seele-engram-import/src/lib.rs:201
match to_dest {
    Some(to_dest) => {
        if link_exists_in_tx(&tx, &from_dest, &to_dest, "related_to") {
            links_already_present += 1;
        } else {
            LinkStore::create_in_tx(&tx, LinkInput {
                from_id: from_dest, to_id: to_dest,
                link_type: "related_to".to_string(), metadata: Metadata::new(),
            })?;
            links_created += 1;
        }
    }
    None => { links_dangling += 1; }
}
```

After both passes, `tx.commit()` (`lib.rs:225`) makes the whole import atomic per source file. If a source row's own observation failed to map, the pass-2 loop `continue`s and skips that row's links entirely (`lib.rs:190-193`).

### 16.7 `save_raw_in_tx`: bypassing re-embedding while preserving ids

The import never goes through `ObservationStore::save()` (the normal write path with privacy stripping, topic-key upsert, and the 24h normalized-hash dedup window — `DEDUP_WINDOW_MS = 24 * 60 * 60 * 1000`, `observations.rs:22`). Instead it uses `ObservationStore::save_raw_in_tx` (public wrapper at `crates/seele-storage/src/observations.rs:146-151`, body at `:582-619`), a raw `INSERT OR IGNORE` that takes a caller-owned `id` and timestamps and writes the row verbatim. The store doc-comment states the precondition: "the source data has already been audited / trusted" (`observations.rs:42-43`).

This is the mechanism behind the default (no-`--re-embed`) behavior described in `docs/ENGRAM-MIGRATION.md`: because the raw insert touches only the `observations` table and never invokes the embedder, the source's existing vectors carry over as-is (the guide states this works for any embedder producing 384-dim L2-normalized output, "which ENGRAM's does"). Notably, the `int_id` column is set from `input.id.as_i64()` (`observations.rs:588`). Per the repo's ID convention, `SeeleId::as_i64()` (bytes `9..16` of the ULID — the last 7 bytes / low 56 bits) is the *authoritative* ULID→vec0-rowid bridge, written into the regular **stored** `int_id` column — so writing the authoritative value keeps a preserved ULID consistent with the vec0 mapping. (`int_id` is a stored value, not virtual; `CLAUDE.md`'s "approximate virtual column" note is stale.) (The raw insert also writes a `normalized_hash` of the content and `revision_count`/`duplicate_count` of 0, but does **not** run the dedup window or topic upsert.) `--re-embed` is the rare opt-in to recompute embeddings against SEELE's own model — but see the important caveat in 16.10: in v0.1 the flag is a no-op.

### 16.8 Dry-run

When `dry_run=true` (`lib.rs:113-126`), `import_from` returns *before* checking out any connection. It still opens the source, verifies schema, and maps every row, so it can report `rows_seen`, count `rows_invalid` (`mapped.is_none()`), surface the same `errors`, and report `links_dangling` as the *total* count of all `linked_to` entries across mapped rows (`mapped.iter().map(|r| r.linked_to.len()).sum()`, `lib.rs:122`). Important subtlety: in dry-run, `links_dangling` over-reports — it counts every prospective link as dangling rather than running pass-2 resolution, and `links_created`/`links_already_present`/`rows_inserted`/`rows_skipped_existing` are all forced to 0. `dry_run_writes_nothing_but_returns_counts` (`tests/import_e2e.rs:351`) verifies the destination stays empty.

### 16.9 Error handling and atomicity model

Two error tiers coexist. **Hard errors** (cannot open source, bad schema, SQL/pool/storage failures, a `save_raw_in_tx` or `create_in_tx` returning `Err`) short-circuit via `?` and propagate as `EngramImportError`; because everything happens inside one uncommitted transaction, a hard error mid-loop drops the transaction and rolls back all writes — the destination is left untouched. **Soft errors** (per-row invalid metadata) do not abort: they are counted in `rows_invalid`, collected in `errors`, and the transaction still commits the good rows. The test `import_atomicity_failure_rolls_back_partial_writes` (`tests/import_e2e.rs:288`) documents that true mid-tx crash atomicity is impractical to test, so it instead verifies the soft-error counter advances without panic while the good row still lands. There is no async, locking, or concurrency in this crate beyond the single `r2d2` pool checkout (`self.observations.pool().get()?`, `lib.rs:128`) and one `rusqlite::Transaction`.

### 16.10 CLI integration

`crates/seele-cli/src/commands/import.rs` is the sole caller. `ImportCmd::FromEngram(FromEngramArgs)` exposes `path: PathBuf`, `--re-embed` (field `re_embed`), and `--dry-run` (field `dry_run`). `run_from_engram` (`import.rs:48-100`) resolves the destination DB (`--db` or `default_db_path`), creates parent dirs, calls `init_db` (which runs migrations, so an absent destination is initialized), builds the two stores from one shared pool, and calls `importer.import_from(&args.path, args.dry_run)`. It then emits the report through `output::emit_split` (human text or `--json`). The human text prints `rows seen / rows inserted / rows skipped (exist) / rows invalid / links created / links dangling` and the `errors` list — note it does **not** print `links_already_present` or any session count. The CLI E2E tests `import_from_engram_against_synthetic_source_inserts_rows` and `import_from_engram_rejects_db_without_memories_table` (`crates/seele-cli/tests/subcommands_e2e.rs:286,333`) assert the JSON counters (including idempotent re-run) and that a non-ENGRAM DB exits non-zero with "memories" in stderr.

Two notable gotchas live in the CLI layer, not the crate. First, `--re-embed` is recognized but a **no-op**, surfaced via `tracing::warn!` (`import.rs:64-69`); the doc-comment on the `re_embed` field (`import.rs:27-29`) calls it recognized "but a no-op until Sprint-05 lands the real ONNX backend" — at odds with `docs/ENGRAM-MIGRATION.md`, which documents `--re-embed` as functional (and references SEELE's `all-MiniLM-L6-v2` model). This is a documentation/implementation drift worth flagging to the improvement reviewer. (Note the source comment's "v0.1 ships FakeEmbedder / ONNX lands in Sprint-05" framing itself sits uneasily next to `CLAUDE.md`, which states ONNX is the default in v0.1 — a separate drift in the comment.) Second, the importer itself ignores `--re-embed` entirely; the flag is read only in the CLI and never reaches `import_from`, so even when the ONNX backend exists, re-embedding would need additional wiring.

### 16.11 File-by-file map

- **`crates/seele-engram-import/src/lib.rs`** — the entire crate. Module docs (`:1-24`) state the contract: read-only source, `memories`→`observations` mapping, `save_raw_in_tx` atomic per file, `INSERT OR IGNORE` idempotency, ULID preservation with `engram_id` fallback, and `linked_to` registered during the main pass and resolved once all observations are in. Contains the public API (`EngramImporter`, `import_from`, `ImportReport`, `EngramImportError`, `Result`), the internal `MappedRow`/`ParsedMetadata` structs, the source helpers (`open_source`, `verify_schema`, `pragma_columns`, `read_and_map_rows`, `read_created_at`, `ms_to_dt`), the mappers (`map_one`, `parse_metadata`, `resolve_dest_id`, `synthesize_title`), the tx probes (`observation_exists_in_tx`, `link_exists_in_tx`), and **11 unit tests** (`#[cfg(test)] mod tests`) covering title synthesis, id resolution, and metadata parsing.
- **`crates/seele-engram-import/Cargo.toml`** — declares the crate (description "ENGRAM-to-SEELE migration tool (ADR-13)."), its two internal path deps (`seele-core`, `seele-storage`), the external deps (`serde`, `serde_json`, `thiserror`, `tracing`, `chrono`, `rusqlite`, `r2d2`, `ulid`), and `tempfile` as the sole dev-dependency for the E2E harness.
- **`crates/seele-engram-import/tests/import_e2e.rs`** — **13 integration tests** that synthesize a real ENGRAM-shaped SQLite DB and run the importer against a fresh SEELE destination, guarding the ADR-13 contract end to end: basic insert, ULID preservation, `engram_id` breadcrumb, `linked_to`→links, cross-pass resolution, dangling counting, link de-duplication, ULID idempotency, soft-error handling, both schema rejections (no `memories` table and missing required column), dry-run, and metadata field extraction (`project`/`scope`/`topic_key`).
- **`crates/seele-cli/tests/subcommands_e2e.rs`** — the two CLI-level E2E tests for this subcommand (`import_from_engram_against_synthetic_source_inserts_rows` at `:286`, `import_from_engram_rejects_db_without_memories_table` at `:333`).
- **`docs/ENGRAM-MIGRATION.md`** — the operator guide: back up the source, dry-run first, real run, the (documented-as-functional) `--re-embed`, verification with `list`/`stats`/`doctor`, the `--tool-prefix mnema` and `--legacy-engram-paths` runtime shims, caveats (dangling ids dropped, cross-session `linked_to` chains not reconstructed, topic-key collisions not merged because the raw path is used), and rollback (delete the destination; the source was never touched). Caution: the guide's example dry-run output and a caveat both speak of "sessions" being imported, but the importer code never reads or writes sessions (`session_id` is always `None`) — that is drift in the guide, not behavior of this crate.

### 16.12 Edge cases, invariants, and limitations

- **Read-only source is an invariant** — the source is opened `SQLITE_OPEN_READ_ONLY`; the migration is strictly one-way. The "no SEELE→ENGRAM / no continuous sync" framing is attributed to ADR-13's "Out of scope" section, which is not present in this checkout; the verifiable fact is the read-only open flag in `open_source`.
- **`InvalidMetadata` is dead** — declared (`lib.rs:52-53`) but never constructed; per-row metadata failures travel as soft strings. A reviewer may want to remove it or wire it in.
- **Sessions are not imported** — `map_one` hard-codes `session_id: None`; nothing in the crate reads a session from the source. The migration guide's "Sessions are imported" caveat contradicts this and should be corrected.
- **Dry-run `links_dangling` over-counts** — it reports total `linked_to` entries, not the true dangling subset, since pass-2 resolution is skipped (`lib.rs:122`).
- **Non-ULID source ids are not idempotent across runs** — each run mints a fresh ULID, so re-importing cuid-keyed rows would duplicate them; only ULID-keyed rows are protected by `INSERT OR IGNORE`.
- **No `links` UNIQUE constraint** — link idempotency depends entirely on the `link_exists_in_tx` probe; if a concurrent writer inserted between probe and insert, a duplicate could still appear (no such concurrency exists in the single-tx flow).
- **Deferred resolution case** — cuid `linked_to` targets whose new ULID is behind `metadata.engram_id` are dropped as dangling (`lib.rs:177-180`); the source comment defers a JSON-scan resolution to v0.2.
- **`--re-embed` doc/impl drift** — documented as functional in `ENGRAM-MIGRATION.md` but a `tracing::warn!` no-op in the CLI and entirely unread by `import_from`.


---

## 17. Web Landing & Observability — `web/`

### 17.1 Purpose and place in the system

`web/` is a self-contained Astro static site — the public marketing landing for SEELE plus a live "observability" page. The `astro.config.mjs` header comment calls it "Astro 5.x static", but `package.json` pins `astro: ^6.0.0`, so the running framework is Astro 6. It is **not** a Rust crate and does not appear in the workspace dependency graph; it ships separately to GitHub Pages and has no compile-time coupling to the binary. Its only runtime coupling is over HTTP: three of its client components (`SeeleStatus`, `Observability`, `ChatPanel`) talk to a *user-owned* `seele serve` instance on `localhost:7777`. The hosted site has no database and no server of its own — every dynamic feature degrades to static sample data or a "configure your key" prompt when no local server is reachable. This is the central design contract of the page: "all paths lead to localhost" (the footer copy line, `Footer.astro:62`). It was produced under the **LUMEN protocol** (a frontend analogue of AEGIS), sprints LUMEN-01..03, with the design system frozen in `DESIGN.md` v0.3.0.

The site has a strict, unusual design system: **Brutalist dev-craft**. Three colors, one monospace family, 2px solid borders as the only hierarchy primitive, incomplete borders as deliberate "visual syntax", and zero motion (`transition: none`). The pitch — that the target persona (platform/infra/ML engineers) rewards technical credibility over visual warmth — is captured by the front-matter tagline "credibility before warmth" (`DESIGN.md:4`) and argued in the "Why this direction" prose (`DESIGN.md:156-162`, in Spanish: "la audiencia premia credibilidad técnica antes que calidez visual").

### 17.2 Build configuration

`web/package.json`: name `seele-web`, version `0.1.0`, `type: module`, `private: true`, license MIT, `engines.node: >=20.0.0`. The single runtime dependency is `astro: ^6.0.0` (note: the package `description` field says "Astro 5 static site" but the pin is `^6`). The lone dev dependency is `@playwright/test: ^1.60.0`; the screenshot tool actually imports the bundled `playwright` package directly (`import { chromium } from 'playwright'`). Scripts: `dev`/`build`/`preview`/`check` are stock Astro (`astro dev|build|preview|check`); `visual` runs `node scripts/visual-critique.mjs`; `visual:preview` adds `--url http://localhost:4321/seele`.

`web/astro.config.mjs` is small but load-bearing:

```js
// web/astro.config.mjs:11-25
export default defineConfig({
  site: 'https://orlando-vazquez-career.github.io',
  base: '/seele',
  trailingSlash: 'never',
  output: 'static',
  build: { inlineStylesheets: 'always', assets: 'assets' },
  vite: { build: { cssCodeSplit: false } },
});
```

| Option | Value | Why |
|---|---|---|
| `site` + `base: '/seele'` | GH Pages subpath | Dev preview and built paths line up with the `/seele/` subdirectory served from GH Pages (the config comment notes the `gh-pages` branch source) |
| `trailingSlash: 'never'` | no `/` suffix | `Header.astro` link logic strips trailing slashes to honor this |
| `output: 'static'` | pure static | No SSR; everything is prerendered, all dynamic behavior is client JS |
| `inlineStylesheets: 'always'` + `cssCodeSplit: false` | single inlined CSS | Brutalist perf budget: zero render-blocking CSS requests, no webfont downloads |

Because `base` is `/seele`, components consume `import.meta.env.BASE_URL` for internal links rather than hard-coding paths (e.g. `Header.astro:4`, `Layout.astro:14`).

### 17.3 Deployment — `deploy-web.yml`

`.github/workflows/deploy-web.yml` triggers on `push` to `main` filtered to `web/**` or the workflow file itself, plus `workflow_dispatch`. Permissions `contents: read`, `pages: write`, `id-token: write`; a `concurrency` group `pages` with `cancel-in-progress: false`. Two jobs:

1. **build** (`ubuntu-latest`): `checkout@v4` → `setup-node@v4` (node 20, npm cache keyed on `web/package-lock.json`) → `actions/configure-pages@v5` → `npm ci` (cwd `web`) → `npm run build` (cwd `web`) → `upload-pages-artifact@v3` with `path: web/dist`.
2. **deploy** (`ubuntu-latest`): `needs: build`, environment `github-pages` (url from `steps.deployment.outputs.page_url`), uses `actions/deploy-pages@v4`.

The `configure-pages` step carries a comment that it flips the Pages site to `build_type: workflow` automatically, otherwise `deploy-pages` fails against a site initialised with a branch source (the manual `gh-pages` fallback).

### 17.4 The design system — `DESIGN.md` + `tokens.css`

`DESIGN.md` is YAML-front-matter + prose, version `0.3.0`, scope `web/`, direction `brutalist-dev-craft`, sprint `LUMEN-02`. It is the authoritative visual contract; `tokens.css` is its machine-readable projection. Key axioms:

- **Three colors, period** (`DESIGN.md:164-171`; raw values in front-matter `DESIGN.md:17-19`): `--bg: oklch(0.08 0 0)`, `--fg: oklch(0.94 0 0)`, `--accent: oklch(0.65 0.18 50)` (electric orange). A fourth color is "prohibido". Derived greys `--fg-muted` (oklch 0.65) and `--fg-faint` (oklch 0.42) are explicitly framed as opacity/mix variations of `fg`, not new colors. `--danger`/`--success` are feedback-only (success is documented as "solo para SeeleStatus detected"); the five network brand hexes (`--network-btc` `#f7931a` etc.) are reserved literally for the donate widget.
- **Single family**: `--font-mono` is a `ui-monospace`→JetBrains Mono→Cascadia Code→Menlo→Monaco→Consolas→Liberation Mono→Courier New→`monospace` fallback chain. No webfont download. Inter/Roboto/Helvetica/Arial and "any non-monospace family" are explicitly banned (`DESIGN.md:39-48`). Hierarchy emerges from weight contrast (800 hero vs 400 body) + an abrupt size jump.
- **Borders as the hierarchy primitive**: `--border-hair: 1px`, `--border-base: 2px`, `--border-bold: 3px`. All `box-shadow` is `none`, all `border-radius` is `0` (sole exception `--rounded-pill: 999px` for status pills), and gradients/filters/`backdrop-filter` are banned (`DESIGN.md:218-234`).
- **Incomplete borders as visual syntax** (`DESIGN.md:187-196`): a 3-sided border (top+right+bottom, no left) is a deliberate "connector open to the left margin", not a bug. `tokens.css` ships utility classes `.border-3-sides`, `.border-2-sides-tl`, `.border-left-only`, `.border-top-only` (lines 320–350) encoding this. Note: the Hero data block and the Install post-install `<details>` re-implement the 3-sided pattern with *inline* CSS rather than these utility classes (`Hero.astro:107-117`, `Install.astro:123-131`).
- **Motion: zero**: `tokens.css` sets `transition: none` on `*,*::before,*::after` (line 165). The only animation in the whole site is a `@keyframes pulse` opacity loop on status dots (SeeleStatus pill, DonateButtons detect pill, Observability dot). Hover states are instant color/background swaps. `prefers-reduced-motion: reduce` is honored by clamping every animation/transition duration to `0.01ms !important` (lines 272–280), disabling even the pulse.
- **Asymmetric layout**: `.page-container` (lines 291–307) has `max-width: 1100px`, `margin-inline-start: clamp(var(--space-3), 4vw, var(--space-14))` and a far larger `margin-inline-end: clamp(var(--space-3), 12vw, var(--space-50))` — "left commit, right breathe". Above 1400px it flips to a centered `max-width: 1300px` with symmetric `margin-inline: auto` (asymmetry past that point is "wasted space"). `.page-container-centered` is the symmetric variant used by the footer and the observability page.

`tokens.css` carries a header version of v0.3.0 but documents a **scale rebalance v0.4.1** (lines 34–44): small sizes bumped +1px for readability, large end cut ~25%. The actual shipped sizes are `--text-meta: 13px`, `--text-xs: 15px`, `--text-sm: 16px`, `--text-base: 18px`, `--text-lg: 22px`, `--text-2xl: 28px`, `--text-hero: 64px` — the comment annotates the cut as `hero 84→64, lg 24→22, 2xl 32→28`. (These differ from the `DESIGN.md` front-matter sizes, which still list the older scale — meta 11px, base 15px, hero 72px — so `tokens.css` is ahead of the prose spec here.) `tokens.css` also defines weights (300/400/500/700/800), leadings, trackings (`--tracking-caps: 0.12em` for ALL-CAPS labels), a 12-step spacing scale (4..200px), z-index tokens, and a full semantic token layer (`--semantic-*`) that components consume instead of raw primitives. Light mode is supported two ways: a `@media (prefers-color-scheme: light)` block scoped to `:root:not([data-theme="dark"])`, plus manual `:root[data-theme="light"|"dark"]` overrides driven by the header toggle.

`docs/design/` holds the LUMEN plan trail under `docs/design/plans/executed/`: personas (`lens/01/personas/marisol-platform-dev.md`), JTBD (`lens/01/jtbd/eval-and-donate.md`), scaffold wireframes/sitemap/flows (`scaffold/01/`), four aesthetic variations (`material/02/variations/01-brushed-metal-editorial` … `04-editorial-warm`; variation 2 "brutalist-dev-craft" was chosen), five "wow" ADRs (`material/02/adr/wow-01-monospace-only` … `wow-05-motion-zero`), and a visual-critique doc. Plus a11y/perf/heuristic evidence reports under `docs/design/evidence/{01,02}/`, three LUMEN devlogs under `docs/design/devlogs/`, and per-component specs under `docs/design/components/` (Header, Hero, Features, Install, Support, Footer, DonateButtons).

### 17.5 Internationalization mechanism

The site is bilingual EN/ES with **no routing or build duplication**. Every translatable element renders *both* languages as sibling inline spans, e.g. `<span lang="en">features</span><span lang="es">características</span>`. CSS hides the inactive one based on the `<html lang>` attribute (`tokens.css:268-269`): `html[lang="en"] [lang="es"]{display:none}` and the mirror `html[lang="es"] [lang="en"]{display:none}`. `<span lang="la">` (the Latin footer motto) is intentionally unmatched and always visible. The active language (and theme) are resolved before paint by an `is:inline` script in `Header.astro` (lines 79–106): localStorage `seele-theme`/`seele-lang` → `prefers-color-scheme`/`navigator.language` → defaults `dark`/`en`. A second module script (lines 108–160) wires the toggle buttons, persists choices to localStorage, updates icon glyphs and `data-placeholder-en/es` inputs. The lang-toggle icon shows the language a click switches *to* (`updateLangIcons`, `Header.astro:143-149`).

### 17.6 File-by-file map

**Pages & layout.** `src/layouts/Layout.astro` is the HTML shell: `<head>` with charset/viewport, description, `theme-color #141414`, SVG favicon (base-prefixed via `${base}/favicon.svg`), canonical URL (`https://orlando-vazquez-career.github.io/seele/`), `og:type`/`og:title`/`og:description` and a `twitter:card` summary meta (no OG/Twitter image), a default title (`SEELE — memory engine for serious Rust devs`) and description (both overridable via `Props { title?, description? }`), and a single `<slot/>`; it imports `tokens.css` once. `src/pages/index.astro` composes the landing: `Header → main(Hero, Features, Install, Support) → Footer`. `src/pages/observability.astro` is the second route (`/observability`): it sets a custom title/description, renders a `// LOCAL-ONLY` banner explaining the localhost contract (setup copy: `cargo install --path crates/seele-cli`, then `seele serve --cors-allow {Astro.url.origin}`), a bilingual page header, then `<Observability/>` and `<ChatPanel/>`. Its scoped `<style>` defines the page-only banner/header/lead classes.

**Header / brand / footer.** `Header.astro` is sticky, bordered-bottom, contains the DevZen brand (Triangle + "DevZen"/"SEELE" stack), a context-aware nav (different links on the landing vs observability page, computed from `import.meta.env.BASE_URL` and `Astro.url.pathname` with trailing slashes stripped to honor `trailingSlash:'never'`), a GitHub link, and the lang/theme toggle buttons. It owns the anti-FOUC inline script and the toggle module script. `Triangle.astro` renders the DevZen motif as inline SVG; `Props { size=24, variant: 'framed'|'outline', class='', decorative=true }`. The `framed` variant draws a 22×22 `rect` (x/y=1, viewBox `0 0 24 24`) + a `polygon` triangle, both `fill="none" stroke="currentColor" stroke-width="2"`; `decorative` toggles `aria-hidden`/`role`. Its doc-comment says it is used at 24px in the brand mark, but the header actually instantiates it at `size={28}` (`Header.astro:30`). `Footer.astro` is a 5-column grid (brand / links / inspired-by / connect / privacy) collapsing `5 → 3 (≤1100px) → 2 (≤720px) → 1 (≤540px)` columns by breakpoint, with a bottom row carrying `© {year} DevZen SpA · all paths lead to localhost` and the Latin motto `Super stellatum firmamentum iudicat Deus, sicut nos iudicamus.` (with trailing period). The "inspired by" column credits `Gentleman-Programming/engram` with a separate `MIT · clean-room` line.

**Hero.** `Hero.astro` renders a meta line (`// SEELE` + `[v0.2.0]` as two spans), the H1 (bilingual, `font-size: clamp(28px, 5.5vw, var(--text-hero))`), a subline naming the stack, a 3-sided-border **data block** (`latency < 5ms p99`, `backend Rust 1.85+`, `tests 322 green`, `status ALPHA` in accent, `license MIT`), two CTAs (`INSTALL NOW` primary linking `#install`, `GITHUB ↗` secondary), and an embedded `<SeeleStatus/>`. Note these stats are hard-coded marketing copy, not live values.

**Features.** `Features.astro` defines a local `features` array (4 entries: CLI/MCP/HTTP/TUI) with bilingual blurbs, a `meta` command snippet, and a `href`, and maps each to `<FeatureCard/>` in an asymmetric grid (`grid-template-columns: 1.5fr 1.25fr 1.25fr 1fr` — "CLI dominates", collapsing to `1fr 1fr` ≤960px and `1fr` ≤540px). The copy states "17 subcommands", "19 tools under `seele_*`", "18 paths + OpenAPI 3.1", and "5 panes" for the TUI. `FeatureCard.astro` is a typed link card: `Props { num, title, blurbEn, blurbEs, meta, href }`; full-height flex column with number/title/blurb/`<pre>` meta/`view →` link; hover inverts to fg-background. `http`-prefixed `href`s open in a new tab.

**Install.** `Install.astro` defines `installPaths` (3 entries: install script, `cargo install --git`, build from source), the first marked `recommended: true`, and renders `<InstallCard {...p}/>` plus a `<details>` "after install" example (save + search + setup). `InstallCard.astro` (`Props` with a single `notes?` string *and* `notesEn?`/`notesEs?` bilingual variants, plus `recommended?`) shows num/badge/title/description, a `<pre>` command prefixed `$ ` with an absolutely-positioned **COPY** button. The copy script uses `navigator.clipboard.writeText`, flips the label to `COPIED` (adding `.is-copied`) for 1500ms, and fails silently in a `catch`. The recommended card gets an accent border.

**Support / donate.** `Support.astro` is the human-funding section: attribution to Orlando Nahuel Vazquez Gonzalez under DevZen SpA, an ordered list of three "ways" (star/share, open issues, hire/consult), then a crypto-donations block (labelled `// 04 — crypto donations`) hosting `<DonateButtons/>` and the tagline "EIP-1193 for EVM. BIP-21 for Bitcoin. Solana Pay for Phantom. No WalletConnect, no tracking."

### 17.7 SeeleStatus — the localhost probe

`SeeleStatus.astro` (`Props { endpoint = 'http://localhost:7777/health' }`) is a small pill that tells the visitor whether SEELE is running on their machine. Constants: `TIMEOUT_MS = 1500`, `CACHE_TTL_MS = 30_000`, `STORAGE_KEY = 'seele:status-cache:v1'`. The state machine is `type Status = 'checking' | 'detected' | 'not-detected' | 'blocked'`. On load it checks a `sessionStorage` cache (30s TTL); on miss it runs `detect()`:

```ts
// web/src/components/SeeleStatus.astro:61-75
async function detect(endpoint: string): Promise<Status> {
  if (location.protocol === 'https:' && endpoint.startsWith('http:')) {
    return 'blocked';
  }
  try {
    await fetch(endpoint, { mode: 'no-cors', cache: 'no-store',
      signal: AbortSignal.timeout(TIMEOUT_MS) });
    return 'detected';
  } catch { return 'not-detected'; }
}
```

Two gotchas worth flagging for downstream improvement: (1) on the hosted HTTPS site, an `http://localhost` probe is **mixed content** and returns `'blocked'` immediately without ever attempting the fetch — so the landing's status pill shows "auto-detect needs local hosting" in production. (2) The probe is `mode: 'no-cors'`, an *opaque* fetch — it resolves as long as the server responds at all, so `'detected'` means "something answered on :7777", not "SEELE specifically answered". Messages are localized at apply time by reading `<html lang>`. The CTA (`install →`) is hidden only when status is `detected` (`cta.hidden = status === 'detected'`). The dot pulses while `checking`; `detected` turns the border/dot `--success` green.

### 17.8 Observability — the live panel

`Observability.astro` (`Props { endpoint = 'http://localhost:7777' }`) is the largest component. It reads directly from a local `seele serve` over CORS and renders, in the `online` state, six visible sections (Stats, By type, By project, Search, Recent, Sparkline) plus a seventh **detail panel** that stays `hidden` until an item is clicked; if anything fails it degrades gracefully. Its TS state machine is `type Status = 'probing' | 'offline' | 'cors-blocked' | 'online'`, surfaced via `data-state` + `[data-show-when]` blocks. Constants: `TIMEOUT_MS = 1500`, `POLL_INTERVAL_MS = 15_000`, `RECENT_LIMIT = 10`, `SPARK_DAYS = 14`, `SPARK_BLOCKS = '▁▂▃▄▅▆▇█'` (8 levels).

The **probe** is the clever part: it first does a readable cross-origin `GET /health`. If that succeeds and is 2xx → `online`. If it throws, it can't tell "server down" from "CORS rejected", so it retries with `mode: 'no-cors'`; success there means the server is up but blocking CORS → `cors-blocked`, failure → `offline` (`Observability.astro:317-344`). Only `online` proceeds to `refreshAll` + a 15s polling interval + `bindSearch`.

The `offline` and `cors-blocked` states render **identical hard-coded sample data** (total 142, by-type/by-project bars, three fake recent items, a 14-char sparkline `▁▂▁▃▂▄▃▅▄▆▅▇▆█`) behind a preview banner at `opacity: 0.72`, so the panel "feels alive" even with no server. The `cors-blocked` banner additionally prints the fix as `$ seele serve --cors-allow {endpoint}` (with a trailing slash stripped from `endpoint`).

When `online`, JS hydrates the live sections from three endpoints:

| Section | Endpoint | Shape consumed |
|---|---|---|
| Stats (total = `active`, projects) + By-type bars | `GET /stats` | `StatsResponse.observations { active, projects, by_type[] }` |
| Recent list, last-save age, By-project bars, sparkline | `GET /memories?limit=10` | `MemoryRecord[]` (`id, title, type, project, scope, topic_key, content, created_at, updated_at, …`) |
| Search | `POST /search` `{query, limit:8}` | `{ results: [{ memory, score? }] }` |

By-project counts and the saves/day sparkline are derived **client-side** from the `/memories` array (`renderProjectBars`, `renderSparkline`), not separate endpoints. `fetchJson<T>` swallows all errors and non-2xx into `null`, and `refreshAll` uses `Promise.allSettled` over `refreshStats` + `refreshRecent`, so a single failing endpoint never blanks the whole panel. Bar charts render the top 6 items, each scaled to the max (floored at 4% width). Search is debounced 220ms, min query length 2, and aborts the previous in-flight request via `AbortController`. Clicking or Enter/Space on a recent or result item opens the inline **detail panel** showing id/type/project/topic/created (ISO) + full content. All injected strings pass through `escapeHtml()` (defence against XSS from memory content); titles are truncated to 60 chars via `truncate()`. The state shapes (`MemoryRecord`, `StatsResponse`) mirror the seele-http response types, which is the contract this component depends on.

### 17.9 ChatPanel — chat-with-your-DB

`ChatPanel.astro` (`Props { endpoint = 'http://localhost:7777' }`) is a chat widget for the observability page. Per its header doc-comment, it POSTs to SEELE's local `/chat`, which runs a **server-side tool-use loop**: the model decides when to call `seele_search`, the server executes it and feeds results back, and only the final assistant text returns. The user's AI API key lives in this browser's `localStorage` (`SETTINGS_KEY = 'seele-chat-settings'`) and is forwarded only to the local SEELE server — "never reaches a third party from this page".

States: `'probing' | 'needs-config' | 'enabled'`. On init it `GET /chat/info` (3s timeout) and reads localStorage. It becomes `enabled` if **either** the server reports chat config (`ChatInfo.enabled`) **or** the browser has saved settings — browser settings win at request time (`ChatPanel.astro:221-232`). Otherwise `needs-config` shows an `[ OPEN SETTINGS ]` CTA. The settings `<dialog>` (provider select, model text, password `api_key`, optional endpoint override) persists to localStorage; submitting with an empty `api_key` deliberately **clears** stored credentials and reloads. The provider list (`minimax, openai, anthropic, openrouter, together, groq, deepseek`) and `DEFAULT_MODELS` map exactly mirror the seele-http chat backend (verified against `crates/seele-http/src/handlers.rs`: `default_model_for` and `default_endpoint_for` route the same provider names to their `/chat/completions` base URLs, e.g. `minimax → MiniMax-M2`, `anthropic → claude-haiku-4-5-20251001`, `groq → llama-3.3-70b-versatile`).

Sending: the client keeps a `Message[]` history, POSTs `{ messages, provider?, api_key?, model?, endpoint? }` with a **60s** `AbortSignal.timeout`, and the server returns the *full* history. The client diffs with `data.messages.slice(history.length + 1)` — the `+1` skips the system prompt the server prepends (`ChatPanel.astro:389`) — and renders only the new turns. Rendering hardening worth noting: `stripThinking()` removes `<think>`, `<thinking>`, and `<|thinking|>` reasoning traces (its comment cites Minimax M2 / DeepSeek-R1 / Qwen QwQ, older Claude, and Together/vLLM variants); `renderAssistantBody()` escapes first, then applies a tiny markdown subset (`**bold**`, `` `code` ``) on the escaped string ("no markdown can ever inject HTML"); tool turns are collapsed to "N results" (from the parsed `count` field); system turns are never displayed; assistant turns that strip to empty with no tool calls are dropped entirely. Enter submits, Shift+Enter newlines, IME composition is respected (`e.isComposing`). The log scrolls internally only (`log.scrollTop = log.scrollHeight`) — never the page.

### 17.10 DonateButtons — real wallet integration

`DonateButtons.astro` is the most logic-heavy client component. Its `targets` array holds five chains (BTC, ETH, Base, Syscoin, Solana) with addresses, `method` (`bip21|evm|solana-pay`), `uri`, `install` URL, `walletName`, and `chainId`. The three methods:

- **EVM** (ETH chainId 1 / Base 8453 / Syscoin 57): discovers providers via **EIP-6963** (dispatches `eip6963:requestProvider`, listens for `eip6963:announceProvider`), preferring rdns in order `io.metamask`, `io.metamask.flask`, `io.pali`, `io.paliwallet`, `app.phantom`, else the first announced provider, else legacy `window.ethereum`. On click it `eth_requestAccounts`, ensures the chain via `wallet_switchEthereumChain` with a `4902 → wallet_addEthereumChain` fallback (Base/Syscoin carry full `add` configs incl. RPC + explorer; chain 1 has no `add`), then `eth_sendTransaction` with `value: '0x0'`. Error codes are mapped: `4001 → REJECTED`, `-32002 → CHECK WALLET`, else `ERROR`.
- **Solana** (`solana-pay`): if a Phantom/`window.solana` provider exists, navigates to the `solana:` URI; after 700ms, if the document is still visible (no app intercepted), it copies the address and shows `COPIED`.
- **Bitcoin** (`bip21`): prefers a browser extension — UniSat (`sendBitcoin(addr, 1000)` sats), or Xverse / Leather via `sats-connect`-style `request('sendTransfer', …)` (Xverse: `recipients:[{address, amount:1000}]`; Leather: `{address, amount:'0.00001'}`). Otherwise navigates to the `bitcoin:` URI; after 700ms-still-visible it opens the install page and copies as last resort.

A `data-detect` pill (`scanning → detected/none`) scans wallet globals 200ms after load and re-runs on every `eip6963:announceProvider`, and tags each button's wallet label `data-available='yes'` when its method is satisfiable. Every method shares a fallback: no wallet → open the official extension install page + copy the address. A **bug to flag**: the status CSS references `--warning` (`DonateButtons.astro:546` and `Observability.astro:672`) but no `--warning` token is defined in `tokens.css` — the `pending`/`cors-blocked` dot color silently falls back. The Observability rule uses `var(--warning, var(--accent))` so it degrades to accent; the donate `pending` rule (`color: var(--warning)`) has no fallback and inherits instead. This is a real, low-severity defect in the improvement surface.

### 17.11 Visual-critique tooling

`web/scripts/visual-critique.mjs` is a Playwright (`chromium`, headless) screenshot harness for the LUMEN "Visual Critique Multi-Resolution" phase. It captures the full matrix: 2 pages (`home` = `''`, `observability` = `/observability`) × 5 viewports (320/768/1024/1440/1920 px) × 2 themes (dark/light) × 2 langs (en/es) = **40 shots**, written as `{page}-{lang}-{theme}-{viewport-label}.png` under `test-results/visual/` (the viewport label is e.g. `320-mobile-narrow`). Per shot it opens a fresh context with `reducedMotion: 'reduce'` and the matching `colorScheme`, injects `localStorage` `seele-theme`/`seele-lang` via `addInitScript` (so the anti-FOUC script picks them up), navigates with `waitUntil:'networkidle'` (15s cap), waits 800ms, then waits for the Observability `[data-obs]` to leave `data-state="probing"` (3s cap, captures anyway on timeout), waits another 200ms, then takes a `fullPage` PNG. `--url` overrides the base (default `http://localhost:4321/seele`, trailing slashes stripped). Exit code 1 if any shot fails. It produces artifacts only — it does no automated assertion or diffing; the "critique" is done by a human/agent reviewing the PNGs.

### 17.12 Cross-system boundary summary

The web subsystem touches the rest of SEELE only at runtime, only over HTTP, and only against a user-local server:

| Component | Calls | seele-http endpoint(s) | Notes |
|---|---|---|---|
| SeeleStatus | `fetch` no-cors | `GET /health` | opaque probe; returns `blocked` under HTTPS mixed-content |
| Observability | `fetch` (readable then no-cors) | `GET /health`, `GET /stats`, `GET /memories?limit=10`, `POST /search` | needs `seele serve --cors-allow <origin>` |
| ChatPanel | `fetch` | `GET /chat/info`, `POST /chat` | forwards user-held API key to local server only |
| DonateButtons | none against SEELE | — | talks to browser wallet extensions via EIP-6963/EIP-1193/sats-connect, not SEELE |

The CORS contract surfaced by these components corresponds to the server side: `seele serve --cors-allow <origin>` populates `cors_origins`, and `seele-http/src/server.rs:105-117` builds the CORS layer in two branches — when `cors_origins` is **empty** it is a bare `CorsLayer::new()` (no cross-origin exposure), and when **any** origin is requested it builds a permissive `CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any)` (with an in-code note that a per-origin allowlist is "Block D" future refinement, and that `Any` methods/headers are required so a JSON `POST` clears preflight). The privacy posture advertised in the footer ("no tracking / no analytics / no cookies / no SDKs") is backed by the actual implementation: no analytics scripts, no webfonts, all state in localStorage/sessionStorage, and every network call pointed at the user's own machine.


---

## 18. Build, CI/CD, Distribution & Release

This section documents how SEELE is compiled, linted, tested, packaged, and shipped: the Cargo workspace layout and release profile, the pinned toolchain and platform-specific build flags, the three GitHub Actions workflows (CI, Release, web deploy), the Dependabot configuration, the curl/irm installers with SHA256 verification, the STELE-residual static checks, the v0.1.0 acceptance smoke, and the two load-bearing version pins (`ort =2.0.0-rc.10` and the vendored `sqlite-vec` v0.1.9). It is the "how it gets built and out the door" layer; the runtime crates it builds are covered in sections 3–17.

### 18.1 The Cargo workspace

The root `Cargo.toml` (`C:/dev/tools/SEELE/Cargo.toml`) declares a 13-member virtual workspace (no root package — the binary lives in `seele-cli`) using `resolver = "2"`. The members enumerate every crate in `crates/`, leaf-to-root:

```toml
# Cargo.toml:1-17
[workspace]
members = [
    "crates/seele-core", "crates/seele-storage", "crates/seele-embedder",
    "crates/seele-search", "crates/seele-chat", "crates/seele-mcp",
    "crates/seele-http", "crates/seele-tui", "crates/seele-sync",
    "crates/seele-setup", "crates/seele-project", "crates/seele-engram-import",
    "crates/seele-cli",
]
resolver = "2"
```

(The CLAUDE.md text says "12 crates"; the live `members` list has 13 because `seele-chat` was added later. The compendium intro count of 13 crates is authoritative.)

`[workspace.package]` centralizes shared metadata that each crate inherits via `field.workspace = true`: `version = "0.2.0"`, `edition = "2021"`, `rust-version = "1.85"` (the MSRV), `authors = ["DevZen SpA"]`, `license = "MIT"`, and `repository = "https://github.com/orlando-vazquez-career/seele"`. Member crates pick these up with one-liners (e.g. `seele-cli/Cargo.toml`: `version.workspace = true`, `edition.workspace = true`, etc.), so the version and MSRV are bumped in exactly one place.

`[workspace.dependencies]` is the single source of truth for third-party crate versions. Member crates reference them with `<crate>.workspace = true` (sometimes adding features locally). The set is grouped by purpose:

| Group | Key crates (version) | Purpose |
| --- | --- | --- |
| Async | `tokio` 1.42 (`features=["full"]`), `futures` 0.3 | multi-thread runtime |
| Serialization | `serde` 1, `serde_json` 1 (`preserve_order`) | DTOs, JSON-RPC, OpenAPI |
| Errors | `thiserror` 2, `anyhow` 1 | typed errors / CLI top-level |
| Tracing | `tracing` 0.1, `tracing-subscriber` 0.3 (`env-filter`) | structured logs |
| Storage | `rusqlite` 0.32 (`bundled`,`load_extension`), `r2d2` 0.8, `r2d2_sqlite` 0.25, `refinery` 0.8 (`rusqlite`) | SQLite pool + migrations |
| Embedder | `ort` `=2.0.0-rc.10`, `ndarray` 0.16, `tokenizers` 0.20, `hf-hub` 0.3 | ONNX runtime + tokenizer |
| HTTP | `axum` 0.8, `tower` 0.5, `tower-http` 0.6 (`cors`,`trace`,`compression-gzip`), `utoipa` 5 (`axum_extras`), `utoipa-swagger-ui` 9 (`axum`), `reqwest` 0.12 (test-only, `default-features=false`, `json`+`rustls-tls`) | REST API + OpenAPI/Swagger |
| CLI / TUI | `clap` 4.5 (`derive`,`env`), `ratatui` 0.29, `crossterm` 0.28, `tempfile` 3 | CLI + TUI |
| IDs / misc | `ulid` 1.1 (`serde`), `chrono` 0.4, `once_cell` 1.20, `regex` 1, `sha2` 0.10, `dirs` 5, `hex` 0.4, `flate2` 1 | SeeleId, hashing, gzip sync |
| Test | `assert_cmd` 2, `predicates` 3, `proptest` 1, `insta` 1 | E2E / property / snapshot tests |

The inline comment at `Cargo.toml:51-53` explains the most fragile pin: `ort` is pinned exactly because no 2.0.0 stable exists as of 2026-05, and Dependabot will open a PR when stable lands — see §18.7. `rusqlite` uses `bundled` (compiles its own SQLite, no system dependency) plus `load_extension` (required to load the vendored vec0 — see §18.6).

#### Release profile

```toml
# Cargo.toml:94-98
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
```

`opt-level = 3` maximizes runtime speed (matters for ONNX inference and FTS/vector queries); `lto = "thin"` enables thin link-time optimization across crates (a good size/perf trade-off vs `lto = "fat"`, which would lengthen the already-heavy ONNX+SQLite link); `codegen-units = 1` forces a single codegen unit per crate so LTO sees the whole picture (slower compile, smaller/faster binary); `strip = true` removes symbols, shrinking the shipped binary. This profile is what `cargo build --release -p seele-cli` (and the release workflow) produce. The local `*.pdb` files are gitignored (`.gitignore:4`).

### 18.2 Toolchain pin and rustfmt

`rust-toolchain.toml` (`C:/dev/tools/SEELE/rust-toolchain.toml`) pins the toolchain via rustup auto-selection:

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

It pins the *channel* to `stable` (not a frozen version like `1.85.0`); the MSRV floor of 1.85 is asserted separately by `rust-version = "1.85"` in `[workspace.package]`, which `cargo` enforces at build time. `components` guarantees rustfmt and clippy are present for the lint job; `profile = "minimal"` keeps installs small (no docs/rust-src). The MSRV was bumped 1.83 → 1.85 to allow `clap_lex` (a transitive dep) compiled with `edition2024` (CHANGELOG.md:348-351).

`rustfmt.toml` (`C:/dev/tools/SEELE/rustfmt.toml`) is intentionally minimal — a single line `edition = "2021"`. The project relies on rustfmt defaults and enforces them with `cargo fmt --all -- --check` in CI; there are no custom style overrides to drift.

### 18.3 Cargo config and the MSVC CRT workaround

`.cargo/config.toml` (`C:/dev/tools/SEELE/.cargo/config.toml`) exists solely to solve a Windows link-time mismatch. The `ort` 2.x prebuilt binaries link against the dynamic CRT (`/MD`), while the C++ pulled in transitively by `tokenizers` (`esaxx-rs`) defaults to the static CRT (`/MT`); mixing them produces `LNK2038`/`LNK2005` errors when the test binary links. The fix forces dynamic CRT for both Rust and `cc`-built C/C++ on the MSVC target:

```toml
# .cargo/config.toml:5-14
[target.x86_64-pc-windows-msvc]
rustflags = [
    "-C", "target-feature=-crt-static",
    "-C", "link-arg=/NODEFAULTLIB:libcmt.lib",
    "-C", "link-arg=/NODEFAULTLIB:libcpmt.lib",
]
[env]
CFLAGS_x86_64_pc_windows_msvc = "/MD"
CXXFLAGS_x86_64_pc_windows_msvc = "/MD"
```

`-crt-static` selects the DLL CRT for Rust code; the two `/NODEFAULTLIB` link-args exclude the static-CRT libraries so the linker doesn't pull in conflicting symbols; the `CFLAGS`/`CXXFLAGS` env vars push `/MD` into the `cc` compilations. This file only affects the Windows MSVC target — Linux and macOS builds ignore it. It is a real gotcha: anyone changing `ort`, `tokenizers`, or the Windows toolchain must keep these aligned or the Windows CI leg fails to link.

### 18.4 CI workflow (`ci.yml`)

`.github/workflows/ci.yml` runs on `push` to `main` and on every `pull_request`. It defines four independent jobs:

| Job | Runner(s) | What it does |
| --- | --- | --- |
| `test` | matrix `ubuntu-latest`, `macos-latest`, `windows-latest` (`fail-fast: false`) | `cargo build --workspace --all-features` then `cargo test --workspace --all-features` |
| `lint` | `ubuntu-latest` | `cargo clippy --workspace --all-features -- -D warnings` then `cargo fmt --all -- --check` |
| `static-checks-bash` | `ubuntu-latest` | `bash scripts/check-no-stele-residual.sh` |
| `static-checks-pwsh` | `windows-latest` | `pwsh -File scripts/check-no-stele-residual.ps1` |

Every job starts with `actions/checkout@v4`. The `test` and `lint` jobs additionally use `dtolnay/rust-toolchain@stable` and `Swatinem/rust-cache@v2` (incremental caching); the lint job's toolchain step explicitly requests `with: components: clippy, rustfmt` (ci.yml:28-29). The two `static-checks-*` jobs do **not** install a Rust toolchain or the cache — they only check out and run the script (the residual checks are pure text greps, no compilation needed). `fail-fast: false` on the test matrix means a failure on one OS does not cancel the others — useful given the Windows-specific CRT issues. Clippy runs with `-D warnings`, so any lint is a hard build break — this is the documented closure criterion (CLAUDE.md). Note the CI clippy invocation is `--all-features` (ci.yml:32), whereas the CLAUDE.md "comandos" snippet shows `--all-targets`; the workflow is the source of truth for what actually gates merges. The STELE residual check runs on *both* bash and pwsh because the two scripts are siblings that must stay in sync; the bash script's comment (check-no-stele-residual.sh:10-15) records a real incident where the bash variant silently missed residuals the pwsh job caught (a prior basename-based `--exclude` bug), after which both were path-synchronized. Note the `--ignored` ONNX-download tests are *not* exercised specially in CI — the CI `cargo test --workspace --all-features` does not pass `-- --ignored`, so the four ignored tests (2 ONNX-download requiring ~90 MB models + 2 perf smokes) are skipped; the ONNX ones are run manually with `cargo test -p seele-embedder -- --ignored`.

### 18.5 Release workflow (`release.yml`)

`.github/workflows/release.yml` triggers on two events: a tag push matching `v*.*.*` (full release), or a manual `workflow_dispatch` carrying an optional `crates_io_publish` input (default `'false'`). The header comment (lines 5-10) records the rationale: per Sprint-05 plan §C and Risk #6, the full pipeline is validated on a release candidate (`v0.1.0-rc.N`) before tagging the real `v0.1.0`, and crates.io publishing is OFF by default and requires explicit opt-in. The workflow holds `permissions: contents: write` (needed to create a GitHub Release). It has three jobs:

**`build`** — a 5-target matrix (`fail-fast: false`) producing prebuilt binaries:

| Target triple | Runner | Cross? | Archive |
| --- | --- | --- | --- |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | no | tar.gz |
| `aarch64-unknown-linux-gnu` | `ubuntu-latest` | yes (`cross`) | tar.gz |
| `x86_64-apple-darwin` | `macos-13` (last Intel runner) | no | tar.gz |
| `aarch64-apple-darwin` | `macos-latest` | no | tar.gz |
| `x86_64-pc-windows-msvc` | `windows-latest` | no | zip |

Each leg installs the toolchain with the matrix `target`, caches with a per-target key, and builds `cargo build --release --target <triple> -p seele-cli` — except the `aarch64-linux` leg, which installs `cargo install cross --locked` and runs `cross build` (gated by `if: matrix.cross`). The `Package binary` step (a bash script even on Windows via `shell: bash`) derives `version="${GITHUB_REF_NAME#v}"`, names the asset `seele-${version}-${target}` following the predictable `<name>-<version>-<target>.<ext>` convention so the installers can compute the download URL from a tag alone, archives the binary (`7z` zip on Windows, `tar -czf` elsewhere), and writes a `.sha256` sidecar using the platform tool (`shasum -a 256` on macOS, PowerShell `Get-FileHash` on Windows, `sha256sum` on Linux). Both the archive and its sidecar are uploaded via `actions/upload-artifact@v4` with `retention-days: 7`. The binary inside is named `seele`/`seele.exe`, matching the `[[bin]] name = "seele"` in `seele-cli/Cargo.toml`.

**`release`** — `needs: build`, runs on `ubuntu-latest`, gated `if: startsWith(github.ref, 'refs/tags/v')` so `workflow_dispatch` runs are dry exercises of the matrix only. It downloads all artifacts (`merge-multiple: true`), computes a prerelease flag (`true` when the tag contains a hyphen, e.g. `v0.1.0-rc.1`), then creates the release with `softprops/action-gh-release@v2`, uploading every `*.tar.gz`, `*.zip`, and `*.sha256`, with `generate_release_notes: true` (auto-notes, with the option of CHANGELOG-sourced body).

**`crates-io`** — `needs: build`, runs on every invocation a `cargo publish --dry-run --allow-dirty` for each publishable crate in strict dependency order, looping because `cargo publish` does not support `--workspace` (rust-lang/cargo#10948 is cited inline). The loop covers exactly **12** crates in this order: `seele-core`, `seele-storage`, `seele-embedder`, `seele-search`, `seele-sync`, `seele-engram-import`, `seele-project`, `seele-setup`, `seele-http`, `seele-mcp`, `seele-tui`, `seele-cli` (release.yml:171-173 and 185-187). Note this is one fewer than the 13 workspace members: `seele-chat` is **not** in the publish list (it is built and tested by CI but excluded from the crates.io loop). The inline comment claims `seele-cli` is last "because everything else depends-on it transitively," which is phrased backwards (it is `seele-cli` that depends on the rest); the operative fact is that leaf crates publish before the crates that depend on them. The actual publish step is gated `if: github.event_name == 'workflow_dispatch' && github.event.inputs.crates_io_publish == 'true'`, uses `CARGO_REGISTRY_TOKEN` from secrets, and republishes in the same 12-crate order; a second publish of an already-published version is a hard crates.io error, which the comment notes is the intended idempotency guard for re-runs.

### 18.6 The two critical version pins

**`sqlite-vec` v0.1.9 (vendored, not a crate).** The vec0 loadable extension is *not* consumed as a Rust crate; precompiled upstream binaries live in `crates/seele-storage/vendor/sqlite-vec/` and are embedded into `seele-storage` via `include_bytes!`. `vec0_loader.rs` selects the right blob with `#[cfg(all(target_os=..., target_arch=...))]` for the five supported targets (e.g. `crates/seele-storage/src/vec0_loader.rs:11-12` for linux-x86_64) and exposes `pub fn vec0_bytes() -> Option<&'static [u8]>`; unsupported targets get `None`, which the README says should surface as a clear startup error. Embedding (rather than a `build.rs` download) keeps `cargo install seele` working offline with no runtime fetch, at the documented cost of ~880 KB added to the crate (ADR-11). The vendor README (`crates/seele-storage/vendor/sqlite-vec/README.md`) carries the authoritative bump procedure: bump the version in the README + CHANGELOG, download the upstream `loadable-{target}.tar.gz` archives, extract each `vec0.{so|dylib|dll}` into its subdir, replace `CHECKSUMS-upstream.txt`, run `cargo test -p seele-storage` to confirm the load contract, and commit `vendor: bump sqlite-vec to vX.Y.Z`. `CHECKSUMS-upstream.txt` is the upstream `checksums.txt` for v0.1.9, allowing manual re-verification of the embedded bytes. The vendored extension is dual-licensed Apache-2.0 OR MIT (copyright 2024 Alex Garcia), so both license texts are kept alongside (`LICENSE-APACHE-upstream.txt`, `LICENSE-MIT-upstream.txt`). CLAUDE.md explicitly forbids touching the vendored binaries without updating that README.

**`ort =2.0.0-rc.10` (exact pin).** The embedder's ONNX runtime is pinned exactly because 2.0.0 stable does not exist as of 2026-05. `Cargo.lock` confirms the resolved version (`Cargo.lock:1758-1759`, `name = "ort"`, `version = "2.0.0-rc.10"`). The documented bump procedure is *do not bump manually* — wait for Dependabot to open a PR when upstream releases stable, then unpin and add a CHANGELOG "Pinned" entry (Cargo.toml:51-53, dependabot.yml:20-22, CLAUDE.md "No hacer"). This pin is what forces the MSVC CRT workaround in §18.3.

### 18.7 Dependabot (`dependabot.yml`)

`.github/dependabot.yml` (version 2) configures two ecosystems. The `cargo` updater (root directory) runs on a **monthly** schedule (Monday 09:00 `America/Santiago`) with `open-pull-requests-limit: 5`, commit prefix `deps` (scope included), and labels `dependencies`/`rust`. The monthly cadence is deliberate: it avoids weekly minor-bump noise while real CVEs still arrive via security advisories regardless of interval (inline comment, lines 6-9). Crucially, the comment at lines 20-22 records that `ort`'s pin is *not* ignored — the team explicitly wants the bump PR when 2.0.0 stable lands. The `github-actions` updater runs monthly too, limit 3, prefix `ci`, labels `dependencies`/`github-actions`, keeping the action versions in the three workflows current.

### 18.8 Web deploy workflow (`deploy-web.yml`)

`.github/workflows/deploy-web.yml` publishes the Astro landing site (covered in §17) to GitHub Pages. It triggers on `push` to `main` filtered to `web/**` and the workflow file itself, plus `workflow_dispatch`. Permissions are `contents: read`, `pages: write`, `id-token: write` (OIDC for the Pages deploy), and a `concurrency: group: pages` with `cancel-in-progress: false` serializes deploys. The `build` job (`ubuntu-latest`) checks out, sets up Node 20 with npm cache keyed on `web/package-lock.json`, runs `actions/configure-pages@v5` (which flips the site to `build_type: workflow` — the comment notes this is required or `deploy-pages` fails against a branch-source site), then `npm ci` + `npm run build` in `web/`, and uploads `web/dist` via `actions/upload-pages-artifact@v3`. The `deploy` job (`needs: build`) binds the `github-pages` environment and runs `actions/deploy-pages@v4`. This pipeline is independent of the Rust release pipeline; `web/` build artifacts (`node_modules`, `dist`, `.astro`, `.cache`) are gitignored (`.gitignore:46-51`).

### 18.9 Install scripts

Two mirrored installers in `scripts/` provide the `curl | bash` and `irm | iex` one-liners, both verifying SHA256 before installing.

`scripts/install.sh` (Unix) honors `$SEELE_VERSION` (or `$1`; defaults to latest stable via the GitHub releases API) and `$SEELE_INSTALL_DIR` (default `$HOME/.local/bin`). It runs `set -euo pipefail`, detects OS/arch (`uname -s`/`uname -m`) and maps to a target triple (`unknown-linux-gnu`/`apple-darwin` × `x86_64`/`aarch64`), dying with a clear message on unsupported platforms. It resolves the latest tag by grepping `tag_name` from the API, strips the leading `v` for the asset name (`seele-${version_bare}-${target}.tar.gz`), downloads the archive and `.sha256` sidecar into a `mktemp -d` (cleaned via `trap`), and verifies: prefer `sha256sum -c`, fall back to comparing `shasum -a 256` output, and `die` if neither tool exists — it refuses to install without verification. On success it extracts, moves `seele` into the install dir, `chmod +x`, warns if the dir is not on `$PATH`, and runs `seele --version`.

`scripts/install.ps1` (Windows) mirrors this with `$ErrorActionPreference = 'Stop'`, honoring `$env:SEELE_VERSION` and `$env:SEELE_INSTALL_DIR` (default `$env:USERPROFILE\.seele\bin`). It rejects any arch other than `X64` (v0.1 ships Windows x86_64 only), resolves the latest tag via `Invoke-RestMethod`, downloads the `.zip` + `.sha256`, parses the expected hash robustly with `-split '\s+'` (the sidecar's whitespace differs by which builder wrote it), compares against `Get-FileHash -Algorithm SHA256`, and `Write-Error`s on mismatch. It then `Expand-Archive`s, moves `seele.exe` into the install dir, hints how to add it to the user PATH, and runs `seele.exe --version`. Both scripts depend entirely on the release workflow's asset-naming and sidecar contract (§18.5); a change to either side breaks installs silently except for the version line.

### 18.10 STELE residual checks and the v0.1.0 smoke

`scripts/check-no-stele-residual.{sh,ps1}` enforce that the legacy project name "STELE"/"stele" never reappears in source. Each greps the repo (over `*.md,*.rs,*.toml,*.yaml,*.yml,*.json,*.sh,*.ps1`, excluding `target`/`.git`/`node_modules`) for the word-boundary pattern `\b(STELE|stele)\b` and reports any hit outside a hand-maintained allowlist. The allowlist has two parts that must stay identical between the two scripts: `ALLOWLIST_FILES`/`$allowlistFiles` (exact relative paths — historical docs, the CI YAML, the two scripts themselves, CHANGELOG/CLAUDE/INDEX) and `ALLOWLIST_DIRS`/`$allowlistDirs` (path prefixes — devlogs and táctica plan dirs that legitimately reference the old name). The bash script's comments flag two portability gotchas: `\b` is not portable to busybox grep (so an alpine CI runner would need a different pattern — `check-no-stele-residual.sh:16-23`), and the previous basename-based exclusion bug was fixed after the pwsh job caught residuals the bash job silently passed. Both exit 1 on any violation, which fails the corresponding CI job.

`scripts/v0.1.0-smoke.sh` is the v0.1.0 acceptance harness — an idempotent script that builds the release binary into a tempdir and exercises eleven criteria against ephemeral DBs with `SEELE_FAKE_EMBEDDER=1` (so it never depends on the ONNX download): (1) `cargo build --release -p seele-cli`; (2) a full CLI round-trip save→list→show→search→delete→restore→link→stats→projects→doctor with JSON assertions; (3) `seele mcp` `tools/list` returns exactly 19 tools; (4) `seele serve` `/openapi.json` exposes ≥15 paths (the comment at lines 99-103 records the plan's "25+" as an early overestimate — v0.1 ships 18 unique URLs / 22 operations — and relaxes the threshold to avoid chasing it); (5) a gated, slow perf smoke (`SEELE_SMOKE_PERF=1`); (6) `seele-project` `detect_e2e`; (7) sync export→import round-trip; (8) `seele tui --smoke` one-frame render; (9)–(10) CI matrix green and `release.yml` firing, both verified out-of-band; (11) ENGRAM credit present in README + CREDITS. Criterion 6's comment (lines 116-122) documents a known gap carried into v0.2: the CLI does not yet auto-call `seele_project::detect` from `seele save` (the `commands/save.rs` doc string references "Sprint-04 Bloque D.2" wiring that did not ship).

### 18.11 File-by-file map

| File | Role |
| --- | --- |
| `Cargo.toml` | virtual workspace, shared package metadata, `workspace.dependencies`, release profile |
| `Cargo.lock` | resolved dependency graph; confirms `ort` 2.0.0-rc.10 and checksums |
| `rust-toolchain.toml` | pins channel `stable` + rustfmt/clippy, minimal profile |
| `rustfmt.toml` | single line `edition = "2021"`; relies on rustfmt defaults |
| `.cargo/config.toml` | MSVC dynamic-CRT workaround for the ort/tokenizers link mismatch |
| `.github/workflows/ci.yml` | test matrix (3 OS) + clippy/fmt + bash & pwsh STELE checks |
| `.github/workflows/release.yml` | 5-target prebuilt binaries + SHA256 sidecars + GH Release + gated crates.io |
| `.github/workflows/deploy-web.yml` | Astro `web/` build + GitHub Pages deploy |
| `.github/dependabot.yml` | monthly cargo + github-actions updates; documents the ort pin watch |
| `scripts/install.sh` | Unix curl\|bash installer with SHA256 verify |
| `scripts/install.ps1` | Windows irm\|iex installer with SHA256 verify |
| `scripts/check-no-stele-residual.sh` | bash static check for legacy-name residuals |
| `scripts/check-no-stele-residual.ps1` | pwsh sibling of the residual check |
| `scripts/v0.1.0-smoke.sh` | 11-criterion acceptance smoke against the release binary |
| `.gitignore` | excludes `target/`, `*.pdb`, secrets, `*.db`, embedder cache, web build artifacts |
| `crates/seele-storage/vendor/sqlite-vec/README.md` | vec0 vendoring rationale + bump procedure |
| `crates/seele-storage/vendor/sqlite-vec/CHECKSUMS-upstream.txt` | upstream v0.1.9 checksums for re-verification |

### 18.12 Connections, gotchas, and known limitations

The build subsystem ties the whole codebase together: the release profile and toolchain apply to all 13 crates; `seele-cli` is the only `[[bin]]` (`name = "seele"`) and the artifact every installer fetches; the vendored vec0 binaries flow into `seele-storage` and therefore into the final executable; the `ort` pin governs `seele-embedder` and forces the Windows CRT config. The asset-naming contract (`seele-<version>-<target>.<ext>` + `.sha256`) is shared, untyped glue between `release.yml` and both installers — a divergence breaks installs silently. Notable invariants and gotchas: the two STELE-residual allowlists must remain byte-identical between the bash and pwsh scripts (a desync caused a real escaped-residual incident); the bash `\b` pattern is non-portable to busybox grep if an alpine runner is ever added; `ort` must never be unpinned manually (Dependabot owns that bump); and the vendored vec0 binaries must never be replaced without updating the vendor README. The one documented functional gap surfaced here is criterion 6 of the smoke: project detection is not wired into `seele save`, deferred to v0.2.


---

## 19. End-to-End Data Flows

This section traces the five canonical SEELE data flows hop by hop, naming the exact function and file at every step. Where storage rows cross into the vec0 INTEGER rowid space, the `SeeleId::as_i64()` bridge is called out explicitly. The recurring shape is: a **transport** (CLI / MCP / HTTP) deserializes a request into a DTO, calls **one method on `SeeleService`** (`crates/seele-http/src/service.rs`), which calls into the **stores** (`crates/seele-storage/`), the **embedder** (`crates/seele-embedder/`), or the **search engine** (`crates/seele-search/`). The service layer is shared by both HTTP and MCP; the CLI builds the same `SeeleService` and calls the same methods directly (no network hop).

The unifying entry point for every DB-touching command is `build_service()` in `crates/seele-cli/src/app.rs:128`, which calls `init_db()` (`crates/seele-storage/src/lib.rs:34`) → `init_pool()` → `migrations::run_pending()`, then `pick_embedder()` (ONNX by default, `FakeEmbedder` if `--fake-embedder` / `SEELE_FAKE_EMBEDDER` is set or ONNX init fails), and finally `SeeleService::new(pool, embedder)`.

### 19.1 The SeeleId → vec0 rowid bridge (used by every flow)

vec0 (`observations_vec`) and FTS5 (`observations_fts`) are virtual tables keyed by an INTEGER `rowid`, but the canonical PK is the ULID string `observations.id`. The authoritative bridge is `SeeleId::as_i64()` (`crates/seele-core/src/id.rs:35`):

```rust
// crates/seele-core/src/id.rs:35
pub fn as_i64(&self) -> i64 {
    let bytes = self.0.to_bytes();
    let mut int_bytes = [0u8; 8];
    int_bytes[1..8].copy_from_slice(&bytes[9..16]); // last 7 ULID bytes, top byte zeroed → non-negative
    i64::from_be_bytes(int_bytes)
}
```

This value is stored verbatim into the `observations.int_id` column at INSERT time (`save_in_tx`, `save_raw_in_tx`). The schema comment at `V001__initial_schema.sql:7-13` is emphatic that the SQL "virtual generated column" idea for `int_id` was abandoned because `CAST` of a base32 ULID yields 0; `int_id` is a **real** `INTEGER NOT NULL UNIQUE` column populated from Rust. The FTS triggers (`observations_ai/ad/au`, `V001:102-117`) mirror `new.int_id`/`old.int_id` into `observations_fts.rowid`, and `set_embedding` writes `observations_vec.rowid = int_id`. Searches then JOIN back via `JOIN observations o ON o.int_id = vec.rowid` / `= fts.rowid`. So the chain is: `SeeleId::as_i64()` → `observations.int_id` → FTS/vec `rowid`. The shared-context caveat holds: `as_i64()` is the authoritative mapping; `int_id` is the column that materializes it for JOINs.

---

### 19.2 (A) SAVE flow — write path

The SAVE flow has three transport entries that all converge on `SeeleService::save_observation` (`service.rs:73`).

| Entry | File:line | Builds |
|-------|-----------|--------|
| CLI `seele save` | `crates/seele-cli/src/commands/save.rs:48` (built), `:59` (dispatched) | `SaveRequest` (sets `tool_name = Some("seele-cli")`, `session_id = None`) |
| MCP `seele_save` | `crates/seele-mcp/src/tool_impls/memories.rs:20` | `serde_json::from_value::<SaveRequest>` |
| HTTP `POST /memories` (and legacy `POST /save`) | `crates/seele-http/src/handlers.rs:23` | `Json<SaveRequest>` |

Step by step:

1. **Transport deserialization.** The CLI fills `SaveRequest` (`dto.rs:17`) from clap args; `r#type` defaults to `"memory"` via `default_type`. MCP/HTTP deserialize JSON directly. All three then call `svc.save_observation(req)`.
2. **Field parsing (`service.rs:74-93`).** `session_id` is parsed via `parse_id`; `scope` via `parse_scope` (defaults to `Scope::Project`); `kind` via `parse_type` (relaxed — any string is accepted, unknown maps to a custom type); `metadata` via `parse_metadata`. These assemble a `SaveInput` (`observations.rs:26`).
3. **`ObservationStore::save` (`observations.rs:119`)** checks a connection out of the r2d2 pool, opens a `transaction()`, and calls the free function `save_in_tx` (`observations.rs:439`). Everything below 4–7 runs inside that one transaction; `tx.commit()` at `observations.rs:124` is the atomic boundary for the row + its FTS trigger writes.
4. **Privacy strip + normalized hash (`observations.rs:440-443`).** `strip_private_tags` (`privacy.rs:18`) removes every `(?si)<private>.*?</private>` block from **both** `title` and `content`; unclosed tags are left intact on purpose. `normalized_hash` (`hash.rs:17`) lowercases, collapses whitespace runs, trims, and SHA-256-hexes the *stripped* content. Order matters: stripping happens before hashing and before the row is written (so FTS never indexes private text).
5. **Topic-key upsert (`observations.rs:447-491`).** If `topic_key` is `Some`, the tx queries for an active row with the same `(project, scope, topic_key)` (`idx_obs_topic_upsert`). On a hit it `UPDATE`s title/content/hash/metadata/type/tool_name, sets `revision_count = rev + 1` and `last_seen_at = updated_at = now`, and returns `SaveOutcome::UpsertedTopic { id, revision_count }`. The UPDATE fires the `observations_au` trigger which re-syncs FTS (delete old rowid + insert new). No new ULID is minted; `int_id` is unchanged.
6. **24h normalized-hash dedup (`observations.rs:493-535`).** If no topic upsert happened, the tx looks for an active row matching `normalized_hash` AND `project` (NULL-safe) AND `scope` AND `type` AND `title` AND `last_seen_at >= now - DEDUP_WINDOW_MS` (`DEDUP_WINDOW_MS = 24*60*60*1000`, `observations.rs:22`). On a hit it bumps `duplicate_count` and `last_seen_at`, returning `SaveOutcome::DuplicateMerged { id, duplicate_count }`. This is ENGRAM-inherited and uses `idx_obs_dedup`.
7. **Insert new row (`observations.rs:537-579`).** Otherwise a fresh `SeeleId::new()` is generated, `int_id = id.as_i64()` (← the bridge), and the row is `INSERT`ed with `revision_count = duplicate_count = 0` and `created_at = updated_at = last_seen_at = now`. The `observations_ai` trigger (`V001:102`) immediately copies `title, content, tool_name, type, project` into `observations_fts` keyed by `new.int_id`. If the INSERT hits a `UNIQUE` violation specifically on `observations.int_id` (an `as_i64()` tail collision), the loop regenerates the ULID up to `ID_COLLISION_RETRIES = 5` (`observations.rs:24,565-572`); exhausting retries yields `StorageError::Conflict`. Returns `SaveOutcome::Created(id)`.
8. **Post-save embedding (best effort) (`service.rs:96-107`).** Back in the service, *after* `save()` commits, it calls `self.embedder.embed(&req.content)`. For ONNX the `Embedder::embed` entry is `onnx.rs:197`, delegating to `run_inference` (`onnx.rs:115`) which tokenizes, runs the session under a `Mutex<Session>` (lock released before pooling, `onnx.rs:142-167`), mean-pools with attention-mask weighting, and L2-normalizes to a 384-dim vector (`DEFAULT_DIM = 384`, `onnx.rs:31`); for `FakeEmbedder` (`embed` at `fake.rs:45` → `hash_to_vector` at `fake.rs:23`) it SHA-256-hashes the text per axis-block into a deterministic L2-normalized 384-dim vector. The vector is written by `ObservationStore::set_embedding` (`observations.rs:318`), which looks up `int_id` for the (active) row, packs the `f32`s little-endian, and runs `INSERT OR REPLACE INTO observations_vec(rowid, embedding) VALUES (int_id, bytes)` (`observations.rs:333-336`) — the **second** place the `int_id`/`as_i64()` bridge is materialized in the write path. Crucially this is **best effort**: an embedder or write failure only logs `tracing::warn!` and does **not** turn into a 5xx; the observation row persists and the vec branch simply skips it until a reindex. This is a documented gotcha (`service.rs:69-72`): the row and the embedding are *not* in the same transaction. (Note: the service always embeds `req.content` — the *raw* request content, not the privacy-stripped content that `save_in_tx` indexed into FTS; for inputs without `<private>` blocks these are identical.)
9. **Response mapping (`service.rs:109-131`).** The `SaveOutcome` is mapped to a `SaveResponse { id, outcome: "created"|"upserted_topic"|"duplicate_merged", revision_count?, duplicate_count? }` (`dto.rs:45`). HTTP returns `Json<SaveResponse>` (200); MCP wraps it (see 19.4); the CLI prints `saved <id> (<outcome>)`.

> Edge case: because dedup keys on the post-strip `title`+`content`, a reformat-only edit (whitespace/case) within 24h merges as a duplicate (`hash.rs` is intentionally aggressive). Topic upsert takes precedence over dedup — if `topic_key` is set, the dedup branch is never reached.

---

### 19.3 (B) SEARCH flow — hybrid FTS + vec0 + RRF

Entries converge on `SeeleService::search_observations` (`service.rs:136`).

| Entry | File:line | Empty-query gate |
|-------|-----------|------------------|
| CLI `seele search` | `crates/seele-cli/src/commands/search.rs:47` | calls `enforce_search_query_or_filter` before dispatch |
| MCP `seele_search` | `crates/seele-mcp/src/tool_impls/memories.rs:28` | calls `enforce_search_query_or_filter` |
| HTTP `POST /search` | `crates/seele-http/src/handlers.rs:35` | calls `enforce_search_query_or_filter` |

1. **Anti-exfiltration gate (`service.rs:442`).** `enforce_search_query_or_filter` rejects an empty `query` when *no* filter (`project`/`scope`/`type`) is present, returning `ApiError::BadRequest`. This is applied at the transport layer by all three callers (the service method itself does not re-check — see its doc at `service.rs:134`). It mitigates the "list-all-DB" vector flagged by Cloven (2026-05-10).
2. **Build `SearchQuery` (`service.rs:141-152`).** `scope` is parsed; the request maps to `seele_search::SearchQuery` (`engine.rs:17`) carrying `per_method_limit`, `limit`, `include_purist`, `score_boost_multiplier`, `max_vec_distance`, `include_annotations`.
3. **`SearchEngine::search` (`engine.rs:96`).** If `query.text` is blank it short-circuits to `list_by_filters` (`engine.rs:143`) — a plain `SELECT ... ORDER BY created_at DESC LIMIT` with no embedder call (score `0.0`, no ranks). Otherwise it computes `per_method = per_method_limit.unwrap_or(50)` and `final_limit = limit.unwrap_or(10)`.
4. **FTS branch `fts_query` (`engine.rs:191`).** Runs `SELECT o.id FROM observations_fts fts JOIN observations o ON o.int_id = fts.rowid WHERE observations_fts MATCH ?1 AND o.deleted_at IS NULL ... ORDER BY rank LIMIT ?`. The user text is wrapped by `escape_fts` (`engine.rs:501`) into a quoted phrase with doubled internal quotes. Filters (`project`/`scope`/`type`/non-purist) are appended. Returns a `Vec<SeeleId>` in rank order. The JOIN here is the read-side use of the `int_id` bridge.
5. **Vec branch `vec_query` (`engine.rs:232`).** Calls `self.embedder.embed(&query.text)` and checks the returned length equals `embedder.dim()` (else `SearchError::DimensionMismatch`). The vector is packed little-endian and the query is `SELECT o.id, vec.distance FROM observations_vec vec JOIN observations o ON o.int_id = vec.rowid WHERE vec.embedding MATCH ?1 AND vec.k = ?2 AND o.deleted_at IS NULL ... ORDER BY vec.distance`. `vec.k = per_method` is the KNN bound; optional `max_vec_distance` adds `AND vec.distance <= ?`. Again JOINs through `int_id`. Returns `Vec<SeeleId>` ordered by ascending distance.
6. **RRF fusion `rrf::combine` (`engine.rs:107` → `rrf.rs:31`).** Both ranked lists are passed as `[("fts", fts_rank), ("vec", vec_rank)]` with `k = DEFAULT_K = 60` (`rrf.rs:15`). Each doc accrues `sum 1/(k + rank)` (rank starts at 1), and `per_source` records `(source, rank)` per appearance. A doc appearing in both lists outranks one in a single list. Results sort by descending score with stable ties (first-seen first).
7. **Optional score boost (`engine.rs:113` → `apply_score_boost`, `engine.rs:290`).** If `score_boost_multiplier != 0`, each score is multiplied by `1 + multiplier * meta_score` (read from the `meta_score` virtual column, null treated as 1.0 per ADR-03), then re-sorted. Default `0.0` is a no-op. The list is truncated to `final_limit` (`engine.rs:114`).
8. **Hydration (`engine.rs:116` → `hydrate`, `engine.rs:431`).** The surviving `SeeleId`s are batch-fetched (`SELECT ... WHERE id IN (...)`) and mapped into `SearchHit { observation, score, fts_rank, vec_rank, annotations: [] }`, with `unpack_per_source` (`engine.rs:458`) splitting the RRF `per_source` back into `fts_rank`/`vec_rank`.
9. **Optional annotations (`engine.rs:135` → `attach_annotations`/`fetch_annotations`, `engine.rs:343/354`).** When `include_annotations`, one extra query against `memory_relations` (joined to `observations` for the other endpoint's title) classifies each hit as `Supersedes`/`SupersededBy`/`ConflictsWith`/`ContestedBy` via `annotation_for_source`/`annotation_for_target` (`engine.rs:474/486`). Default off to keep the sub-300ms target.
10. **DTO + response (`service.rs:154-156`).** Hits become `SearchHitDto` (`dto.rs:84`, exposing `id`, `title`, `content`, `score`, `fts_rank`, `vec_rank`, `annotations`, etc.) inside `SearchResponse { hits, count }`. HTTP returns `Json`; MCP stringifies (19.4); the CLI prints `<id> [<type>] <score> <title>` per hit.

> Gotcha: if the embedder failed at SAVE time (step A.8), the row has an FTS entry but no vec0 row, so it can only surface through the FTS branch. The vec branch silently omits it. With `FakeEmbedder`, vec hits are hash-deterministic rather than semantic — the CLI warns about degraded quality on ONNX-init fallback (`app.rs:151`).

---

### 19.4 (C) MCP `tools/call` lifecycle — stdin frame to stdout frame

Driven by `seele mcp` (`crates/seele-cli/src/commands/mcp.rs:14`), which builds the `SeeleService` and calls `McpServer::new(svc, McpServerConfig { tool_prefix })`. `McpServer::new` (`server.rs:31`) calls `build_index(prefix)` to materialize the tool name → `Tool` map.

1. **Read loop (`server.rs:46` `run_io`).** `run_stdio` wires real `tokio::io::stdin/stdout`; `run_io` wraps the reader in `BufReader::lines()` and reads one line per iteration. Blank lines are skipped. The transport is line-delimited JSON-RPC 2.0 (not Content-Length framed).
2. **Parse + route (`handle_line`, `server.rs:68`).** The line is `serde_json::from_str::<Request>` (`jsonrpc.rs:11`). A parse failure returns `Response::err(Null, PARSE_ERROR=-32700)`. If `req.id` is `None` it is a **notification** → returns `None` and the loop writes nothing. Otherwise it matches `req.method`:
   - `"initialize"` → returns `serverInfo {name:"seele", version}`, `protocolVersion:"2024-11-05"`, `capabilities.tools`.
   - `"tools/list"` → emits every registered `ToolDescriptor { name, description, inputSchema }`, sorted by name.
   - `"tools/call"` → `dispatch_tool_call`.
   - anything else → `METHOD_NOT_FOUND=-32601`.
3. **Dispatch (`dispatch_tool_call`, `server.rs:113`).** Reads `params.name` (missing → `INVALID_PARAMS=-32602`) and `params.arguments` (defaulting to `{}`). Looks the tool up in the index (unknown → `INVALID_PARAMS "unknown tool: <name>"`).
4. **Handler invocation (`server.rs:137`).** `(tool.handler)(&self.service, arguments)` runs the `fn(&SeeleService, Value) -> Result<Value, ToolError>` shim. For `seele_save`/`seele_search` these are the same `memories::save`/`memories::search` shown in 19.2/19.3 — i.e. the MCP path **reuses the HTTP service layer**, deserializing `arguments` into the very same `SaveRequest`/`SearchRequest` DTOs (`tool_impls/memories.rs:21,27`). `seele_search` re-applies `enforce_search_query_or_filter` before calling the service.
5. **Result framing (`server.rs:138-154`).** On `Ok(v)`, the handler's arbitrary JSON is stringified and wrapped in the MCP `CallToolResult` shape `{ "content": [{ "type": "text", "text": <stringified-json> }], "isError": false }`. (`structuredContent` from the 2025-06-18 spec is deferred — see the comment at `server.rs:139-144`.) On `Err(e)`, `ToolError::to_jsonrpc` (`tools.rs:31`) maps `BadParams → INVALID_PARAMS(-32602)`, `NotFound`/`Conflict → TOOL_ERROR(1001)`, `Internal → INTERNAL_ERROR(-32603)`. `ApiError` from the service is converted into `ToolError` via the `From` impl at `tools.rs:41`.
6. **Write frame (`server.rs:56-61`).** The `Response` is serialized and a single line + `\n` is written to stdout and flushed.

> Tool naming (ADR-13): `build_index` (`tools.rs:205`) renames tools when a prefix is given. `--tool-prefix mnema` produces `mnema_save`, `mnema_recall` (the special `search → recall` rename in `RENAMES`, `tools.rs:223`), etc. `None` or `"seele"` keeps canonical `seele_*`. The handler set is identical regardless of prefix.

---

### 19.5 (D) SYNC export → import round-trip across machines

Sync (`crates/seele-sync/src/lib.rs`) moves observations as git-friendly gzip chunks. Both halves go through the CLI `seele sync` subcommand (`crates/seele-cli/src/commands/sync.rs:35`).

**Export (machine A):**

1. CLI `sync export <dir> [--project P]` builds the service and calls `seele_sync::export_to_dir(&svc.observations, &dir, ExportFilter { project })` (`sync.rs:47`).
2. `export_to_dir` (`lib.rs:181`) → `build_chunk` (`lib.rs:136`) runs `ObservationStore::list` with `include_deleted: false` (soft-deleted rows never travel) and assembles a `ChunkPayload { format_version: 1, seele_version, exported_at, project, observations }` (`lib.rs:65`).
3. `compute_chunk_id` (`lib.rs:162`) SHA-256-hashes `format_version` (LE) + `project` bytes + a `0u8` separator + the observations serialized as JSON **after sorting by `id`**. `exported_at` and `seele_version` are deliberately excluded, so two exports of the same set produce the **same** `chunk_id` (content-addressed, deterministic).
4. `write_chunk_file` (`lib.rs:201`) gzips the full payload JSON (`GzEncoder`, default compression) to `<dir>/<chunk_id>.json.gz`. The report carries `chunk_id`, path, `observation_count`, `bytes_on_disk`. The file can be committed to a git repo and pushed.

**Import (machine B):**

1. CLI `sync import <path> --target-key <key>` calls `seele_sync::import_from_file(&svc.observations, &svc.chunks, &target_key, &path)` (`sync.rs:62`). `target_key` identifies the local node (hostname/UUID).
2. `read_chunk_file` (`lib.rs:216`) gunzips and deserializes the payload. It rejects `format_version > CURRENT_FORMAT_VERSION (1)` and, if the filename stem is a 64-hex string, verifies it equals the **recomputed** `chunk_id` (integrity check catching silent corruption, `lib.rs:235-242`).
3. **Idempotency pre-check (`lib.rs:291`).** `ChunkStore::was_imported(target_key, chunk_id)` (`chunks.rs:58`) queries `sync_chunks`. If present, returns `ImportReport { outcome: AlreadyImported, ... observation_count_skipped_chunk_level = N }` without opening a transaction.
4. **Atomic import (`lib.rs:305-335`).** A single connection is checked out and `conn.transaction()` opened. For each observation, a `RawSaveInput` is built and saved via `ObservationStore::save_raw_in_tx` (`observations.rs:582`) — the **raw** path: it bypasses privacy strip, topic-key upsert, and the dedup window, and uses `INSERT OR IGNORE` keyed on `id`. This **preserves source ULIDs**, so the same observation arriving from two machines collapses on the shared `id` (returns `RawSaveOutcome::AlreadyExisted`) rather than duplicating. `int_id` is recomputed as `input.id.as_i64()` (`observations.rs:588`) — the bridge again — and the `observations_ai` trigger indexes the new row into FTS. Note: embeddings are **not** part of the chunk and are not written during import; vec0 rows for imported observations only appear after a reindex/re-embed.
5. **`session_id` is dropped (`lib.rs:317`).** The raw input forces `session_id: None` because sessions don't travel in v0.1 chunks; keeping a foreign id would trip the `observations.session_id` FK (`REFERENCES sessions(id)`) and abort the whole tx (closes Cloven 2026-05-11 [MEDIO 1]).
6. **Ledger write + commit (`lib.rs:334-335`).** `ChunkStore::mark_imported_in_tx(&tx, target_key, chunk_id)` (`chunks.rs:44`) records the chunk in `sync_chunks` *inside the same tx*, then `tx.commit()`. A crash mid-loop rolls back both the saves and the ledger mark, so a re-run reprocesses cleanly (closes Cloven 2026-05-11 [CRITICO]).
7. The report distinguishes `observation_count_saved` (freshly inserted), `observation_count_already_present` (per-row `INSERT OR IGNORE` skips), and `observation_count_skipped_chunk_level` (whole-chunk skip). The CLI prints all three.

> Round-trip invariant: A→export→git→B→import preserves ULIDs and content; re-importing the same file under the same `target_key` is a no-op (ledger); importing the same observation from a different chunk/target collapses on `INSERT OR IGNORE`. The cost is that `metadata` carries the only breadcrumb of the originating session.

---

### 19.6 (E) HTTP request lifecycle — auth, handler, service, storage, response

Server is launched by `seele serve` (`crates/seele-cli/src/commands/serve.rs:49`), which builds the service, resolves optional chat config, and calls `Server::run` (`crates/seele-http/src/server.rs:190`).

1. **Router assembly (`server.rs:99` `router`).** `AppState { service: Arc<SeeleService>, chat }` is built. Protected routes (`/memories`, `/search`, `/sessions`, `/links`, `/relations`, `/conflicts`, `/stats`, `/embedder`, `/chat`, ...) are registered on one sub-router; legacy ENGRAM aliases (`POST /save`, `GET /show/{id}`) are added only if `legacy_engram_paths` (ADR-13, `server.rs:154`). Public routes `/health`, `/version`, and the OpenAPI/Swagger routes (`openapi::routes()`) are merged **outside** the auth layer. Layers applied outermost-to-innermost: `TraceLayer`, `CorsLayer`, `CompressionLayer`.
2. **Bind + announce (`Server::run`, `server.rs:190-198`).** `TcpListener::bind` (`server.rs:192`) then `eprintln!("seele http listening on http://{addr}")` (`server.rs:194`, using the resolved `local_addr()` so `--port 0` callers can discover the OS-chosen port), then `axum::serve` (`server.rs:196`). The default port is `7777` and default bind `127.0.0.1` (`serve.rs:11,14`).
3. **Auth middleware (`server.rs:165` → `auth.rs:37`).** When `auth_bearer = Some(token)`, the protected sub-router is wrapped with `require_bearer`. `check_bearer` (`auth.rs:17`) reads `Authorization`, strips the mandatory `Bearer ` prefix, and compares the token (case-sensitive); a missing/mismatched token short-circuits to `401` before any handler runs. Public routes never see this layer.
4. **Extraction + handler (`handlers.rs`).** axum extracts `State<Arc<SeeleService>>` (via the `FromRef` impl, `server.rs:33`), plus `Path`/`Query`/`Json` as the route requires. Example: `save_memory` (`handlers.rs:23`) takes `Json<SaveRequest>`; `get_memory` (`handlers.rs:40`) takes `Path<String>` and calls `parse_id`. The handler is a thin shim that calls exactly one `SeeleService` method.
5. **Service + storage.** The service method runs the domain logic (for `/memories` and `/search`, the identical flows of 19.2/19.3, including the post-save embedding and the FTS/vec0 writes via the `int_id` bridge). Stores use the r2d2 pool (`init_pool`, `pool.rs:30-48`, `max_size = 8` per `PoolConfig::with_path`); each physical connection runs the canonical PRAGMAs (`journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`, `temp_store=MEMORY`) and loads the vec0 extension (entry point `sqlite3_vec_init`) once at creation via the `SqliteConnectionManager::with_init` closure — failure to load vec0 is fatal.
6. **Response / error mapping (`error.rs:38`).** A handler returning `Ok` serializes to `Json<T>` (200) or `StatusCode::NO_CONTENT` (204) for deletes/judgments. A handler returning `Err(ApiError)` is converted by `IntoResponse` into a JSON `ErrorBody { code, message }` with the canonical status: `BadRequest→400`, `Unauthorized→401`, `NotFound→404`, `Conflict→409`, `Internal→500`. The `From` impls at `error.rs:58-96` collapse `StorageError`/`SearchError`/`EmbedderError`/`SeeleError` into `ApiError` (e.g. `StorageError::NotFound → ApiError::NotFound`, embedder errors → `Internal`).

> The `/chat` handler (`handlers.rs:254`) is the one stateful exception: it builds a `ChatProvider` — `AnthropicProvider` when `provider == "anthropic"` (case-insensitive), otherwise `OpenAICompatibleProvider` driving a `/v1/chat/completions` endpoint (`handlers.rs:304-315`) — and registers a `seele_search` tool whose handler closure calls `svc.search_observations` (`handlers.rs:343`) — i.e. the LLM's tool-use loops back through the same SEARCH flow, with `limit = args.limit.unwrap_or(5).min(20)` (`handlers.rs:331`, default 5, hard cap 20) and `score_boost_multiplier = 1.0` (`handlers.rs:339`), returning 280-char content snippets (`handlers.rs:355`). Per-request `api_key`/`provider`/`model`/`endpoint` override the CLI config and are used once, never persisted (`handlers.rs:215`).

---

### 19.7 Cross-flow summary

- **Single service core.** CLI, MCP, and HTTP all funnel into `SeeleService` methods; the only differences are (de)serialization and the error envelope (`ApiError` JSON vs `ToolError`/JSON-RPC vs anyhow/stdout). The empty-query gate is enforced identically by all three search entries.
- **Two materializations of the `as_i64()` bridge on write** (`observations.int_id` at INSERT, `observations_vec.rowid` at `set_embedding`) and **two reads** (FTS and vec JOINs `ON o.int_id = <rowid>`).
- **Atomicity boundaries differ:** SAVE commits the row + FTS trigger writes in one tx but writes the embedding *after* commit (best-effort, can desync); SYNC import commits all raw saves + the ledger mark in one tx (strict). The known limitation is the row/embedding split in SAVE — flagged in the service doc and worth a reindex command in a future section.


---

## 20. Database Schema Reference (Data Dictionary)

This section is a pure reference appendix: a column-by-column data dictionary for every table, virtual table, index, and trigger in the SEELE SQLite database. Everything here is grounded in the single migration file `crates/seele-storage/src/migrations/V001__initial_schema.sql` (the only migration in the tree) plus the column usage confirmed in the store modules (`observations.rs`, `prompts.rs`, `sessions.rs`, `relations.rs`, `links.rs`, `chunks.rs`).

### 20.1 Migration / schema version

SEELE uses [`refinery`](https://crates.io/crates/refinery) for migrations. The migration set is embedded at compile time:

```rust
// crates/seele-storage/src/migrations/mod.rs:7,9
use refinery::embed_migrations;
embed_migrations!("./src/migrations");
```

`run_pending(conn)` (`migrations/mod.rs:14-17`) runs all pending migrations via `migrations::runner().run(conn)`; `init_db(path)` (`lib.rs:34-39`) opens the pool, then checks out a connection and calls `run_pending`. The migrations directory currently contains exactly **one** file, `V001__initial_schema.sql`, so the **current applied migration version is V1** (refinery records this in its own bookkeeping table `refinery_schema_history`, created and managed automatically by refinery — it is not declared in any SEELE SQL). The header of V001 labels the logical schema as `SEELE schema v0.1.0` (`V001:2`). There is **no** hand-rolled `schema_version` table in the live DB (the genesis plans proposed one, but the implementation delegates to refinery). When the MCP `seele_doctor` tool returns a `schema_version` field, that value is `env!("CARGO_PKG_VERSION")` (`crates/seele-mcp/src/tool_impls/meta.rs:30`), i.e. the binary/crate version — `0.2.0` per the workspace `Cargo.toml` (`seele-mcp` uses `version.workspace = true`) — **not** the SQL migration number. Do not conflate the two.

A note on a table referenced elsewhere: `sync_apply_deferred` appears in `CREDITS.md` and the genesis ENGRAM-audit docs (`genesis/plans/estrategia/02-reimplementacion-inspirada.md`, `05-engram-feature-audit.md` marks it as "v0.2", needed only with cloud sync) as part of the inherited 9-table design, but it is **not** created by V001 and **does not exist** in the SEELE database. V001 declares exactly **9 table objects**: 6 ordinary `WITHOUT ROWID` tables (`sessions`, `observations`, `user_prompts`, `links`, `memory_relations`, `sync_chunks`) and 3 virtual tables (`observations_fts`, `observations_vec`, `prompts_fts`). FTS5 and vec0 each silently create additional internal shadow tables beyond those 9 (see 20.4 / 20.5).

Connection-level invariants applied to every pooled connection in `init_pool`'s `with_init` callback (`pool.rs:33-41`): `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`, `temp_store=MEMORY` (executed as a single `execute_batch`, then the vec0 extension is loaded). Because `foreign_keys=ON` is set per connection, the FK constraints below are actually enforced. Loading the vec0 extension is fatal on failure — the pool refuses to build without it.

#### Conventions used in this dictionary
- All `*_at` timestamp columns are stored as `INTEGER` epoch **milliseconds** (Rust `chrono::Utc::now().timestamp_millis()`); they are decoded back via `Utc.timestamp_millis_opt(ms).single()`.
- All `id`/`*_id` columns hold **ULID text** (26-char Crockford base32), the textual form of `SeeleId`, except `int_id` (see below).
- "Nullable" reflects the `NOT NULL` constraints in DDL.

### 20.2 `sessions`

A coding session. `WITHOUT ROWID`, PK on the ULID `id`.

| Column | SQL type | Null | Default | Meaning |
|---|---|---|---|---|
| `id` | TEXT | no | — | PK. Session ULID. |
| `project` | TEXT | no | — | Project name the session belongs to. |
| `directory` | TEXT | yes | — | Working directory path of the session. |
| `started_at` | INTEGER | no | — | Epoch-ms session start. |
| `ended_at` | INTEGER | yes | — | Epoch-ms end; NULL while active. |
| `summary` | TEXT | yes | — | End-of-session summary text. |
| `status` | TEXT | no | `'active'` | Lifecycle. `CHECK (status IN ('active','ended','aborted'))`. |

- **Primary key:** `id`.
- **Indexes:** `idx_sessions_project(project)`, `idx_sessions_started_at(started_at)`, `idx_sessions_status(status)` — all non-partial.
- **State transitions (sessions.rs):** `start` inserts `status='active'` with only `(id, project, directory, started_at, status)` set (`sessions.rs:34-57`); `end` flips to `'ended'` setting `ended_at`+`summary` **only** `WHERE status='active'` (`sessions.rs:59-73`); `abort` flips to `'aborted'` setting `ended_at` only `WHERE status='active'` (`sessions.rs:75-89`). Both `end`/`abort` return `StorageError::NotFound` if zero rows updated (no active session by that id).

### 20.3 `observations` (core entity)

The central memory record. `WITHOUT ROWID`, PK on ULID `id`, with a separate `int_id` used as the stable rowid bridge to FTS5 and vec0.

| Column | SQL type | Null | Default | Meaning |
|---|---|---|---|---|
| `id` | TEXT | no | — | PK. Observation ULID. |
| `int_id` | INTEGER | no | — | `UNIQUE`. ULID **random tail** mapped to i64; the FTS5/vec0 rowid bridge. |
| `session_id` | TEXT | yes | — | FK → `sessions(id)` `ON DELETE SET NULL`. |
| `type` | TEXT | no | — | Observation kind (e.g. `code`, `decision`, `learning`). FTS-indexed. |
| `title` | TEXT | no | — | Short title. Privacy-stripped before insert. FTS-indexed. |
| `content` | TEXT | no | — | Body text. Privacy-stripped; hashed for dedup. FTS-indexed. |
| `tool_name` | TEXT | yes | — | Originating tool. FTS-indexed. |
| `project` | TEXT | yes | — | Project scoping key. |
| `scope` | TEXT | no | `'project'` | `CHECK (scope IN ('project','personal'))`. |
| `topic_key` | TEXT | yes | — | Upsert key (e.g. `architecture/db`). NULL = no upsert. |
| `normalized_hash` | TEXT | yes | — | sha256 hex of normalized `content`; **dedup hash column**. |
| `revision_count` | INTEGER | no | `0` | Incremented on each topic_key upsert. |
| `duplicate_count` | INTEGER | no | `0` | Incremented on each dedup-window merge. |
| `last_seen_at` | INTEGER | no | — | Epoch-ms; touched on upsert/dedup merge. |
| `created_at` | INTEGER | no | — | Epoch-ms creation. |
| `updated_at` | INTEGER | no | — | Epoch-ms last mutation. |
| `deleted_at` | INTEGER | yes | — | **Soft-delete** marker; NULL = active. |
| `metadata` | TEXT | no | `'{}'` | Arbitrary JSON blob. |

**Virtual generated columns** (added by `ALTER TABLE` after table creation, `GENERATED ALWAYS AS ... VIRTUAL`, projecting `json_extract` paths out of `metadata`; these are computed on read, never stored — `V001:55-65`):

| Column | SQL type | JSON path |
|---|---|---|
| `meta_kind` | TEXT | `$.kind` |
| `meta_domain` | TEXT | `$.domain` |
| `meta_axiomatic` | INTEGER | `$.axiomatic` |
| `meta_score` | REAL | `$.score` |
| `meta_context_mode` | TEXT | `$.context_mode` |

- **Primary key:** `id`. **Unique:** `int_id`.
- **Foreign keys:** `session_id → sessions(id) ON DELETE SET NULL`.

**Indexes (`V001:67-87`; most live-row indexes are partial on `deleted_at IS NULL`):**

| Index | Columns | Partial predicate |
|---|---|---|
| `idx_obs_session` | `(session_id)` | `WHERE deleted_at IS NULL` |
| `idx_obs_project` | `(project)` | `WHERE deleted_at IS NULL` |
| `idx_obs_topic_upsert` | `(project, scope, topic_key)` | `WHERE topic_key IS NOT NULL AND deleted_at IS NULL` |
| `idx_obs_dedup` | `(normalized_hash, project, scope, type, title)` | `WHERE deleted_at IS NULL` |
| `idx_obs_meta_kind` | `(meta_kind)` | `WHERE meta_kind IS NOT NULL AND deleted_at IS NULL` |
| `idx_obs_meta_domain` | `(meta_domain)` | `WHERE meta_domain IS NOT NULL AND deleted_at IS NULL` |
| `idx_obs_meta_kind_domain` | `(meta_kind, meta_domain)` | `WHERE deleted_at IS NULL` |
| `idx_obs_meta_axiomatic` | `(meta_axiomatic)` | `WHERE meta_axiomatic = 1` |
| `idx_obs_meta_score` | `(meta_score)` | `WHERE deleted_at IS NULL` |
| `idx_obs_meta_context_mode` | `(meta_context_mode)` | `WHERE meta_context_mode IS NOT NULL AND deleted_at IS NULL` |
| `idx_obs_created_at` | `(created_at)` | `WHERE deleted_at IS NULL` |
| `idx_obs_deleted_at` | `(deleted_at)` | `WHERE deleted_at IS NOT NULL` (inverted: indexes the trash) |

**Column-usage notes confirmed in `observations.rs`:**
- `idx_obs_dedup` mirrors the dedup query key `(normalized_hash, project, scope, type, title)` plus a `last_seen_at >= now-DEDUP_WINDOW_MS` window and `deleted_at IS NULL`, where `DEDUP_WINDOW_MS = 24*60*60*1000` (24h, `observations.rs:22`). The dedup query (`observations.rs:495-520`) matches `project` with explicit NULL handling (`(project IS NULL AND ?2 IS NULL) OR project = ?2`). A matching live row increments `duplicate_count` and updates `last_seen_at` instead of inserting (`SaveOutcome::DuplicateMerged`, `observations.rs:521-535`).
- `idx_obs_topic_upsert` backs the topic upsert (`observations.rs:447-491`): a live row with the same `(project, scope, topic_key)` is updated in place. The UPDATE overwrites `title`, `content`, `normalized_hash`, `metadata`, `type`, `tool_name`, bumps `revision_count` by 1, and touches `last_seen_at`/`updated_at` (`SaveOutcome::UpsertedTopic`). Note the topic-key project match uses `project IS ?1` (NULL-safe equality), not `=`.
- `normalized_hash` is sha256-hex over normalized content: lowercase → split on whitespace and rejoin with single spaces (which also trims) → sha256 → hex (`hash.rs:17-28`); always 64 lowercase hex chars when present.
- `deleted_at` drives soft delete/restore: `soft_delete` sets `deleted_at = now` (and `updated_at = now`) only `WHERE deleted_at IS NULL` (`observations.rs:267-281`); `restore` clears it (and sets `updated_at = now`) only `WHERE deleted_at IS NOT NULL` (`observations.rs:283-297`); `hard_delete` does a real `DELETE` and then best-effort purges the vec0 row (`observations.rs:299-313`). Both `soft_delete` and `restore` return `NotFound` if zero rows matched.
- **`int_id` is a regular stored column, not a virtual generated column.** The DDL comment (`V001:7-13`) is explicit: an earlier plan tried a virtual generated `int_id` via `SUBSTR(id,...) AS INTEGER`, which is broken because ULIDs are base32 strings (`CAST` yields 0). SEELE instead stores `int_id` as a regular `INTEGER NOT NULL UNIQUE` column populated at INSERT time from Rust `SeeleId::as_i64()` (`observations.rs:540` in `save_in_tx`, `observations.rs:588` in `save_raw_in_tx`). The canonical mapping is `id.as_i64()`, which takes **ULID bytes 9..16** (the 7-byte / 56-bit random tail) into a big-endian i64 with the top byte zeroed to stay non-negative (`crates/seele-core/src/id.rs:35-41`) — note this is the random tail, **not** the timestamp prefix, and **not** "the first 6 bytes" (as some older ADR-02 / `CLAUDE.md` text claims; the running code uses bytes 9..16). On `int_id` collision the insert retries up to `ID_COLLISION_RETRIES = 5` times with a fresh ULID (`observations.rs:24, 538-579`), detecting the collision by matching a `ConstraintViolation` whose message contains `"observations.int_id"`; exhausting retries yields `StorageError::Conflict`.

### 20.4 `observations_fts` (FTS5 shadow table)

External-content FTS5 index over `observations`.

```sql
-- V001:90-99
CREATE VIRTUAL TABLE observations_fts USING fts5(
    title, content, tool_name, type, project,
    content='observations',
    content_rowid='int_id',
    tokenize='porter unicode61 remove_diacritics 2'
);
```

- **Indexed columns:** `title`, `content`, `tool_name`, `type`, `project`.
- **External content:** `content='observations'`, `content_rowid='int_id'` — the FTS rows are keyed on `observations.int_id`, not the table's implicit rowid (there is none; the table is `WITHOUT ROWID`). FTS5 also materializes its own internal shadow tables (`observations_fts_data`, `_idx`, `_docsize`, `_config`) automatically; these are FTS5 implementation detail and are not declared in SEELE SQL.
- **Tokenizer:** Porter stemmer over `unicode61`, `remove_diacritics 2`.

**FTS sync triggers** (`V001:101-117`; keep the external-content index consistent using the standard FTS5 external-content idiom of inserting the special `'delete'` command row):

| Trigger | Fires | One-line body summary |
|---|---|---|
| `observations_ai` | `AFTER INSERT` | Inserts a new FTS row `(rowid=new.int_id, title, content, tool_name, type, project)` (`V001:102-105`). |
| `observations_ad` | `AFTER DELETE` | Emits the `'delete'` command row with `old.*` to retract the FTS entry for `old.int_id` (`V001:107-110`). |
| `observations_au` | `AFTER UPDATE` | First emits the `'delete'` row for `old.*`, then inserts the fresh row for `new.*` (delete-then-reinsert). |

```sql
-- V001:112-117 (observations_au)
CREATE TRIGGER observations_au AFTER UPDATE ON observations BEGIN
    INSERT INTO observations_fts(observations_fts, rowid, title, content, tool_name, type, project)
    VALUES('delete', old.int_id, old.title, old.content, old.tool_name, old.type, old.project);
    INSERT INTO observations_fts(rowid, title, content, tool_name, type, project)
    VALUES (new.int_id, new.title, new.content, new.tool_name, new.type, new.project);
END;
```

Gotcha: the triggers fire on **physical** row changes. A soft delete (`UPDATE ... SET deleted_at`) fires `observations_au`, which re-indexes the (still-present) row — so soft-deleted observations remain in the FTS index; callers must filter on `deleted_at IS NULL` after the FTS join. Only `hard_delete` (physical `DELETE`) removes the FTS entry via `observations_ad`.

### 20.5 `observations_vec` (vec0 virtual table)

KNN vector index from the vendored sqlite-vec (`vec0`) extension.

```sql
-- V001:119-122
CREATE VIRTUAL TABLE observations_vec USING vec0(
    embedding FLOAT[384]
);
```

- **Single column:** `embedding FLOAT[384]` — a 384-dim float32 vector (matching all-MiniLM-L6-v2 output).
- **Implicit `rowid`:** the vec0 INTEGER rowid is set to `observations.int_id` at write time. `set_embedding` resolves `int_id` for an **active** (non-soft-deleted) observation, packs the `&[f32]` little-endian via `f.to_le_bytes()`, and does `INSERT OR REPLACE INTO observations_vec(rowid, embedding)` (`observations.rs:318-338`). It returns `NotFound` if no active observation has that id. Caller is responsible for normalizing the vector and for the dimension matching the column.
- **No triggers** keep vec rows in sync — embedding writes/deletes are explicit (`set_embedding` at `observations.rs:318`, `delete_embedding` at `observations.rs:341-349`, and the vec cleanup inside `hard_delete` at `observations.rs:307-311`). Both the `hard_delete` cleanup and `delete_embedding` delete via `WHERE rowid IN (SELECT int_id FROM observations WHERE id = ?1)`. vec0 also creates internal shadow tables (`observations_vec_chunks`, `_rowids`, `_vector_chunks00`, etc.) automatically.

### 20.6 `user_prompts` + `prompts_fts`

Short records of user input. `WITHOUT ROWID`, PK on ULID `id`.

| Column | SQL type | Null | Default | Meaning |
|---|---|---|---|---|
| `id` | TEXT | no | — | PK. Prompt ULID. |
| `session_id` | TEXT | yes | — | FK → `sessions(id)` `ON DELETE SET NULL`. |
| `content` | TEXT | no | — | Prompt text. FTS-indexed (see asymmetry below). |
| `project` | TEXT | yes | — | Project scope. FTS-indexed. |
| `created_at` | INTEGER | no | — | Epoch-ms creation. |

- **Indexes:** `idx_prompts_session(session_id)`, `idx_prompts_project(project)`, `idx_prompts_created_at(created_at)` — all non-partial (`V001:133-135`).
- **No soft delete:** `PromptStore::delete` is a hard `DELETE` returning `NotFound` if zero rows removed (`prompts.rs:112-119`); there is no `deleted_at` on this table.
- **Privacy note:** the docstring at `prompts.rs:1` claims user prompts are "FTS-indexed", and `CLAUDE.md` states privacy stripping is applied in `PromptStore::save()`. **Neither is true in the current code.** `PromptStore::save` (`prompts.rs:44-66`) inserts `input.content` verbatim — it does **not** call `strip_private_tags`, and (per the asymmetry below) the insert is not mirrored into `prompts_fts`. Treat both as known gaps for the improvement pass.

**`prompts_fts`** FTS5 shadow:

```sql
-- V001:137-142
CREATE VIRTUAL TABLE prompts_fts USING fts5(
    content, project,
    content='user_prompts',
    tokenize='porter unicode61 remove_diacritics 2'
);
```

Indexed columns `content`, `project`; external content `content='user_prompts'`. **Notable asymmetry:** unlike `observations_fts`, `prompts_fts` declares **no `content_rowid`** and V001 defines **no `_ai`/`_ad`/`_au` triggers** for it. So `prompts_fts` is created but is **not auto-populated** by writes to `user_prompts` — it is effectively an unmaintained/empty index in the current schema (a known gap worth flagging for the improvement pass).

### 20.7 `links` (general-purpose graph)

Generic directed edge between observations. `WITHOUT ROWID`, PK on ULID `id`. Used by MNEMA's derivative graph and by the ENGRAM importer's `linked_to[]` mapping.

| Column | SQL type | Null | Default | Meaning |
|---|---|---|---|---|
| `id` | TEXT | no | — | PK. Link ULID. |
| `from_id` | TEXT | no | — | FK → `observations(id)` `ON DELETE CASCADE`. Edge source. |
| `to_id` | TEXT | no | — | FK → `observations(id)` `ON DELETE CASCADE`. Edge target. |
| `link_type` | TEXT | no | — | Edge label (free-form string). |
| `metadata` | TEXT | no | `'{}'` | JSON blob. |
| `created_at` | INTEGER | no | — | Epoch-ms creation. |

- **Primary key:** `id`. **Unique:** `UNIQUE(from_id, to_id, link_type)` — at most one edge of a given type between an ordered pair.
- **Foreign keys:** both `from_id` and `to_id` → `observations(id) ON DELETE CASCADE` (deleting an observation hard-deletes its edges; soft-delete leaves edges intact since the row still exists).
- **Indexes:** `idx_links_from(from_id)`, `idx_links_to(to_id)`, `idx_links_type(link_type)` (`V001:155-157`).
- `LinkStore` provides `create` (`links.rs:39`), `create_in_tx` for atomic import (`links.rs:69`), `list` (`links.rs:96`), `delete` (`links.rs:129`, returns `NotFound` if zero rows removed). No update path — edges are immutable once created.

### 20.8 `memory_relations` (judgment lifecycle)

ENGRAM-inherited table tracking proposed relations between observations and their judgment lifecycle. `WITHOUT ROWID`, PK on ULID `id`.

| Column | SQL type | Null | Default | Meaning |
|---|---|---|---|---|
| `id` | TEXT | no | — | PK. Relation ULID. |
| `sync_id` | TEXT | no | — | `UNIQUE`. Stable cross-machine sync identity. |
| `source_id` | TEXT | no | — | FK → `observations(id)` `ON DELETE CASCADE`. |
| `target_id` | TEXT | no | — | FK → `observations(id)` `ON DELETE CASCADE`. |
| `relation` | TEXT | no | — | Relation kind (`RelationKind`). |
| `judgment_status` | TEXT | no | `'pending'` | `CHECK (judgment_status IN ('pending','judged','orphaned','ignored'))`. |
| `reason` | TEXT | yes | — | Free-text rationale. |
| `evidence` | TEXT | yes | — | Supporting evidence. |
| `confidence` | REAL | yes | — | Confidence score (float). |
| `marked_by_actor` | TEXT | yes | — | Who/what marked it. |
| `marked_by_kind` | TEXT | yes | — | Actor kind. |
| `marked_by_model` | TEXT | yes | — | Model name if marked by an LLM. |
| `session_id` | TEXT | yes | — | FK → `sessions(id)` `ON DELETE SET NULL`. |
| `created_at` | INTEGER | no | — | Epoch-ms creation. |

- **Primary key:** `id`. **Unique:** `sync_id`.
- **Foreign keys:** `source_id`/`target_id → observations(id) ON DELETE CASCADE`; `session_id → sessions(id) ON DELETE SET NULL`.
- **Indexes:** `idx_relations_source(source_id)`, `idx_relations_target(target_id)`, `idx_relations_status(judgment_status)`, `idx_relations_relation(relation)` (`V001:178-181`).
- **Lifecycle (relations.rs):** `create` always inserts `judgment_status='pending'` (the literal is hard-coded in the INSERT, `relations.rs:53-95`); `judge` updates the status and `COALESCE`-merges `reason`/`evidence`/`confidence` — i.e. it only overwrites a column when the supplied value is non-NULL, otherwise keeps the existing value (`relations.rs:97-118`), returning `NotFound` if the id is absent.

### 20.9 `sync_chunks` (git-sync dedup ledger)

Idempotency ledger for the git-friendly sync importer. `WITHOUT ROWID`, **composite PK**.

| Column | SQL type | Null | Default | Meaning |
|---|---|---|---|---|
| `target_key` | TEXT | no | — | PK part 1. Sync target / source identity. |
| `chunk_id` | TEXT | no | — | PK part 2. SHA-256-derived chunk identity. |
| `imported_at` | INTEGER | no | — | Epoch-ms when the chunk was imported. |

- **Primary key:** `PRIMARY KEY (target_key, chunk_id)` (`V001:188`). No secondary indexes, no FKs.
- **Idempotency:** `mark_imported` / `mark_imported_in_tx` use `INSERT OR IGNORE`, returning `true` only when a new row was recorded (`inserted == 1`), `false` if the chunk was already applied (`chunks.rs:30-56`). `was_imported` probes existence via `SELECT count(*)` (`chunks.rs:58-66`). The `_in_tx` variant lets the importer commit the ledger write atomically with the observation saves of the same chunk. `ChunkStore` also exposes `list_for_target` (`chunks.rs:68-90`).

### 20.10 Cross-cutting invariants & gotchas

- **All ordinary tables are `WITHOUT ROWID`** and keyed on ULID text, so they have no implicit integer rowid; `observations.int_id` exists precisely because FTS5/vec0 need an INTEGER rowid to bridge to.
- **Timestamps are epoch-ms INTEGER**, decoded with `Utc.timestamp_millis_opt(ms).single().unwrap_or_else(Utc::now)` — a malformed/out-of-range timestamp silently becomes "now" rather than erroring (e.g. `observations.rs:691-695`, `sessions.rs:199-203`, and the same `ms_to_dt` idiom inline in `prompts.rs:144-147`, `relations.rs:229-232`, `links.rs:160-163`, `chunks.rs:83-87`).
- **Privacy stripping** (`<private>...</private>` removal via the lazy regex `(?si)<private>.*?</private>`, `privacy.rs:11-13`; unclosed tags are left intact on purpose) is applied to **both `title` and `content`** in the normal `save` path (`save_in_tx`, `observations.rs:440-441`), but in the `update` path **only `content` is stripped — the title is taken verbatim** (`observations.rs:224-227`). It is **not** applied at all in `save_raw_in_tx` (the import fast-path, which trusts pre-audited input — `observations.rs:582-619`). And, as noted in 20.6, `PromptStore::save` does **not** strip prompts despite `CLAUDE.md` saying it does.
- **Soft-delete vs FTS skew:** soft-deleted observations stay in `observations_fts` (triggers fire on the physical UPDATE); always re-filter `deleted_at IS NULL` after an FTS match.
- **`prompts_fts` is unmaintained** — declared with no `content_rowid` and no sync triggers, so it never gets populated by the current write path.
- **No `schema_version` table** in the live DB; version bookkeeping is refinery's `refinery_schema_history`, and the doctor's `schema_version` field is the crate version string (`0.2.0`), not the migration number.
- **CHECK constraints** enforce the closed enums: `sessions.status` ∈ {active, ended, aborted}; `observations.scope` ∈ {project, personal}; `memory_relations.judgment_status` ∈ {pending, judged, orphaned, ignored}. `observations.type` and `memory_relations.relation` are **not** constrained at the SQL level — validation lives in Rust on read (`ObservationType::from_str_relaxed` is lenient; `Scope::from_str_strict`, `RelationKind::from_str_strict`, `JudgmentStatus::from_str_strict`, and `SessionStatus::from_str_strict` reject unknown values during row parsing).


---

## 21. Interface Catalog — MCP Tools × HTTP Endpoints × CLI Subcommands

SEELE exposes one storage/service core (`seele_http::SeeleService`) through three transports. The invariant that makes this catalog coherent: **all three transports funnel into the same `SeeleService` methods** (`crates/seele-http/src/service.rs`). The HTTP handlers (`handlers.rs`) and the MCP tool shims (`tool_impls/`) are deliberately thin — deserialize, call a `SeeleService` method, serialize. The CLI commands call `SeeleService` directly in-process (no HTTP round trip). Therefore the *capability* surface is shared, but the *shape* of each transport (param names, defaults, response envelope, error mapping) differs. This section reconciles those differences exactly.

Two cross-cutting facts hold across every table:

- **MCP reuses HTTP's DTOs.** Across its `tool_impls/` modules `seele-mcp` imports `seele_http::dto::{SaveRequest, SearchRequest, ListRequest, LinkCreateRequest, JudgeRequest, RelationCreateRequest, SessionStartRequest, SessionEndRequest}`, plus `seele_http::SeeleService` (the crate-root re-export of `service::SeeleService`) and `seele_http::service::enforce_search_query_or_filter`. The MCP crate depends on `seele-http` precisely to avoid re-declaring the request/response types — there is no single aggregated import; the DTOs are pulled in per-module (`memories.rs`, `relations.rs`, `sessions.rs`).
- **The anti-empty-query gate is shared.** `enforce_search_query_or_filter` (`service.rs:442`) is called by *both* the HTTP `/search` handler (`handlers.rs:35`) and the MCP `seele_search` shim (`tool_impls/memories.rs:28`). It rejects a blank query when no `project`/`scope`/`type` filter is present (`service.rs:442-453`), mitigating a list-all-DB exfiltration vector Cloven flagged (2026-05-10). The CLI applies the same gate (`commands/search.rs:47`).

### 21.1 Table 1 — The 19 MCP `seele_*` Tools

Registered statically in `all_tools()` (`crates/seele-mcp/src/tools.rs:79`). Names may be rewritten at the boundary by `build_index(prefix)` (`tools.rs:205`), which falls through `translate_name` (`tools.rs:225`). The `mnema_*` alias column reflects `RENAMES` (`tools.rs:223`): only one suffix is remapped — `search` → `recall` — because ENGRAM called search "recall". Every other tool is a literal `seele_<x>` → `mnema_<x>` rename. (A `Some("seele")` prefix is also special-cased to keep canonical names, `tools.rs:210`.) The `tools/list` JSON-RPC method returns descriptors sorted by name (`server.rs:99`); `tools/call` is dispatched by `dispatch_tool_call` (`server.rs:113`) and, on success, the handler's JSON is stringified into a single `{content:[{type:"text",text}],isError:false}` block (`server.rs:147-153`).

| MCP tool (`seele_*`) | `--tool-prefix mnema` alias | Required params | Optional params | Effect / underlying op |
|---|---|---|---|---|
| `seele_save` | `mnema_save` | `title`, `content` | `type`(=memory), `project`, `scope`, `topic_key`, `session_id`, `tool_name`, `metadata` | `SaveRequest` → `svc.save_observation` → `ObservationStore::save` (privacy strip, topic-key upsert, dedup) + best-effort embedding write |
| `seele_search` | `mnema_recall` | — (query OR filter) | `query`, `project`, `scope`, `type`, `limit`, `include_purist`, `include_annotations`, `score_boost_multiplier`, `max_vec_distance` | gate → `svc.search_observations` → `SearchEngine::search` (FTS+vec+RRF) |
| `seele_show` | `mnema_show` | `id` (ULID) | — | `svc.get_observation`; 404→`ToolError::NotFound` |
| `seele_list` | `mnema_list` | — | `project`, `scope`, `type`, `topic_key`, `session_id`, `limit`, `include_deleted` | `ListRequest` → `svc.list_observations` |
| `seele_update_metadata` | `mnema_update_metadata` | `id`, `metadata_patch`(object) | — | `svc.merge_observation_metadata` (flat shallow merge); returns `{ok,id}` |
| `seele_delete` | `mnema_delete` | `id` | — | `svc.soft_delete_observation` (sets `deleted_at`); returns `{ok,id}` |
| `seele_restore` | `mnema_restore` | `id` | — | `svc.restore_observation` (clears `deleted_at`); returns `{ok,id}` |
| `seele_link` | `mnema_link` | `from_id`, `to_id`, `link_type` | `metadata` | `LinkCreateRequest` → `svc.create_link` → `LinkStore::create` |
| `seele_stats` | `mnema_stats` | — (empty object) | — | `svc.stats` → `StatsResponse` |
| `seele_session_start` | `mnema_session_start` | `project` | `directory` | `svc.start_session` → `SessionStore::start` |
| `seele_session_end` | `mnema_session_end` | `id` | `summary` | `svc.end_session(id, {summary})`; returns `{ok,id}` |
| `seele_session_summary` | `mnema_session_summary` | `session_id`, `title`, `summary` | `project` | composes a `SaveRequest` (`type=memory`, `topic_key="session/<id>"`, `tool_name="seele_session_summary"`) → `svc.save_observation` (so repeated summaries upsert in place) |
| `seele_capture_passive` | `mnema_capture_passive` | `transcript` | `project`, `session_id` | parses `## Key Learnings:` bullets, saves each as a `type=learning` observation; returns `{saved:[ids],count}` |
| `seele_judge` | `mnema_judge` | `relation_id`, `status` | `reason`, `evidence`, `confidence` | `svc.judge_relation` → `RelationStore::judge`; returns `{ok,id}` |
| `seele_compare` | `mnema_compare` | `sync_id`, `source_id`, `target_id` | `reason` | `svc.create_relation` with hard-coded `relation="conflicts_with"`, `marked_by_actor="seele_compare"`, `marked_by_kind="tool"` |
| `seele_suggest_topic_key` | `mnema_suggest_topic_key` | `title` | `content` | pure local heuristic (no DB): keyword-match against 7 families (architecture, bug, decision, pattern, config, discovery, learning); returns `{family,score,suggestion:"<family>/auto"}` for the highest-scoring family, or `{family:null,suggestion:null}` (no `score` key) when nothing matches |
| `seele_projects` | `mnema_projects` | — (empty object) | — | `svc.list_projects`; returns `{projects:[...]}` |
| `seele_doctor` | `mnema_doctor` | — (empty object) | — | `svc.embedder_info` + `svc.stats`; returns `{status,embedder{model_id,dim,expected_sha256},observations_active,sessions_total,schema_version}` |
| `seele_version` | `mnema_version` | — (empty object) | — | static `{name:"seele",version:CARGO_PKG_VERSION}` (no `svc` touch) |

Gotchas worth flagging for the improvement pass:

- **`seele_doctor` mislabels the version.** It returns `"schema_version": env!("CARGO_PKG_VERSION")` (`tool_impls/meta.rs:30`) — that is the *package* version, not a DB schema version. The key name is misleading. Note also `"status": "ok"` is hard-coded (`meta.rs:22`), not derived from a real liveness check — the tool's own description string ("returns DB pool status") overstates what it actually computes.
- **`seele_compare` cannot set confidence/evidence.** It hard-codes `relation: "conflicts_with"`, `evidence: None`, `confidence: None`, `marked_by_actor: Some("seele_compare")`, `marked_by_kind: Some("tool")`, `marked_by_model: None`, and `session_id: None` (`relations.rs:66-78`), accepting only `sync_id`/`source_id`/`target_id` (required) and an optional `reason`. So the richer relation fields (evidence, confidence, session_id, and any relation kind other than `conflicts_with`) are reachable only via HTTP `POST /relations`.
- **No MCP coverage for sessions list/get, relations list, conflicts, link delete/list, or chat.** Those capabilities exist only on HTTP (see §21.4). MCP exposes session *lifecycle* (start/end/summary) but not session *query*.

### 21.2 Table 2 — HTTP Endpoints

Routes are wired in `Server::router` (`crates/seele-http/src/server.rs:99`). `/health` and `/version` are registered on the outer router, and `/openapi.json` + `/docs/*` (Swagger UI) are merged in from `openapi::routes()` — all four sit outside the `protected` sub-router, so they are always public; every other route sits in the `protected` sub-router. Auth is opt-in: when `ServerConfig.auth_bearer = Some(token)`, the `require_bearer` middleware (`auth.rs:37`) requires `Authorization: Bearer <token>` on all protected routes. The check (`check_bearer`, `auth.rs:17`) strips the mandatory `"Bearer "` prefix, then `.trim()`s the remainder before an exact byte comparison against the configured token; a missing header, missing prefix, or mismatch all map to `401`. Errors elsewhere flow through `ApiError::into_response` (`error.rs:38`): `BadRequest→400`, `Unauthorized→401`, `NotFound→404`, `Conflict→409`, `Internal→500`, all with body `{code,message}` per `ErrorBody` (`error.rs:32`).

The two legacy ENGRAM aliases (`POST /save`, `GET /show/{id}`) are mounted **only** when `legacy_engram_paths` is set (`server.rs:154`); they reuse the same handlers as `POST /memories` and `GET /memories/{id}`. No other route gets a legacy alias.

| Method | Path | Legacy alias | Auth* | Request DTO | Response DTO / status | Maps to |
|---|---|---|---|---|---|---|
| GET | `/health` | — | public | — | `{status:"ok"}` | inline `health()` |
| GET | `/version` | — | public | — | `{name,version}` | inline `version()` |
| GET | `/openapi.json` | — | public | — | OpenAPI 3.1 doc | `openapi::build_openapi` |
| GET | `/docs/*` | — | public | — | Swagger UI HTML | `utoipa_swagger_ui::SwaggerUi` |
| POST | `/memories` | `POST /save` | ✓ | `SaveRequest` (JSON) | `SaveResponse` 200 | `save_memory` → `save_observation` |
| GET | `/memories` | — | ✓ | `ListRequest` (query) | `Vec<ObservationDto>` 200 | `list_memories` → `list_observations` |
| GET | `/memories/{id}` | `GET /show/{id}` | ✓ | path `id` | `ObservationDto` 200 / 404 | `get_memory` → `get_observation` |
| DELETE | `/memories/{id}` | — | ✓ | path `id` | 204 | `soft_delete_memory` → `soft_delete_observation` |
| POST | `/memories/{id}/restore` | — | ✓ | path `id` | 204 | `restore_memory` → `restore_observation` |
| GET | `/memories/{id}/links` | — | ✓ | path `id` | `Vec<LinkDto>` 200 | `list_links_for_memory` (links on either side, deduped) |
| POST | `/search` | — | ✓ | `SearchRequest` (JSON) | `SearchResponse` 200 | `search_memories` (gate first) → `search_observations` |
| POST | `/sessions` | — | ✓ | `SessionStartRequest` | `SessionDto` 200 | `start_session` |
| GET | `/sessions` | — | ✓ | `SessionListQuery` (query) | `Vec<SessionDto>` 200 | `list_sessions` |
| GET | `/sessions/{id}` | — | ✓ | path `id` | `SessionDto` 200 / 404 | `get_session` |
| PUT | `/sessions/{id}/end` | — | ✓ | `SessionEndRequest` | 204 | `end_session` |
| PUT | `/sessions/{id}/abort` | — | ✓ | path `id` | 204 | `abort_session` |
| POST | `/links` | — | ✓ | `LinkCreateRequest` | `LinkDto` 200 | `create_link` |
| DELETE | `/links/{id}` | — | ✓ | path `id` | 204 | `delete_link` |
| POST | `/relations` | — | ✓ | `RelationCreateRequest` | `RelationDto` 200 | `create_relation` |
| GET | `/relations` | — | ✓ | `RelationListQuery` (query) | `Vec<RelationDto>` 200 | `list_relations` |
| PUT | `/relations/{id}/judge` | — | ✓ | `JudgeRequest` | 204 | `judge_relation` |
| GET | `/conflicts` | — | ✓ | `ConflictsQuery` (`limit`) | `Vec<RelationDto>` 200 | `list_pending_conflicts` (conflicts_with + pending) |
| GET | `/stats` | — | ✓ | — | `StatsResponse` 200 | `get_stats` |
| GET | `/embedder` | — | ✓ | — | `EmbedderInfo` 200 | `get_embedder_info` |
| POST | `/chat` | — | ✓ | `ChatRequest` | `ChatResponse` 200 | `chat` (AI provider w/ `seele_search` tool-use) |
| GET | `/chat/info` | — | ✓ | — | `ChatInfoResponse` 200 | `chat_info` |

\* "Auth ✓" means the route is gated **only when** `--auth-bearer` is configured; with no token, every route is open.

Notes on the HTTP surface:

- **OpenAPI/route drift.** `openapi.rs` registers schemas via the `utoipa::OpenApi` derive but hand-authors the paths section in `build_paths()` (`openapi.rs:72`). It does **not** describe `/chat`, `/chat/info`, the `/save` or `/show/{id}` legacy aliases. It also documents only the `200`/`204` success responses — no `404` or error responses are attached to any path, even though the `ErrorBody` schema *is* registered in `components.schemas` (`openapi.rs:49`) and goes otherwise unreferenced. The router (`server.rs`) is the source of truth for what actually serves; the spec is a curated subset. This is a known doc-vs-reality gap worth surfacing to the improvement agent.
- **`SearchRequest.score_boost_multiplier` default trap.** It is `#[serde(default)]` on `f64`, so a JSON request omitting it sends `0.0`. The CLI deliberately passes `0.0` too (`commands/search.rs:44`), while the chat tool-handler passes `1.0` (`handlers.rs:339`). Whether `0.0` zeroes the boost or is treated as "unset" by the search engine is a §7 concern, but the inconsistency originates here.
- **`/chat` keeps secrets local.** `ChatProviderConfig` (`server.rs:62`) and per-request `api_key` never persist; the handler (`chat`, `handlers.rs:254`) resolves provider/key/model/endpoint in priority order (per-request fields > CLI config) and runs `seele_chat::run_chat` with a single `seele_search` tool whose handler closure (`handlers.rs:318`) calls back into `svc.search_observations`. Provider selection is `provider_name.eq_ignore_ascii_case("anthropic")` → `AnthropicProvider`, else `OpenAICompatibleProvider` (`handlers.rs:304`). `default_model_for`/`default_endpoint_for` (`handlers.rs:378`/`391`) map provider labels (`minimax`, `openai`, `openrouter`, `together`, `groq`, `deepseek`, `anthropic`) to defaults; the catch-all (`_`) falls back to the OpenAI model/endpoint, so anything non-`anthropic` is treated as OpenAI-compatible. The tool-call `limit` is clamped to `min(20)` with a default of 5 (`handlers.rs:331`).

### 21.3 Table 3 — The CLI Subcommands

The clap command tree is `Command` in `crates/seele-cli/src/app.rs:44`. Sixteen top-level variants; `Sync` and `Import` each nest a subcommand, and the documented "17 subcommands" count reflects the leaf operations (`sync export`, `sync import`, `import from-engram`). Three **global** flags apply to all (`app.rs:25`): `--db <path>` (default `~/.seele/seele.db` via `default_db_path`, `main.rs:42`), `--fake-embedder` (or env `SEELE_FAKE_EMBEDDER` non-empty → `FakeEmbedder`, else ONNX with transparent fallback on init failure, `app.rs:144`), and `--json` (machine output). DB-touching commands build the service via `build_service` (`app.rs:128`).

| Subcommand | Key flags / args | Behavior |
|---|---|---|
| `save <title> <content>` | `--type`(=memory), `--project`, `--scope`, `--topic-key`, `--metadata <json>` | builds `SaveRequest` with `tool_name="seele-cli"`, `session_id=None`; `save_observation`; prints `saved <id> (<outcome>)` |
| `search [query]` | `--project`, `--scope`, `--type`, `--limit`(=20), `--include-purist`, `--include-annotations` | gate (empty query needs a filter) → `search_observations`; sends `score_boost_multiplier=0.0` |
| `show <id>` | — | `get_observation`; errors if not found; text view shows id/type/scope/project/title/indented content |
| `list` | `--project`, `--scope`, `--type`, `--topic-key`, `--session-id`, `--limit`(=50), `--include-deleted` | `ListRequest` → `list_observations` |
| `delete <id>` | — | `soft_delete_observation`; prints `soft-deleted <id>` |
| `restore <id>` | — | `restore_observation`; prints `restored <id>` |
| `link <from_id> <to_id> <link_type>` | `--metadata <json>` | `create_link`; prints `linked <from> -[<type>]-> <to> (id=<id>)` |
| `stats` | — | `stats`; renders observation/session counters incl. by_type/by_scope/by_status |
| `doctor` | — | `stats` + `embedder_info`; `DoctorReport` with a `fake_embedder_warning` when `model_id` contains "fake" |
| `projects` | — | `list_projects` |
| `sync export <dir>` | `--project` | `seele_sync::export_to_dir(&svc.observations, dir, filter)` → gzip JSON chunk |
| `sync import <path>` | `--target-key <key>` (required) | `seele_sync::import_from_file(&svc.observations,&svc.chunks,target_key,path)`; idempotent per (target_key, chunk_id) |
| `import from-engram <path>` | `--re-embed` (no-op in v0.1), `--dry-run` | builds its own `ObservationStore`+`LinkStore` (does not use `build_service`), runs `EngramImporter::import_from` (preserves ULIDs, maps `linked_to[]`) |
| `setup` | `--agent <name>` \| `--all` \| `--list`, `--dry-run`, `--no-backup`, `--seele-binary <path>` | installs MCP entry into agent configs via `seele_setup::install`; `--all` iterates only implemented agents (`implemented_agent_names`) |
| `mcp` | `--tool-prefix <p>` | `McpServer::new(...).run_stdio()` — JSON-RPC 2.0 stdio loop; `mnema` activates ENGRAM aliases |
| `serve` | `--port`(=7777), `--bind`(=127.0.0.1), `--legacy-engram-paths`, `--auth-bearer`, `--cors-allow <origin>`(repeatable), `--chat-provider`, `--chat-key`, `--chat-model`, `--chat-endpoint` | builds `ServerConfig` + optional `ChatProviderConfig`, runs the axum server; `--chat-key $ENV` resolves from env (`resolve_key`, `serve.rs:88`); `--chat-provider` and `--chat-key` must be set together or both omitted (else hard error) |
| `tui` | `--smoke` (hidden) | interactive path calls `seele_tui::run_tui(svc).await`; `--smoke` instead calls `seele_tui::run_tui_smoke(svc)`, which renders one off-screen frame, prints `tui smoke ok`, and exits 0 |

CLI-specific notes:

- **No CLI command for sessions, relations, judge, conflicts, update-metadata, or chat.** Those are HTTP/MCP-only. The CLI is the *operator* surface (save/search/introspect/sync/migrate/serve), not the full memory-graph API.
- **`save --metadata` and `link --metadata`** parse a raw JSON string; invalid JSON is a hard CLI error (`save.rs:45`, `link.rs:30`).
- **`import --re-embed` is recognized but a documented no-op** in v0.1 (`commands/import.rs:64`, logs a `tracing::warn!`).
- **`mcp`, `serve`, `tui` are long-running processes**, not request/response ops; they host the *other two* transports. `tui` and `mcp` talk to the in-process `SeeleService` with no HTTP hop.

### 21.4 Capability Cross-Map (MCP × HTTP × CLI)

Rows are logical capabilities; a marker means a first-class operation exists on that transport. `✓` = present; `—` = absent; parenthetical notes show divergences.

| Capability | MCP tool | HTTP endpoint | CLI subcommand |
|---|---|---|---|
| Save observation | ✓ `seele_save` | ✓ `POST /memories` (+`/save` legacy) | ✓ `save` |
| Hybrid search | ✓ `seele_search`/`mnema_recall` | ✓ `POST /search` | ✓ `search` |
| Show one by id | ✓ `seele_show` | ✓ `GET /memories/{id}` (+`/show/{id}` legacy) | ✓ `show` |
| List observations | ✓ `seele_list` | ✓ `GET /memories` | ✓ `list` |
| Soft-delete | ✓ `seele_delete` | ✓ `DELETE /memories/{id}` | ✓ `delete` |
| Restore | ✓ `seele_restore` | ✓ `POST /memories/{id}/restore` | ✓ `restore` |
| Update metadata (merge) | ✓ `seele_update_metadata` | — (only via re-save/upsert) | — |
| Create link | ✓ `seele_link` | ✓ `POST /links` | ✓ `link` |
| List links for memory | — | ✓ `GET /memories/{id}/links` | — |
| Delete link | — | ✓ `DELETE /links/{id}` | — |
| Create relation | ✓ `seele_compare` (conflicts_with only) | ✓ `POST /relations` (all 6 kinds) | — |
| List relations | — | ✓ `GET /relations` | — |
| Judge relation | ✓ `seele_judge` | ✓ `PUT /relations/{id}/judge` | — |
| List pending conflicts | — | ✓ `GET /conflicts` | — |
| Session start | ✓ `seele_session_start` | ✓ `POST /sessions` | — |
| Session end | ✓ `seele_session_end` | ✓ `PUT /sessions/{id}/end` | — |
| Session abort | — | ✓ `PUT /sessions/{id}/abort` | — |
| Session get / list | — | ✓ `GET /sessions/{id}` · `GET /sessions` | — |
| Session summary | ✓ `seele_session_summary` | — (compose `POST /memories` manually) | — |
| Passive capture (Key Learnings) | ✓ `seele_capture_passive` | — | — |
| Stats | ✓ `seele_stats` | ✓ `GET /stats` | ✓ `stats` |
| Projects list | ✓ `seele_projects` | — (no dedicated endpoint) | ✓ `projects` |
| Embedder info | (folded into `seele_doctor`) | ✓ `GET /embedder` | (folded into `doctor`) |
| Doctor / health | ✓ `seele_doctor` | ✓ `GET /health` (+`/version`) | ✓ `doctor` |
| Version | ✓ `seele_version` | ✓ `GET /version` | (clap `--version`) |
| Suggest topic key | ✓ `seele_suggest_topic_key` | — | — |
| Chat-with-DB | — | ✓ `POST /chat` · `GET /chat/info` | — (start via `serve --chat-*`) |
| Sync export/import | — | — | ✓ `sync export` / `sync import` |
| ENGRAM import | — | — | ✓ `import from-engram` |
| Agent MCP setup | — | — | ✓ `setup` |
| Run MCP server | (is the transport) | — | ✓ `mcp` |
| Run HTTP server | — | (is the transport) | ✓ `serve` |
| Run TUI | — | — | ✓ `tui` |

The asymmetries are the actionable findings: **operational capabilities (sync, ENGRAM import, setup, server bootstrap, TUI) are CLI-exclusive**; **memory-graph query capabilities (relation listing, conflicts, session query, link listing/deletion) are HTTP-exclusive**; **`update_metadata`, `suggest_topic_key`, `capture_passive`, and `session_summary` are MCP-exclusive niceties** (the last two are conveniences MCP composes from primitives the other transports also expose). The single deepest seam is `POST /relations` vs `seele_compare`: HTTP can create any of the six `RelationKind` values with full evidence/confidence, while MCP can only create a bare `conflicts_with`. No transport is a strict superset of another.


---

## 22. Design Rationale & ADR Digest

This section synthesizes the design intent behind SEELE as recorded in its planning corpus: the 13 architecture ADRs in `genesis/plans/arquitectura/` (`00-INDEX.md` through `13-engram-compatibility.md`), the 5 strategy documents in `genesis/plans/estrategia/`, the attribution contract in `CREDITS.md`, and the repo operating rules in `CLAUDE.md`. Where the architecture ADRs diverge from what the running code actually does (and several do, because the ADRs were written 2026-05-09 before the code stabilized at v0.1.0 on 2026-05-11), the divergence is flagged explicitly — these gaps are exactly the kind of "ADR-vs-reality drift" a downstream improvement agent must know about.

A note on document status: every ADR carries an `**Estado**` line. All 13 are stamped **Aceptado** (Accepted). Eight were dated 2026-05-09 (ADR-01 through ADR-09), four 2026-05-10 (ADR-10, ADR-11, ADR-12, ADR-13), one (ADR-12) is itself a follow-up tracker. None is marked Superseded, Deprecated, or Proposed, even where later code overrode the decision (e.g. ADR-08's "8 crates" became 13 in the manifest — 12 of them documented in `CLAUDE.md`; ADR-02's `memories` table became the `observations` table and its proposed virtual `int_id` column became a Rust-populated STORED column; ADR-01's "Rust 1.83+" became 1.85). The ADRs are therefore a snapshot of *intent at genesis*, not a maintained record of the final state. This is itself a finding worth surfacing.

### 22.1 The ADR digest

The table below indexes all 13 ADRs with file, date, and one-line decision. The subsections that follow give decision / rationale / rejected alternatives / status for each.

| ADR | File | Date | Decision (one line) |
|---|---|---|---|
| 01 | `01-rust-y-crates.md` | 2026-05-09 | Implement in Rust on tokio with a locked-in crate set |
| 02 | `02-schema-sqlite.md` | 2026-05-09 | Canonical SQLite schema: main table + FTS5 + vec0 + virtual generated columns + partial indexes |
| 03 | `03-search-hybrid.md` | 2026-05-09 | 3-layer hybrid search (metadata pre-filter → FTS + vector → RRF fusion) |
| 04 | `04-embedder-onnx.md` | 2026-05-09 | Local CPU-only ONNX embedder via `ort` + `all-MiniLM-L6-v2`, auto-download |
| 05 | `05-mcp-server.md` | 2026-05-09 | MCP server from spec: stdio v0.1, HTTP v0.2; `seele_*` tool naming |
| 06 | `06-http-api.md` | 2026-05-09 | axum REST API, OpenAPI 3.1 via utoipa, optional Bearer auth |
| 07 | `07-tui-design.md` | 2026-05-09 | ratatui + crossterm TUI, 5 views, vi-style keybindings |
| 08 | `08-repo-layout.md` | 2026-05-09 | Cargo workspace, hierarchical crate deps, tri-OS CI from day one |
| 09 | `09-distribution-license.md` | 2026-05-09 | GitHub Releases (5 targets) + crates.io, MIT, explicit ENGRAM credit |
| 10 | `10-mapping-mnema-seele.md` | 2026-05-10 | Conceptual contract mapping MNEMA's Counsel vocabulary onto SEELE's schema |
| 11 | `11-sqlite-vec-vendored.md` | 2026-05-10 | Vendor precompiled `sqlite-vec` binaries for 5 targets, embed via `include_bytes!` |
| 12 | `12-embedder-hardening-followups.md` | 2026-05-10 | Track 2 conscious embedder gaps: empty `TRUSTED_HASHES` + silent quantized fallback |
| 13 | `13-engram-compatibility.md` | 2026-05-10 | ENGRAM drop-in compat: `import --from-engram` + `--tool-prefix` + `--legacy-engram-paths` |

#### ADR-01 — Rust + crate ecosystem

**Decision.** Build SEELE in Rust (stated "1.83+", edition 2021, on the tokio async runtime) with a locked-in crate set: `rusqlite` (bundled + load_extension), `r2d2`/`r2d2_sqlite`, `ort`, `tokenizers`, `hf-hub`, `axum`, `tower`/`tower-http`, `utoipa`, `clap`, `ratatui`/`crossterm`, `serde`/`serde_json`, `thiserror`, `anyhow` (CLI-only), `tracing`, `ulid`. MCP is hand-rolled from the spec because "no hay crate canónico todavía en 2026."

**Rationale.** The engine needs a cross-compilable single binary, predictable sub-200ms search at 10K memories, native SQLite + extension bindings, CPU-only ONNX, async I/O for two servers, and a rich TUI. Rust satisfies all of these and is "la elección del User para componentes de sistema." Per-crate justifications are explicit: `axum` over `actix-web`/`rocket` (tokio-team, types-driven extractors, community default); `rusqlite` over `sqlx` (sqlx SQLite support is secondary and cannot load extensions — the dealbreaker for sqlite-vec); `ratatui` over `cursive`/raw crossterm (immediate-mode, successor to deprecated tui-rs); `ort` over `candle` (maturity, proven CPU performance, universal ONNX operator support).

**Alternatives rejected.** **Go** — "Si elegimos Go, ¿por qué no usar ENGRAM directo?" (choosing Go forfeits the clean-room technical justification and adds a language to the ecosystem). **Zig** — ecosystem immature, pre-1.0 in 2026. **TypeScript/Bun** — does not satisfy single-binary distribution, slow ONNX, no ratatui equivalent, awkward SQLite extensions; TS is reserved for the MNEMA orchestrator layer.

**Status / drift.** Accepted and implemented. **Drift:** the MSRV was bumped from the ADR's 1.83 to **1.85** to use `clap_lex` with `edition2024` (documented in `CLAUDE.md` and `CHANGELOG.md`). `ort` is pinned exactly at `=2.0.0-rc.10` (no stable 2.0 exists as of 2026-05; Dependabot owns the un-pin). The ADR's "refinery o sqlx-cli migrations — no decidido aún" resolved to **refinery**.

#### ADR-02 — SQLite schema

**Decision.** A canonical schema with a primary memory table, an FTS5 virtual table over the body, a vec0 virtual table for embeddings (`FLOAT[384]`), **virtual generated columns** extracting five JSON metadata fields (`meta_kind`, `meta_domain`, `meta_axiomatic`, `meta_score`, `meta_context_mode`), partial B-tree indexes over those columns, a `links` table, and a `schema_version` table. IDs are ULID. FTS tokenizer: `porter unicode61 remove_diacritics 2`. FTS kept in sync via `_ai`/`_ad`/`_au` triggers.

**Rationale.** The schema is "el contrato más estable del engine." Metadata is stored as a JSON TEXT column rather than explicit columns to stay consumer-agnostic (MNEMA defines its own schema; other consumers define theirs). The five virtual columns earn dedicated index space because they are transversally useful — `kind`, `domain`, `axiomatic`, `score` are universal in MNEMA-style patterns, and `context_mode` (purist/contextual) is "crítico para Recall correcto" (see ADR-10). Partial indexes (`WHERE deleted_at IS NULL`) minimize index size by indexing only live rows. ULID over UUIDv7/autoincrement: timestamp-sortable (`ORDER BY id ≈ ORDER BY created_at`), 26-char Crockford-base32, random low bits (no enumeration). The vec0 bridge maps ULID→int64.

**Alternatives rejected.** Explicit metadata columns or BLOB (rejected for flexibility); UUIDv7 (equivalent but 36 chars with hyphens, more verbose); autoincrement (no sortability, enumerable); `sonic-rs` JSON (deferred unless benchmarks show a bottleneck); int8 embedding quantization (deferred to v0.2+ behind a flag).

**Status / drift.** Accepted, but the ADR is the *least faithful to final code* of the set, because the ENGRAM feature audit (strategy 05) landed alongside/after it and replaced the single `memories` table with ENGRAM's 9-table model (`sessions`, `observations`, `observations_fts`, `user_prompts`, `prompts_fts`, `memory_relations`, `sync_chunks`, `sync_apply_deferred`, `schema_version`) — see `CREDITS.md`. So the running primary table is `observations`, not `memories`.

**Most important drift to flag — the int64 bridge (verified against shipped code).** ADR-02 proposed the bridge as a SQL *virtual generated* column `int_id GENERATED ALWAYS AS (CAST(SUBSTR(id, 1, 16) AS INTEGER)) VIRTUAL`, claiming the "first 48 bits" of the ULID give a deterministic, unique mapping ("colisiones astronómicamente improbables"). The shipped code diverges on **three** points, and the ADR/`CLAUDE.md` wording must NOT be read as current fact:

- **The ADR formula is broken and was discarded.** ULIDs are Crockford-base32 *text*, so `CAST(SUBSTR(id, 1, 16) AS INTEGER)` yields `0` for any leading letter. The shipped migration says so explicitly and replaces it: "We instead store `int_id` as a regular INTEGER column populated at INSERT time from Rust (`SeeleId::as_i64`)." (`crates/seele-storage/src/migrations/V001__initial_schema.sql:7-13,35`).
- **`int_id` is a regular STORED column, not virtual and not "approximate."** It is declared `int_id INTEGER NOT NULL UNIQUE`, written from Rust on every insert, and is the authoritative rowid that **both** `observations_fts` (`content_rowid='int_id'`) and `observations_vec` reference. The five `meta_*` columns are the *only* virtual generated columns in the schema. (`CLAUDE.md` still describes `int_id` as "aproximada y solo sirve para queries de JOIN" — that wording is stale relative to the shipped schema and should not be treated as authoritative.)
- **`as_i64()` uses the last 7 bytes, not the first 6.** `SeeleId::as_i64()` takes ULID **bytes 9..16** (the random tail / low 56 bits), zeroing the top byte to stay non-negative — NOT the "first 6 bytes" some prose asserts (`crates/seele-core/src/id.rs:35-41`). Because it draws from 56 *random* bits rather than the timestamp prefix, collisions are a real (if rare) birthday-bound risk: the storage layer retries the insert on the `observations.int_id` UNIQUE violation (`crates/seele-storage/src/observations.rs:23,537-578`). So the ADR's "astronomically improbable" framing is wrong on the mechanism *and* on which bytes are used.

#### ADR-03 — Hybrid search FTS + vector + metadata with RRF

**Decision.** A five-layer pipeline: (1) metadata pre-filter via B-tree partial indexes returns candidate IDs; (2) FTS5 `bm25()` ranking over candidates; (3) `vec_distance_cosine` vector ranking over the same candidates; (4) **Reciprocal Rank Fusion** combines the two rankings; (5) optional boost by a `score` metadata field. RRF constant `k = 60` (paper default), formula `score(doc) = Σ 1/(k + rank_i(doc))`.

**Rationale.** RRF over weighted sum because weighted sum requires normalizing scores from incompatible distributions (BM25 is unbounded; cosine is [0,2]); RRF is scale-invariant (only rank matters), robust to outliers, and empirically outperforms weighted sum on BEIR/MS-MARCO (Cormack et al. 2009 is cited). Pre-filter before scoring keeps the candidate set small (sub-10ms for ~1K–10K candidates at 100K rows). Edge cases are handled by RRF's structure: FTS-empty queries degrade to vector-only (absent ranking contributes 0); empty-query-with-filters skips FTS+vector and lists by `created_at DESC`; invalid filters return an empty array, not an error.

**Alternatives rejected.** Weighted sum / α·fts + β·vec (normalization problem). Deferred to later versions: LLM/cross-encoder reranking (v0.2), learned personalization (out of charter), query expansion/synonyms (v0.3).

**Status / drift.** Accepted and central — RRF is SEELE's headline differentiator over ENGRAM. The ADR's `SearchResult` struct sketch and `rrf_combine` Rust snippet are illustrative; the real implementation lives in `seele-search` (`rrf.rs`), property-tested in `seele-search/tests/rrf_properties.rs` (score ≥ 0, output IDs = union of inputs, descending order, a doc present in N sources outscores one in N−1).

#### ADR-04 — Local ONNX embedder

**Decision.** A local CPU-only embedder using `ort` (ONNX Runtime Rust bindings) + `all-MiniLM-L6-v2` (384-dim) + the HF `tokenizers` crate, with auto-download from Hugging Face on first init into `~/.seele/embedder/`. Mean-pooling + L2 normalization after the forward pass. INT8 quantized model by default; `--no-quantize` to force full precision. Singleton runtime (init is ~50–200ms). SHA256 verification of downloaded artifacts to detect tampering.

**Rationale.** Local-first (no external API calls at runtime), CPU-only (most user machines lack NVIDIA GPUs), small model (~80 MB, ~25 MB quantized), open weights. `ort` wins over `candle` (younger, mixed CPU perf for small models), `tract` (restricted operator support), `burn` (training framework, too much scope). `all-MiniLM-L6-v2` wins over `bge-base` (better quality but 768-dim, more DB space), `nomic-embed-text` (8K context overkill), `e5-small-v2` (less 2026 benchmark coverage) — it is the "good enough default." Over-512-token bodies are truncated (mean-of-chunks deferred to v0.2; reject rejected as bad UX).

**Alternatives rejected.** `candle`/`tract`/`burn` runtimes; larger models; rejecting long input; CUDA/Metal/DirectML default (deferred to v0.2 behind feature flags — v0.1 is CPU only).

**Status / drift.** Accepted. **Drift:** ADR-04 says ONNX is the embedder; the actual v0.1 default was ONNX with a **transparent fallback to `FakeEmbedder`** if ONNX init fails (no network, HF blocked), with an `eprintln!` warning — implemented in Sprint-05 Bloque B (`seele-cli/src/app.rs::build_service`, `pick_embedder`). `--fake-embedder` / `SEELE_FAKE_EMBEDDER=1` forces Fake. The ADR's `Embedder` struct and `EmbedderError` enum are sketches; the real types live in `seele-embedder`. The SHA256 verification infrastructure shipped but unpopulated (see ADR-12).

#### ADR-05 — MCP server

**Decision.** Implement an MCP server directly from Anthropic's spec (no canonical Rust crate exists in 2026), JSON-RPC 2.0 over **stdio in v0.1** (HTTP transport deferred to v0.2). Tools are consumer-agnostic with a `seele_` prefix. stdin reads newline-delimited requests; stdout carries responses; stderr carries logs (so it doesn't contaminate the protocol). Standard JSON-RPC error codes (−32600/−32601/−32602/−32603) plus SEELE-specific 1001+.

**Rationale.** MCP is the standard Claude Code, Cursor, OpenCode et al. use to call external tools — exposing MCP means any agent can use SEELE immediately with no custom integration. stdio is what clients invoke by default, trivial to test with `assert_cmd`, and avoids HTTP's auth/TLS/port complexity. The "MCP as primary transport" bet — that the MCP surface is the canonical interface and CLI/HTTP are siblings over the same core — is the architectural through-line.

**Alternatives rejected.** HTTP transport in v0.1 (deferred). Dynamic tool registration / plugins (hardcoded set in v0.1). stdio auth (process trust suffices). Streaming responses (v0.3).

**Status / drift.** Accepted; this is the transport SEELE leads with. **Drift:** ADR-05 enumerates only ~9 tools; the final count is **19** `seele_*` tools (the ENGRAM-parity set audited in strategy 05). The ADR predates ADR-13's `--tool-prefix` shim, which lets `--tool-prefix mnema` expose `mnema_*` aliases. A later patch devlog (`2026-05-20-patch-mcp-call-tool-result.md`) shows the MCP `tools/call` result shape needed post-v0.1 correction — evidence the spec-from-scratch approach carried real maintenance cost.

#### ADR-06 — HTTP REST API

**Decision.** An axum 0.8 REST API with OpenAPI 3.1 auto-generated by `utoipa`, Swagger UI at `/docs`, spec at `/openapi.json`, optional Bearer auth (off by default), optional CORS. Conventions: kebab-case paths, snake_case query params, camelCase JSON bodies, minimal response envelope, uniform error objects (`{error: {code, message, details}}`). Middleware stack: tracing → cors → auth → gzip compression → handler. r2d2 pool `max_size = 16`; embedder shared as `Arc<Embedder>`.

**Rationale.** HTTP is the second transport for consumers that run SEELE out-of-process (MNEMA TS+Bun orchestrator, a future MNEMA frontend, remote agents, monitoring scripts). No-auth-by-default optimizes ergonomics over security for the v0.1 single-user localhost case ("Ergonomía > security para v0.1 single-user"). Bearer is opt-in via `--auth-bearer` / `SEELE_AUTH_BEARER`.

**Alternatives rejected.** WebSockets/SSE streaming (v0.3); rate limiting (v0.2 when needed); **gRPC — "nunca"**; **GraphQL — "nunca" (over-engineering)**.

**Status / drift.** Accepted. **Drift:** the ADR-06 endpoint list (~22 ops over `/memories`) predates the ENGRAM-audited surface; the real API is built around `/observations`, `/sessions`, `/prompts`, `/conflicts`, `/sync`, etc., and `CLAUDE.md` describes it as "~26 endpoints," with `--legacy-engram-paths` (ADR-13) adding `/save` and `/show/{id}` aliases. **Port discrepancy (sources conflict):** ADR-06's config example *and* the shipped `CLAUDE.md` (`seele serve [--port 7777]`) both use **7777** as the SEELE default, while the ENGRAM-parity layer documents **7437** as the inherited default "para compatibilidad de configs MCP existentes" (strategy 02/05; `CREDITS.md`; `SEELE_PORT` default 7437). So 7777 is the figure that appears in the operating rules; 7437 is the ENGRAM-compatible value the audit promises. This unreconciled split is itself a drift to flag, not a settled fact in either direction.

#### ADR-07 — TUI design

**Decision.** A built-in TUI from v0.1 (a User decision on 2026-05-09 for immediate dogfooding) using ratatui 0.29 + crossterm, with five views (Home / Browse / Search / Detail / Stats), vi-style keybindings with a persistent footer hint bar, `$EDITOR` integration for editing body/metadata, and a single hardcoded dark theme. `seele tui` or bare `seele` launches it.

**Rationale.** The User wanted to inspect SEELE state without SQL or curl. The async event loop uses `tokio::select!` over a crossterm `EventStream` plus a 200ms tick. Keybinding philosophy is drawn explicitly from lazygit/k9s/atuin.

**Alternatives rejected.** Deferred to later: mouse support, graph visualization of links via petgraph (v0.2), multi-pane/tabs (v0.2), custom theming (v0.2), undo stack (v0.3); plugins "nunca."

**Status / drift.** Accepted. **Drift documented inside the ADR itself:** the spec called for live-debounced search (200ms), but the v0.1 implementation deferred it to an explicit Enter trip. The ADR's note (lines 154–159) records that with `FakeEmbedder` per-keystroke querying is cheap but meaningless, so Enter is the better UX for now, and that **"Cloven flagged the spec/code drift on 2026-05-11."** This is the cleanest example in the corpus of a maintained ADR acknowledging that code overrode intent.

#### ADR-08 — Repo layout

**Decision.** A Cargo workspace with internal crates in a strict dependency hierarchy (`seele-core` at the bottom with no heavy external deps), shared `[workspace.dependencies]` and `[workspace.package]` metadata, a thin-LTO release profile (`opt-level=3`, `lto="thin"`, `codegen-units=1`, `strip=true`), and a tri-OS CI matrix (Linux/macOS/Windows × stable/beta, excluding Windows+beta) from day one plus a tag-driven release workflow for 5 targets.

**Rationale.** Clean layering keeps `seele-core` reusable and compile-time costs bounded; tri-OS CI catches platform-specific SQLite/ONNX issues early; `Swatinem/rust-cache` makes CI builds incremental.

**Alternatives rejected.** `cargo-make`/justfile/xtask task runners (cargo + Make if needed); enforced pre-commit hooks (documented, not enforced).

**Status / drift.** Accepted. **Drift (count corrected against the workspace manifest):** the ADR specifies **8 crates**; `CLAUDE.md` documents **12**; the actual `Cargo.toml` workspace ships **13** members — `CLAUDE.md`'s list silently omits `seele-chat`, which is a real crate in `crates/`. So relative to the 8-crate ADR-08 plan, **five** crates were added: `seele-sync`, `seele-setup`, `seele-project`, `seele-engram-import` (a direct consequence of the ENGRAM feature audit revealing git sync, the setup wizard, 5-case project detection, and the migration tool) plus `seele-chat`. The full leaf-to-root layering, verified from the per-crate `Cargo.toml` files, is: `seele-core` at the bottom; `seele-embedder`, `seele-storage`, `seele-project`, `seele-setup`, and `seele-chat` (standalone, no internal deps) sit above it; `seele-search` → core+storage+embedder; `seele-sync` and `seele-engram-import` → core+storage; `seele-http` → core+storage+search+embedder+chat; `seele-mcp` → core+storage+search+embedder+http (reuses `seele-http`); `seele-tui` → core+storage+search+http+embedder; `seele-cli` depends on everything. The CLAUDE.md/manifest crate-list mismatch is itself a documentation drift to flag.

#### ADR-09 — Distribution & license (MIT, clean-room credit)

**Decision.** Tag-driven GitHub Releases for 5 targets (linux gnu+musl x86_64, macOS arm64+x86_64, windows x86_64), each tarball bundling binary + LICENSE + CREDITS.md + README, with `SHA256SUMS`. Publish two crates to crates.io (`seele-core` + the `seele` binary); the rest stay `publish = false` until their APIs stabilize. Install scripts: `curl … | sh` (bash) and `iwr … | iex` (PowerShell). License: **MIT, copyright DevZen SpA**, with explicit, detailed credit to Gentleman-Programming/ENGRAM in README, CREDITS, and release notes. Homebrew tap deferred to v0.2.

**Rationale.** MIT over Apache-2.0 for simplicity (21 lines), broad compatibility, ecosystem consistency (AEGIS/LUMEN/MNEMA are all MIT), and because Apache's patent-grant clause is overkill here. Vendored binaries ship in tarballs so `cargo install seele` works without network. `cargo-deny` enforces a license whitelist and fails CI on copyleft; `cargo-audit` runs weekly.

**Alternatives rejected.** Apache-2.0 or mixed MIT/Apache (a Cloven follow-up explicitly required "MIT pura"); per-file license headers (root LICENSE suffices); native packages (apt/dnf/pacman) and container images (deferred); SaaS hosting ("out of charter — SEELE es local-first").

**Status / drift.** Accepted and central to the project's ethics. The credit obligation is satisfied by `CREDITS.md` and the README "Inspiration" section. Cloven's 2026-05-10 review forced the license to pure MIT (closed in commits `47d87ac`/`9478201` per `CLAUDE.md`).

#### ADR-10 — MNEMA ↔ SEELE conceptual mapping

**Decision.** A binding contract mapping MNEMA's Counsel vocabulary onto SEELE's schema across ~8 layers, so the first MNEMA-consumes-SEELE integration needs no re-modeling. Key mappings: a MNEMA Counsel = a SEELE `session`; each intermediate output (advisor output, blind review, verdict, skill, decision) = an `observation` with a `type` and a metadata JSON blob; lifecycle fields (`earn_score`, `axiomatic`, `core`, `context_mode`, `blind_id`, `model`, `tokens_used`) live in metadata, with `meta_score`/`meta_axiomatic`/`meta_context_mode` as virtual columns. Two relational mechanisms are distinguished: **`links`** for derivative/explanatory graph edges (`derives_from`, `related_to`, `evidence_for`, `part_of_verdict`) and **`memory_relations`** for invalidation with a judgment lifecycle (`supersedes`, `conflicts_with`, statuses `pending|judged|orphaned|ignored`).

**Rationale.** Without an explicit mapping, the first integration sprint would generate ad-hoc inconsistent mapping. The crucial design choice (Layer 4.5) is `context_mode`: purist advisors (Primeros Principios, Outsider) get no Recall; contextual advisors (Contrarian, Expansionista, Ejecutor) do. SEELE deliberately does **not** implement the purist filter as special logic — MNEMA filters `context_mode='purist'` when building its Recall query — keeping SEELE consumer-agnostic. Verdicts and skills are the "cristalización" of a counsel and enter Recall without a context_mode filter.

**Alternatives rejected.** SEELE knowing the Counsel pattern natively (rejected to stay agnostic); native `mnema_*` virtual columns (rejected — MNEMA adds its own via `seele schema add-virtual-col`).

**Status / drift.** Accepted. This ADR was triggered by a Cloven [ALTO] observation (2026-05-10) that the mapping was missing. It encodes the dual-relational-mechanism gotcha (`links` vs `memory_relations`) that the storage section (5) must keep straight.

#### ADR-11 — Vendored sqlite-vec

**Decision.** Ship precompiled upstream `sqlite-vec` binaries (`vec0.so`/`.dylib`/`.dll`) for 5 targets in `crates/seele-storage/vendor/sqlite-vec/<target>/`, embed them in the final executable via `include_bytes!`, and at runtime write them idempotently to `~/.cache/seele/vec0-<sha><suffix>` (SHA256-named, written 0644 then synced) and load via `rusqlite::Connection::load_extension`. Tracked upstream version: **v0.1.9** (2026-05-10). Env override `SEELE_VEC_PATH` for unsupported targets. `vec0_loader` exposes `vec0_bytes() -> Option<&'static [u8]>` and `vec0_extension_suffix() -> &'static str`; an unsupported target yields `StorageError::VecNotSupportedTarget`.

**Rationale.** No maintained stable Rust binding for sqlite-vec exists at genesis. Vendoring makes `cargo install seele` work offline out-of-the-box, gives deterministic control of the embedded binary, and is trivially auditable (files + upstream `CHECKSUMS-upstream.txt` in-repo). Cost: ~880 KB added to the repo — **"Aceptado por el User el 2026-05-10"** with the rationale "soluciones completas."

**Alternatives rejected.** `build.rs` + curl download (breaks offline, CI flakiness); an upstream Rust crate wrapper (abandoned/lagging at genesis); compiling from source in `build.rs` (needs C toolchains on Windows+macOS CI, +5–10 min/build); git submodule (upstream distributes binaries, not easily compilable source — "el peor de varios mundos").

**Status / drift.** Accepted. Bumping the binary is a manual human PR (Dependabot can *suggest* but cannot bump the binary); `CLAUDE.md` forbids touching the vendored `vec0.*` without updating the vendor README's documented 5-step bump procedure. License preservation (Apache-2.0 OR MIT, © 2024 Alex Garcia) is satisfied by in-repo upstream LICENSE copies + `CREDITS.md`.

#### ADR-12 — Embedder hardening follow-ups

**Decision.** A tracker ADR (not a new technical decision) recording two conscious gaps left in Sprint-02 to be revisited before v0.1.0: **(1)** `TRUSTED_HASHES` is an empty table — the SHA256 verification infra exists but no hashes are registered, so the default behavior is "not listed → log debug + proceed"; **(2)** INT8 quantized fallback is silent-with-warn — if HF temporarily pulls the quantized model, the embedder falls back to full precision with `tracing::warn` and keeps working.

**Rationale.** Both were flagged by Cloven (2026-05-10) as acceptable but trackable [NIT]s; this ADR makes them followable. Follow-up 1 (populate `TRUSTED_HASHES` in `crates/seele-embedder/src/onnx.rs`, format `&[(&str, &str, &str)]` of model/file/hex) is **blocking for v0.1.0 release**, owned by the release-prep sprint (Sprint-05). Follow-up 2 (an opt-in `strict_quantized` flag on `OnnxConfig` returning a new `QuantizedNotAvailable` error instead of falling back) is **nice-to-have**, else punted to the v0.2 backlog.

**Alternatives rejected.** (Not an alternatives ADR.) Out of scope: cosine-similarity regression tests (Sprint-05 polish), CUDA/Metal (v0.2 per ADR-04), embedder swap CLI (Sprint-04).

**Status / drift.** Accepted. This ADR is the clearest record of the AEGIS↔Cloven loop turning informal review NITs into tracked, owner-assigned, release-gated work items.

#### ADR-13 — ENGRAM compatibility

**Decision.** Three mechanisms so ENGRAM consumers (including MNEMA) migrate without re-implementing their wrapping layer: **(1)** a CLI migration command (the ADR writes it as `seele import --from-engram <path> [--re-embed]`; the shipped binary exposes it as the subcommand `seele import from-engram <path> [--dry-run] [--re-embed]` per `CLAUDE.md`) that migrates an ENGRAM SQLite DB into SEELE's schema (read-only source open, ULID preservation, `memories.body`→`observations.content`, `metadata.linked_to[]`→`links` table, idempotent by original ULID, inserts via a `save_raw`/`save_raw_in_tx` path that bypasses privacy-strip + dedup since data is already audited); **(2)** a `--tool-prefix` shim on MCP (`seele mcp --tool-prefix mnema` → `mnema_save`, with `seele_search`→`mnema_recall` specifically) and `--legacy-engram-paths` on HTTP (`POST /save`, `GET /show/{id}` aliases); **(3)** a decision *not* to rename virtual columns to `mnema_*` — `meta_*` stays canonical, MNEMA adds its own aliases in its wrapping layer.

**Rationale.** This ADR closes the "high timeline risk" Cloven detected on 2026-05-10: MNEMA depends on ENGRAM today, and a naive `engram`→`seele` swap breaks the user's direct SQL queries (`WHERE mnema_kind = ?`), the MCP wrappers (`mnema_save`), and renders the existing data unreadable (SEELE has no `memories` table). Total estimated cost ~600–900 LOC vs a person-week of manual migration.

**Alternatives rejected.** Native `mnema_*` virtual columns in SEELE (contaminates SEELE, doesn't scale to other consumers). Out of scope: bidirectional SEELE→ENGRAM conversion, continuous ENGRAM↔SEELE sync, multiple ENGRAM schema versions.

**Status / drift.** Accepted; implemented across Sprints 03–05 (`seele-engram-import` crate). It is the latest-conceived ADR and the most code-aligned because it was written with implementation immediately following.

### 22.2 Strategy docs digest

The five `estrategia/` docs precede and frame the ADRs.

- **`01-overview.md`** states the problem: MNEMA is a protocol, not a disk substrate; it needs a memory engine. ENGRAM (Go, MIT) is the niche reference but using it upstream has three costs — no roadmap control, an opaque external binary at the heart of persistence (technical debt from day one), and adding Go to a TS/Python/Astro ecosystem. SEELE solves these as a from-scratch Rust engine, protocol-agnostic but MNEMA-affine, distributed as a binary, reusable by any MCP-speaking agent. It also lists what's inherited from ENGRAM vs SEELE's own differentiators (embeddings, RRF, virtual columns, ULID, Rust, ratatui-vs-bubbletea) and what's deferred to v0.2+ (cloud replication, LLM conflict judging, semantic conflict scan, Obsidian export).

- **`02-reimplementacion-inspirada.md`** fixes the legal/ethical posture precisely: ENGRAM is MIT, so SEELE may read its code line-by-line, extract business logic, and reimplement — provided it gives credit and copies **no source verbatim**. It draws a sharp line between three postures in a comparison table (Fork / strict clean-room / **inspired reimplementation**) and adopts the third: *zero shared lines, but we do read the original*, with explicit, per-feature credit. The operational safeguards are a 5-step discipline (read ENGRAM file → understand → close it → implement in idiomatic Rust → annotate origin in CREDITS) and 7 standing rules (e.g. "PRs con 'tomado literal de ENGRAM' se rechazan"). This is the document the shared-context phrase "clean-room reimplementation inspired by ENGRAM" derives from — strictly, SEELE's own wording is *"reimplementación inspirada,"* explicitly "no un fork ni un clean-room estricto."

- **`03-naming-options.md`** records the naming process. The Orchestrator first proposed **STELE** (Greek στήλη, inscribed stone slab); the User counter-offered **SEELE** (German "soul/spirit") with the rationale "es el alma de todos los proyectos." Criteria: 5–7 letters, clear pronunciation in Spanish + English, evocative not acronym, no class-9 software trademark collision, fits "physical substrate of memory," domain availability, free GitHub slug. Rejected: CODEX (OpenAI collision), CALAMUS (less direct), VELLUM (Vellum AI collision), HEBB (pronunciation slur risk), SCRIBA (collision). The doc handles the Evangelion association with a README disclaimer ("'Seele' is the German word for 'soul'… not a reference to any specific franchise") and a trademark analysis (German common word, not copyrightable; avoid Eva visual branding). This is also why the legacy name **STELE** must be scrubbed from generated code — `CLAUDE.md`'s `check-no-stele-residual.{sh,ps1}` static check enforces it.

- **`04-scope-mvp.md`** defines v0.1 as "funcional para que MNEMA lo use en producción" and "paritaria con ENGRAM en lo local (sin cloud)." It enumerates the in-scope surface (the 9-table storage, ONNX embedder, RRF search, 5-case project detection, configurable topic-key families, privacy stripping, capture-passive, sessions lifecycle, read-only conflict storage, git sync, full CLI/HTTP/MCP/TUI, doctor/stats/export/import) and the deferred set (cloud Postgres + dashboard, semantic conflict scan, automatic decay, remote embeddings, encryption at rest, web UI, Obsidian export, Catppuccin theme). It also records two User decisions that *expanded* scope: TUI included from v0.1 and the **setup wizard "completo para todos los agentes"** (8 agents), on the rationale that a partial MVP forces the consumer to do half the work manually. (Note: the final `seele-setup` shipped 3 implemented — claude-code/cursor/windsurf — + 5 skeleton agents per `CLAUDE.md`; and the skeleton set that shipped (`opencode`/`aider`/`cody`/`continue`/`zed`) does not match the agents the strategy doc planned (`vs-code`/`gemini-cli`/`codex`/`antigravity`/`generic`), so the wizard's ambition both shrank and changed roster — a drift to flag.) It closes with an 11-point v0.1 acceptance checklist and a sizing estimate (~11 crates, ~10–15K LOC, 5–7 AEGIS sprints).

- **`05-engram-feature-audit.md`** is the file-by-file ENGRAM audit (against `Gentleman-Programming/engram` v1.15.10, via WebFetch of README/DOCS/ARCHITECTURE/CODEBASE-GUIDE/main.go — never cloned, per the legal posture). It is organized as parity tables A–J (Storage, Search, CLI, MCP tools, HTTP, Cloud, Algorithms, Config/Env, Operational, Conflict detection). Its executive summary: ~85% of ENGRAM's local features adopted, cloud entirely deferred, dashboard-templ + goreleaser never adopted, and four net-new differentiators added (embeddings, RRF, virtual columns, ULID). Crucially, this audit is *why* the plan grew from 8 crates / 7–10K LOC to ~11 crates / 10–15K LOC and from 3–4 to 5–7 sprints — it retroactively obsoletes the table shapes in ADR-02/05/06 that were written the same day.

### 22.3 ENGRAM lineage, precisely

The lineage is best stated as: **SEELE is an inspired Rust reimplementation of a Go memory engine, sharing zero source lines, with detailed per-feature attribution.** Per `CREDITS.md`, SEELE *inherits* (with its own implementation) ENGRAM's 9-table SQLite schema, the 19 MCP tool semantics (renamed `seele_*`, originally `mem_*`), the HTTP REST surface, the CLI verb layout, the 5-case project detection algorithm (config.json → git remote → git root → git child scan → dir basename), the topic-key family heuristics, two-layer privacy stripping, the capture-passive `## Key Learnings:` parser, git-friendly compressed chunk sync, the default port **7437**, and the `~/.<name>/` data-dir convention. SEELE *adds* sqlite-vec embeddings, RRF hybrid search, virtual generated columns + partial indexes, ULID IDs, and per-consumer configurable topic-key families.

Two differences are load-bearing:

- **Go vs Rust.** ENGRAM is Go with bubbletea TUI, goreleaser, INTEGER autoincrement IDs, and GC. SEELE is Rust with ratatui (no GC pauses), GitHub-Actions custom release, ULID IDs, `Result<T,E>` + thiserror typed errors, and `tokio::spawn_blocking` to run sync rusqlite off the async runtime. The strategy explicitly notes "Si elegimos Go, ¿por qué no usar ENGRAM directo?" — the language switch *is* the justification for re-doing the work.
- **Retrieval bet: FTS+LLM-judge vs FTS+embeddings+RRF.** ENGRAM ranks with FTS5 only and resolves conflicts with LLM-based judging. SEELE ranks with FTS5 *and* cosine similarity over local embeddings, fused by RRF, and ships the `memory_relations` schema for conflicts but defers the LLM semantic scan to v0.2. As `02-reimplementacion-inspirada.md`'s README block puts it: "different stacks (Go vs Rust) and a different bet on retrieval (FTS+LLM-judge vs FTS+embeddings+RRF)."

### 22.4 The AEGIS development protocol & Cloven review

SEELE was built under **AEGIS v2.0.0** ("sin agentes en background, todo en consola"). The genesis phase produced four plan sub-phases under `genesis/plans/` (`estrategia`, `arquitectura`, `tactica`, `executed`); when a sprint closes, its plan moves to `executed/`, a devlog lands in `docs/aegis/devlogs/YYYY-MM-DD-sprint-NN-<tema>.md`, and a cost entry appends to `cost-ledger.jsonl`. Per `CLAUDE.md`, **five AEGIS sprints** closed for v0.1.0 (released 2026-05-11), each with a tag and devlog:

| Sprint | Tag / theme | Devlog | Scope |
|---|---|---|---|
| 01 | `sprint-01-foundation` | `2026-05-10-…-foundation.md` | BE foundation: core types, storage, schema, vec0 loading |
| 02 | `sprint-02-embedder-search` | `2026-05-10-…-embedder-search.md` | ONNX embedder + RRF search (left ADR-12's 2 gaps) |
| 03 | `sprint-03-interfaces` | `2026-05-10-…-interfaces.md` | MCP + HTTP + auth + OpenAPI + ADR-13 aliasing |
| 04 | `sprint-04-ops-ux` | `2026-05-10-…-ops-ux.md` | CLI, TUI, sync, setup, project detection, ENGRAM import |
| 05 | `sprint-05-polish-release` | `2026-05-11-…-polish-release.md` | Property tests, ONNX default + Fake fallback, release pipeline, docs, v0.1.0 |

Two **human gates** are mandatory: Gate 1 (tactical-plan review before Execution) and Gate 2 (closure approval before state-sync). The "regla de oro AEGIS": a cycle is not closed unless `executed/`, the devlog, and `CLAUDE.md`/`INDEX.md` updates all exist. v0.1.0 was tagged after a `v0.1.0-rc.1` dry-run to validate `release.yml`; the build closed at "322 tests verde + 4 ignored" (per `CLAUDE.md`; Sprint-05's own devlog says ~340) with clippy/fmt/STELE-residual green. Development continued past v0.1 (e.g. `2026-05-20-patch-mcp-call-tool-result.md`).

**Cloven** is the external-review agent (`C:/dev/buddys/cloven/`), invoked between cycles ("`/cloven`"). Per `CLAUDE.md` it surfaced four documented findings during genesis: (1) NIT 2026-05-10 — scrub the legacy "STELE" name from generated Rust (closed via `check-no-stele-residual.{sh,ps1}` + CI jobs); (2) follow-up 2026-05-10 — license must be pure MIT, tests in order, Dependabot for the `ort` pin (closed in commits `47d87ac`/`9478201`); (3) Sprint-04 mid-review — four findings: CRITICO 1 `seele-sync::import` lacked a transaction, CRITICO 2 `seele-setup::write_atomic` used non-atomic `std::fs::write`, ALTO `setup --all` iterated skeletons + dead `--fake-embedder` UI, MEDIO `seele-project` git subprocess lacked a timeout — all closed in commit `d152842` (a 1500ms thread+mpsc cap for the git timeout). Beyond `CLAUDE.md`, Cloven's sight directly *produced* ADRs: the missing MNEMA mapping (→ ADR-10), the embedder NITs (→ ADR-12), the ENGRAM-migration timeline risk (→ ADR-13), and the TUI live-search spec/code drift (noted in ADR-07). Cloven is therefore best understood as the mechanism that converted external review into tracked, ADR-grade decisions.

### 22.5 The LUMEN protocol for the web

The web landing in `web/` (Astro) was built under a *separate* protocol, **LUMEN**, tracked in `docs/design/` (not `docs/aegis/`), with its own INDEX, devlogs, cost-ledger, and a versioned visual contract in `/DESIGN.md`. Per `docs/design/INDEX.md`, three LUMEN sprints closed (LUMEN-04, a Kimi K2 chat against the SEELE DB, is deferred): LUMEN-01 (landing + donate widget — "funcionalmente completo, visualmente mediocre," which itself triggered a full MNEMA counsel that bumped the LUMEN protocol to v0.10.0), LUMEN-02 (brutalist dev-craft redesign run by 4 parallel sub-agents, landing v0.3.0: 10.7 KB gz, 3-color palette, JetBrains Mono, `transition: none`), and LUMEN-03 (a separate `/observability` route, multi-resolution responsive, light+dark mode, Playwright matrix; LUMEN bumped to v0.11.0). LUMEN-02 produced five visual "wow" ADRs (`wow-01-monospace-only` … `wow-05-motion-zero`). The salient design-rationale point: SEELE deliberately runs **two protocols** — AEGIS for the engine (correctness, gates, devlogs, Cloven) and LUMEN for the frontend (visual critique loops, evidence reports on a11y/perf/heuristics) — and even fed its own dogfood loop, persisting LUMEN counsel verdicts and rejected design variations back into SEELE under `project=mnema`.

### 22.6 Cross-cutting findings for the improvement agent

1. **ADRs are genesis snapshots, not living records.** ADR-02/05/06/08 describe shapes (single `memories` table, ~9 MCP tools, `/memories` endpoints, 8 crates) that the same-day-or-next-day ENGRAM audit (strategy 05) obsoleted. Only ADR-07 self-annotates its drift. Reconciling each ADR with the final code is a concrete improvement surface. Even the *current* docs drift: `CLAUDE.md` lists 12 workspace crates but the manifest ships 13 (it omits `seele-chat`), and the HTTP default port is documented as both 7777 (ADR-06/`CLAUDE.md`) and 7437 (strategy/`CREDITS.md`, ENGRAM-parity) — both inconsistencies should be reconciled.
2. **The int64 bridge is the one true gotcha — and the ADR is wrong about it.** ADR-02's proposed virtual column `int_id GENERATED ALWAYS AS (CAST(SUBSTR(id,1,16) AS INTEGER))` is *broken* (base32 text casts to 0) and was discarded. The shipped `int_id` is a regular STORED `INTEGER NOT NULL UNIQUE` column written from Rust via `SeeleId::as_i64()`, which maps the ULID's **last 7 bytes (bytes 9..16, low 56 bits)** — NOT the "first 6 bytes" some prose claims, and NOT a timestamp-derived value (`crates/seele-core/src/id.rs:35-41`; `crates/seele-storage/src/migrations/V001__initial_schema.sql:7-13,35`). It is the authoritative rowid for both FTS5 and vec0, with insert-time retry on collision. Any change to ID generation or rowid mapping must respect this, and the stale `CLAUDE.md` "aproximada / solo JOIN" wording should be corrected at the source.
3. **Two conscious security gaps remain open or partial (ADR-12):** empty `TRUSTED_HASHES` and silent quantized fallback. Follow-up 1 was release-blocking; verify it was actually populated.
4. **Vendored sqlite-vec is a manual-bump liability (ADR-11):** Dependabot cannot bump the binary; a stale `v0.1.9` with an upstream security fix would require a human PR following the vendor README.
5. **Setup-wizard ambition outran v0.1 (strategy 04 vs `CLAUDE.md`):** planned 8-agent completeness, shipped 3 implemented (claude-code/cursor/windsurf) + 5 skeleton — and the skeleton roster that shipped (opencode/aider/cody/continue/zed) differs from the planned one (vs-code/gemini-cli/codex/antigravity/generic). A candidate for finishing, and for re-aligning planned vs implemented agents.
6. **MCP-from-scratch carries maintenance cost:** the post-v0.1 `tools/call` result-shape patch is evidence; an emerging canonical MCP crate (ADR-01/05 anticipated this) would be worth re-evaluating.


---

## 23. Known Limitations, Technical Debt & Improvement Surface

This section is the primary input for a downstream agent that will propose
improvements. It compiles, with file:line citations, every limitation,
deferred feature, code-debt marker, dependency risk, and architectural
constraint that is grounded in the actual source. Items are grouped by
theme: correctness risks, scalability/performance, security/privacy,
developer experience, missing features, dependency risk, and test gaps.
Nothing here is invented — each entry points at code, a comment, the
`CHANGELOG`, `CLAUDE.md`, or a doc that states it.

### 23.1 Declared v0.2 candidate features (the explicit backlog)

`CHANGELOG.md` `[Unreleased]` is currently small — it records only bug
fixes and test-fixture repairs, not new features. The substantive
roadmap lives in `CLAUDE.md:20`:

> Próximas features candidatas: 5 skeleton agents
> (`opencode`/`aider`/`cody`/`continue`/`zed`), sync chunk splitter
> (~1 MB cap), TUI editing in-place, `claude mcp add` delegación,
> Homebrew tap, project-detection wired in `seele save`.

The `[Unreleased]` section itself (`CHANGELOG.md:6-41`) is all debt
repayment / hardening rather than new features — two `Fixed`, one
`Added` (a regression-guard test), one `Changed`:

| Item | File | Nature |
|---|---|---|
| MCP `tools/call` wire-envelope fix | `crates/seele-mcp/src/server.rs` | Spec-compliance bug fix (`CallToolResult` envelope) |
| `seele-http` test-helper compile fix | six `tests/*.rs` files | Stale fixtures missing `chat: None` after the `ServerConfig.chat` field landed |
| MCP envelope shape regression test | `crates/seele-mcp/tests/call_tool_result_envelope.rs` | New `Added` test pinning the `CallToolResult` shape |
| STELE residual allowlist extension | `scripts/check-no-stele-residual.{sh,ps1}` | CI carve-out for `docs/plans/tactica/` (and `docs/plans/executed/tactica/`) |

The envelope bug (`CHANGELOG.md:10-19`) is notable as a *resolved*
correctness risk worth remembering: every `tools/call` response was
returning raw handler JSON instead of the `{ content: [...], isError }`
envelope, so Claude Code / Cursor / Windsurf rendered every call as
"completed with no output" while the handlers were actually running fine.
The regression guard is `crates/seele-mcp/tests/call_tool_result_envelope.rs`
(`CHANGELOG.md:29-33`). The stale-fixture story (`CHANGELOG.md:20-25`) is
a symptom of a broader pattern: integration tests construct `ServerConfig`
by hand, so any new field breaks six files at once.

### 23.2 Correctness risks

#### ULID→i64 collision surface (the vec0 rowid bridge)

`SeeleId::as_i64()` (`crates/seele-core/src/id.rs:35-41`) maps **only the
last 7 bytes** (`bytes[9..16]`, 56 bits) of the ULID into a non-negative
i64, zeroing the top byte to keep the sign bit clear:

```rust
// crates/seele-core/src/id.rs:35
pub fn as_i64(&self) -> i64 {
    let bytes = self.0.to_bytes();
    let mut int_bytes = [0u8; 8];
    int_bytes[1..8].copy_from_slice(&bytes[9..16]); // 56 bits
    i64::from_be_bytes(int_bytes)
}
```

This is the **authoritative** PK→rowid mapping (`CLAUDE.md:88`). The
doc-comment (`id.rs:28-34`) is honest about the risk: birthday-bound
collision probability for 10^6 IDs is "roughly 1 in 10^4", deemed
acceptable for the expected ~100K observations, and "when collisions
occur on insert, the storage layer regenerates the ULID." That retry
loop exists at `crates/seele-storage/src/observations.rs:538`
(`for _ in 0..ID_COLLISION_RETRIES`), keyed off the `int_id INTEGER NOT
NULL UNIQUE` constraint (`V001__initial_schema.sql:35`). **Residual
risk:** (1) the retry count is bounded; a sufficiently large/unlucky DB
can exhaust it; (2) the 56-bit space caps practical DB size well below
SQLite's row limit — at ~10^7 rows the birthday math becomes
unfavorable; (3) `save_raw_in_tx` on the sync/import path preserves
source ULIDs and therefore source `int_id`s, so two machines that
minted colliding `int_id`s independently will collide on import even
though their textual ULIDs differ.

#### The `int_id` SQL column is a regular STORED mirror of `as_i64()` — and `CLAUDE.md` describes it incorrectly

The migration comment is explicit (`V001__initial_schema.sql:7-13`): the
original plan to compute `int_id` as a *virtual generated* column from
`SUBSTR(id, ...)` was **broken** because ULIDs are base32 strings and
`CAST(letter AS INTEGER)` yields 0. The column is therefore a plain,
**stored** `INTEGER NOT NULL UNIQUE` (`V001:35`) populated from Rust at
insert time (`observations.rs:540`, `let int_id = id.as_i64()`). It is an
**exact** copy of `as_i64()`, not an "approximation": the only VIRTUAL
generated columns in the schema are `meta_kind` / `meta_domain` /
`meta_axiomatic` / `meta_score` / `meta_context_mode`
(`V001:56-65`). FTS5 (`content_rowid='int_id'`, `V001:97`) and vec0 both
JOIN on `int_id`:

```sql
-- engine.rs:194  (FTS path)
JOIN observations o ON o.int_id = fts.rowid
-- engine.rs:244  (vec path)
JOIN observations o ON o.int_id = vec.rowid
```

**Documentation-vs-code drift to be aware of:** `CLAUDE.md:88` still
describes this convention with two stale claims — that `as_i64()` takes
"primeros 6 bytes" (it actually takes bytes `9..16`, the last 7 bytes /
low 56 bits, top byte zeroed — `id.rs:35-41`), and that the SQL `int_id`
is "una virtual column ... aproximada." Both are wrong relative to the
shipped code: `int_id` is stored, not virtual, and equals `as_i64()`
exactly. The real residual risk is not approximation but
**desynchronization**: any future change to `as_i64`'s byte selection
would silently break the JOIN between existing FTS/vec rows and their
observations, since the stored `int_id`s were minted under the old
mapping.

#### Privacy strip leaks on nested `<private>` blocks

`strip_private_tags` (`crates/seele-storage/src/privacy.rs:11-20`) uses a
lazy regex `(?si)<private>.*?</private>`. The unit test at
`privacy.rs:67-74` documents the gotcha as intended behavior, but it is
a real data-leak surface: for nested tags the lazy match closes at the
**first** `</private>`, leaving trailing content un-stripped.

```rust
// privacy.rs:68
// Input:  "<private>outer<private>inner</private>tail</private>"
// Output: "tail</private>"   ← "tail" survives, was meant to be private
```

Unclosed `<private>` tags are also left intact by design
(`privacy.rs:48-53`) — a malformed block means nothing gets stripped, so
a typo in the closing tag silently exposes the entire intended-private
body.

#### Dedup normalization is aggressive and lossy by design

`normalized_hash` (`crates/seele-storage/src/hash.rs:17-28`) lowercases,
collapses all whitespace, and trims before SHA-256. The doc-comment
calls this "intentionally aggressive" (`hash.rs:15-16`) so reformat-only
edits merge as duplicates. The correctness trade-off: two *semantically
different* observations that differ only in case/whitespace (e.g. a code
snippet vs. prose that normalizes to the same bytes) will be treated as
duplicates and merged. The dedup window also includes
`(project, scope, type, title)` (`hash.rs:3`,
`idx_obs_dedup` at `V001:72-73`), which mitigates but does not eliminate
this.

#### Sync import drops `session_id`

`import_from_file` (`crates/seele-sync/src/lib.rs:281-345`) deliberately
sets `session_id: None` for every imported observation
(`lib.rs:317`, doc at `lib.rs:271-280`). Reason: the
`observations.session_id` FK (`V001:36`,
`REFERENCES sessions(id)`) would abort the whole transaction when a
chunk carries an observation whose session is unknown to the
destination. So **thread-level history does not travel across machines**
in v0.1; the breadcrumb relies on upstream consumers (MNEMA) stamping
session info into `metadata`. The code itself flags `format_version`
bump to 2 as the future fix (`lib.rs:278-279`).

### 23.3 Scalability & performance

- **SQLite single-writer.** The pool (`crates/seele-storage/src/pool.rs`)
  is `r2d2` over `SqliteConnectionManager` with `max_size: 8`
  (`pool.rs:22`) and `PRAGMA journal_mode=WAL` +
  `synchronous=NORMAL` (`pool.rs:34-39`). WAL allows concurrent
  readers with one writer, but **all writes serialize** through SQLite's
  single-writer lock. Under HTTP load with many concurrent saves, writers
  queue and can hit `SQLITE_BUSY`; there is no explicit `busy_timeout`
  PRAGMA set in `init_pool`, so contention surfaces as immediate lock
  errors rather than bounded waits. This is an architectural ceiling for
  the HTTP transport, acceptable for the local-first design but a real
  limit for multi-client deployments.
- **56-bit `int_id` caps practical scale** (see 23.2) — the design target
  is ~100K observations (`id.rs:33`), not millions.
- **Performance target is asserted only behind `#[ignore]`.** The sub-300ms
  @ 10K-rows criterion lives in
  `crates/seele-search/tests/perf_smoke.rs:25-51` (and a sub-100ms @ 1K
  variant, `perf_smoke.rs:55-71`), each carrying its own `#[ignore]`
  attribute (`perf_smoke.rs:24` and `:54`; the rationale is in the module
  doc-comment at `perf_smoke.rs:8-10`) so CI never runs them. There is
  **no automated regression gate on search latency**; a perf regression
  ships silently.
- **Sync exports a single chunk regardless of size.** `export_to_dir`
  (`seele-sync/src/lib.rs:181-199`) writes one `<chunk_id>.json.gz`
  containing the entire filtered observation set; the crate doc
  (`lib.rs:19-20`) defers the "~1 MB per chunk" splitter to a later
  sprint. A large project export produces one big blob — bad for the
  git-friendly diff story the crate is built around, and a memory spike
  (the whole payload is serialized in-memory in `compute_chunk_id` /
  `write_chunk_file`).
- **The annotation query builds an IN-list by string-formatting placeholders.**
  `attach_annotations` constructs `id_in = "(?,?,…)"` (`engine.rs:361-364`)
  and interpolates it into the SQL twice (`engine.rs:374`,
  `WHERE r.source_id IN {id_in} OR r.target_id IN {id_in}`). The *values*
  are still bound as parameters (`engine.rs:377-383`), so this is not a
  SQL-injection hole — but it is an O(N) per-query SQL-text rebuild that
  defeats statement caching, and it only fires on the opt-in
  `include_annotations` path. The hot FTS and vec queries themselves
  (`fts_query` at `engine.rs:191-230`, `vec_query` from `engine.rs:232`)
  are fully parameterized.

### 23.4 Security & privacy

- **Permissive CORS when any origin is allowed.** When
  `cors_origins` is non-empty, the server installs
  `CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any)`
  (`crates/seele-http/src/server.rs:113-116`). The inline comment admits
  it: "Block D refines per-origin allowlist; today we open up permissively
  when any origin is requested" (`server.rs:108-110`). The
  `CHANGELOG.md:128-130` repeats this: "Non-empty = permissive
  `Access-Control-Allow-Origin: *`. Per-origin allowlist refinement
  remains on the backlog." So the user-facing flag name (`--cors-allow
  <ORIGIN>`) implies an allowlist that is not actually enforced — any
  origin passes once CORS is enabled at all.
- **No ONNX model integrity verification by default.** `TRUSTED_HASHES`
  is an empty array (`crates/seele-embedder/src/onnx.rs:47-53`): "No
  hashes are pinned by default at v0.1." `verify_hash_if_listed`
  (defined `onnx.rs:295`; called for the model and tokenizer at
  `onnx.rs:96-97`) logs-and-proceeds when a file is not listed, so it is
  effectively a no-op for the shipped model — a swapped or tampered
  `all-MiniLM-L6-v2` download from Hugging Face would not be detected. The
  machinery exists; the pinned-hash data does not.
- **Silent ONNX→Fake degradation.** `pick_embedder`
  (`crates/seele-cli/src/app.rs:144-161`) falls back to `FakeEmbedder` on
  any `OnnxEmbedder::new()` error, emitting only a stderr warning
  (`app.rs:151-157`). `FakeEmbedder` (`crates/seele-embedder/src/fake.rs`)
  is a SHA-256 hash-to-vector (`fake.rs:23-41`) — deterministic but
  **not semantic**. If the warning is swallowed (e.g. a host that
  redirects stderr, or an MCP client that hides server stderr), retrieval
  quality silently collapses to keyword-ish behavior while the system
  reports success. `seele doctor` surfaces a `fake_embedder_warning`
  (asserted to contain "FakeEmbedder is active" by
  `crates/seele-cli/tests/subcommands_e2e.rs:178-193`,
  `doctor_emits_fake_embedder_warning`); per `CHANGELOG.md:237-238` it is
  emitted when `model_id` contains "fake". This is a detection mechanism
  but requires the operator to actively run doctor — nothing surfaces the
  degradation automatically at save/search time.
- **vec0 extension is loaded as unsafe native code.** `load_vec0`
  (`pool.rs:50-61`) calls `conn.load_extension` inside `unsafe` with the
  vendored binary; integrity is only a 16-hex-char SHA-256 *prefix* check
  (`vec0_install.rs:20`, `SHA_PREFIX_LEN = 16`,
  `verify_integrity` at `vec0_install.rs:102-113`). A truncated-prefix
  check is weaker than a full-digest check, though the binary is
  `include_bytes!`-embedded so the surface is the on-disk cache copy, not
  a network download.
- **Chat API keys ride in process env / flags.** The `ChatProvider`
  implementations hold the key as a plain `String`
  (`crates/seele-chat/src/lib.rs:117,225`) and inject it via
  `bearer_auth` / `x-api-key` (`lib.rs:186,283`). The design keeps the
  key off the browser (`CHANGELOG.md:118-120`), which is good, but the
  key is supplied via `--chat-key` (supports `$ENVVAR`) and lives in the
  `seele serve` process — standard for the threat model, worth noting for
  hardened deployments.

### 23.5 Developer experience & code debt

A full grep of `crates/` for `TODO|FIXME|HACK|XXX|todo!()|unimplemented!()`
returns **no occurrences** of the imperative debt markers — there are no
`todo!()`/`unimplemented!()` stubs and no `TODO`/`FIXME`/`HACK`/`XXX`
comments. The debt is instead encoded as typed `NotImplemented` errors,
`#[ignore]` tests, `#[allow(dead_code)]`, and prose comments. Concrete
markers:

| Marker | Location | Meaning |
|---|---|---|
| `#![allow(dead_code)]` | `crates/seele-search/tests/common/mod.rs:8` | Shared test fixture helpers, some unused per-test (only this + the next are the entire `allow(dead_code)` surface in `crates/`) |
| `#[allow(dead_code)]` | `crates/seele-mcp/src/jsonrpc.rs:13` | `Request.jsonrpc` field deserialized but never read |
| `panic!` (test-only) | `crates/seele-core/src/memory.rs:153` | `_ => panic!("expected Other")` — inside `#[cfg(test)] mod tests`, the `observation_type_other_passthrough` test; **not** a production path |
| `panic!` (test-only) | `crates/seele-cli/tests/binary_e2e.rs:150` | test assertion helper |
| `SetupError::NotImplemented` | `crates/seele-setup/src/lib.rs:38-39` | error variant returned for the 5 skeleton agents |

- **`seele save` does not auto-detect the project.** `seele-project` is a
  dependency of `seele-cli` (`crates/seele-cli/Cargo.toml:24`) but the
  save command never calls `detect()` — `save.rs` only forwards the
  optional `--project` flag (`crates/seele-cli/src/commands/save.rs:48-58`).
  The arg doc even says so: "project detection (`seele-project`) wires in
  Sprint-04 Bloque D.2" (`save.rs:21-22`). In practice `seele-project::detect`
  (`crates/seele-project/src/lib.rs:79`) is exercised **only by its own
  tests** (`crates/seele-project/tests/detect_e2e.rs`); it is dead from
  the binary's perspective. `CLAUDE.md:20` lists "project-detection wired
  in `seele save`" as a v0.2 candidate.
- **`--content -` stdin path unimplemented.** `save.rs:14-15` documents
  "`--content -` for stdin (Sprint-05 wires the stdin path)" but the code
  treats `content` as a plain positional string; there is no stdin
  branch.
- **Hand-rolled `ServerConfig` in tests** (see 23.1) makes adding a
  `ServerConfig` field a six-file breakage — a builder or `..Default`
  pattern would remove the recurring fixture churn.

### 23.6 Missing features (declared skeletons)

- **Five MCP-install skeleton agents.** `seele-setup` validates the names
  `opencode`, `aider`, `cody`, `continue`, `zed` but returns
  `SetupError::NotImplemented("agent {0} not implemented in v0.1 (planned
  for v0.2)")` (`crates/seele-setup/src/lib.rs:22-23,38-39`). The status
  table is in `docs/AGENT-SETUP.md:17-21` (🟡 skeleton) and the behavior
  is pinned by `crates/seele-setup/tests/install_e2e.rs:141-152`
  (`skeleton_agents_return_not_implemented`). `--all` filters to
  implemented-only via `AgentKind::is_implemented`
  (defined at `crates/seele-setup/src/agents.rs:69`; used by
  `implemented_agent_names()` at `lib.rs:140-146`,
  `CHANGELOG.md:206-213`), so skeletons are not reported as errors during
  bulk install — but they remain no-ops.
- **TUI is read-only.** `CLAUDE.md:20` lists "TUI editing in-place" as a
  v0.2 candidate; the five views (Home/Browse/Search/Detail/Stats) are
  display-only.
- **`claude mcp add` delegation, Homebrew tap, sync chunk splitter** —
  all v0.2 candidates per `CLAUDE.md:20`.

### 23.7 Dependency risk

- **`ort = "=2.0.0-rc.10"` exact pin on a release candidate.** Declared
  in `Cargo.toml:54` (with the rationale comment at `Cargo.toml:51-53`)
  and `CLAUDE.md:57` and `CHANGELOG.md:353-355`: "no hay 2.0.0 stable a
  2026-05." The RC API can break on bump. Dependabot
  (`.github/dependabot.yml`, cargo ecosystem, **monthly** schedule,
  `open-pull-requests-limit: 5`) is configured to surface the `ort` bump
  via PR rather than ignore it (explicit comment at `dependabot.yml:20-22`).
  `CLAUDE.md:157` explicitly forbids manual unpinning. This is the single
  largest external-dependency risk: a pre-1.0/RC crate sitting on the
  embedding-generation critical path.
- **Vendored `sqlite-vec` v0.1.9, manually bumped.** Not a Rust crate —
  raw `vec0.{so,dylib,dll}` binaries embedded via `include_bytes!` for 5
  targets (`CHANGELOG.md:156-158`, `CLAUDE.md:58`). Bumps are a manual
  procedure documented in
  `crates/seele-storage/vendor/sqlite-vec/README.md`; `CLAUDE.md:158`
  forbids touching the binary without updating that README. No automated
  upgrade path, and `sqlite-vec` itself is pre-1.0 (0.1.x) — schema/ABI
  changes in vec0 would require re-vendoring all five binaries.
- **`utoipa-swagger-ui` was force-bumped 8→9.0.2** to support axum 0.8
  (`CHANGELOG.md:339-340`); the OpenAPI/Swagger stack is tightly coupled
  to the axum version and will need coordinated bumps.
- **MSRV 1.85 floor** (`Cargo.toml:22`) was forced by `clap_lex`'s
  `edition2024` requirement (`CHANGELOG.md:348-352`); downgrading deps to
  lower the MSRV would cost axum 0.8 / utoipa 5 features.

### 23.8 Test gaps

- **4 `#[ignore]`d tests, none run in CI.** Two ONNX integration tests
  (`crates/seele-embedder/src/onnx.rs:465-488`, ignored because they
  "download ~30-90 MB from Hugging Face", `onnx.rs:460-464`) and two perf
  smokes (`#[ignore]` at `perf_smoke.rs:24` and `:54`). Consequence: **the real ONNX embedding
  path and the latency targets are never validated in CI** — the entire
  default-production embedder is only tested locally with `--ignored`.
  The whole CI test suite runs against `FakeEmbedder`
  (`crates/seele-mcp/tests/stdio_e2e.rs:19`,
  `crates/seele-http/tests/handlers_*.rs`,
  `crates/seele-cli/tests/subcommands_e2e.rs:12`).
- **ONNX integrity verification is untested with real data** because
  `TRUSTED_HASHES` is empty; the mismatch arm is tested only with a
  synthetic, hand-constructed `EmbedderError::HashMismatch`
  (`onnx.rs:440-457`).
- **Property tests run at low case counts.** `CLAUDE.md:67`: 32 cases per
  property, 8 for DB-touching storage properties — thin coverage for the
  ULID/i64 collision and roundtrip invariants
  (`crates/seele-core/tests/types_roundtrip.rs:122-131`).
- **No concurrency/contention test** for the single-writer pool — the
  `SQLITE_BUSY`/serialization behavior under concurrent HTTP writes is
  unexercised.
- **CORS allowlist behavior is not tested** beyond presence/absence; the
  "permissive when non-empty" semantics have no negative test asserting a
  disallowed origin is rejected (because it isn't rejected).

### 23.9 Resolved-but-worth-remembering (Cloven findings)

`CLAUDE.md:143-149` records the external-review (Cloven) findings; all are
**closed**, but each leaves a residual pattern the improvement agent
should respect rather than regress:

| Finding | Severity | Resolution | Residual pattern |
|---|---|---|---|
| `seele-sync::import` had no transaction | CRITICO 1 | Single-tx import (`seele-sync/src/lib.rs:281-345`) | Atomicity must be preserved on any future multi-step write |
| `seele-setup::write_atomic` used non-atomic `std::fs::write` | CRITICO 2 | tmp+rename (`vec0_install.rs:72-100` mirrors the pattern) | All config/cache writes must stay tmp+rename |
| `setup --all` iterated skeletons; `--fake-embedder` dead UI | ALTO | implemented-only filter + hidden flag | Skeleton agents must stay filtered from bulk ops |
| `seele-project` git subprocess had no timeout | MEDIO | 1500ms thread+mpsc cap (`CHANGELOG.md:200-205`) | Any subprocess shell-out needs a timeout |
| STELE legacy-name residue | NIT | `scripts/check-no-stele-residual.{sh,ps1}` + CI | New files must not reintroduce the legacy name |
| sync import counter ambiguity | NIT | documented dual counters (`seele-sync/src/lib.rs:106-112`) | Cross-layer counters need explicit docs |

The sync import comment at `lib.rs:269` ("Closes the Cloven 2026-05-11
[CRITICO] sync save-path finding") and `lib.rs:280` ("[MEDIO 1]
FK-violation finding") are the in-code provenance of these fixes.

### 23.10 Summary of the highest-leverage improvement targets

1. **Wire `seele-project::detect` into `seele save`** — declared,
   dependency already present, code path stubbed (`save.rs:21-22`).
2. **Make the ONNX→Fake fallback loud and detectable** beyond a stderr
   warning (`app.rs:151-157`) — e.g. persist embedder identity into the
   DB / `doctor` exit code, since silent degradation is the most damaging
   correctness-of-results risk.
3. **Implement the per-origin CORS allowlist** the flag already promises
   (`server.rs:108-116`).
4. **Add the ~1 MB sync chunk splitter** (`seele-sync/src/lib.rs:19-20`)
   to restore the git-friendly diff property at scale.
5. **Populate `TRUSTED_HASHES`** (`onnx.rs:47-53`) to activate model
   integrity verification.
6. **Run the ONNX + perf tests in a scheduled (non-PR) CI lane** so the
   production embedder and latency budget stop being untested.
7. **Track `ort` and re-vendor `sqlite-vec`** as the two pre-stable
   dependencies on the critical path.


---

## 24. Glossary & References

### 24.1 Glossary

**Domain & product**

| Term | Meaning |
|---|---|
| **SEELE** | German for "soul/spirit." The memory engine documented here — "memory as the soul of an agent." Not a franchise reference. |
| **Observation** | SEELE's canonical name for one stored memory record (title + content + metadata). The row in the `observations` table. |
| **Engram / Memory** | Synonyms for an observation, inherited from the ENGRAM lineage; used interchangeably in APIs and prose. |
| **Prompt** | A separately stored text record (the `prompts` table) — reusable prompt/snippet memory, distinct from observations. |
| **Topic key** | A `family/slug` label (e.g. `architecture/cache-strategy`) used to group and **upsert** observations. Default families: `architecture`, `bug`, `decision`, `pattern`, `config`, `discovery`, `learning`. |
| **Scope** | A second axis of grouping alongside project and topic key; part of the upsert key `(project, scope, topic_key)`. |
| **Project** | The workspace an observation belongs to, detected by `seele-project` (git/dir heuristics) and used to partition memory. |
| **Revision count** | Counter bumped each time an existing `(project, scope, topic_key)` is upserted rather than inserted anew. |
| **Privacy stripping** | Removal of `<private>...</private>` spans from title+content before hashing, indexing, and storage. |
| **Normalized hash** | A hash of normalized text used to detect and suppress duplicate saves within a 24-hour window. |
| **Soft delete** | Tombstoning a row via `deleted_at` instead of physically removing it; reversible via `restore`. |
| **Link** | A first-class typed edge between two observations (the `links` table). |
| **Relation** | A memory relation carrying a judgment (`relations`/memory-relations), distinct from a plain link. |
| **Chunk** | A gzipped JSON export unit used by sync; identified by a SHA-256 id. |
| **Ledger** | Per-target record of which chunks have been imported, making re-import idempotent. |

**Retrieval & ML**

| Term | Meaning |
|---|---|
| **FTS5** | SQLite's Full-Text Search module, version 5. Provides the lexical/keyword retrieval path via `MATCH`. |
| **sqlite-vec / vec0** | A SQLite extension (vendored at v0.1.9) adding vector columns and KNN search through `vec0` virtual tables. Not a Rust crate — a loadable C extension. |
| **Embedding** | A 384-dimensional float vector representing text semantics. |
| **all-MiniLM-L6-v2** | The sentence-transformer model SEELE uses for embeddings (384 dims), run locally. |
| **ONNX** | Open Neural Network Exchange — the model format; executed via the `ort` runtime. |
| **RRF (Reciprocal Rank Fusion)** | The algorithm that merges the FTS and vector ranked lists: `score(d) = Σ 1/(k + rank_i(d))`. |
| **KNN** | K-Nearest-Neighbors search over embeddings, served by vec0. |
| **FakeEmbedder** | A deterministic, model-free embedder used in tests and via `--fake-embedder` / `SEELE_FAKE_EMBEDDER`, and as the automatic fallback if ONNX init fails. |

**Identifiers & platform**

| Term | Meaning |
|---|---|
| **ULID** | Universally Unique Lexicographically Sortable Identifier — 128-bit, time-ordered, Crockford base32. |
| **SeeleId** | Newtype wrapping a `Ulid`; the primary key for every record. Stored as text in SQL. |
| **`as_i64()`** | The authoritative bridge mapping a `SeeleId` to the INTEGER rowid vec0 needs — packs ULID **bytes `9..16`** (the last 7 bytes / low 56 bits) into an i64 with the top byte zeroed. |
| **`int_id`** | A regular **stored** SQL column holding the `as_i64()` value — the canonical ULID→vec0-rowid mapping. *Not* virtual (the only virtual generated columns are the five `meta_*` columns). `CLAUDE.md`'s "approximate virtual column" wording is stale. |
| **WAL** | Write-Ahead Logging — the SQLite journal mode SEELE uses for better concurrency/crash recovery. |
| **MCP** | Model Context Protocol (Anthropic) — the JSON-RPC 2.0 tool protocol agents use to call SEELE. |
| **MNEMA / ENGRAM** | The Go memory engine (by Gentleman-Programming) that inspired SEELE; MNEMA is its consumer-facing naming. `--tool-prefix mnema` and the importer provide compatibility. |

**Process & tooling**

| Term | Meaning |
|---|---|
| **AEGIS** | The internal engineering protocol (planned sprints, devlogs, human gates) under which SEELE was built. |
| **LUMEN** | The design protocol governing the `web/` landing's visual system. |
| **Cloven** | The external code-review persona invoked between development cycles. |
| **STELE** | A legacy project name; CI runs a static check to ensure no residue of it appears in new files. |
| **Genesis plans** | The pre-v0.1 planning corpus (`genesis/plans/`): strategy, 13 ADRs, tactical sprint plans. |

**Key Rust dependencies**

| Crate | Used for |
|---|---|
| `rusqlite` (bundled, load_extension) | SQLite access + loading the vec0 extension. |
| `r2d2` / `r2d2_sqlite` | Connection pooling. |
| `refinery` | Embedded SQL migrations. |
| `ort` (`=2.0.0-rc.10`) | ONNX Runtime bindings (pinned to a release candidate). |
| `tokenizers`, `hf-hub`, `ndarray` | Tokenization, model download, tensor math for the embedder. |
| `axum`, `tower`, `tower-http`, `utoipa`, `utoipa-swagger-ui` | HTTP server, middleware, OpenAPI, Swagger UI. |
| `clap` | CLI parsing (derive). |
| `ratatui`, `crossterm` | TUI rendering and terminal events. |
| `ulid` | Identifier generation. |
| `tokio` | Async runtime (multi-thread). |
| `thiserror` / `anyhow` | Typed errors (libraries) / convenience errors (CLI). |
| `flate2`, `sha2`, `serde`/`serde_json` | Sync chunk gzip, hashing, serialization. |
| `proptest`, `insta`, `assert_cmd`, `predicates` | Property tests, TUI snapshots, CLI integration tests. |

### 24.2 References

**In-repository**
- `README.md` — overview, quick start, credits.
- `CLAUDE.md` — operating rules and current-state summary.
- `DESIGN.md` — the Brutalist web design system.
- `CHANGELOG.md` — Keep-a-Changelog; the `[Unreleased]` section drives §23.
- `CREDITS.md` — ENGRAM attribution.
- `docs/INDEX.md` — canonical documentation map.
- `docs/INSTALLATION.md`, `docs/AGENT-SETUP.md`, `docs/ENGRAM-MIGRATION.md` — user guides.
- `genesis/plans/arquitectura/01..13` — the architecture ADRs (digested in §22).
- `genesis/plans/estrategia/01..05` — strategy, scope/MVP, ENGRAM feature audit, naming.
- `docs/aegis/devlogs/` — sprint devlogs (incl. the 2026-05-20 MCP call-tool-result patch).

**External**
- ENGRAM — `https://github.com/Gentleman-Programming/engram`
- Model Context Protocol — `https://modelcontextprotocol.io`
- sqlite-vec — `https://github.com/asg017/sqlite-vec`
- SQLite FTS5 — `https://www.sqlite.org/fts5.html`
- `all-MiniLM-L6-v2` — `https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2`
- `ort` (ONNX Runtime for Rust) — `https://ort.pyke.io`

