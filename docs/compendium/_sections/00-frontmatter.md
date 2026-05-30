# SEELE — Technical Compendium

> *Super stellatum firmamentum iudicat Deus, sicut nos iudicamus.*
> *Over the starry firmament God judges, as we judge.*

| | |
|---|---|
| **Subject** | SEELE — a local-first memory engine for AI agents |
| **Version documented** | `v0.2.0` (workspace), shipped baseline `v0.1.0` |
| **Repository** | `C:/dev/tools/SEELE` — `https://github.com/orlando-vazquez-career/seele` |
| **Language / stack** | Rust 1.85 (edition 2021) · SQLite (FTS5 + sqlite-vec/vec0) · ONNX (`all-MiniLM-L6-v2`) · axum · ratatui · clap |
| **Scale** | ~19,500 lines of Rust across 13 workspace crates + an Astro static site |
| **License** | MIT © 2026 DevZen SpA (clean-room reimplementation inspired by ENGRAM) |
| **Document date** | 2026-05-29 |
| **Document purpose** | Exhaustive technical reference of the entire system, intended to be handed to another AI so it can propose improvements |

---

## How this document was produced

Every detailed section (§3–§23) was written by a dedicated analysis pass that read the actual source of that subsystem, and was then **adversarially fact-checked** by a second pass that re-read the source and corrected any claim it could not verify. The orientation sections (§1, §2, §24) and this front matter were authored from a whole-system reading of the repository's `README.md`, `DESIGN.md`, `CLAUDE.md`, `Cargo.toml`, `docs/INDEX.md`, the 13 architecture ADRs, and the inter-crate dependency graph.

Where the verification pass found and corrected inaccuracies, they are reflected directly in the prose; a consolidated note on verification confidence is recorded at the end of the assembly.

## How to read it

- **If you want the 60-second model**, read §1 (Executive Summary) and §2 (Architecture & Crate Topology).
- **If you are tracing behavior**, jump to §19 (End-to-End Data Flows), then dive into the specific subsystem section.
- **If you are proposing improvements** (the primary intent of this document), §23 (Known Limitations, Technical Debt & Improvement Surface) is the highest-signal section, backed by §22 (Design Rationale & ADR Digest) for the *why* behind each decision.
- **If you need a precise contract**, §20 (Database Schema Reference) and §21 (Interface Catalog) are the normative appendices.

## Document conventions

- Code references use `path:line` form relative to the repository root (`C:/dev/tools/SEELE`).
- "Observation" is SEELE's canonical name for a stored memory record; "engram" and "memory" appear as synonyms inherited from the ENGRAM lineage (see §24 Glossary).
- "vec0" refers to the virtual-table interface of the vendored `sqlite-vec` extension.
- Throughout, **MCP** = Model Context Protocol (Anthropic), **RRF** = Reciprocal Rank Fusion, **FTS5** = SQLite's full-text search module v5.

---

## Table of Contents

**Part I — Orientation**
- §1 — Executive Summary
- §2 — System Architecture & Crate Topology

**Part II — Foundation & Data Plane**
- §3 — Domain Core (`seele-core`)
- §4 — Storage Layer I — Schema, Migrations, Connection Pool & vec0 Loading
- §5 — Storage Layer II — Stores, Dedup, Privacy, Links, Relations & Chunks
- §6 — Embeddings (`seele-embedder`)
- §7 — Hybrid Search & Reciprocal Rank Fusion (`seele-search`)

**Part III — Transports & Interfaces**
- §8 — MCP Server (`seele-mcp`)
- §9 — HTTP REST API (`seele-http`)
- §10 — Multi-Provider Chat Backend (`seele-chat`)
- §11 — Command-Line Interface (`seele-cli`)
- §12 — Terminal UI (`seele-tui`)

**Part IV — Operations, Migration & Surface**
- §13 — Multi-Machine Sync (`seele-sync`)
- §14 — Agent Setup Wizard (`seele-setup`)
- §15 — Project Detection (`seele-project`)
- §16 — ENGRAM / MNEMA Import (`seele-engram-import`)
- §17 — Web Landing & Observability (`web/`)
- §18 — Build, CI/CD, Distribution & Release

**Part V — Cross-Cutting References**
- §19 — End-to-End Data Flows
- §20 — Database Schema Reference (Data Dictionary)
- §21 — Interface Catalog — MCP Tools × HTTP Endpoints × CLI Subcommands

**Part VI — Rationale & Improvement**
- §22 — Design Rationale & ADR Digest
- §23 — Known Limitations, Technical Debt & Improvement Surface

**Appendix**
- §24 — Glossary & References
