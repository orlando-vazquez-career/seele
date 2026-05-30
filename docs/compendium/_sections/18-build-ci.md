## 18. Build, CI/CD, Distribution & Release

This section documents how SEELE is compiled, linted, tested, packaged, and shipped: the Cargo workspace layout and release profile, the pinned toolchain and platform-specific build flags, the three GitHub Actions workflows (CI, Release, web deploy), the Dependabot configuration, the curl/irm installers with SHA256 verification, the STELE-residual static checks, the v0.1.0 acceptance smoke, and the two load-bearing version pins (`ort =2.0.0-rc.10` and the vendored `sqlite-vec` v0.1.9). It is the "how it gets built and out the door" layer; the runtime crates it builds are covered in sections 3–17.

### 18.1 The Cargo workspace

The root `Cargo.toml` (`C:/dev/tools/SEELE/Cargo.toml`) declares a 13-member virtual workspace (no root package — the binary lives in `seele-cli`) using `resolver = "2"`. The members enumerate every crate in `crates/`, leaf-to-root:

```toml
# Cargo.toml:1-17
[workspace]
members = [
    "crates/seele-core", "crates/seele-storage", "crates/seele-embedder",
    "crates/seele-search", "crates/seele-chat", "crates/seele-mcp",
    "crates/seele-http", "crates/seele-tui", "crates/seele-sync",
    "crates/seele-setup", "crates/seele-project", "crates/seele-engram-import",
    "crates/seele-cli",
]
resolver = "2"
```

(The CLAUDE.md text says "12 crates"; the live `members` list has 13 because `seele-chat` was added later. The compendium intro count of 13 crates is authoritative.)

`[workspace.package]` centralizes shared metadata that each crate inherits via `field.workspace = true`: `version = "0.2.0"`, `edition = "2021"`, `rust-version = "1.85"` (the MSRV), `authors = ["DevZen SpA"]`, `license = "MIT"`, and `repository = "https://github.com/orlando-vazquez-career/seele"`. Member crates pick these up with one-liners (e.g. `seele-cli/Cargo.toml`: `version.workspace = true`, `edition.workspace = true`, etc.), so the version and MSRV are bumped in exactly one place.

`[workspace.dependencies]` is the single source of truth for third-party crate versions. Member crates reference them with `<crate>.workspace = true` (sometimes adding features locally). The set is grouped by purpose:

| Group | Key crates (version) | Purpose |
| --- | --- | --- |
| Async | `tokio` 1.42 (`features=["full"]`), `futures` 0.3 | multi-thread runtime |
| Serialization | `serde` 1, `serde_json` 1 (`preserve_order`) | DTOs, JSON-RPC, OpenAPI |
| Errors | `thiserror` 2, `anyhow` 1 | typed errors / CLI top-level |
| Tracing | `tracing` 0.1, `tracing-subscriber` 0.3 (`env-filter`) | structured logs |
| Storage | `rusqlite` 0.32 (`bundled`,`load_extension`), `r2d2` 0.8, `r2d2_sqlite` 0.25, `refinery` 0.8 (`rusqlite`) | SQLite pool + migrations |
| Embedder | `ort` `=2.0.0-rc.10`, `ndarray` 0.16, `tokenizers` 0.20, `hf-hub` 0.3 | ONNX runtime + tokenizer |
| HTTP | `axum` 0.8, `tower` 0.5, `tower-http` 0.6 (`cors`,`trace`,`compression-gzip`), `utoipa` 5 (`axum_extras`), `utoipa-swagger-ui` 9 (`axum`), `reqwest` 0.12 (test-only, `default-features=false`, `json`+`rustls-tls`) | REST API + OpenAPI/Swagger |
| CLI / TUI | `clap` 4.5 (`derive`,`env`), `ratatui` 0.29, `crossterm` 0.28, `tempfile` 3 | CLI + TUI |
| IDs / misc | `ulid` 1.1 (`serde`), `chrono` 0.4, `once_cell` 1.20, `regex` 1, `sha2` 0.10, `dirs` 5, `hex` 0.4, `flate2` 1 | SeeleId, hashing, gzip sync |
| Test | `assert_cmd` 2, `predicates` 3, `proptest` 1, `insta` 1 | E2E / property / snapshot tests |

