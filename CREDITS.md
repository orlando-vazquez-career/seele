# Credits

## Inspiration: ENGRAM by Gentleman-Programming

SEELE's architecture, data model, MCP tool set, project detection algorithm, topic key conventions, privacy stripping, capture passive parser, and many other design decisions are directly inspired by [ENGRAM](https://github.com/Gentleman-Programming/engram) by [Gentleman-Programming](https://github.com/Gentleman-Programming).

ENGRAM is licensed under the MIT License (Copyright Gentleman-Programming). SEELE is a Rust reimplementation under MIT (Copyright DevZen SpA). The ENGRAM source code was studied as reference; **no Go code was copied verbatim into SEELE**.

### What SEELE inherits from ENGRAM (with our own implementation)

- The 9-table SQLite schema (`sessions`, `observations`, `observations_fts`, `user_prompts`, `prompts_fts`, `memory_relations`, `sync_chunks`, `sync_apply_deferred`, `schema_version`).
- The 19 MCP tool semantics (renamed `seele_*` to differentiate).
- The HTTP REST API surface.
- The CLI verbs and subcommand layout.
- The 5-case project detection algorithm (config.json → git remote → git root → git child scan → dir basename).
- The topic-key family heuristics (`architecture/*`, `bug/*`, `decision/*`, `pattern/*`, `config/*`, `discovery/*`, `learning/*`).
- The privacy stripping in two layers (plugin + store).
- The capture passive parser pattern (`## Key Learnings:` extraction).
- The git-friendly compressed chunks sync.
- Default port 7437 and `~/.<name>/` data directory convention.

### What SEELE adds beyond ENGRAM

- **Vector embeddings** via `sqlite-vec` (`all-MiniLM-L6-v2`, dim 384).
- **Hybrid search** with Reciprocal Rank Fusion (RRF) over FTS5 + cosine similarity.
- **Virtual generated columns** + partial B-tree indexes for sub-10ms metadata filtering on JSON fields.
- **ULID identifiers** sortable by timestamp.
- **Configurable topic key families per consumer** — MNEMA registers its own (`verdict/*`, `axiomatica/*`, `skill/*`, `disenso/*`) without imposing ENGRAM's coding-agent set.

We thank Gentleman-Programming for proving the viability of this category and for choosing MIT as the license, enabling work like SEELE.

## Models

### `all-MiniLM-L6-v2`

Default embedder. By [Sentence-Transformers](https://www.sbert.net/) (UKP Lab + Microsoft). Apache-2.0. https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2

## Major Rust dependencies

- **`tokio`** — async runtime. MIT.
- **`rusqlite`** — SQLite bindings. MIT.
- **`sqlite-vec`** by Alex Garcia — vector extension for SQLite. Apache-2.0/MIT.
- **`ort`** by pykeio — ONNX Runtime bindings. Apache-2.0/MIT.
- **`tokenizers`** by Hugging Face — Apache-2.0.
- **`hf-hub`** — Apache-2.0.
- **`axum`** by the Tokio team — web framework. MIT.
- **`tower`** + **`tower-http`** — middleware. MIT.
- **`utoipa`** — OpenAPI generation. Apache-2.0/MIT.
- **`clap`** — CLI parser. Apache-2.0/MIT.
- **`ratatui`** + **`crossterm`** — TUI framework. MIT.
- **`serde`** + **`serde_json`** — serialization. Apache-2.0/MIT.
- **`thiserror`** + **`anyhow`** — error handling. Apache-2.0/MIT.
- **`tracing`** + **`tracing-subscriber`** — observability. MIT.
- **`ulid`** — ID generation. MIT.
- **`refinery`** — migrations. MIT.

Full dependency list will be in `Cargo.lock` once the workspace is initialized. License audit via `cargo-deny`.

---

> *Super stellatum firmamentum iudicat Deus, sicut nos iudicamus.*

— **DevZen SpA**, 2026
