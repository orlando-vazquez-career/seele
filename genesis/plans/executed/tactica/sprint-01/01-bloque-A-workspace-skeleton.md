# Sprint-01 Bloque A — Cargo workspace skeleton

**Tema**: Crear el workspace Cargo con los 11 crates stub + toolchain pinning + CI básico de GitHub Actions.

**Pre-requisitos**: ninguno (repo ya inicializado y pusheado en bloque genesis).

## Archivos a crear

### `rust-toolchain.toml`

```toml
[toolchain]
channel = "1.83"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

### `Cargo.toml` (workspace root)

Dependencias workspace centralizadas. Detalles en ADR-01 + ADR-08.

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
    "crates/seele-sync",
    "crates/seele-setup",
    "crates/seele-project",
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
# Async runtime
tokio = { version = "1.42", features = ["full"] }
futures = "0.3"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Errors
thiserror = "2"
anyhow = "1"

# Tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Storage
rusqlite = { version = "0.32", features = ["bundled", "load_extension"] }
r2d2 = "0.8"
r2d2_sqlite = "0.25"
refinery = { version = "0.8", features = ["rusqlite"] }

# Embedder
ort = "2"
tokenizers = "0.20"
hf-hub = "0.3"

# HTTP
axum = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace", "compression-gzip"] }
utoipa = { version = "5", features = ["axum_extras"] }
utoipa-swagger-ui = { version = "8", features = ["axum"] }

# CLI
clap = { version = "4.5", features = ["derive", "env"] }

# TUI
ratatui = "0.29"
crossterm = "0.28"
tempfile = "3"

# IDs
ulid = "1.1"

# Misc
chrono = { version = "0.4", features = ["serde"] }
once_cell = "1.20"
regex = "1"
sha2 = "0.10"

# Test deps
assert_cmd = "2"
predicates = "3"
proptest = "1"
insta = "1"

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
```

### Estructura por crate (11 crates)

Cada uno arranca con:

```
crates/<name>/
├── Cargo.toml
└── src/
    └── lib.rs    # (o main.rs en seele-cli)
```

#### `crates/seele-core/Cargo.toml`

```toml
[package]
name = "seele-core"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
description = "Core types, errors, and traits shared across SEELE crates."

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
ulid.workspace = true
chrono.workspace = true

[dev-dependencies]
proptest.workspace = true
```

#### `crates/seele-core/src/lib.rs` (stub)

```rust
//! SEELE core types, errors, and shared traits.
//!
//! See ADR-02 (schema-sqlite) and ADR-10 (mapping-mnema-seele) for the
//! data model rationale.

pub mod error;
pub mod id;
pub mod memory;
// pub mod link;
// pub mod filter;

pub use error::SeeleError;
pub use id::SeeleId;
```

(Modules como stubs en bloque A, implementación en bloque B.)

#### `crates/seele-storage/Cargo.toml`

```toml
[package]
name = "seele-storage"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
description = "SQLite + FTS5 + sqlite-vec storage layer for SEELE."

[dependencies]
seele-core = { path = "../seele-core" }
rusqlite.workspace = true
r2d2.workspace = true
r2d2_sqlite.workspace = true
refinery.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true
chrono.workspace = true
ulid.workspace = true
regex.workspace = true
sha2.workspace = true
tokio = { workspace = true, features = ["sync", "rt"] }

[dev-dependencies]
tempfile.workspace = true
proptest.workspace = true
```

#### `crates/seele-embedder/Cargo.toml`

```toml
[package]
name = "seele-embedder"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
description = "ONNX-based local embedder (default: all-MiniLM-L6-v2)."

[dependencies]
seele-core = { path = "../seele-core" }
ort.workspace = true
tokenizers.workspace = true
hf-hub.workspace = true
thiserror.workspace = true
tracing.workspace = true
once_cell.workspace = true
serde.workspace = true
serde_json.workspace = true
```

#### `crates/seele-search/Cargo.toml`

```toml
[package]
name = "seele-search"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
description = "Hybrid FTS + vector search with Reciprocal Rank Fusion."

[dependencies]
seele-core = { path = "../seele-core" }
seele-storage = { path = "../seele-storage" }
seele-embedder = { path = "../seele-embedder" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true
```

#### `crates/seele-mcp/Cargo.toml`

```toml
[package]
name = "seele-mcp"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
description = "MCP (Model Context Protocol) server over stdio."

[dependencies]
seele-core = { path = "../seele-core" }
seele-storage = { path = "../seele-storage" }
seele-search = { path = "../seele-search" }
seele-embedder = { path = "../seele-embedder" }
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true
```

#### `crates/seele-http/Cargo.toml`

```toml
[package]
name = "seele-http"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
description = "HTTP REST API for SEELE (axum)."

[dependencies]
seele-core = { path = "../seele-core" }
seele-storage = { path = "../seele-storage" }
seele-search = { path = "../seele-search" }
seele-embedder = { path = "../seele-embedder" }
tokio.workspace = true
axum.workspace = true
tower.workspace = true
tower-http.workspace = true
utoipa.workspace = true
utoipa-swagger-ui.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true
```

#### `crates/seele-tui/Cargo.toml`

