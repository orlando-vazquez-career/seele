# Sprint-01 BE Foundation — devlog

**Fecha**: 2026-05-10
**Fases ejecutadas**: estrategia → arquitectura → táctica → ejecución → guardrails → state-sync
**Plan origen**: `genesis/plans/executed/tactica/sprint-01/00-INDEX.md`
**Repos afectados**: `tools/SEELE`
**Migraciones**: schema vacío → V001 (initial schema v0.1.0)
**Tests**: 99 passing / 0 failing / 2 ignored (ONNX downloads)
**Build**: OK Linux + macOS + Windows (CI matrix verde)

## Resumen

Cierra el cimiento del workspace SEELE: 11 crates Cargo creados, `seele-core` con todos los tipos canónicos (`SeeleId`, `Observation`, `Session`, `Link`, `MemoryRelation`, `Metadata`), `seele-storage` con migrations versionadas via refinery, schema completo (sessions, observations, user_prompts, links, memory_relations, sync_chunks) con virtual generated columns + partial indexes + FTS5 sync triggers + vec0 virtual table cargado por extensión vendorizada, CRUD completo para todas las tablas, privacy stripping `<private>`, normalized hash dedup, topic key upserts. CI matrix verde en 3 OS, clippy + fmt + STELE residual checks pasando.

Sobre la base se ejecutó además trabajo parcial de Sprint 02 (commit `8d67f48` — `seele-embedder` ONNX runtime + `seele-search` con RRF combiner + storage embedding API) que **no está cerrado todavía**: la fase Táctica de Sprint 02 nunca se escribió formalmente. Se completa como primer paso del Sprint 02 antes de seguir codeando.

## Cambios entregados

### Workspace skeleton (Bloque A — commit `9e3f9a4`)

- `Cargo.toml` workspace con 11 miembros + dependencias centralizadas (tokio, axum, ratatui, ort, rusqlite, sqlite-vec via vendor, etc).
- `rust-toolchain.toml` (channel `stable`, components `rustfmt` + `clippy`).
- `rustfmt.toml` (edition 2021).
- `.github/workflows/ci.yml` — jobs `test` (matriz Linux+Mac+Win), `lint` (clippy + fmt), `static-checks-bash`, `static-checks-pwsh`.
- `.github/dependabot.yml` con monitoreo de `ort` para despinear cuando salga 2.0.0 stable.
- `scripts/check-no-stele-residual.{sh,ps1}` — static check de residuos del nombre legacy.
- `CHANGELOG.md` arrancado.
- 11 crates con `Cargo.toml` propio + `lib.rs`/`main.rs` stub.

### Core types (Bloque B — commit `90d4446`)

- `seele-core`: 8 módulos (`error`, `id`, `memory`, `session`, `link`, `relation`, `filter`, `metadata`).
- `SeeleError` enum con `thiserror` cubriendo storage/embedder/search/mcp/http/io/serde/invalid_input/not_found/conflict.
- `SeeleId` wrapper sobre ULID con `as_i64()` para puente con vec0 INTEGER rowid.
- `Observation` con 17 campos incluyendo `topic_key`, `normalized_hash`, `revision_count`, `duplicate_count`, `last_seen_at`, `deleted_at`.
- `ObservationType` con 12 variants canónicas + `Other(String)` extensible.
- `Scope` (project | personal), `SessionStatus` (active | ended | aborted), `JudgmentStatus` (pending | judged | orphaned | ignored), `RelationKind` (supersedes | conflicts_with | scoped | related | compatible | not_conflict).
- `MetadataFilter` + `ObservationQuery` para queries.
- 11 tests integration roundtrip + 22 unit tests.

### Storage layer (Bloque C — commits `4ec1e0b` y `f34b78f`)

