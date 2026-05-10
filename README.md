# SEELE

> *Super stellatum firmamentum iudicat Deus, sicut nos iudicamus.*
> Sobre el firmamento estrellado juzga Dios, como nosotros juzgamos.

A Rust memory engine for AI agents — local SQLite + FTS5 + sqlite-vec embeddings + MCP server + HTTP API + TUI.

**Status**: in development. Sprint-01 BE Foundation closed 2026-05-10 — workspace skeleton + `seele-core` types + `seele-storage` with SQLite/FTS5/vec0/CRUD migrations. 99 tests green on Linux + macOS + Windows CI matrix. Sprints 02–05 ahead. See `docs/INDEX.md` for the doc map and `docs/aegis/devlogs/` for sprint devlogs.

## What it does (planned)

- Persists agent memories with hybrid full-text + vector search (Reciprocal Rank Fusion).
- Exposes 19 MCP tools (`seele_*`) for Claude Code, Cursor, VS Code, OpenCode, Gemini CLI, Codex, Windsurf, Antigravity.
- Local-first: SQLite single-file DB, all embeddings via local ONNX (`all-MiniLM-L6-v2`).
- Multi-machine via git-friendly compressed chunks (no merge conflicts).
- TUI for interactive browsing.
- HTTP REST API for programmatic consumers.

## On the name

"Seele" is the German word for "soul" or "spirit". It is a common word of the German language, not a trademark we claim and not a reference to any specific franchise. We chose it because the metaphor — memory as what constitutes the soul of an agent — captures what this engine is for.

## Inspiration

SEELE is a Rust reimplementation inspired by [ENGRAM](https://github.com/Gentleman-Programming/engram) by [Gentleman-Programming](https://github.com/Gentleman-Programming). ENGRAM proved that local-first memory engines for AI agents are viable and demonstrated many of the patterns SEELE adopts: session lifecycle, project detection, topic key upserts, memory relations with judgment, git sync chunks, privacy stripping, MCP-as-primary-transport.

ENGRAM is licensed MIT (Copyright Gentleman-Programming). SEELE is also MIT (Copyright DevZen SpA). They are sibling tools in the same niche, with different stacks (Go vs Rust) and a different bet on retrieval (FTS+LLM-judge vs FTS+embeddings+RRF).

If SEELE is useful, please also try ENGRAM — the broader ecosystem benefits from multiple options. See [`CREDITS.md`](./CREDITS.md) for full attribution.

## Genesis docs

- `genesis/plans/estrategia/` — overview, naming, scope MVP, ENGRAM feature audit, reimplementación inspirada postura.
- `genesis/plans/arquitectura/` — 10 ADRs covering Rust + crates / SQLite schema / search RRF / embedder / MCP / HTTP / TUI / repo layout / distribución / mapping MNEMA↔SEELE.
- `genesis/plans/tactica/` — sprints v0.1 (5 sprints, see `tactica/00-INDEX.md`).
- `genesis/plans/executed/` — closed plans. Sprint-01 lives here as of 2026-05-10.
- `docs/INDEX.md` — doc map + devlogs index.
- `docs/aegis/devlogs/` — sprint devlogs + cost ledger.

See `CLAUDE.md` for repo operating rules.

## License

[MIT](./LICENSE) — Copyright (c) 2026 DevZen SpA.
