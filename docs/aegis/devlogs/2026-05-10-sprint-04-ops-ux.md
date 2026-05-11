# Sprint-04 — Ops & UX

**Fecha**: 2026-05-10
**Estado**: Cerrado
**Tag**: `sprint-04-ops-ux`

## Resumen

Sprint que cierra el set de crates "humanos" de v0.1: project detection, agent-setup wizard, git-friendly sync, ENGRAM migration tool, clap CLI completo, y TUI ratatui. Tras este sprint un usuario tiene todo lo que espera de un memory engine local — sin SQL, sin curl, todo desde `seele <subcommand>` o `seele tui`.

5 nuevos crates entregados (`seele-project`, `seele-setup`, `seele-sync`, `seele-engram-import`, `seele-tui`) más el binary `seele` rewriting hand-rolled-argv → `clap derive` con 17 subcommands. ENGRAM → SEELE migration tool desbloquea la transición MNEMA → SEELE (ADR-13). 304 tests verde (+102 vs Sprint-03), clippy + fmt + STELE residual pasando.

## Cambios entregados

### Bloque A — `seele-project` 5-case detection (commit `97fa485`)

Algoritmo heredado de ENGRAM para asignar `project name` a las observations:

1. `.seele/config.json` override explícito.
2. `git remote get-url origin` → parse → basename sin `.git`.
3. `git rev-parse --show-toplevel` → basename.
4. Git child scan depth 1 (max 20 dirs, timeout 200ms, skip `node_modules`/`target`/`.git`/`vendor`/`.venv`/`venv`/`__pycache__`/`dist`/`build`/`.next`/`.nuxt`).
5. Basename del `cwd` como fallback.

API: `detect(cwd) -> Result<ProjectName>` retorna `(name, source: DetectSource)` para que `seele doctor` lo exponga. ~280 LOC + 15 tests (7 unit `parse_remote_basename` + 8 E2E con tempdirs y git real, skip-if-no-git).

### Bloque B — `seele-setup` wizard agentes (commit `aeb5b21`)

Wizard que instala SEELE como MCP server en agent configs:

- **Implementados**: `claude-code` (`~/.claude.json`), `cursor` (`~/.cursor/mcp.json`), `windsurf` (`~/.codeium/windsurf/mcp_config.json`).
- **Skeleton** (declarados → `SetupError::NotImplemented`): `opencode`, `aider`, `cody`, `continue`, `zed`.

`install_mcp_json` compartido para los tres implementados — patrón `{mcpServers: {seele: {command, args}}}`. Backup automático `<path>.<ext>.bak.<unix_ms>`. Outcomes `Created`/`Added`/`Unchanged`/`Updated`/`DryRun`. 14 tests (12 E2E con `home_override` apuntando a tempdir).

### Bloque C — `seele-sync` chunks gzip JSON git-friendly (commit `f705016`)

`export_to_dir(observations, dir, filter) -> ExportReport` con `chunk_id = SHA-256` del payload determinista (observations ordenadas por id, `exported_at` y `seele_version` excluidos del hash). `import_from_file(observations, chunks, target_key, path) -> ImportReport` dedupea via `ChunkStore::was_imported(target_key, chunk_id)`. `read_chunk_file` verifica `format_version ≤ 1` + filename↔chunk_id mismatch. v0.1 ship 1 chunk per export; size-bound splitter (~1MB/chunk) diferido a Sprint-05.

### Bloque D.1+D.2 — `seele-cli` clap completo (commit `ef69c8b`)

Reemplazo completo del argv parser hand-rolled de Sprint-03 con `clap derive`. 16 subcommands: `save`, `search`, `show`, `list`, `delete`, `restore`, `link`, `stats`, `doctor`, `projects`, `sync export|import`, `import from-engram`, `setup`, `mcp`, `serve`. Global flags: `--db`/`--fake-embedder`/`--json`. Patrón uniforme: cada subcomando en `commands/<area>.rs` con su `ClapArgs` propio + `fn run()`.

