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
