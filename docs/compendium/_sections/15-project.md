## 15. Project Detection — `seele-project`

`seele-project` is a leaf crate whose single job is to answer one question: *given the current working directory, what is the name of the project this memory belongs to?* SEELE partitions observations by `project` (the column that scopes searches, `seele list`, `seele projects`, and topic-key upserts keyed on `(project, scope, topic_key)`), so a good project name matters for retrieval quality. When an AI agent calls `seele save` without an explicit `--project`, the intent is that SEELE infers one. This crate implements that inference as a deterministic 5-case cascade ported from ENGRAM (`crates/seele-project/src/lib.rs:1`).

In the crate graph it sits near the bottom. Its `Cargo.toml` declares `seele-core` as its only internal dependency (and it does not actually use any core symbol in `lib.rs` — the dependency is structural, keeping the crate inside the workspace's core-anchored layering), plus four external crates: `serde`/`serde_json` (config parsing + `Serialize` on the result), `thiserror` (typed errors), and `tracing` (debug logging on timeout paths). Dev-dependency `tempfile` backs the E2E tests. The entire crate is a single 355-line `lib.rs` plus one integration test file.

### Public API surface

| Item | Signature | Purpose |
|------|-----------|---------|
| `detect` | `pub fn detect(cwd: &Path) -> Result<ProjectName>` | Runs the full 5-case cascade against `cwd`. |
| `parse_remote_basename` | `pub fn parse_remote_basename(url: &str) -> Option<String>` | Pure URL→basename parser (case 2 helper); public so it can be unit-tested and reused. |
| `ProjectName` | `pub struct { name: String, source: DetectSource }` | The result: the inferred name plus which case produced it. Derives `Debug, Clone, PartialEq, Eq, Serialize`. |
| `DetectSource` | `pub enum { Config, GitRemote, GitRoot, GitChild, DirBasename }` | Which of the five cases fired. `Copy`; `#[serde(rename_all = "snake_case")]`. |
| `ProjectError` | `pub enum { NoBasename, Io(io::Error), InvalidConfig(String) }` | Typed errors via `thiserror`. |
| `Result<T>` | `pub type = std::result::Result<T, ProjectError>` | Crate-local result alias. |

`ProjectName.source` is deliberately carried alongside the name: the crate doc notes it is "util para debugging (`seele doctor` lo expone) y para tests" (`lib.rs:39-40`). The `Serialize` derive plus `snake_case` rename means `DetectSource::GitRemote` serializes as `"git_remote"`, ready to surface in JSON diagnostics.

`ProjectError` has only three variants, and only two are practically reachable: `InvalidConfig` (malformed `.seele/config.json`) and `Io` (a non-`NotFound` filesystem error reading that file). `NoBasename` is the pathological case where even `cwd.file_name()` yields nothing — effectively only a filesystem root. `detect` is documented to "always return `Ok` because case 5 (basename) is reachable" except for that one degenerate path (`lib.rs:76`).

### The 5-case detection cascade

The ordering is load-bearing. The crate doc states the rationale plainly: "El orden importa: cada caso solo corre si el anterior fallo. Esto evita pisar el `project` correcto con un fallback" (`lib.rs:4`). Each case is tried in turn; the first to yield `Some`/a value wins and short-circuits.

| # | `DetectSource` | Mechanism | Helper |
|---|---------------|-----------|--------|
| 1 | `Config` | Read `<cwd>/.seele/config.json`, parse `{"project": "..."}`, accept if non-blank | `read_config_override` |
| 2 | `GitRemote` | `git -C <cwd> remote get-url origin` → basename of URL | `git_remote_name` → `parse_remote_basename` |
| 3 | `GitRoot` | `git -C <cwd> rev-parse --show-toplevel` → basename of path | `git_root_basename` |
| 4 | `GitChild` | Scan depth-1 subdirs of `cwd` for one containing `.git/` | `git_child_scan` |
| 5 | `DirBasename` | `cwd.file_name()` (final fallback) | inline in `detect` |

The control flow in `detect` is a flat chain of `if let Some(name) = … { return … }` blocks, ending with the basename fallback (`lib.rs:79-122`):

```rust
// crates/seele-project/src/lib.rs:79-110 (condensed)
pub fn detect(cwd: &Path) -> Result<ProjectName> {
    if let Some(name) = read_config_override(cwd)? { return Ok(ProjectName { name, source: DetectSource::Config }); }
    if let Some(name) = git_remote_name(cwd)       { return Ok(ProjectName { name, source: DetectSource::GitRemote }); }
    if let Some(name) = git_root_basename(cwd)     { return Ok(ProjectName { name, source: DetectSource::GitRoot }); }
    if let Some(name) = git_child_scan(cwd)        { return Ok(ProjectName { name, source: DetectSource::GitChild }); }
    let name = cwd.file_name().and_then(|s| s.to_str()).map(String::from).ok_or(ProjectError::NoBasename)?;
    Ok(ProjectName { name, source: DetectSource::DirBasename })
}
```

Note that only case 1 can propagate an error (the `?` on `read_config_override`); cases 2–4 return `Option` and silently fall through on any failure, which is why an unreadable git or a flaky filesystem degrades gracefully to the basename.

#### Case 1 — `.seele/config.json` override

`read_config_override` (`lib.rs:132`) joins `cwd/.seele/config.json` and reads it. A `NotFound` error is mapped to `Ok(None)` (no config is normal); any other IO error becomes `ProjectError::Io`. The file is parsed into `struct ConfigFile { #[serde(default)] project: Option<String> }`. Parse failure becomes `ProjectError::InvalidConfig(format!("{path:?}: {e}"))` — a *loud* error rather than a silent fall-through, so a typo in the config surfaces immediately (the test `case1_invalid_json_errors_loudly` asserts the message contains `"invalid config.json"`). The extracted project is run through `.filter(|s| !s.trim().is_empty())`, so a blank or whitespace-only value (`"   "`) is treated as absent and falls through to later cases — confirmed by `case1_empty_project_falls_through`.

#### Cases 2 & 3 — git-derived signals

Both shell out to `git` via the shared `run_git` helper. Case 2 (`git_remote_name`, `lib.rs:146`) runs `remote get-url origin` and feeds the URL into `parse_remote_basename`. Case 3 (`git_root_basename`, `lib.rs:227`) runs `rev-parse --show-toplevel` and takes `PathBuf::file_name()` of the result. Remote wins over toplevel because the remote name is the more canonical, machine-stable identity of a repo (two clones of the same repo into differently-named directories should map to the same project).

`parse_remote_basename` (`lib.rs:199`) is a careful, allocation-light URL normalizer that handles every common git URL shape. Its steps, in order: trim; bail on empty; strip query/fragment (`split(['?','#'])`); strip a trailing `/`; strip a trailing `.git`; take the segment after the last `:` (handles `git@host:org/repo`), then the segment after the last `/`; trim and reject if empty. The documented coverage (`lib.rs:191`):

```rust
// crates/seele-project/src/lib.rs:191-198
// - `git@github.com:org/repo.git`      → `repo`
// - `https://github.com/org/repo.git`  → `repo`
// - `ssh://git@host:22/group/repo`     → `repo`
// - `file:///tmp/repo`                 → `repo`
// - `bare/path/repo`                   → `repo`
```

Seven unit tests in `lib.rs:301-353` lock these down (https-with-suffix, ssh-short, no-suffix, ssh-with-port, file URL, trailing slash, empty→`None`).

##### The git subprocess timeout (Cloven MEDIO fix)

`run_git` (`lib.rs:160`) is the concurrency-sensitive heart of the crate, added to close a MEDIO finding from the Sprint-04 Cloven review: "`seele-project` git subprocess sin timeout (cerrado, 1500ms thread+mpsc cap)" (CLAUDE.md). The mechanism: spawn `git -C <cwd> <args>` on a worker `thread`, hand the `Command::output()` result back over an `mpsc::channel`, and block the caller only via `rx.recv_timeout(GIT_SUBPROCESS_TIMEOUT)`.

```rust
// crates/seele-project/src/lib.rs:160-189 (condensed)
fn run_git(cwd: &Path, args: &[&str], timeout: Duration) -> Option<String> {
    let (tx, rx) = mpsc::channel();
    // ... clone cwd + args into the worker ...
    thread::spawn(move || { let _ = tx.send(Command::new("git").arg("-C").arg(&cwd).args(&worker_args).output()); });
    match rx.recv_timeout(timeout) {
        Ok(Ok(output)) if output.status.success() => { /* trim stdout, None if empty else Some */ }
        Ok(_) => None,                                            // git ran but failed / non-zero exit
        Err(mpsc::RecvTimeoutError::Timeout)      => { tracing::debug!(args = ?owned_args, "..."); None }
        Err(mpsc::RecvTimeoutError::Disconnected) => None,        // worker panicked before sending
    }
}
```

`GIT_SUBPROCESS_TIMEOUT = Duration::from_millis(1500)` (`lib.rs:37`). The constant's rationale is documented at length: it is sized for "git is unresponsive" not "git is slow today" — a healthy local git on Windows pays ~50–300ms in process startup alone, so 1500ms never trips on a normal repo, while a stuck git (credential prompt, NFS hang) cannot block `detect` past ~3 seconds total across cases 2 and 3 combined (`lib.rs:28-36`).

The critical design decision: **a timed-out subprocess is abandoned, not killed.** The comment justifies this — git on this read path "has no externally visible side effects," and the orphaned worker thread "cleans itself up when git eventually exits" (`lib.rs:155-159`, `28-36`). Inputs are cloned into owned `PathBuf`/`Vec<String>` before the `move` closure so the worker outlives the caller's borrow safely. Success requires both `output.status.success()` *and* non-empty trimmed stdout; an empty result returns `None` (so a repo with no `origin` remote correctly falls through from case 2 to case 3).

#### Case 4 — git child scan

`git_child_scan` (`lib.rs:260`) handles the "monorepo parent" / "I'm one level above the actual repo" case: it reads the immediate children of `cwd` and returns the name of the first subdirectory that contains a `.git/` entry. It is bounded three ways to keep it cheap:

| Bound | Constant | Value | Effect |
|-------|----------|-------|--------|
| Time budget | `SCAN_TIMEOUT` | `200ms` | Aborts the scan if elapsed > budget. |
| Visit cap | `SCAN_MAX_DIRS` | `20` | Aborts after 20 *non-noise* dirs visited. |
| Noise filter | `NOISE_DIRS` | 11 names | Skips dirs that never hold project roots. |

`NOISE_DIRS` (`lib.rs:246`): `node_modules`, `target`, `.git`, `vendor`, `.venv`, `venv`, `__pycache__`, `dist`, `build`, `.next`, `.nuxt`. Each loop iteration first checks the timeout and visit cap (both log `tracing::debug!` and return `None` when hit), skips non-directories, skips noise dirs (without incrementing `visited`), then increments `visited` and tests `path.join(".git").exists()`. Iteration order follows `read_dir`, which is filesystem-defined and *not* sorted — so when multiple sibling repos exist, "first found" is non-deterministic across platforms. `case4_skips_noise_dirs` proves a `node_modules/foo/.git` is ignored in favor of a sibling `legit/.git`; `case4_child_scan_finds_first_subdir_with_dot_git` proves the basic find. Note this case detects only `.git` presence, not language manifests — despite a sprint planning note suggesting language-agnostic manifest detection, the shipped code keys solely on `.git/`.

#### Case 5 — directory basename

The final fallback: `cwd.file_name()` → `&str` → `String`, source `DirBasename`. Reachable for any directory that isn't a filesystem root. Only here can `detect` return `Err(ProjectError::NoBasename)`.

### Normalization, concurrency, and error handling

"Normalization" in this crate is light and per-case rather than a single shared pass: each case trims and basenames its own way. Cases 2/3/4/5 all reduce to a last-path-segment basename; case 2 additionally strips `.git`, query strings, and trailing slashes. There is **no** lowercasing, no whitespace-to-dash slugification, and no length cap — the detected name is used verbatim as the `project` value. The only emptiness guards are the case-1 blank filter and the empty-string rejections inside `parse_remote_basename`/`run_git`.

Concurrency is confined to `run_git`: one detached `std::thread` per git invocation, communicating over a single-shot `mpsc` channel, with the timeout enforced on the receive. There is no `tokio`/async here — `detect` is a blocking synchronous call, intended to run before/around the async save path in the CLI. Error handling is bimodal: case 1 is fail-loud (typed errors propagate), cases 2–4 are fail-soft (any failure → `Option::None` → next case), guaranteeing the cascade always reaches a usable answer.

### Wiring into SEELE — and the known gap

This is the crate's most important caveat for an improver. **`detect` is not called from production code.** A repo-wide search shows the only call sites of `detect`, `parse_remote_basename`, `DetectSource`, etc. are inside the crate's own `tests/detect_e2e.rs`. Notably, `seele-cli/Cargo.toml:24` *does* declare `seele-project = { path = "../seele-project" }` as a dependency, yet nothing under `crates/seele-cli/src/` imports the `seele_project` module — the link is present but unused. The CLI `save` command exposes `--project` as a plain `Option<String>` and, when omitted, passes that `None` straight through to `SaveRequest.project` (`save.rs:52`) — it never invokes `seele-project`. The code comment is explicit (`crates/seele-cli/src/commands/save.rs:20-24`):

```rust
// crates/seele-cli/src/commands/save.rs:20-24
/// Project name. If omitted, the caller is expected to set
/// `--project ""` explicitly when this matters; project detection
/// (`seele-project`) wires in Sprint-04 Bloque D.2.
#[arg(long)]
pub project: Option<String>,
```

That wiring "nunca shipeó." The Sprint-05 devlog records it as a known v0.2 gap: "Project detection auto-wired en `seele save` — la crate `seele-project` está testeada end-to-end pero no se llama desde el CLI… esa wiring nunca shipeó. Gap conocido, v0.2." (`docs/aegis/devlogs/2026-05-11-sprint-05-polish-release.md:151`). CLAUDE.md lists "project-detection wired in `seele save`" among v0.2 candidate features. So at v0.2.0 the crate is a complete, tested, dead-code island: correct and ready, but the data it would produce never crosses into the storage layer.

When wired, the intended boundary is straightforward: `detect(cwd)` would supply the `project` field of `SaveRequest` (`crates/seele-cli/src/commands/save.rs:48-58`) whenever `--project` is absent, partitioning the resulting observation in storage. The `DetectSource` is also a natural fit for `seele doctor` diagnostics (per the doc comment), though no `doctor` call site exists today either.

### File-by-file map

- **`crates/seele-project/src/lib.rs`** — the entire implementation: crate doc describing the cascade and noise list, the timeout/scan constants, `ProjectName`/`DetectSource`/`ProjectError`/`Result` public types, the `detect` cascade, the five case helpers (`read_config_override`, `git_remote_name`+`parse_remote_basename`, `git_root_basename`, `git_child_scan`), the `run_git` thread+mpsc wrapper, and a `#[cfg(test)]` module of seven `parse_remote_basename` unit tests.
- **`crates/seele-project/Cargo.toml`** — package metadata (description: "5-case project detection algorithm (config / git remote / git root / child scan / dir basename)"); deps `seele-core`, `serde`, `serde_json`, `thiserror`, `tracing`; dev-dep `tempfile`.
- **`crates/seele-project/tests/detect_e2e.rs`** — eight integration tests exercising all five cases against real `TempDir`s and real `git` subprocesses, with `git_available()` self-skip guards for minimal CI. Covers config-wins-over-git, blank-falls-through, invalid-JSON-errors, ssh remote basename, no-remote toplevel, child scan find, noise-dir skip, and the pure basename fallback. This file is also run as smoke criterion 6 in `scripts/v0.1.0-smoke.sh:116`.

### Edge cases, gotchas, and invariants

- **Dead code at v0.2.0** — fully tested, never invoked outside tests (the headline gotcha above).
- **No name normalization** — the detected name is used raw; mixed case, spaces, and unusual characters pass through unchanged.
- **Non-deterministic child scan order** — `read_dir` order is unsorted; with multiple sibling repos, the chosen project is platform/filesystem-dependent.
- **Orphaned git processes on timeout** — by design (`lib.rs:155-159`); safe only because this is a side-effect-free read path. A hung git leaks a thread + process until git itself exits.
- **Worst-case latency ~3.2s** — two 1500ms git timeouts (cases 2+3) plus a 200ms scan (case 4) before the basename fallback, if git hangs on every call.
- **Case-1 is the only fail-loud case** — a malformed `.seele/config.json` aborts detection rather than degrading; a blank `project` value, however, is treated as absent and falls through.
- **Case 4 detects `.git/` only**, not language manifests, contrary to one sprint planning note (`genesis/plans/executed/tactica/sprint-04/00-INDEX.md:56`, which states "`seele-project` no asume Rust — es lang-agnostic. Detecta `package.json`, `Cargo.toml`, `pyproject.toml`, `go.mod`, `pom.xml`, etc.") — a divergence between plan and shipped code worth noting for anyone extending it.
