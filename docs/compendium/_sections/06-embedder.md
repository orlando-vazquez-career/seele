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