- `seele-storage` con 13 módulos: `error`, `pool`, `migrations`, `vec0_install`, `vec0_loader`, `sessions`, `observations`, `prompts`, `links`, `relations`, `chunks`, `privacy`, `hash`.
- `vec0_loader` con `sqlite-vec` v0.1.9 vendorizado en `crates/seele-storage/vendor/sqlite-vec/` para 5 targets (linux/mac/win × x86_64 + linux/mac aarch64) embebido via `include_bytes!`. Override por env `SEELE_VEC_PATH` para targets no soportados.
- `vec0_install::write_atomic` con file lock para tests paralelos (commit `a1fadf3`).
- Migration `V001__initial_schema.sql` con todas las tablas, virtual generated columns (`meta_kind`, `meta_domain`, `meta_axiomatic`, `meta_score`, `meta_context_mode`, `int_id`), 11 partial indexes, FTS5 virtual tables (`observations_fts`, `prompts_fts`) con triggers AI/AD/AU, vec0 virtual table `observations_vec(embedding FLOAT[384])`.
- Connection pool (`r2d2_sqlite`) con PRAGMAs canónicos (WAL, NORMAL, foreign_keys, temp_store=MEMORY) + load_extension del vec0.
- `ObservationStore::save` con SaveOutcome (Created | UpsertedTopic | DuplicateMerged) — privacy strip + normalized_hash + topic_key window + dedup window.
- CRUD completo: get / list (con MetadataFilter) / update / soft_delete / restore / hard_delete para observations, sessions, prompts, links, relations, chunks.
- Privacy stripping con regex `(?si)<private>.*?</private>` aplicado en `save()`.

### Tests integración (Bloque D — distribuidos en commits anteriores)

- `crates/seele-core/tests/types_roundtrip.rs` — 11 tests JSON serde + ULID + ObservationType + RelationKind.
- `crates/seele-storage/tests/foundation_smoke.rs` — 4 tests migration smoke (todas las tablas + indexes + triggers + schema_version row).
- `crates/seele-storage/tests/sessions_crud.rs` — 7 tests lifecycle (start/end/abort/list filters/idempotency).
- `crates/seele-storage/tests/observations_crud.rs` — 21 tests save/get/list/update/soft_delete/restore/hard_delete + topic_key upsert + dedup hash + FTS sync.
- `crates/seele-storage/tests/secondary_crud.rs` — 5 tests links UNIQUE + relations judgment lifecycle + chunks idempotent + prompts roundtrip.
- 22 unit tests adicionales repartidos en módulos `privacy`, `hash`, `vec0_loader`, `vec0_install`, etc.

### Trabajo Sprint-02 parcial (commit `8d67f48`)

Avanzó sin Táctica formal — se incluye acá para trazabilidad pero **se cierra en Sprint 02**, no en este devlog:

- `seele-embedder` con `ort` 2.0.0-rc.10 + tokenizers + hf-hub, `OnnxEmbedder` para `all-MiniLM-L6-v2` con auto-download, `FakeEmbedder` para tests downstream.
- `seele-search` con `RrfHit`, `SearchEngine` híbrido FTS+vec, RRF combiner k=60, boost por metadata score.
- Storage embedding API: `ObservationStore::save_embedding`, `get_embedding`, integración con vec0 virtual table.
- 6 tests `hybrid_search.rs` + 9 unit tests del embedder + 7 search.

## Decisiones técnicas tomadas

- **MSRV bump 1.83 → 1.85**. `clap_lex` (transitivo via `clap` 4.5) y deps modernas requieren `edition2024`. Bumpear MSRV una vez ahora antes de v0.1.0 es preferible a hacerlo en cada minor release. Documentado en `CHANGELOG.md`.
- **`ort = "=2.0.0-rc.10"`** pin exacto. No hay 2.0.0 stable a 2026-05. Dependabot monitorea para PR automático cuando salga.
- **`sqlite-vec` vendorizado** (ADR-11). Binarios `vec0.{so,dylib,dll}` para 5 targets embebidos en el ejecutable via `include_bytes!`. Resultado: `cargo install seele` funciona out-of-the-box sin descargas runtime y sin requerir extensión preinstalada. Override por `SEELE_VEC_PATH` para casos avanzados.
- **`int_id` virtual column** sobre ULID para mapear a vec0 INTEGER rowid. Implementación SQL es aproximada; el cálculo correcto vive en Rust (`SeeleId::as_i64`) y es lo que se usa en INSERT.
- **FTS5 con `porter unicode61 remove_diacritics 2`**. Tokenizer estándar adecuado para español + inglés.
- **`assert!(true)` smoke tests removidos** en stubs de crates 03–04 (sprint-03/04). Clippy 1.95 los rechaza por `assertions_on_constants`. La verificación "el crate compila" la cubre `cargo build --workspace`; un test trivial no aporta. Reparación menor durante Guardrails.

