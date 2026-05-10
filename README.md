# SEELE

> *Super stellatum firmamentum iudicat Deus, sicut nos iudicamus.*
> Sobre el firmamento estrellado juzga Dios, como nosotros juzgamos.

A Rust memory engine for AI agents — local SQLite + FTS5 + sqlite-vec embeddings + MCP server + HTTP API + TUI.

**Status**: pre-development (genesis 2026-05-09 / 2026-05-10). The plan lives under `genesis/plans/`. No Rust code yet.

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
- Táctica + Ejecución pending.

## License

[MIT](./LICENSE) — Copyright (c) 2026 DevZen SpA.
