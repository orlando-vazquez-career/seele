# ADR-01 — Rust + crate ecosystem

**Estado**: Aceptado · 2026-05-09
**Decisión**: Implementar SEELE en **Rust 1.83+** sobre el ecosystem `tokio` async, con un set definido de crates principales.

## Contexto

El engine necesita:
- Single-binary distributable multi-OS.
- Performance predictible (sub-200ms search a 10K memorias).
- Bindings nativos a SQLite + extensions (sqlite-vec).
- ONNX runtime CPU-only para embeddings.
- Async I/O para HTTP server + MCP server.
- TUI rica.

## Opciones evaluadas

### A — Rust ⭐
- Pros: single-binary cross-compilable, async maduro (tokio), bindings SQLite robustos (rusqlite), ONNX (ort), TUI excelente (ratatui), HTTP top-tier (axum), cero overhead, memoria segura.
- Contras: curva de aprendizaje empinada para nuevos contributors; tiempos de build (mitigable con incremental + sccache).
- **Veredicto**: encaja con todos los requisitos. Es la elección del User para componentes de sistema.

### B — Go
- Pros: compile times rápidos, single-binary trivial, runtime simple. ENGRAM es Go.
- Contras: para SEELE significa repetir el stack de ENGRAM (perdiendo la justificación clean-room técnica), GC stops potenciales en queries grandes, ecosystem ONNX menos maduro.
- **Veredicto**: descartado. Si elegimos Go, ¿por qué no usar ENGRAM directo?

### C — Zig
- Pros: zero-runtime, control absoluto, interop C nativo.
- Contras: ecosystem inmaduro (2026 todavía pre-1.0), pocos crates equivalentes, curva más empinada.
- **Veredicto**: descartado por inmadurez del ecosystem para tooling production-ready.

### D — TypeScript / Bun
- Pros: ecosystem familiar para el resto del stack del User (MNEMA backend está en TS+Bun).
- Contras: NO encaja con "single-binary distributable" — Bun produce binarios pero limitados; embeddings ONNX en TS son lentos; no hay ratatui equivalente; SQLite extensions menos directos.
- **Veredicto**: descartado. TS es para MNEMA orchestrator (lógica de alto nivel), no para el engine de bajo nivel.

## Decisión

**Rust 1.83+** con edition 2021. Toolchain instalado via rustup, MSRV definido en `rust-toolchain.toml`.

## Crate ecosystem locked-in

### Runtime async
- **`tokio`** 1.42+ — async runtime único. `features = ["full"]` para v0.1; reducir en producción si rentable.

### Storage
- **`rusqlite`** 0.32+ — bindings SQLite. `features = ["bundled", "load_extension"]`. Bundled para evitar dependencia de SQLite del sistema; load_extension para sqlite-vec.
- **`sqlite-vec`** — extensión vectorial loadable. Se carga en runtime via `db.load_extension()`.
- **`r2d2`** + **`r2d2_sqlite`** — connection pool. Para HTTP server concurrent reads.
- **`refinery`** o **`sqlx-cli`** style migrations — no decidido aún. Default: refinery por simplicidad.

### Embedder
- **`ort`** 2.0+ (oficial: `pykeio/ort`) — ONNX Runtime bindings. CPU EP por default.
- **`tokenizers`** (Hugging Face) — para BPE / WordPiece tokenization de los inputs antes del modelo.
- **`hf-hub`** — auto-download del modelo desde Hugging Face en el primer init.

### HTTP
- **`axum`** 0.8+ — web framework. Ergonomía top-tier, `tower` middleware ecosystem, async-first.
- **`tower`** + **`tower-http`** — middleware (cors, trace, compression).
- **`utoipa`** — OpenAPI 3.1 docs auto-generadas desde tipos.

### CLI
- **`clap`** 4.5+ — parser CLI. `features = ["derive", "env"]`. Subcommands para `init`, `save`, `search`, etc.

### TUI
- **`ratatui`** 0.29+ — TUI framework moderno (post-tui-rs).
- **`crossterm`** — backend cross-platform (Windows/Mac/Linux).

### MCP
- **No hay crate canónico** todavía en 2026. SEELE implementa MCP server desde el spec usando `serde_json` + `tokio` stdio + `axum` para HTTP transport.
- Si emerge un crate canónico durante v0.1, se evalúa swap.

### Serialization / errors / observability
- **`serde`** + **`serde_json`** — serialización.
- **`thiserror`** — definición de error types.
- **`anyhow`** — solo en CLI/binary, no en libraries.
- **`tracing`** + **`tracing-subscriber`** — logging estructurado.

### Testing
- **`assert_cmd`** + **`predicates`** — integration tests del binary.
- **`proptest`** — property tests para schema/queries.
- **`tempfile`** — DB temp files para tests.

## Justificación de las elecciones más críticas

### ¿Por qué `axum` y no `actix-web` / `rocket`?

`axum` es del equipo de tokio, integración nativa, types-driven extractors, momentum 2024-2026 (el framework "default" en la comunidad). `actix-web` tiene su propio runtime; `rocket` tiene history de attestation lenta y syntax macros pesadas.

### ¿Por qué `rusqlite` y no `sqlx`?

`sqlx` es excelente para Postgres async, pero su soporte SQLite es secundario y NO permite cargar extensiones (sqlite-vec) fácilmente. `rusqlite` es sync (manejado con `tokio::task::spawn_blocking`) pero permite extensions y es el bindings canónico SQLite en Rust.

### ¿Por qué `ratatui` y no `cursive` / `crossterm` directo?

`ratatui` es el sucesor de `tui-rs` (deprecado), inmediate-mode, widget library rica, declarative. `cursive` es event-driven (más complicado para agentic flows). `crossterm` directo es lower-level.

### ¿Por qué `ort` y no `candle`?

`candle` (Hugging Face) es excelente pero más nuevo, ecosistema menos pulido para production. `ort` lleva años, soporta todos los EPs, performance probada. Si `candle` madura para v0.2, se evalúa swap.

## Consecuencias

### Positivas
- Stack moderno, bien soportado, single-binary garantizado.
- Comunidad amplia para contributors externos.
- Performance predecible.

### Negativas
- Onboarding más lento que Go.
- Build times (~1-3 min para builds limpios; cacheables con sccache + GitHub Actions cache).
- ONNX runtime depende de DLL (ort estático evitable con feature `load-dynamic` off).

### Mitigaciones
- Build cache en CI (sccache + actions/cache).
- `rust-toolchain.toml` para reproducibility.
- Feature flags para compilación opcional (ej: `--no-default-features --features cli` para builds sin TUI).

## Referencias

- Rust 2024 roadmap survey results (rust-lang.org/blog).
- Comparativa axum vs actix vs rocket: https://github.com/programatik29/axum-comparison.
- ratatui showcase: https://ratatui.rs/showcase/.
- ort vs candle benchmarks 2025.
