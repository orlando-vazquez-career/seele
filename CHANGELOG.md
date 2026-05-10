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
- `seele-storage/vec0_loader` — carga vendorizada de `sqlite-vec` v0.1.9 vía
  `include_bytes!` para 5 targets (linux/mac/win × x86_64 + linux/mac aarch64).
  Ver `crates/seele-storage/vendor/sqlite-vec/README.md` y ADR-11.
- CI matrix Linux + macOS + Windows (build + test) + lint job (clippy + fmt) +
  static-checks-bash + static-checks-pwsh.
- Static check `scripts/check-no-stele-residual.{sh,ps1}` para asegurar que
  ningún archivo del repo tenga residuos del nombre legacy "STELE" fuera de
  los allowlist documentados.
- Génesis AEGIS completa: 5 docs estrategia + 11 ADRs arquitectura + 5
  sprints táctica.

### Changed
- **MSRV bump a Rust 1.85** desde 1.83 inicial. Razón: `clap_lex` (transitiva
  vía `clap` 4.5) y otras deps modernas requieren `edition2024`. Si bajamos
  versiones de deps el conflicto se resuelve, pero perdemos features de
  `axum 0.8`, `utoipa 5`, etc. — preferimos bumpear MSRV una vez ahora antes
  de v0.1.0 que en cada release minor.

### Pinned
- `ort = "=2.0.0-rc.10"` — no existe `2.0.0` stable a fecha 2026-05.
  Despinear cuando upstream haga release stable. Monitoreado vía
  `.github/dependabot.yml`.

## Histórico de bumps de dependencias vendorizadas

### `sqlite-vec` (vendored)
- 2026-05-10 — `v0.1.9` (génesis). Vendorizado en
  `crates/seele-storage/vendor/sqlite-vec/`. Procedimiento de bump
  documentado en el README de esa carpeta.

---

[Unreleased]: https://github.com/orlando-vazquez-career/seele/compare/main...HEAD
