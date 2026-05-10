# ADR-09 — Distribución, licencia, crédito a ENGRAM

**Estado**: Aceptado · 2026-05-09
**Decisión**: Releases binarios via GitHub Releases para 5 targets, publish en crates.io de los crates principales, MIT con copyright DevZen SpA, crédito explícito a Gentleman-Programming/ENGRAM en README + CREDITS + release notes.

## Distribución

### GitHub Releases

Tag-driven (`v0.1.0`, `v0.2.0`, etc). Cada tag dispara `release.yml` que produce 5 artifacts:

| Target | Filename |
|---|---|
| Linux x86_64 GNU | `seele-v0.1.0-x86_64-unknown-linux-gnu.tar.gz` |
| Linux x86_64 musl | `seele-v0.1.0-x86_64-unknown-linux-musl.tar.gz` |
| macOS arm64 | `seele-v0.1.0-aarch64-apple-darwin.tar.gz` |
| macOS x86_64 | `seele-v0.1.0-x86_64-apple-darwin.tar.gz` |
| Windows x86_64 | `seele-v0.1.0-x86_64-pc-windows-msvc.tar.gz` |

Cada `.tar.gz` contiene:
- `seele` (o `seele.exe`) binary.
- `LICENSE`.
- `CREDITS.md`.
- `README.md` quickstart.

Sha256 sums via `sha256sum *.tar.gz > SHA256SUMS` adjunto al release.

### Install scripts

#### Quick install (bash)

```bash
curl -fsSL https://seele.dev/install.sh | sh
```

`install.sh` detecta OS+arch, descarga el tarball, verifica sha256, copia a `~/.local/bin/seele`. Documentado en README.

#### Quick install (PowerShell)

```powershell
iwr -useb https://seele.dev/install.ps1 | iex
```

Equivalente para Windows.

#### Cargo install

```bash
cargo install seele
```

Compila desde fuente. Para usuarios Rust o cuando no hay binary pre-built (raro en v0.1).

#### Homebrew (v0.2)

Tap en `orlando-vazquez-career/homebrew-seele`. Diferido a v0.2 cuando haya tracción.

### crates.io

Publicar **dos** crates en v0.1:

1. **`seele-core`** — tipos, errores, utilities. Útil para integraciones que no quieren todo el binary.
2. **`seele`** (binary crate) — bin instalable via `cargo install seele`.

Los demás crates (`seele-storage`, `seele-embedder`, etc.) NO se publican en v0.1 — su API es interna y puede cambiar. Se publican cuando estabilice (v0.2+).

`Cargo.toml` workspace tiene `publish = false` por default y solo los dos crates publicables tienen `publish = true`.

## Licencia

### MIT, copyright DevZen SpA

```
MIT License

Copyright (c) 2026 DevZen SpA

Permission is hereby granted, free of charge, ...
```

Mismo template que AEGIS / LUMEN / MNEMA.

### Por qué MIT y no Apache-2.0

- **Simplicidad**: 21 líneas, comprensible al instante.
- **Compatibilidad**: MIT es compatible con MIT/Apache/BSD/GPL — máxima reusabilidad.
- **Consistencia con el ecosistema** del User (los otros tres protocolos también son MIT).
- **Apache-2.0** agrega clausula explícita de patent grant — útil para grandes corps con patents, overkill para este caso.

### Headers en archivos

Sin headers de licencia per-archivo (overhead innecesario; el LICENSE root es suficiente).

## Crédito a ENGRAM

### `README.md` sección "Inspiración"

```markdown
## Inspiración

SEELE existe gracias al trabajo pionero de [Gentleman-Programming](https://github.com/Gentleman-Programming) en [ENGRAM](https://github.com/Gentleman-Programming/engram), que demostró la viabilidad de un memory engine local-first con SQLite + FTS5 + embeddings + MCP server, accesible para desarrolladores sin pretensiones de plataforma cloud.

SEELE es una **reimplementación clean-room en Rust** — sin código compartido, con decisiones técnicas propias y bajo licencia MIT. Si lo que hace SEELE te resulta útil, considerá probar también ENGRAM: son herramientas hermanas del mismo nicho, y el ecosistema gana cuando hay opciones.
```