The inline comment at `Cargo.toml:51-53` explains the most fragile pin: `ort` is pinned exactly because no 2.0.0 stable exists as of 2026-05, and Dependabot will open a PR when stable lands — see §18.7. `rusqlite` uses `bundled` (compiles its own SQLite, no system dependency) plus `load_extension` (required to load the vendored vec0 — see §18.6).

#### Release profile

```toml
# Cargo.toml:94-98
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
```

`opt-level = 3` maximizes runtime speed (matters for ONNX inference and FTS/vector queries); `lto = "thin"` enables thin link-time optimization across crates (a good size/perf trade-off vs `lto = "fat"`, which would lengthen the already-heavy ONNX+SQLite link); `codegen-units = 1` forces a single codegen unit per crate so LTO sees the whole picture (slower compile, smaller/faster binary); `strip = true` removes symbols, shrinking the shipped binary. This profile is what `cargo build --release -p seele-cli` (and the release workflow) produce. The local `*.pdb` files are gitignored (`.gitignore:4`).

### 18.2 Toolchain pin and rustfmt

`rust-toolchain.toml` (`C:/dev/tools/SEELE/rust-toolchain.toml`) pins the toolchain via rustup auto-selection:

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

It pins the *channel* to `stable` (not a frozen version like `1.85.0`); the MSRV floor of 1.85 is asserted separately by `rust-version = "1.85"` in `[workspace.package]`, which `cargo` enforces at build time. `components` guarantees rustfmt and clippy are present for the lint job; `profile = "minimal"` keeps installs small (no docs/rust-src). The MSRV was bumped 1.83 → 1.85 to allow `clap_lex` (a transitive dep) compiled with `edition2024` (CHANGELOG.md:348-351).

`rustfmt.toml` (`C:/dev/tools/SEELE/rustfmt.toml`) is intentionally minimal — a single line `edition = "2021"`. The project relies on rustfmt defaults and enforces them with `cargo fmt --all -- --check` in CI; there are no custom style overrides to drift.

### 18.3 Cargo config and the MSVC CRT workaround

`.cargo/config.toml` (`C:/dev/tools/SEELE/.cargo/config.toml`) exists solely to solve a Windows link-time mismatch. The `ort` 2.x prebuilt binaries link against the dynamic CRT (`/MD`), while the C++ pulled in transitively by `tokenizers` (`esaxx-rs`) defaults to the static CRT (`/MT`); mixing them produces `LNK2038`/`LNK2005` errors when the test binary links. The fix forces dynamic CRT for both Rust and `cc`-built C/C++ on the MSVC target:

```toml
# .cargo/config.toml:5-14
[target.x86_64-pc-windows-msvc]
rustflags = [
    "-C", "target-feature=-crt-static",
    "-C", "link-arg=/NODEFAULTLIB:libcmt.lib",
    "-C", "link-arg=/NODEFAULTLIB:libcpmt.lib",
]
[env]
CFLAGS_x86_64_pc_windows_msvc = "/MD"
CXXFLAGS_x86_64_pc_windows_msvc = "/MD"
```

`-crt-static` selects the DLL CRT for Rust code; the two `/NODEFAULTLIB` link-args exclude the static-CRT libraries so the linker doesn't pull in conflicting symbols; the `CFLAGS`/`CXXFLAGS` env vars push `/MD` into the `cc` compilations. This file only affects the Windows MSVC target — Linux and macOS builds ignore it. It is a real gotcha: anyone changing `ort`, `tokenizers`, or the Windows toolchain must keep these aligned or the Windows CI leg fails to link.

### 18.4 CI workflow (`ci.yml`)

`.github/workflows/ci.yml` runs on `push` to `main` and on every `pull_request`. It defines four independent jobs:

| Job | Runner(s) | What it does |
| --- | --- | --- |
| `test` | matrix `ubuntu-latest`, `macos-latest`, `windows-latest` (`fail-fast: false`) | `cargo build --workspace --all-features` then `cargo test --workspace --all-features` |
| `lint` | `ubuntu-latest` | `cargo clippy --workspace --all-features -- -D warnings` then `cargo fmt --all -- --check` |
| `static-checks-bash` | `ubuntu-latest` | `bash scripts/check-no-stele-residual.sh` |
| `static-checks-pwsh` | `windows-latest` | `pwsh -File scripts/check-no-stele-residual.ps1` |