## Incidentes durante la ejecución

- **Race condition en `vec0_install::write_atomic`** detectada al correr tests en paralelo (varios tests intentaban escribir el binario vec0 al mismo path simultáneamente). Solución: file lock + write atómico via temp file + rename. Commit `a1fadf3`.
- **Cloven follow-up 2026-05-10**: detectó licencia mixta MIT/Apache-2.0 inconsistente y residuos del nombre legacy "STELE". Reparado en commits `47d87ac` y `9478201` con scripts de check + dependabot.
- **Trabajo Sprint-02 fuera de táctica**: se avanzó código de embedder + search sin escribir antes la Táctica del Sprint 02. Cerrar este sprint sin la táctica del 02 es aceptable (Sprint 02 vive en su propio ciclo). Próximo paso: escribir la táctica de Sprint 02 antes de seguir codeando ahí.

## Cómo reproducir / verificar

```powershell
cd C:\dev\tools\SEELE
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bash scripts/check-no-stele-residual.sh
```

Resultado esperado: build OK, 99 tests passing (2 ignored: ONNX integration que descarga ~90MB de HF), clippy verde, fmt verde, STELE residual OK.

## Pendiente

- **Property tests con `proptest`** explícitos (Bloque D plan original). Los CRUD tests cubren paths felices y edge cases; faltan los `proptest!` para `save_then_get_roundtrip`, `topic_key_upsert_increments_revision`, `normalized_hash_dedup_increments_duplicate_count`, `soft_delete_hides_from_list`. Se difiere a follow-up dentro de Sprint 05 (polish) o como ticket dedicado en backlog.
- **Workspace-level tests** (`tests/storage_e2e.rs`, `tests/migration_e2e.rs`, `tests/fixtures/sample_observations.json`) tampoco se crearon. La cobertura actual via crate-level integration tests es suficiente para el cimiento; los E2E del binary llegan en Sprint 03 (con HTTP/MCP) y Sprint 05 (smoke E2E).
- **Tag git** `sprint-01-foundation` no se creó. Se hace al cierre del state-sync junto con el commit de devlog.
- **Sprint 02**: escribir Táctica formal en `genesis/plans/tactica/sprint-02/` antes de tocar más código. El embedder + search ya están parcialmente implementados pero sin contrato escrito.

## Uso y costo

| Modelo | Input tokens | Output tokens | Total | USD | Duración |
|---|---|---|---|---|---|
| claude-opus-4-7 | ~1,200,000 | ~250,000 | ~1,450,000 | ~$36 | 3 sesiones |

Estimación conservadora — el sprint se ejecutó en 3 sesiones (genesis 2026-05-09 + 2026-05-10 + audit/cierre 2026-05-10) con planificación + ejecución + auditoría. Detalle más fino en `cost-ledger.jsonl`.

## Referencias

- Estrategia: `genesis/plans/estrategia/00-INDEX.md`
- Arquitectura: `genesis/plans/arquitectura/00-INDEX.md` (10 ADRs)
- Táctica: `genesis/plans/executed/tactica/sprint-01/00-INDEX.md` (movido en este state-sync)
- Cloven follow-ups: commits `47d87ac`, `9478201`
- Bloque A: commit `9e3f9a4`
- Bloque B: commit `90d4446`
- Bloque C.1: commit `4ec1e0b`
- Bloque C: commit `f34b78f`
- Sprint-02 parcial: commit `8d67f48`
- Fix race vec0: commit `a1fadf3`
