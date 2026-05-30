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
