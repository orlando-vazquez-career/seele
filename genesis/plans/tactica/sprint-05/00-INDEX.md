# Sprint-05 — Polish + CI/CD + Release

**Fecha**: 2026-05-11
**Estado**: Táctica escrita, ejecución pendiente (Gate 1 abierto)
**Pre-requisitos**: Sprint-04 Ops & UX cerrado (tag `sprint-04-ops-ux`, commit `31af8bd`) + post-cierre fixes (`181795f`, `130f9a2`) + CI fix (`c8a7e36`) + NIT close (`8ba46c8`). 316 tests verde. Sprint-04 todos los Cloven findings cerrados.

## Objetivo

Convertir lo que hoy es un workspace funcional pero "interno" en un **release v0.1.0 distribuible** — binarios firmados en GitHub Releases, ONNX como embedder por defecto, docs polish para usuarios reales, y propiedad tests que cierren la sight de Cloven de Sprint-02. Tras este sprint:

- Un usuario externo puede `cargo install seele` o bajar el binario de su OS desde el GH Release y empezar a usar SEELE sin compilar el workspace.
- El default funcional (ONNX `all-MiniLM-L6-v2`) ya no es FakeEmbedder — los usuarios obtienen embeddings reales out-of-the-box.
- README, INSTALLATION, AGENT-SETUP, ENGRAM-MIGRATION docs están al día y referenciadas entre sí.
- El pipeline de release está probado punta a punta con un tag `v0.1.0` real.

Sale como **v0.1.0** etiquetada en main, con devlog + state-sync + Cloven review post-cierre.

## Bloques

### Bloque A — Property tests workspace-wide

Cierre del **Cloven sight Sprint-02** (deferido en su momento): "property tests workspace-wide con `proptest`". Cubre las invariantes del dominio que los unit tests por casos no atrapan.

Áreas:

- **`seele-core`**:
  - `SeeleId` parse/display roundtrip.
  - `SeeleId::as_i64()` siempre no-negativo, determinista para el mismo ULID.
  - `Metadata` JSON roundtrip (serialize→deserialize idempotente).
  - `ObservationType::from_str_relaxed` no panic en input arbitrario.
- **`seele-storage`**:
  - `normalized_hash` determinista bajo whitespace variations.
  - `save → get` returns the same row (after privacy strip).
  - `save → list` con un solo filter retorna la row.
- **`seele-search`**:
  - RRF combiner invariants: el output siempre tiene `score >= 0`, las row IDs son union de FTS+vec inputs, sorted descending by score.
- **`seele-sync`**:
  - `compute_chunk_id` determinista para el mismo set de observations (independiente de orden o timestamps).

Patrón: cada crate gana `tests/properties.rs` (cargo lo recoge automáticamente). Targets `proptest::proptest!` con shrinkers default. ~250-350 LOC test.

### Bloque B — ONNX por defecto + fallback inteligente

**Cloven sight Sprint-03** [MEDIO]: "ONNX por default en CLI". Hoy `build_service` siempre construye `FakeEmbedder` y el flag `--fake-embedder` es no-op `hide = true`. En este bloque:

- `build_service(db, fake_embedder)` ahora:
  - Si `fake_embedder == true` → `FakeEmbedder` (testing path, mantiene la suite verde).
  - Si `false` (default) → intenta `OnnxEmbedder::default()`. Si la descarga del modelo o la carga falla, **fallback a FakeEmbedder con un `tracing::warn!`** explícito y se setea un flag en el service que `doctor` puede leer.
- `--fake-embedder` deja de estar `hide = true` (vuelve a `--help` con la descripción real).
- `seele doctor` reporta el backend efectivo: `onnx` cuando ONNX corrió OK, `fake` cuando se forzó o se cayó al fallback. El warning de FakeEmbedder solo se emite cuando el fallback se disparó O cuando el usuario forzó `--fake-embedder` y no está en modo test.
- Las suites de tests existentes pasan a usar `--fake-embedder` explícito donde no lo hacían (auditar `subcommands_e2e.rs` + `binary_e2e.rs`).
- Documentar el cache `~/.seele/embedder/` y el override `SEELE_EMBEDDER_DIR` en `docs/INSTALLATION.md` (queue para Bloque D).

~150-200 LOC. Defended decision: la fallback transparente vs error-out — preferimos no romper la primera-corrida del usuario por una falla de red transitoria. El warning visible cubre la pérdida de calidad.

### Bloque C — Release pipeline

Workflow `.github/workflows/release.yml` que se dispara con tags `v*.*.*`:

- **Matrix 5 targets**:
  - `x86_64-unknown-linux-gnu` (ubuntu-latest)
  - `aarch64-unknown-linux-gnu` (ubuntu-latest, cross o native runner)
  - `x86_64-apple-darwin` (macos-13)
  - `aarch64-apple-darwin` (macos-latest)
  - `x86_64-pc-windows-msvc` (windows-latest)