Every job starts with `actions/checkout@v4`. The `test` and `lint` jobs additionally use `dtolnay/rust-toolchain@stable` and `Swatinem/rust-cache@v2` (incremental caching); the lint job's toolchain step explicitly requests `with: components: clippy, rustfmt` (ci.yml:28-29). The two `static-checks-*` jobs do **not** install a Rust toolchain or the cache — they only check out and run the script (the residual checks are pure text greps, no compilation needed). `fail-fast: false` on the test matrix means a failure on one OS does not cancel the others — useful given the Windows-specific CRT issues. Clippy runs with `-D warnings`, so any lint is a hard build break — this is the documented closure criterion (CLAUDE.md). Note the CI clippy invocation is `--all-features` (ci.yml:32), whereas the CLAUDE.md "comandos" snippet shows `--all-targets`; the workflow is the source of truth for what actually gates merges. The STELE residual check runs on *both* bash and pwsh because the two scripts are siblings that must stay in sync; the bash script's comment (check-no-stele-residual.sh:10-15) records a real incident where the bash variant silently missed residuals the pwsh job caught (a prior basename-based `--exclude` bug), after which both were path-synchronized. Note the `--ignored` ONNX-download tests are *not* exercised specially in CI — the CI `cargo test --workspace --all-features` does not pass `-- --ignored`, so the four ignored tests (2 ONNX-download requiring ~90 MB models + 2 perf smokes) are skipped; the ONNX ones are run manually with `cargo test -p seele-embedder -- --ignored`.

### 18.5 Release workflow (`release.yml`)

`.github/workflows/release.yml` triggers on two events: a tag push matching `v*.*.*` (full release), or a manual `workflow_dispatch` carrying an optional `crates_io_publish` input (default `'false'`). The header comment (lines 5-10) records the rationale: per Sprint-05 plan §C and Risk #6, the full pipeline is validated on a release candidate (`v0.1.0-rc.N`) before tagging the real `v0.1.0`, and crates.io publishing is OFF by default and requires explicit opt-in. The workflow holds `permissions: contents: write` (needed to create a GitHub Release). It has three jobs:

**`build`** — a 5-target matrix (`fail-fast: false`) producing prebuilt binaries:

| Target triple | Runner | Cross? | Archive |
| --- | --- | --- | --- |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | no | tar.gz |
| `aarch64-unknown-linux-gnu` | `ubuntu-latest` | yes (`cross`) | tar.gz |
| `x86_64-apple-darwin` | `macos-13` (last Intel runner) | no | tar.gz |
| `aarch64-apple-darwin` | `macos-latest` | no | tar.gz |
| `x86_64-pc-windows-msvc` | `windows-latest` | no | zip |

Each leg installs the toolchain with the matrix `target`, caches with a per-target key, and builds `cargo build --release --target <triple> -p seele-cli` — except the `aarch64-linux` leg, which installs `cargo install cross --locked` and runs `cross build` (gated by `if: matrix.cross`). The `Package binary` step (a bash script even on Windows via `shell: bash`) derives `version="${GITHUB_REF_NAME#v}"`, names the asset `seele-${version}-${target}` following the predictable `<name>-<version>-<target>.<ext>` convention so the installers can compute the download URL from a tag alone, archives the binary (`7z` zip on Windows, `tar -czf` elsewhere), and writes a `.sha256` sidecar using the platform tool (`shasum -a 256` on macOS, PowerShell `Get-FileHash` on Windows, `sha256sum` on Linux). Both the archive and its sidecar are uploaded via `actions/upload-artifact@v4` with `retention-days: 7`. The binary inside is named `seele`/`seele.exe`, matching the `[[bin]] name = "seele"` in `seele-cli/Cargo.toml`.

**`release`** — `needs: build`, runs on `ubuntu-latest`, gated `if: startsWith(github.ref, 'refs/tags/v')` so `workflow_dispatch` runs are dry exercises of the matrix only. It downloads all artifacts (`merge-multiple: true`), computes a prerelease flag (`true` when the tag contains a hyphen, e.g. `v0.1.0-rc.1`), then creates the release with `softprops/action-gh-release@v2`, uploading every `*.tar.gz`, `*.zip`, and `*.sha256`, with `generate_release_notes: true` (auto-notes, with the option of CHANGELOG-sourced body).

