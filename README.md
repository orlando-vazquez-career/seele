# SEELE

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![Release](https://img.shields.io/github/v/release/orlando-vazquez-career/seele?include_prereleases&label=release)](https://github.com/orlando-vazquez-career/seele/releases)
[![CI](https://github.com/orlando-vazquez-career/seele/actions/workflows/ci.yml/badge.svg)](https://github.com/orlando-vazquez-career/seele/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange?logo=rust)](./rust-toolchain.toml)
[![GitHub Sponsors](https://img.shields.io/github/sponsors/orlando-vazquez-career?label=sponsors)](https://github.com/sponsors/orlando-vazquez-career)

> *Super stellatum firmamentum iudicat Deus, sicut nos iudicamus.*
> Sobre el firmamento estrellado juzga Dios, como nosotros juzgamos.

A local-first memory engine for AI agents — Rust + SQLite + FTS5 +
sqlite-vec + ONNX embeddings + hybrid search via Reciprocal Rank
Fusion. One binary, three transports: CLI, MCP stdio, HTTP REST.

**Status: v0.1.0** — first stable release, shipped under AEGIS over 5
sprints. See [`CHANGELOG.md`](./CHANGELOG.md) for what's in this
version and [`docs/aegis/devlogs/`](./docs/aegis/devlogs/) for the
sprint-by-sprint trail.

---

## Quick start

```bash
# Install (Linux/macOS — see docs/INSTALLATION.md for other paths)
curl -fsSL https://raw.githubusercontent.com/orlando-vazquez-career/seele/main/scripts/install.sh | bash

# Save something
seele save "WAL is faster than DELETE for crash recovery" \
  "Postgres uses WAL by default since 9.1." --project notes

# Search it back (hybrid FTS + vec)
seele search "crash recovery" --project notes

# Wire SEELE into Claude Code (also: cursor, windsurf)
seele setup --agent claude-code

# Or run as MCP server directly
seele mcp
```

## What you get

- **`seele save / search / show / list / delete / restore / link /
  stats / projects / doctor`** — 17-subcommand CLI against a local
  SQLite database. Privacy stripping (`<private>...</private>`),
  topic-key upserts, 24h normalized-hash dedup, soft delete + restore.
  See [`crates/seele-cli/`](./crates/seele-cli/).
- **MCP stdio server** — 19 tools under the `seele_*` namespace (or
  `mnema_*` for ENGRAM/MNEMA compat). One `seele mcp` invocation per
  agent. See [`docs/AGENT-SETUP.md`](./docs/AGENT-SETUP.md).
- **HTTP REST API** — 18 paths / 22 operations + OpenAPI 3.1 + Swagger
  UI at `/docs`. Bearer auth, optional legacy ENGRAM route aliases.
  See [`crates/seele-http/`](./crates/seele-http/).
- **TUI** — `seele tui` opens a ratatui interface with 5 panes
  (Home/Browse/Search/Detail/Stats), vi-style keymap.
- **Multi-machine sync** — `seele sync export → import` ships
  observations between machines as a single git-friendly gzipped JSON
  chunk. Idempotent via SHA-256 chunk id + per-target ledger.
- **ENGRAM migration** — `seele import from-engram <path>` ports an
  existing ENGRAM/MNEMA SQLite DB in one shot, preserving ULIDs and
  mapping `linked_to[]` to first-class links. See
  [`docs/ENGRAM-MIGRATION.md`](./docs/ENGRAM-MIGRATION.md).

## Install

Three paths, pick whichever fits:

- **Install scripts** (recommended): `curl … install.sh | bash` on
  Linux/macOS, `irm … install.ps1 | iex` on Windows. SHA256-verified
  pre-built binary.
- **`cargo install`**: `cargo install --git
  https://github.com/orlando-vazquez-career/seele.git --locked
  seele-cli`. Compiles from source.
- **Build from source**: clone + `cargo build --release -p seele-cli`.

Full matrix + cache locations + ONNX fallback notes in
[`docs/INSTALLATION.md`](./docs/INSTALLATION.md).

## On the name

"Seele" is the German word for "soul" or "spirit". It is a common
word, not a trademark we claim and not a reference to any specific
franchise. We chose it because the metaphor — memory as what
constitutes the soul of an agent — captures what this engine is for.

## Inspiration

SEELE is a clean-room Rust reimplementation inspired by
[ENGRAM](https://github.com/Gentleman-Programming/engram) by
[Gentleman-Programming](https://github.com/Gentleman-Programming).
ENGRAM proved that local-first memory engines for AI agents are
viable and demonstrated many of the patterns SEELE adopts: session
lifecycle, project detection, topic-key upserts, memory relations
with judgment, git sync chunks, privacy stripping,
MCP-as-primary-transport.

ENGRAM is licensed MIT (Copyright Gentleman-Programming). SEELE is
also MIT (Copyright DevZen SpA). Sibling tools in the same niche
with different stacks (Go vs Rust) and a different retrieval bet
(FTS+LLM-judge vs FTS+embeddings+RRF).

If SEELE is useful, please also try ENGRAM — the broader ecosystem
benefits from multiple options. See [`CREDITS.md`](./CREDITS.md) for
full attribution.

## Docs

- [`docs/INSTALLATION.md`](./docs/INSTALLATION.md) — install matrix,
  cache locations, troubleshooting.
- [`docs/AGENT-SETUP.md`](./docs/AGENT-SETUP.md) — wiring SEELE into
  Claude Code, Cursor, Windsurf.
- [`docs/ENGRAM-MIGRATION.md`](./docs/ENGRAM-MIGRATION.md) — porting
  an existing ENGRAM/MNEMA database.
- [`docs/INDEX.md`](./docs/INDEX.md) — full doc map, devlogs, ADRs.
- [`CLAUDE.md`](./CLAUDE.md) — operating rules for Claude Code when
  working in this repo.
- [`genesis/plans/`](./genesis/plans/) — strategy, 13 ADRs, 5 sprint
  plans. Closed plans live in [`executed/`](./genesis/plans/executed/).

## Support / Apoyar

SEELE is built as a labor of love by one freelance developer
([Orlando Nahuel Vazquez Gonzalez](https://github.com/orlando-vazquez-career))
in his spare hours. If it saves you time or you want to see it grow,
here are the ways to help:

### Star + share

The cheapest and most useful: star the repo, share it where AI-agent
folks hang out, open issues with feedback. Visibility brings
contributors and contributors make the tool better for everyone.

### GitHub Sponsors

The "♥ Sponsor" button at the top of the repo accepts one-time or
monthly contributions via card (Stripe-backed). No fees taken by
GitHub during the matching-fund period — your $1 lands as $1
(plus whatever GitHub matches at the time).

→ [github.com/sponsors/orlando-vazquez-career](https://github.com/sponsors/orlando-vazquez-career)

### Crypto

Wallet-to-wallet, zero intermediary fees. Any network below works:

<!--
TODO Orlando: replace the `<…>` placeholders with your real wallet
addresses before going public. Lines without a real address SHOULD
be removed entirely — an empty placeholder confuses donors.
-->

| Network | Address |
|---|---|
| Bitcoin (BTC, on-chain) | `<BTC_ADDRESS>` |
| Bitcoin Lightning | `<LIGHTNING_ADDRESS_or_BOLT12>` |
| Ethereum (ETH + ERC-20 stables like USDC, USDT) | `<ETH_ADDRESS>` |
| Solana (SOL + SPL stables like USDC) | `<SOL_ADDRESS>` |

Verify addresses against the latest commit before sending — never
trust a copy that came from somewhere other than this README on
`main`.

### Hire / consult

If your team uses SEELE in production and needs help wiring it in,
extending it, or running a private fork: hiring me is the most
direct way to ensure the feature you need ships sooner.

→ Open a [Discussion](https://github.com/orlando-vazquez-career/seele/discussions)
or email through the GitHub profile.

## License

[MIT](./LICENSE) — Copyright (c) 2026 DevZen SpA.
