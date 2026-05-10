# ADR-08 — Repo layout (Cargo workspace, 8 crates, CI matriz)

**Estado**: Aceptado · 2026-05-09
**Decisión**: Cargo workspace con 8 crates internos. Dependencias jerárquicas claras. CI matriz Linux + macOS + Windows desde el día uno.

## Estructura del repositorio

```
seele/
├── Cargo.toml                          # workspace manifest
├── Cargo.lock
├── rust-toolchain.toml                 # MSRV pinning
├── .github/
│   └── workflows/
│       ├── ci.yml                      # build + test matrix
│       └── release.yml                 # binary releases tag-driven
├── README.md
├── LICENSE                             # MIT, copyright DevZen SpA
├── CREDITS.md                          # crédito a Gentleman-Programming/engram
├── CHANGELOG.md                        # keep-a-changelog
├── CONTRIBUTING.md
├── docs/
│   ├── aegis/                          # planes y devlogs bajo AEGIS
│   ├── design/                         # planes y devlogs bajo LUMEN (TUI/CLI UX)
│   └── api/                            # OpenAPI generado + handcrafted guides
├── crates/
│   ├── seele-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── id.rs                   # ULID generator
│   │       ├── memory.rs               # Memory struct
│   │       ├── link.rs                 # Link struct
│   │       ├── error.rs                # SteleError enum
│   │       └── filter.rs               # MetadataFilter
│   ├── seele-storage/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── connection.rs           # r2d2 pool
│   │       ├── migrations.rs           # refinery
│   │       ├── memories.rs             # CRUD
│   │       ├── links.rs
│   │       └── schema_version.rs
│   ├── seele-embedder/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── ort_session.rs
│   │       ├── tokenizer.rs
│   │       ├── pooling.rs              # mean-pool + L2 norm
│   │       └── download.rs             # hf-hub auto-download
│   ├── seele-search/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── fts.rs
│   │       ├── vec.rs
│   │       └── rrf.rs                  # RRF combiner
│   ├── seele-mcp/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── stdio.rs                # stdio transport
│   │       ├── jsonrpc.rs
│   │       └── tools.rs                # tool definitions
│   ├── seele-http/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── server.rs               # axum app
│   │       ├── handlers/
│   │       │   ├── memories.rs
│   │       │   ├── search.rs
│   │       │   ├── links.rs
│   │       │   ├── stats.rs
│   │       │   └── embedder.rs
│   │       ├── auth.rs                 # bearer middleware
│   │       └── error.rs                # IntoResponse mapping
│   ├── seele-tui/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── app.rs                  # AppState
│   │       ├── views/
│   │       │   ├── home.rs
│   │       │   ├── browse.rs
│   │       │   ├── search.rs
│   │       │   ├── detail.rs
│   │       │   └── help.rs
│   │       ├── keys.rs                 # keybindings
│   │       └── editor.rs               # $EDITOR integration
│   └── seele-cli/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs                 # binary entry
│           └── commands/
│               ├── init.rs
│               ├── save.rs
│               ├── search.rs
│               ├── show.rs
│               ├── list.rs
│               ├── delete.rs
│               ├── serve.rs
│               ├── mcp.rs
│               └── tui.rs
├── tests/                              # workspace integration tests
│   ├── cli_smoke.rs
│   ├── http_smoke.rs
│   ├── mcp_smoke.rs
│   └── fixtures/
│       └── tiny_minilm.onnx            # tiny model for tests
└── examples/
    ├── basic_save_search.rs
    ├── consume_via_http.rs
    └── embed_only.rs
```

## Dependencias entre crates

```
seele-cli
  ├── seele-storage
  ├── seele-embedder
  ├── seele-search
  ├── seele-mcp
  ├── seele-http
  ├── seele-tui
  └── seele-core

seele-mcp ─────► seele-storage, seele-embedder, seele-search, seele-core
seele-http ────► seele-storage, seele-embedder, seele-search, seele-core
seele-tui ─────► seele-storage, seele-embedder, seele-search, seele-core
seele-search ──► seele-storage, seele-embedder, seele-core
seele-storage ─► seele-core
seele-embedder ► seele-core
```