**`crates-io`** — `needs: build`, runs on every invocation a `cargo publish --dry-run --allow-dirty` for each publishable crate in strict dependency order, looping because `cargo publish` does not support `--workspace` (rust-lang/cargo#10948 is cited inline). The loop covers exactly **12** crates in this order: `seele-core`, `seele-storage`, `seele-embedder`, `seele-search`, `seele-sync`, `seele-engram-import`, `seele-project`, `seele-setup`, `seele-http`, `seele-mcp`, `seele-tui`, `seele-cli` (release.yml:171-173 and 185-187). Note this is one fewer than the 13 workspace members: `seele-chat` is **not** in the publish list (it is built and tested by CI but excluded from the crates.io loop). The inline comment claims `seele-cli` is last "because everything else depends-on it transitively," which is phrased backwards (it is `seele-cli` that depends on the rest); the operative fact is that leaf crates publish before the crates that depend on them. The actual publish step is gated `if: github.event_name == 'workflow_dispatch' && github.event.inputs.crates_io_publish == 'true'`, uses `CARGO_REGISTRY_TOKEN` from secrets, and republishes in the same 12-crate order; a second publish of an already-published version is a hard crates.io error, which the comment notes is the intended idempotency guard for re-runs.

### 18.6 The two critical version pins

**`sqlite-vec` v0.1.9 (vendored, not a crate).** The vec0 loadable extension is *not* consumed as a Rust crate; precompiled upstream binaries live in `crates/seele-storage/vendor/sqlite-vec/` and are embedded into `seele-storage` via `include_bytes!`. `vec0_loader.rs` selects the right blob with `#[cfg(all(target_os=..., target_arch=...))]` for the five supported targets (e.g. `crates/seele-storage/src/vec0_loader.rs:11-12` for linux-x86_64) and exposes `pub fn vec0_bytes() -> Option<&'static [u8]>`; unsupported targets get `None`, which the README says should surface as a clear startup error. Embedding (rather than a `build.rs` download) keeps `cargo install seele` working offline with no runtime fetch, at the documented cost of ~880 KB added to the crate (ADR-11). The vendor README (`crates/seele-storage/vendor/sqlite-vec/README.md`) carries the authoritative bump procedure: bump the version in the README + CHANGELOG, download the upstream `loadable-{target}.tar.gz` archives, extract each `vec0.{so|dylib|dll}` into its subdir, replace `CHECKSUMS-upstream.txt`, run `cargo test -p seele-storage` to confirm the load contract, and commit `vendor: bump sqlite-vec to vX.Y.Z`. `CHECKSUMS-upstream.txt` is the upstream `checksums.txt` for v0.1.9, allowing manual re-verification of the embedded bytes. The vendored extension is dual-licensed Apache-2.0 OR MIT (copyright 2024 Alex Garcia), so both license texts are kept alongside (`LICENSE-APACHE-upstream.txt`, `LICENSE-MIT-upstream.txt`). CLAUDE.md explicitly forbids touching the vendored binaries without updating that README.

**`ort =2.0.0-rc.10` (exact pin).** The embedder's ONNX runtime is pinned exactly because 2.0.0 stable does not exist as of 2026-05. `Cargo.lock` confirms the resolved version (`Cargo.lock:1758-1759`, `name = "ort"`, `version = "2.0.0-rc.10"`). The documented bump procedure is *do not bump manually* — wait for Dependabot to open a PR when upstream releases stable, then unpin and add a CHANGELOG "Pinned" entry (Cargo.toml:51-53, dependabot.yml:20-22, CLAUDE.md "No hacer"). This pin is what forces the MSVC CRT workaround in §18.3.

### 18.7 Dependabot (`dependabot.yml`)

`.github/dependabot.yml` (version 2) configures two ecosystems. The `cargo` updater (root directory) runs on a **monthly** schedule (Monday 09:00 `America/Santiago`) with `open-pull-requests-limit: 5`, commit prefix `deps` (scope included), and labels `dependencies`/`rust`. The monthly cadence is deliberate: it avoids weekly minor-bump noise while real CVEs still arrive via security advisories regardless of interval (inline comment, lines 6-9). Crucially, the comment at lines 20-22 records that `ort`'s pin is *not* ignored — the team explicitly wants the bump PR when 2.0.0 stable lands. The `github-actions` updater runs monthly too, limit 3, prefix `ci`, labels `dependencies`/`github-actions`, keeping the action versions in the three workflows current.

### 18.8 Web deploy workflow (`deploy-web.yml`)

`.github/workflows/deploy-web.yml` publishes the Astro landing site (covered in §17) to GitHub Pages. It triggers on `push` to `main` filtered to `web/**` and the workflow file itself, plus `workflow_dispatch`. Permissions are `contents: read`, `pages: write`, `id-token: write` (OIDC for the Pages deploy), and a `concurrency: group: pages` with `cancel-in-progress: false` serializes deploys. The `build` job (`ubuntu-latest`) checks out, sets up Node 20 with npm cache keyed on `web/package-lock.json`, runs `actions/configure-pages@v5` (which flips the site to `build_type: workflow` — the comment notes this is required or `deploy-pages` fails against a branch-source site), then `npm ci` + `npm run build` in `web/`, and uploads `web/dist` via `actions/upload-pages-artifact@v3`. The `deploy` job (`needs: build`) binds the `github-pages` environment and runs `actions/deploy-pages@v4`. This pipeline is independent of the Rust release pipeline; `web/` build artifacts (`node_modules`, `dist`, `.astro`, `.cache`) are gitignored (`.gitignore:46-51`).

### 18.9 Install scripts

Two mirrored installers in `scripts/` provide the `curl | bash` and `irm | iex` one-liners, both verifying SHA256 before installing.

`scripts/install.sh` (Unix) honors `$SEELE_VERSION` (or `$1`; defaults to latest stable via the GitHub releases API) and `$SEELE_INSTALL_DIR` (default `$HOME/.local/bin`). It runs `set -euo pipefail`, detects OS/arch (`uname -s`/`uname -m`) and maps to a target triple (`unknown-linux-gnu`/`apple-darwin` × `x86_64`/`aarch64`), dying with a clear message on unsupported platforms. It resolves the latest tag by grepping `tag_name` from the API, strips the leading `v` for the asset name (`seele-${version_bare}-${target}.tar.gz`), downloads the archive and `.sha256` sidecar into a `mktemp -d` (cleaned via `trap`), and verifies: prefer `sha256sum -c`, fall back to comparing `shasum -a 256` output, and `die` if neither tool exists — it refuses to install without verification. On success it extracts, moves `seele` into the install dir, `chmod +x`, warns if the dir is not on `$PATH`, and runs `seele --version`.

`scripts/install.ps1` (Windows) mirrors this with `$ErrorActionPreference = 'Stop'`, honoring `$env:SEELE_VERSION` and `$env:SEELE_INSTALL_DIR` (default `$env:USERPROFILE\.seele\bin`). It rejects any arch other than `X64` (v0.1 ships Windows x86_64 only), resolves the latest tag via `Invoke-RestMethod`, downloads the `.zip` + `.sha256`, parses the expected hash robustly with `-split '\s+'` (the sidecar's whitespace differs by which builder wrote it), compares against `Get-FileHash -Algorithm SHA256`, and `Write-Error`s on mismatch. It then `Expand-Archive`s, moves `seele.exe` into the install dir, hints how to add it to the user PATH, and runs `seele.exe --version`. Both scripts depend entirely on the release workflow's asset-naming and sidecar contract (§18.5); a change to either side breaks installs silently except for the version line.

### 18.10 STELE residual checks and the v0.1.0 smoke

`scripts/check-no-stele-residual.{sh,ps1}` enforce that the legacy project name "STELE"/"stele" never reappears in source. Each greps the repo (over `*.md,*.rs,*.toml,*.yaml,*.yml,*.json,*.sh,*.ps1`, excluding `target`/`.git`/`node_modules`) for the word-boundary pattern `\b(STELE|stele)\b` and reports any hit outside a hand-maintained allowlist. The allowlist has two parts that must stay identical between the two scripts: `ALLOWLIST_FILES`/`$allowlistFiles` (exact relative paths — historical docs, the CI YAML, the two scripts themselves, CHANGELOG/CLAUDE/INDEX) and `ALLOWLIST_DIRS`/`$allowlistDirs` (path prefixes — devlogs and táctica plan dirs that legitimately reference the old name). The bash script's comments flag two portability gotchas: `\b` is not portable to busybox grep (so an alpine CI runner would need a different pattern — `check-no-stele-residual.sh:16-23`), and the previous basename-based exclusion bug was fixed after the pwsh job caught residuals the bash job silently passed. Both exit 1 on any violation, which fails the corresponding CI job.

`scripts/v0.1.0-smoke.sh` is the v0.1.0 acceptance harness — an idempotent script that builds the release binary into a tempdir and exercises eleven criteria against ephemeral DBs with `SEELE_FAKE_EMBEDDER=1` (so it never depends on the ONNX download): (1) `cargo build --release -p seele-cli`; (2) a full CLI round-trip save→list→show→search→delete→restore→link→stats→projects→doctor with JSON assertions; (3) `seele mcp` `tools/list` returns exactly 19 tools; (4) `seele serve` `/openapi.json` exposes ≥15 paths (the comment at lines 99-103 records the plan's "25+" as an early overestimate — v0.1 ships 18 unique URLs / 22 operations — and relaxes the threshold to avoid chasing it); (5) a gated, slow perf smoke (`SEELE_SMOKE_PERF=1`); (6) `seele-project` `detect_e2e`; (7) sync export→import round-trip; (8) `seele tui --smoke` one-frame render; (9)–(10) CI matrix green and `release.yml` firing, both verified out-of-band; (11) ENGRAM credit present in README + CREDITS. Criterion 6's comment (lines 116-122) documents a known gap carried into v0.2: the CLI does not yet auto-call `seele_project::detect` from `seele save` (the `commands/save.rs` doc string references "Sprint-04 Bloque D.2" wiring that did not ship).

### 18.11 File-by-file map

| File | Role |
| --- | --- |
| `Cargo.toml` | virtual workspace, shared package metadata, `workspace.dependencies`, release profile |
| `Cargo.lock` | resolved dependency graph; confirms `ort` 2.0.0-rc.10 and checksums |
| `rust-toolchain.toml` | pins channel `stable` + rustfmt/clippy, minimal profile |
| `rustfmt.toml` | single line `edition = "2021"`; relies on rustfmt defaults |
| `.cargo/config.toml` | MSVC dynamic-CRT workaround for the ort/tokenizers link mismatch |
| `.github/workflows/ci.yml` | test matrix (3 OS) + clippy/fmt + bash & pwsh STELE checks |
| `.github/workflows/release.yml` | 5-target prebuilt binaries + SHA256 sidecars + GH Release + gated crates.io |
| `.github/workflows/deploy-web.yml` | Astro `web/` build + GitHub Pages deploy |
| `.github/dependabot.yml` | monthly cargo + github-actions updates; documents the ort pin watch |
| `scripts/install.sh` | Unix curl\|bash installer with SHA256 verify |
| `scripts/install.ps1` | Windows irm\|iex installer with SHA256 verify |
| `scripts/check-no-stele-residual.sh` | bash static check for legacy-name residuals |
| `scripts/check-no-stele-residual.ps1` | pwsh sibling of the residual check |
| `scripts/v0.1.0-smoke.sh` | 11-criterion acceptance smoke against the release binary |
| `.gitignore` | excludes `target/`, `*.pdb`, secrets, `*.db`, embedder cache, web build artifacts |
| `crates/seele-storage/vendor/sqlite-vec/README.md` | vec0 vendoring rationale + bump procedure |
| `crates/seele-storage/vendor/sqlite-vec/CHECKSUMS-upstream.txt` | upstream v0.1.9 checksums for re-verification |

### 18.12 Connections, gotchas, and known limitations

The build subsystem ties the whole codebase together: the release profile and toolchain apply to all 13 crates; `seele-cli` is the only `[[bin]]` (`name = "seele"`) and the artifact every installer fetches; the vendored vec0 binaries flow into `seele-storage` and therefore into the final executable; the `ort` pin governs `seele-embedder` and forces the Windows CRT config. The asset-naming contract (`seele-<version>-<target>.<ext>` + `.sha256`) is shared, untyped glue between `release.yml` and both installers — a divergence breaks installs silently. Notable invariants and gotchas: the two STELE-residual allowlists must remain byte-identical between the bash and pwsh scripts (a desync caused a real escaped-residual incident); the bash `\b` pattern is non-portable to busybox grep if an alpine runner is ever added; `ort` must never be unpinned manually (Dependabot owns that bump); and the vendored vec0 binaries must never be replaced without updating the vendor README. The one documented functional gap surfaced here is criterion 6 of the smoke: project detection is not wired into `seele save`, deferred to v0.2.