### `CREDITS.md`

Más extenso. Estructura:

```markdown
# Credits

## Influences

### ENGRAM by Gentleman-Programming

SEELE's core architecture (SQLite + FTS5 + embeddings + MCP) was inspired by ENGRAM. The clean-room reverse engineering process documented in `genesis/plans/estrategia/02-clean-room-engram.md` outlines what we observed (public README, CLI behavior, README schema) and what we did not consult (code, internal data structures, license headers).

We thank the ENGRAM author for proving this category of tool is viable.

## Models

### all-MiniLM-L6-v2

Default embedder. By the Sentence-Transformers team (UKP Lab + Microsoft). Apache-2.0. https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2

## Libraries

Major dependencies that make SEELE possible:

- **rusqlite** — SQLite bindings. MIT.
- **sqlite-vec** by Alex Garcia — vector extension for SQLite. Apache-2.0/MIT.
- **ort** by pykeio — ONNX Runtime bindings. Apache-2.0/MIT.
- **axum** by Tokio team — HTTP framework. MIT.
- **ratatui** — TUI framework (successor to tui-rs). MIT.
- **clap** by clap-rs team — CLI parser. Apache-2.0/MIT.
- **tokio** — async runtime. MIT.
- **tokenizers** by Hugging Face — Apache-2.0.

Full dependency list in `Cargo.lock`. See `cargo deny` configuration for license audit.
```

### Release notes

Cada release tag tiene un body en GitHub Releases. v0.1.0 incluye:

```markdown
## SEELE v0.1.0 — first release

SEELE is a Rust memory engine for AI agents — local SQLite + FTS5 + embeddings + MCP server.

This is the first release. Inspired by [ENGRAM](https://github.com/Gentleman-Programming/engram).

## What's in v0.1.0

- ✅ SQLite + FTS5 + sqlite-vec storage
- ✅ ONNX embedder (all-MiniLM-L6-v2)
- ✅ Hybrid search (FTS + vector + metadata, RRF combiner)
- ✅ CLI: init / save / search / show / list / delete / serve / mcp / tui
- ✅ HTTP REST API
- ✅ MCP stdio server
- ✅ TUI (browse / search / detail)

## Install

(...quick install commands...)

## Credit

ENGRAM (https://github.com/Gentleman-Programming/engram) inspired SEELE's architecture. See CREDITS.md for full attribution.
```

### Linkbacks

En el README de SEELE link al repo de ENGRAM. NO un blog post comparativo (sería oportunista). Solo crédito.

## Versioning policy

Semver estricto:

- **0.x.y**: pre-1.0, breaking changes pueden ocurrir entre minors.
- **1.0.0**: cuando MNEMA + ≥2 otros consumers usen SEELE en producción 3+ meses.
- **Patches**: bug fixes only.
- **Minors**: backward-compatible features.
- **Majors (post-1.0)**: breaking changes con migración documentada.

Cambios de DB schema requieren migrations versionadas (refinery) — el caller no rompe.

## Comunicación

- **CHANGELOG.md** keep-a-changelog format, updated cada release.
- **Issues / Discussions**: GitHub native, en español o inglés.
- **No Slack/Discord** v0.1 (overhead sin beneficio claro).
- **Twitter / X**: opcional, solo announcements de release mayores.

## Out of scope

- **Distribución por paquetes nativos** (apt/dnf/pacman): out of v0.1.
- **Container images** (Docker Hub, ghcr): v0.2 si emerge.
- **SaaS hosting**: out of charter (SEELE es local-first).

## Compliance

- **Cargo audit**: weekly via GitHub Action. Issues opened automatically.
- **cargo-deny**: license whitelist (MIT, Apache-2.0, BSD-3, BSD-2, ISC, Unicode-DFS-2016). CI fails on copyleft.
- **Renovate** (o Dependabot): updates de deps automatic con tests gate.

## Referencias

- semver.org spec.
- keep-a-changelog: https://keepachangelog.com.
- softprops/action-gh-release.
- Cargo book — publishing.
