# Installation

SEELE ships as a single binary `seele`. Pick one of the three paths
below; all leave the same artifact on disk.

## Path 1 — install scripts (recommended)

Pre-built binaries are published to GitHub Releases for every tag, with
a `.sha256` sidecar that the install scripts verify before copying the
binary into place.

### Linux / macOS

```bash
curl -fsSL https://raw.githubusercontent.com/orlando-vazquez-career/seele/main/scripts/install.sh | bash
```

To pin a specific version or override the install dir:

```bash
SEELE_VERSION=v0.1.0 SEELE_INSTALL_DIR="$HOME/bin" \
  curl -fsSL https://raw.githubusercontent.com/orlando-vazquez-career/seele/main/scripts/install.sh | bash
```

Default install dir: `~/.local/bin`. The script prints a hint if the
directory is not on your `$PATH`.

### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/orlando-vazquez-career/seele/main/scripts/install.ps1 | iex
```

Pin a version or override the install dir:

```powershell
$env:SEELE_VERSION = 'v0.1.0'
$env:SEELE_INSTALL_DIR = "$env:USERPROFILE\bin"
irm https://raw.githubusercontent.com/orlando-vazquez-career/seele/main/scripts/install.ps1 | iex
```

Default install dir: `%USERPROFILE%\.seele\bin`.

## Path 2 — `cargo install`

Requires a Rust toolchain (1.85 or newer; see [`rust-toolchain.toml`](../rust-toolchain.toml)).

```bash
cargo install --git https://github.com/orlando-vazquez-career/seele.git --locked seele-cli
```

`seele-cli` is the binary crate; the installed executable is named
`seele`. The first build pulls and compiles ~600 crates (~3-6 minutes on
a modern laptop).

## Path 3 — build from source

```bash
git clone https://github.com/orlando-vazquez-career/seele.git
cd seele
cargo build --release -p seele-cli
# binary lands in target/release/seele
```

Useful when you want to develop against SEELE itself or work from a
pinned commit.

## After install — embedder cache

On first `seele save` or `seele search`, the ONNX embedder downloads
`sentence-transformers/all-MiniLM-L6-v2` (~30 MB quantized, ~90 MB full
precision) from Hugging Face into:

- **Linux**: `~/.cache/seele/embedder/`
- **macOS**: `~/Library/Caches/seele/embedder/`
- **Windows**: `%LOCALAPPDATA%\seele\embedder\`

Override with the `SEELE_EMBEDDER_DIR` environment variable. The cache
is shared across SEELE invocations; subsequent runs load the model in
~150-300 ms.

## Disabling ONNX (CI, air-gapped, debugging)

`seele` falls back to a deterministic `FakeEmbedder` automatically if
ONNX init fails (no network, model download blocked, etc) — the CLI
still works, but vector-search quality is degraded to hash-based.

To force `FakeEmbedder` and silence the fallback warning:

- Per command: `seele --fake-embedder save …`
- Process-wide: `export SEELE_FAKE_EMBEDDER=1`

## Database location

`seele` writes to `~/.seele/seele.db` by default. Override with `--db
<path>` (per command) or by passing the path to `seele serve --db` /
`seele mcp --db`. The path's parent directory is created on first run.

## Verifying the install

```bash
seele --version       # prints `seele <crate-version>`
seele doctor          # prints DB + embedder + schema health
seele --help          # lists 17 subcommands
```

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `sha256 mismatch` from install script | Corrupted download or asset tampering | Re-run the script; if it persists, file an issue with the SHA mismatch. |
| `ONNX embedder unavailable` warning | First run with no network, or HF blocked | Re-run when online (cache fills), or set `SEELE_FAKE_EMBEDDER=1` permanently. |
| `Could not resolve cache dir` | `SEELE_EMBEDDER_DIR` is empty or invalid | Unset it or point at a writable path. |
| Slow first run (~5-10 s of pause before saving) | Model download in flight | Expected one-time cost. Subsequent runs are fast. |
| `seele: command not found` | Install dir not on `$PATH` | Add the install dir to your shell's PATH (the install scripts print the exact line to copy). |

See [`docs/AGENT-SETUP.md`](AGENT-SETUP.md) for MCP wiring into Claude
Code / Cursor / Windsurf, and
[`docs/ENGRAM-MIGRATION.md`](ENGRAM-MIGRATION.md) for importing an
existing ENGRAM database.
