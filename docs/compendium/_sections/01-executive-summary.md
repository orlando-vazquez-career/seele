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
