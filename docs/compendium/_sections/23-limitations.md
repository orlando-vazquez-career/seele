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