`seele doctor` incorpora la sight Cloven de Sprint-03: cuando `model_id` contiene `"fake"`, emite `fake_embedder_warning` explícito en JSON + human output. 13 tests CLI E2E en `subcommands_e2e.rs` spawning el binary real con tempdir DB.

### Cloven post-review fixes (commit `d152842`)

4 findings cerrados:

- **CRITICO 1 — `seele-sync::import_from_file` atomicidad**: ahora envuelve cada `ObservationStore::save_in_tx` + el `ChunkStore::mark_imported_in_tx` en una sola transacción SQLite. Crash mid-loop o error tx-time hace rollback completo; un re-run reprocesa limpio. 2 tests de atomicidad reemplazan al redundante `target_key`-test.
- **CRITICO 2 — `seele-setup::write_atomic`**: reescrito como `tmp + rename`. Elimina partial-write corruption en `~/.claude.json`. Race residual con Claude Code corriendo (last-write-wins) documentada — v0.2 cerrará via delegación a `claude mcp add`.
- **ALTO — `setup --all` skeleton-aware**: `AgentKind::is_implemented()` + `implemented_agent_names()` filtra a los 3 agentes wired. `--list` sigue mostrando los 8 con etiqueta `[implemented]`/`[skeleton (v0.2)]`. `--fake-embedder` ocultado de `--help` mientras ONNX no ship (no-op visible mientras tanto).
- **MEDIO — `seele-project` git timeouts**: `git remote get-url` y `git rev-parse --show-toplevel` ahora corren en thread+mpsc con timeout 1500ms cada uno. Worst-case: 5-case detection cae a basename en ~3s vs blocking indefinido si git cuelga.

### Bloque D.3 — `seele import --from-engram` (commit `0b34785`)

Nuevo crate `seele-engram-import` (12vo del workspace) que cierra la primera decisión de ADR-13. `EngramImporter` consume `ObservationStore + LinkStore` y devuelve `ImportReport` con counts (`rows_seen`, `rows_inserted`, `rows_skipped_existing`, `rows_invalid`, `links_created`, `links_dangling`).

Mapeo:

| ENGRAM | SEELE |
|---|---|
| `memories.body` | `observations.content` |
| `memories.metadata` (JSON) | `observations.metadata` (preservado verbatim) |
| `memories.created_at` (int ms or ISO) | `observations.created_at` |
| `metadata.kind` | `ObservationType` typed (fallback Memory) |
| `metadata.project` | `observations.project` |
| `metadata.scope` | `observations.scope` |
| `metadata.topic_key` | `observations.topic_key` |
| `metadata.tool_name` | `observations.tool_name` |
| `metadata.linked_to[]` | `links` table, link_type=`related_to` |

ID handling: source ids que parsean como ULID se preservan verbatim (re-runs short-circuitan vía `INSERT OR IGNORE`). Cuid-style u otros formatos mintean un fresh `SeeleId` con el original stashed en `metadata.engram_id`.

Atomicidad: todas las inserciones de observations + links en una sola transacción SQLite. Metadata inválida es soft-error (contado en `rows_invalid`, listado en `errors`) — la tx aún commitea las good rows. Storage adds: `ObservationStore::save_raw_in_tx` (`RawSaveInput`/`RawSaveOutcome`) y `LinkStore::create_in_tx`. `--re-embed` recognized pero no-op en v0.1 (FakeEmbedder único backend); tracing::warn surface eso.

23 tests (11 unit + 11 E2E con tempdir ENGRAM synthetic DB + 1 CLI subcommand E2E).

### Bloque E — `seele-tui` ratatui (commit `40595cd`)

ADR-07 implementado: 5 vistas vi-keymapped sobre un tokio event-loop, hablando con `SeeleService` in-process.

Vistas:

