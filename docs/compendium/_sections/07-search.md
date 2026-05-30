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
