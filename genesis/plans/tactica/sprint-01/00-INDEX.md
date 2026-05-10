# Sprint-01 BE — Foundation

**Fecha**: 2026-05-10
**Tema**: Cargo workspace + `seele-core` + `seele-storage`. Cierra el cimiento sobre el que descansan todos los crates posteriores.
**Salida**: DB SQLite con todas las tablas + virtual columns + indexes; CRUD funcional para sessions/observations/links/relations/sync_chunks/user_prompts; tests integración pasando.

## Bloques

- `01-bloque-A-workspace-skeleton.md` — Cargo workspace + 11 crates con stubs vacíos + `Cargo.toml` workspace + `rust-toolchain.toml` + `.gitignore` actualizado + GitHub Actions CI básico.
- `02-bloque-B-core.md` — `seele-core` con tipos canónicos (Memory, Session, Observation, Link, MemoryRelation, SyncChunk), errores (`SeeleError` enum con thiserror), ULID generation, MetadataFilter, helpers de validación.
- `03-bloque-C-storage.md` — `seele-storage` con migrations (refinery), connection pool (r2d2), CRUD para todas las tablas, FTS5 + vec0 + virtual cols + indexes, triggers FTS sync, schema_version, soft delete, topic key upsert logic, normalized hash dedup, privacy stripping (`<private>` regex).
- `04-bloque-D-tests-integration.md` — tests integración de seele-storage con DB temporal, fixtures, property tests con proptest, smoke tests de migration.

## Dependencias entre bloques

```
A (workspace) ─→ B (core) ─→ C (storage) ─→ D (tests)
```

Estricto secuencial dentro del sprint.

## Criterios de cierre del sprint

1. ✅ `cargo build --workspace` verde en Windows + macOS + Linux.
2. ✅ `cargo test --workspace` verde con cobertura mínima de paths felices del CRUD + edge cases documentados.
3. ✅ `cargo clippy --workspace -- -D warnings` verde.
4. ✅ `cargo fmt --check` verde.
5. ✅ Static check STELE residual verde.
6. ✅ Commit + push de cada bloque + devlog del sprint en `docs/aegis/devlogs/2026-MM-DD-sprint-01-foundation.md`.
7. ✅ Migración aplicable contra DB vacía produce todas las tablas + indexes + virtual cols + triggers FTS + 3 schema_versions row (initial schema).
8. ✅ Cost-ledger append en `docs/aegis/devlogs/cost-ledger.jsonl` con tokens consumidos.

## Out-of-scope del sprint-01

- Embedder (sprint-02).
- Search híbrido RRF (sprint-02).
- HTTP API (sprint-03).
- MCP server (sprint-03).
- TUI (sprint-04).
- Setup wizard (sprint-04).
- Git sync (sprint-04).
- CLI completo (sprint-04 — solo el binario stub aquí).
- Project detection algorithm (sprint-04).
- Release / CI matrix completa (sprint-05).

## Tamaño esperado

- LOC Rust: ~2-3K (storage es el más grande).
- Tests: ~800-1500 LOC.
- Tiempo: 1 sesión orquestada.

## Riesgos identificados

| Riesgo | Mitigación |
|---|---|
| sqlite-vec extension load falla en Windows | Documentar en INSTALLATION.md + CI catchea desde sprint-01. Alternativa: vendored loadable extension binary. |
| ULID encoding conflict con FTS rowid (TEXT vs INTEGER) | Schema usa `int_id INTEGER UNIQUE GENERATED` derivado del ULID para puente vec0. Documentado en ADR-02. |
| Trigger FTS update + soft delete interaction | Tests específicos en bloque D para validar que soft-deleted no aparece en FTS. |
| Migration rollback si el primer init falla a mitad | refinery aplica una migration por transacción. Si falla, rollback automático. |
