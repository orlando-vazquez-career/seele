# SEELE Genesis — Arquitectura

**Fecha**: 2026-05-09
**Estado**: en ejecución
**Fase previa**: Estrategia (cerrada con SEELE como nombre, `C:\dev\tools\SEELE\` como path, scope incluyendo TUI desde v0.1).

## ADRs de esta fase

- `01-rust-y-crates.md` — Rust como lenguaje + ecosystem de crates (axum, clap, ratatui, ort, rusqlite, sqlite-vec, tokio, serde, tracing, thiserror).
- `02-schema-sqlite.md` — schema canónico de la DB (memories, virtual columns, índices, FTS5 virtual table, vec0 vec table).
- `03-search-hybrid.md` — algoritmo híbrido FTS + vector + metadata (RRF reciprocal rank fusion).
- `04-embedder-onnx.md` — embedder local con ort + all-MiniLM-L6-v2 + auto-download.
- `05-mcp-server.md` — MCP server transport (stdio v0.1, HTTP v0.2), naming convention de tools, schema de inputs.
- `06-http-api.md` — HTTP REST API con axum, auth bearer opcional, OpenAPI docs.
- `07-tui-design.md` — TUI con ratatui + crossterm, vistas, navegación, keybindings.
- `08-repo-layout.md` — workspace Cargo, 8 crates, tests structure, CI matriz.
- `09-distribution-license.md` — releases binarios via GitHub Actions, crates.io publish, MIT + crédito ENGRAM operacional.
- `10-mapping-mnema-seele.md` — contrato de mapping entre vocabulario MNEMA (Counsel, advisors, verdict, skill, axiomatica) y schema SEELE (sessions, observations, memory_relations, links). Cierra observación [ALTO] de Cloven.

## Output esperado

Decisiones técnicas locked-in para que la fase Táctica pueda escribir bloques de sprint accionables sin segundas vueltas.