```toml
[package]
name = "seele-tui"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
description = "Terminal UI for SEELE (ratatui + crossterm)."

[dependencies]
seele-core = { path = "../seele-core" }
seele-storage = { path = "../seele-storage" }
seele-search = { path = "../seele-search" }
ratatui.workspace = true
crossterm.workspace = true
tokio.workspace = true
tempfile.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true
```

#### `crates/seele-sync/Cargo.toml`

```toml
[package]
name = "seele-sync"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
description = "Git-friendly compressed-chunk sync between machines."

[dependencies]
seele-core = { path = "../seele-core" }
seele-storage = { path = "../seele-storage" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true
sha2.workspace = true
```

#### `crates/seele-setup/Cargo.toml`

```toml
[package]
name = "seele-setup"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
description = "Setup wizard for AI agent integrations (8 agents)."

[dependencies]
seele-core = { path = "../seele-core" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true
chrono.workspace = true
```

#### `crates/seele-project/Cargo.toml`

```toml
[package]
name = "seele-project"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
description = "5-case project detection algorithm."

[dependencies]
seele-core = { path = "../seele-core" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true
```

#### `crates/seele-cli/Cargo.toml`

```toml
[package]
name = "seele-cli"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
description = "CLI binary for SEELE."

[[bin]]
name = "seele"
path = "src/main.rs"

[dependencies]
seele-core = { path = "../seele-core" }
seele-storage = { path = "../seele-storage" }
seele-embedder = { path = "../seele-embedder" }
seele-search = { path = "../seele-search" }
seele-mcp = { path = "../seele-mcp" }
seele-http = { path = "../seele-http" }
seele-tui = { path = "../seele-tui" }
seele-sync = { path = "../seele-sync" }
seele-setup = { path = "../seele-setup" }
seele-project = { path = "../seele-project" }
clap.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
anyhow.workspace = true
```

### Stub `lib.rs` por crate (excepto core que se hace en bloque B)

```rust
//! <crate name> — see crates/<name>/README.md for details.
```

(Una línea de doc-comment. La implementación llega en bloque correspondiente.)

### `crates/seele-cli/src/main.rs` (stub mínimo)

```rust
fn main() -> anyhow::Result<()> {
    println!("seele v{} — see `seele --help` once implemented", env!("CARGO_PKG_VERSION"));
    Ok(())
}
```

### `.github/workflows/ci.yml` (matriz mínima)

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:

jobs:
  test:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Build
        run: cargo build --workspace --all-features
      - name: Test
        run: cargo test --workspace --all-features
      - name: Clippy
        run: cargo clippy --workspace --all-features -- -D warnings
      - name: Fmt
        run: cargo fmt --all -- --check
      - name: STELE residual check (Linux)
        if: runner.os == 'Linux'
        run: bash scripts/check-no-stele-residual.sh
      - name: STELE residual check (Windows)
        if: runner.os == 'Windows'
        run: powershell -File scripts/check-no-stele-residual.ps1
```

### `rustfmt.toml`

```toml
edition = "2021"
imports_granularity = "Crate"
group_imports = "StdExternalCrate"
```

## Criterios de aceptación del bloque A

1. `cargo build --workspace` compila en local sin warnings.
2. `cargo check --workspace` < 30s en local.
3. Los 11 crates tienen `Cargo.toml` válido + `lib.rs`/`main.rs` stub.
4. `cargo run --bin seele -- --version` (cuando lleguemos a bloque CLI) imprime "seele 0.1.0".
5. CI workflow `ci.yml` pusheado y triggered al menos una vez (al hacer push del bloque).
6. Static check STELE residual verde post-edits.

## Tests del bloque A

- ~~Ningún test funcional, es scaffolding.~~ Aclaración: hay un test trivial en cada crate stub (`#[test] fn smoke() { assert!(true); }`) para verificar que `cargo test` compila los 11 crates. Esto detecta dependency conflicts temprano.

## Salida del bloque A

```
SEELE/
├── .github/workflows/ci.yml          ← nuevo
├── rust-toolchain.toml               ← nuevo
├── rustfmt.toml                      ← nuevo
├── Cargo.toml                        ← nuevo (workspace)
├── crates/
│   ├── seele-core/Cargo.toml + src/lib.rs (stub con módulos)
│   ├── seele-storage/Cargo.toml + src/lib.rs
│   ├── seele-embedder/Cargo.toml + src/lib.rs
│   ├── seele-search/Cargo.toml + src/lib.rs
│   ├── seele-mcp/Cargo.toml + src/lib.rs
│   ├── seele-http/Cargo.toml + src/lib.rs
│   ├── seele-tui/Cargo.toml + src/lib.rs
│   ├── seele-sync/Cargo.toml + src/lib.rs
│   ├── seele-setup/Cargo.toml + src/lib.rs
│   ├── seele-project/Cargo.toml + src/lib.rs
│   └── seele-cli/Cargo.toml + src/main.rs
└── (.gitignore + LICENSE + README + CREDITS + scripts/* ya existentes)
```

## Commit del bloque A

```
git add -A
git commit -m "sprint-01 bloque-A — Cargo workspace skeleton (11 crates)"
git push
```