`seele-core` es el bottom — tipos compartidos, errores, sin deps externas pesadas.

## Cargo.toml workspace root

```toml
[workspace]
members = [
    "crates/seele-core",
    "crates/seele-storage",
    "crates/seele-embedder",
    "crates/seele-search",
    "crates/seele-mcp",
    "crates/seele-http",
    "crates/seele-tui",
    "crates/seele-cli",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.83"
authors = ["DevZen SpA"]
license = "MIT"
repository = "https://github.com/orlando-vazquez-career/seele"

[workspace.dependencies]
# tokio + async
tokio = { version = "1.42", features = ["full"] }
futures = "0.3"

# serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# errors
thiserror = "2"
anyhow = "1"

# tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# storage
rusqlite = { version = "0.32", features = ["bundled", "load_extension"] }
r2d2 = "0.8"
r2d2_sqlite = "0.25"
refinery = { version = "0.8", features = ["rusqlite"] }

# embedder
ort = "2"
tokenizers = "0.20"
hf-hub = "0.3"

# http
axum = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace", "compression-gzip"] }
utoipa = { version = "5", features = ["axum_extras"] }
utoipa-swagger-ui = { version = "8", features = ["axum"] }

# cli
clap = { version = "4.5", features = ["derive", "env"] }

# tui
ratatui = "0.29"
crossterm = "0.28"
tempfile = "3"

# id
ulid = "1.1"

# misc
chrono = { version = "0.4", features = ["serde"] }
once_cell = "1.20"

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
```

## CI matrix

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        rust: [stable, beta]
        exclude:
          - { os: windows-latest, rust: beta }   # not critical to dual-coverage
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: ${{ matrix.rust }}
      - uses: Swatinem/rust-cache@v2
      - name: Build
        run: cargo build --workspace --all-features
      - name: Test
        run: cargo test --workspace --all-features
      - name: Clippy
        run: cargo clippy --workspace --all-features -- -D warnings
      - name: Fmt check
        run: cargo fmt --all -- --check
```

`Swatinem/rust-cache` cachea `target/` y registry — builds incrementales en CI.

## Release workflow

Tag-driven (semver tags `v0.1.0`):

```yaml
# .github/workflows/release.yml
name: Release
on:
  push:
    tags: ['v*']
jobs:
  build:
    strategy:
      matrix:
        include:
          - { os: ubuntu-latest,   target: x86_64-unknown-linux-gnu   }
          - { os: ubuntu-latest,   target: x86_64-unknown-linux-musl  }
          - { os: macos-latest,    target: aarch64-apple-darwin       }
          - { os: macos-latest,    target: x86_64-apple-darwin        }
          - { os: windows-latest,  target: x86_64-pc-windows-msvc     }
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - name: Build release
        run: cargo build --release --bin seele --target ${{ matrix.target }}
      - name: Package
        run: |
          mkdir dist
          cp target/${{ matrix.target }}/release/seele${{ matrix.os == 'windows-latest' && '.exe' || '' }} dist/
          cd dist && tar czvf seele-${{ github.ref_name }}-${{ matrix.target }}.tar.gz seele*
      - uses: softprops/action-gh-release@v2
        with:
          files: dist/*.tar.gz
```

## Versioning

Semver desde v0.1.0. Versión bumped vía `cargo workspaces version` o manual edit de `Cargo.toml`.

`CHANGELOG.md` keep-a-changelog format. Cada release tag tiene su entry.

## Tooling local

- **rustfmt**: config default + `imports_granularity = "Crate"`.
- **clippy**: deny-warnings en CI; `clippy::pedantic` opcional via `lints` workspace key.
- **cargo-deny**: licencia + advisory checks en CI semanal.
- **cargo-audit**: vulnerability scan (CI weekly).

## Out of scope

- **`cargo-make` o tasks runners externos**: cargo + Make si hace falta multi-step. Sin justfile/xtask v0.1.
- **Pre-commit hooks**: opcional, documentado pero no enforced.

## Referencias

- Cargo workspaces book.
- dtolnay/rust-toolchain action.
- Swatinem/rust-cache.
- softprops/action-gh-release.