- **Per target**: `cargo build --release -p seele-cli`, tar/zip el binario, compute SHA256.
- **Aggregate**: subir a GH Release con notas autogeneradas desde `CHANGELOG.md` (`Unreleased` section).
- **crates.io publish**: workflow_dispatch input opcional, default OFF en v0.1 (no quiero publicar accidental). El `cargo publish --dry-run` corre siempre para validar metadata.
- **Install scripts**:
  - `scripts/install.sh` para Unix-like (curl + sha256 verify + place en `~/.local/bin/`).
  - `scripts/install.ps1` para Windows (Invoke-WebRequest + Get-FileHash + place en `$env:USERPROFILE\.seele\bin\`).

Decisión técnica: **no** Homebrew tap en v0.1 (defer v0.2). **no** publicar a crates.io en este sprint (el workflow lo prepara pero pone el publish gate detrás de un input manual). Razón: una sola publicación accidental a crates.io es irreversible; preferimos hacer un round más de QA antes.

~150-250 LOC YAML + scripts.

### Bloque D — Docs polish

Reescribir el README (hoy refleja estado Sprint-01) + agregar tres docs nuevos:

- **`README.md`** rewrite:
  - Status: v0.1.0 released.
  - Quick start (3-5 comandos).
  - Feature highlights con links a docs por área.
  - Install matrix (cargo install / binary / build from source).
  - MCP setup hint con link a AGENT-SETUP.
  - ENGRAM credit + link a CREDITS.md.
  - License.
- **`docs/INSTALLATION.md`**:
  - Cargo install path.
  - Binary download path con sha256 verify.
  - Build-from-source.
  - El cache `~/.seele/embedder/` y `SEELE_EMBEDDER_DIR` override.
  - Troubleshooting: ONNX descarga falla → fallback Fake.
- **`docs/AGENT-SETUP.md`**:
  - `seele setup --agent claude-code` paso a paso con screenshot del config resultante.
  - Mismo para `cursor` y `windsurf`.
  - Aclaración sobre los 5 skeleton (cuándo llegan).
  - Verificación: `tools/list` desde el agente debe ver `seele_*` tools.
- **`docs/ENGRAM-MIGRATION.md`** (cierra ADR-13 docs deliverable):
  - Quién debería migrar (MNEMA users primaril).
  - Pre-migration: backup de `~/.mnema/mnema.db`.
  - `seele import from-engram --dry-run` first.
  - `seele import from-engram` real.
  - Verificación con `seele list --project <your-project>`.
  - Compat layer: `seele serve --legacy-engram-paths` + `seele mcp --tool-prefix mnema`.
  - Caveats: `linked_to[]` que apunte a non-ULID ids dangling.

~500 LOC markdown. Sin testing automated; review humana suficiente.

### Bloque E — Smoke + state-sync + tag v0.1.0

Cierre AEGIS + release real.

- **Smoke script `scripts/v0.1.0-smoke.sh`** que corre secuencialmente los 11 criterios de aceptación del MVP contra el binary construido:
  1. `cargo install --path crates/seele-cli` (criterio 1).
  2. `seele save / search / list / show / delete / restore / link / stats / projects / doctor` round-trip (criterio 2).
  3. `seele mcp` recibe `tools/list` y retorna 19 tools `seele_*` (criterio 3).
  4. `seele serve --port 0` + curl `/openapi.json` retorna 25+ paths (criterio 4).
  5. Perf smoke 10K observations search < 300ms (criterio 5 — el test `#[ignore]` ya existe en seele-search; en este bloque corre con `--ignored`).
  6. `seele-project` detect against synthetic git tree (criterio 6).
  7. `seele sync export → import` round-trip entre dos DBs (criterio 7).
  8. `seele tui` con `--smoke` (nuevo flag oculto que renderiza una frame y sale; sin esto no podemos automatizar el smoke de TUI) (criterio 8).
  9. CI matrix ya está verde (criterio 9).
  10. Tag `v0.1.0` dispara `release.yml` (criterio 10).
  11. README + CREDITS + release notes mencionan ENGRAM (criterio 11).
- **Devlog**: `docs/aegis/devlogs/2026-05-11-sprint-05-polish-release.md`.
- **State-sync**: plan a `executed/`, CHANGELOG `[Unreleased]` se convierte en `[0.1.0]`, INDEX, CLAUDE.md, memoria persistente, cost-ledger.
- **Tags** (orden importa):
  1. `git tag -a sprint-05-polish-release` (AEGIS sprint tag).
  2. `git tag -a v0.1.0` (SemVer release tag, dispara `release.yml`).
  3. Wait for release workflow to finish, verificar binarios en GH Releases.
- **Cloven review** post-release, antes de declarar v0.1 cerrada.

## Decisiones técnicas locked-in

1. **ONNX por default, fallback a Fake con warning** (Bloque B). No error-out por falla de red transitoria.
2. **crates.io publish DETRÁS de workflow_dispatch input** (Bloque C). Una publicación accidental es irreversible.
3. **No Homebrew tap en v0.1** (defer v0.2). Cargo install + binaries cubren los path mainstream.
4. **`v0.1.0` tag dispara release.yml**. El sprint tag (`sprint-05-polish-release`) es AEGIS, separado.
5. **TUI smoke flag oculto (`--smoke`)** para que el criterio 8 sea automatable sin terminal interactivo.
6. **Property tests con `proptest::proptest!` + shrinkers default**. Cualquier failure incluye automatic minimization.

## Riesgos identificados

| Riesgo | Mitigación |
|---|---|
| ONNX descarga falla en CI por restricción de red HF | Fallback a Fake + warn cierra el caso. CI tests con `--fake-embedder` explícito siguen pasando. |
| Build aarch64-linux requiere cross o emulación | Usar `cross` rust action o `ubuntu-latest` con QEMU. Si rompe, defer ese target a v0.1.1. |
| crates.io requiere `cargo publish` desde el workspace root con todos los crates publicados en orden de dependencia (11 publishes) | `cargo workspaces publish --from-git` o publish manual; mantener crates.io OFF este sprint mitiga el riesgo. |
| README rewrite + tres docs nuevos es mucho texto, propenso a desactualizarse | Cada doc linkea a un punto canónico (CLAUDE.md para reglas internas, sprint devlogs para historia). Evitar duplicación. |
| Sprint demasiado grande, no cierra en una pasada | Subdividir agresivamente. Si E se atrasa, cerrar Sprint-05 con A/B/C/D y dejar v0.1.0 tag para un Sprint-05.5 corto. |
| Tag `v0.1.0` push pre-release-workflow probado → si workflow tiene bug, el release público es público | Hacer un `v0.1.0-rc.1` pre-tag primero para probar el workflow end-to-end. Si rc1 pasa, retag a `v0.1.0`. |

## Out of scope para v0.1 (defer a v0.2 o posterior)

Lista explícita para que cuando alguien pregunte "¿por qué no?" haya respuesta:

- **5 skeleton agentes** (`opencode`, `aider`, `cody`, `continue`, `zed`): siguen retornando `NotImplemented`. v0.2.
- **Size-bound sync chunk splitter** (~1MB/chunk): v0.1 ship 1 chunk por export. v0.2 cuando reportes muestren que es necesario.
- **TUI editing features** ($EDITOR integration, soft-delete confirm, toggle axiomatic, add-link): v0.2.
- **`claude mcp add` delegation** cuando `claude` CLI presente: v0.2 (cierra la race residual con Claude Code corriendo).
- **Homebrew tap**: v0.2.
- **Live-debounced search en TUI**: v0.2 cuando ONNX sea default (cost negligible con embedder real).
- **Property tests para transports** (HTTP/MCP): defer; los E2E E2E del binary los cubren lo suficiente para v0.1.

## Cloven sights de sprints previos a cerrar

- **Cloven Sprint-02 [DEFERRED]**: property tests workspace-wide → Bloque A.
- **Cloven Sprint-03 [MEDIO]**: ONNX por default en CLI → Bloque B.
- **Cloven Sprint-04 deferreds**:
  - `--fake-embedder` unhide cuando ONNX ship → Bloque B.
  - Live-debounced search TUI → out of scope v0.1 (justificado arriba).
  - `claude mcp add` delegation → out of scope v0.1.

## Criterios de salida del Sprint-05

1. `cargo install --path crates/seele-cli` produce un binary funcional en `~/.cargo/bin/seele`.
2. `seele --version` muestra `0.1.0`.
3. ONNX es el embedder default; `seele doctor` reporta `onnx` cuando el modelo cargó, `fake` cuando hubo fallback.
4. Tag `v0.1.0` dispara release workflow, los 5 binarios + sha256 aparecen en GH Releases.
5. README, INSTALLATION, AGENT-SETUP, ENGRAM-MIGRATION están en `main` y se referencian entre sí.
6. ~340-360 tests verde (vs 316 hoy: +property tests). Clippy + fmt + STELE residual + CI matrix verdes.
7. Devlog publicado + plan a `executed/` + tags `sprint-05-polish-release` y `v0.1.0` pushed.
8. Cloven review post-cierre sin findings de severidad CRITICO.

---

**Gate 1**: Orlando aprueba la táctica antes de que arranque Bloque A. Pregunta abierta:

- ¿Scope OK? (5 bloques, ~1.5-2K LOC entre código + tests + docs).
- ¿Algo de "Out of scope" debería bajar a in-scope?
- ¿Te animás a publicar a crates.io en este sprint o lo dejamos detrás de workflow_dispatch como propongo?
- ¿El tag pre-release `v0.1.0-rc.1` antes del `v0.1.0` te suena, o vamos directo?