- **Home**: stats card + breakdown by-kind / by-scope.
- **Browse**: lista plana de las 50 observaciones más recientes (j/k, enter abre Detail).
- **Search**: query prompt + result list (RRF score badge coloreado green/yellow/grey).
- **Detail**: header card + body + metadata pretty-printed JSON.
- **Stats**: contadores fuller (observations + sessions) + breakdowns.

Keymap:

```
1-5     switch pane         q       quit            Ctrl-C   quit
j/k     move selection      g/G     top/bottom      enter    detail
/       jump to Search      r       refresh         esc      back
```

Detalles de safety: `crossterm` con feature `event-stream` para no bloquear el tokio select loop. RAII `TerminalGuard` restaura raw mode + alt screen incluso ante panic. Search pane consume printable input ANTES de los hotkeys globales para que digits typed en una query no cambien de pane.

CLI: nuevo subcomando `seele tui` (`seele --help` lista 17 subcomandos ahora).

17 tests (7 state + 9 keymap + 10 snapshot con `insta` + `TestBackend` 120×30 — width pinned para reproducibilidad cross-OS).

### Bloque F — State-sync + tag (este commit)

- Devlog (este archivo), plan táctico a `genesis/plans/executed/tactica/sprint-04/`, CHANGELOG, INDEX, CLAUDE.md, memoria persistente, cost-ledger.
- Tag `sprint-04-ops-ux`.
- Cloven review post-cierre (referenciado en commit subsiguiente si hay correcciones).

## Decisiones técnicas

1. **CLI llama al service local, no al HTTP**: lockedin del plan táctico. `seele save` y compañía construyen `SeeleService::new(pool, embedder)` directo. HTTP queda reservado para consumers externos (web UI, scripts no-Rust).

2. **TUI llama al service local, no al HTTP**: mismo razonamiento. `seele_tui::run_tui(service)` toma el service en-process; sin loopback.

3. **TUI emergency-restore via RAII guard**: `TerminalGuard` en `Drop` corre `disable_raw_mode` + `LeaveAlternateScreen` con `let _ =`. Pánico no deja al usuario con la terminal rota.

4. **TUI Search pane consume printable input ANTES de globals**: digits typed en query no triggerean pane hotkeys. Test `search_pane_swallows_digits_too` lo guarda.

5. **`seele-sync` import en una sola transacción**: cierre de la sight CRITICO 1 de Cloven. `ObservationStore::save_in_tx` + `ChunkStore::mark_imported_in_tx` + `tx.commit()` en `import_from_file`. Crash mid-loop deja la destination DB intacta y el ledger sin marca, así un re-run reprocesa limpio.

6. **`seele-setup::write_atomic` real**: tmp + rename, no `std::fs::write` directo. Rename atómico en POSIX y en Windows (std::fs::rename usa `MoveFileExW` con `REPLACE_EXISTING`).

7. **`seele-project` git timeouts por thread + mpsc**: 1500ms cap por invocación. Sized para "git no responde" (credential prompt, NFS hang) no para "git lento". Subprocess huérfano abandonado (read-path sin side effects).

8. **ENGRAM ID preservation con breadcrumb fallback**: source ids que parsean como ULID se preservan verbatim. Cuid-style mintean nuevo `SeeleId` con `metadata.engram_id` stash. Re-imports idempotent vía `INSERT OR IGNORE` cuando el id ya está.

9. **Snapshot tests TUI con width pinned**: `TestBackend::new(120, 30)`. Sin pin las diferencias de terminal entre Linux/Mac/Win romperían CI. `insta` snapshots checkeados.

10. **`seele-engram-import` como crate separado**: mirrors el patrón de `seele-sync`. Mantiene ENGRAM-schema knowledge fuera de storage y CLI; futuros importers (JSON, CSV) drop-in side-by-side sin tocar storage o cli.

## Incidentes durante la ejecución

