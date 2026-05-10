# Changelog

Todos los cambios notables a este proyecto se documentan acá. Formato basado en
[Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y SemVer.

## [Unreleased]

### Added
- Estructura inicial del workspace Cargo con 11 crates (`seele-core`,
  `seele-storage`, `seele-embedder`, `seele-search`, `seele-mcp`, `seele-http`,
  `seele-tui`, `seele-sync`, `seele-setup`, `seele-project`, `seele-cli`).
- `seele-core` — tipos canónicos: `SeeleId` (ULID), `Observation`,
  `ObservationType` (12 variants + `Other`), `Scope`, `Session`,
  `SessionStatus`, `Metadata`, `Link`, `MemoryRelation`, `MetadataFilter`.
  22 unit tests + 9 integration tests (incluye proptest).
- `seele-storage` — SQLite + FTS5 + vec0 con migrations refinery, CRUD
  completo sobre `sessions`/`observations`/`user_prompts`/`links`/
  `memory_relations`/`sync_chunks`. Privacy strip `<private>`, normalized
  hash dedup, topic key upserts, soft delete.
- `seele-storage/vec0_loader` — carga vendorizada de `sqlite-vec` v0.1.9 vía
  `include_bytes!` para 5 targets (linux/mac/win × x86_64 + linux/mac aarch64).
  Ver `crates/seele-storage/vendor/sqlite-vec/README.md` y ADR-11.
- `seele-embedder` — ONNX runtime via `ort` 2.0.0-rc.10 + tokenizers + hf-hub
  con auto-download de `all-MiniLM-L6-v2`. `OnnxEmbedder` + `FakeEmbedder`
  para tests downstream. (Trabajo Sprint-02 parcial; cierra en próximo ciclo.)
- `seele-search` — RRF combiner híbrido FTS+vec con boost por metadata score.
  (Trabajo Sprint-02 parcial; cierra en próximo ciclo.)
- CI matrix Linux + macOS + Windows (build + test) + lint job (clippy + fmt) +
  static-checks-bash + static-checks-pwsh.
- Static check `scripts/check-no-stele-residual.{sh,ps1}` para asegurar que
  ningún archivo del repo tenga residuos del nombre legacy "STELE" fuera de
  los allowlist documentados. Allowlist soporta archivos exactos y prefijos
  de directorio (e.g. `docs/aegis/devlogs/`).
- Génesis AEGIS completa: 5 docs estrategia + 11 ADRs arquitectura + 5
  sprints táctica.
- `CLAUDE.md` con reglas operativas del repo + `docs/INDEX.md` + primer
  devlog Sprint-01 + cost-ledger.jsonl arrancado.

### Changed
- **Sprint-01 BE Foundation cerrado** (2026-05-10). Plan táctico movido a
  `genesis/plans/executed/tactica/sprint-01/`. Devlog completo en
  `docs/aegis/devlogs/2026-05-10-sprint-01-foundation.md`. 99 tests verde
  + clippy + fmt + STELE residual checks pasando.
- **Sprint-02 BE Embedder + Search cerrado** (2026-05-10). Plan táctico
  movido a `genesis/plans/executed/tactica/sprint-02/`. Devlog en
  `docs/aegis/devlogs/2026-05-10-sprint-02-embedder-search.md`. Cambios:
  - `seele-embedder`: cache controlada `~/.seele/embedder/` con env
    override `SEELE_EMBEDDER_DIR`, INT8 quantized default con fallback
    automático a full precision + warn, SHA256 verification opcional
    (tabla `TRUSTED_HASHES` vacía hasta primer release), singleton global
    para servers de larga vida, trait method `expected_sha256()`.
  - `seele-search`: boost por `meta_score` (ADR-03 capa 5), empty-query
    path `created_at DESC` en lugar de `InvalidInput`, annotation lines
    de `memory_relations` (Supersedes/SupersededBy/ConflictsWith/
    ContestedBy) opt-in vía `include_annotations`, `max_vec_distance`
    threshold.
  - Tests: 130 verde (+31 vs Sprint-01), 4 ignored (2 ONNX + 2 perf).
    Suite fixtures + 7 E2E + 3 proptest + 2 perf smoke. Clippy + fmt +
    STELE residual checks pasando.

### Changed
- **MSRV bump a Rust 1.85** desde 1.83 inicial. Razón: `clap_lex` (transitiva
  vía `clap` 4.5) y otras deps modernas requieren `edition2024`. Si bajamos
  versiones de deps el conflicto se resuelve, pero perdemos features de
  `axum 0.8`, `utoipa 5`, etc. — preferimos bumpear MSRV una vez ahora antes
  de v0.1.0 que en cada release minor.
- **Pin de dependencia**: `ort = "=2.0.0-rc.10"` — no existe `2.0.0` stable
  a fecha 2026-05. Despinear cuando upstream haga release stable. Monitoreado
  vía `.github/dependabot.yml` (PR automático cuando salga el bump).

## Histórico de bumps de dependencias vendorizadas

### `sqlite-vec` (vendored)
- 2026-05-10 — `v0.1.9` (génesis). Vendorizado en
  `crates/seele-storage/vendor/sqlite-vec/`. Procedimiento de bump
  documentado en el README de esa carpeta.

---

[Unreleased]: https://github.com/orlando-vazquez-career/seele/compare/main...HEAD
