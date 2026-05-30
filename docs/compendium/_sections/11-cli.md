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