- **Clippy 1.95 `type_complexity` en `seele-engram-import`**: tuple de 7 elementos en return type. Refactorizado a `ParsedMetadata` struct con `empty()` (ObservationType no tiene Default así que `derive(Default)` no aplica).
- **utoipa `ObservationDto` requirió `type` field** en construcciones manuales (tests TUI + tests CLI). Inicializaciones agregadas.
- **crossterm `EventStream` necesita feature flag `event-stream`**: agregado en `seele-tui/Cargo.toml`.
- **Cloven CRITICO 1 (sync atomicity)**: import sin tx hacía half-commits si una save fallaba mid-loop. Cerrado por el commit `d152842`.
- **Cloven CRITICO 2 (write_atomic falso)**: `std::fs::write` directo podía dejar `~/.claude.json` partial. Cerrado por el mismo commit con tmp+rename.
- **Linker LNK1104 sporadic en Windows** (file locked) durante runs concurrentes — autoresolved al reintentar.
- **Cargo fmt reformateó archivos varias veces post-Write** (cosmético, sin regresiones).

## Cómo reproducir

```powershell
cargo test --workspace --all-features       # 304 verde + 4 ignored
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bash scripts/check-no-stele-residual.sh

# Binary
cargo build --release -p seele-cli
target/release/seele --help                  # 17 subcommands

# CLI E2E
seele save "first observation" "content here" --project p
seele list --project p
seele search "first" --project p
seele stats
seele doctor

# Sync round-trip
seele --db ~/.seele/src.db save "shared" "thought" --project p
seele --db ~/.seele/src.db sync export ./chunks --project p
seele --db ~/.seele/dst.db sync import ./chunks/*.json.gz

# ENGRAM migration (ADR-13)
seele import from-engram ~/.mnema/mnema.db --dry-run
seele import from-engram ~/.mnema/mnema.db

# Setup
seele setup --list                          # implemented + skeleton tags
seele setup --agent claude-code --dry-run

# TUI
seele tui                                   # opens ratatui interactive
```

## Pendiente para próximos sprints

- **Sprint-05 (Polish + CI/CD + Release)**:
  - ONNX por default (FakeEmbedder solo via flag) — flag `--fake-embedder` se unhide.
  - Size-bound sync chunk splitter (~1MB).
  - 5 skeleton agentes implementados (opencode, aider, cody, continue, zed).
  - Property tests workspace-wide (proptest).
  - Release pipeline (5 binaries + sha256 + auto-changelog).
  - Docs `ENGRAM-MIGRATION.md` linkeada desde README.
  - `claude mcp add` delegation cuando `claude` CLI presente (cierra race residual con Claude Code).
  - TUI: $EDITOR integration, soft-delete confirm, toggle axiomatic, add-link prompt.

## Uso y costo

| Sesión | Modelo | Tokens (estimated) | Notas |
|---|---|---|---|
| Sprint-04 ejecución (A → E) | claude-opus-4-7 | ~620k input + ~140k output | 6 bloques + Cloven fixes + tests + nuevo crate seele-engram-import |
| Sprint-04 state-sync | claude-opus-4-7 | ~30k input + ~14k output | Este devlog + cierre |

Cost-ledger: `docs/aegis/devlogs/cost-ledger.jsonl` (entries con `"estimated": true`).

## Referencias

- Commits: `97fa485` (A), `aeb5b21` (B), `f705016` (C), `ef69c8b` (D.1+D.2), `d152842` (post-Cloven fixes), `0b34785` (D.3), `40595cd` (E), este (F).
- ADRs: `genesis/plans/arquitectura/07-tui-design.md`, `13-engram-compatibility.md`.
- Cloven reviews: 2026-05-10 [post Sprint-04 A→D.2] — 4 findings (CRITICO 1, CRITICO 2, ALTO×2, MEDIO), todos cerrados en commit `d152842` antes de D.3.
- Plan táctico ejecutado: `genesis/plans/executed/tactica/sprint-04/`.
