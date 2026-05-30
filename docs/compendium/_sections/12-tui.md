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
