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
