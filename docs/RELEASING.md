# RELEASING — runbook de release de SEELE

Patrón heredado de los dos runbooks de GRAIL (`release_planning.md` el
"antes", `releasing.md` el "durante"), condensado en uno. Última revisión:
2026-06-10 (sprint GRAIL-H1, Q1).

## Antes de tagear

1. **CHANGELOG**: mover el contenido de `[Unreleased]` a una sección
   `## [X.Y.Z] — YYYY-MM-DD`, dejar `[Unreleased]` vacío, y actualizar los
   link-refs del fondo (`[Unreleased]: ...compare/vX.Y.Z...HEAD` + el ref
   nuevo del tag).
2. **Versión en DOS lugares** de `Cargo.toml` raíz:
   - `workspace.package.version`
   - los 13 `seele-* = { path = ..., version = "X.Y.Z" }` de
     `workspace.dependencies` (crates.io exige `version` en path-deps;
     bumpean juntos).
3. **Gates completos** en el commit candidato:
   `cargo test --workspace` · `cargo clippy --workspace --all-targets -- -D warnings`
   · `cargo fmt --all -- --check` · `pwsh scripts/check-no-stele-residual.ps1`.
4. **docs/COMPARISON.md**: re-fechar y re-verificar números (la comparación
   desactualizada es peor que ninguna).
5. **Primera vez tras tocar `release.yml`**: tagear un RC
   (`vX.Y.Z-rc.1`) y mirar el run completo antes del tag real. El RC
   produce un pre-release (flag automático por el guión del tag).

## Tagear

```bash
git tag vX.Y.Z && git push origin vX.Y.Z
```

El workflow `release.yml` hace: check tag-vs-manifest → build matrix 5
targets (60 min timeout c/u) → GitHub Release con assets + SHA256 →
dry-run de `cargo publish` de los 14 crates en orden de dependencias.

El publish real a crates.io es un paso aparte, manual:
`workflow_dispatch` con `crates_io_publish=true`. **Los nombres en
crates.io son para siempre** — considerar publicar solo `seele-cli` en la
primera ronda (decisión pendiente de ADR-09).

## Después

- Verificar que `install.sh` / `install.ps1` resuelven los assets del tag
  nuevo desde una máquina limpia.
- Actualizar `CLAUDE.md` §Estado actual y cerrar el devlog del sprint.

## Fallos comunes (en prosa, con fix)

**Todos los build jobs mueren en segundos, sin log útil.** Antes de tocar
el YAML, mirá Settings → Billing del owner: las dos corridas fallidas de
v0.1.0 coincidieron con el bloqueo de GitHub Actions por billing
(2026-05-13). Con Actions bloqueadas el workflow muere al instante y el
YAML es inocente.

**Un job de macOS queda colgado horas.** Runner colgado, no build lento:
desde Q1 cada job tiene `timeout-minutes`; si vuelve a pasar, re-run del
job individual.

**`cargo publish --dry-run` falla con "all dependencies must have a
version specified".** Algún path-dep nuevo entró sin `version`: agregarlo
en `workspace.dependencies` del raíz (no inline en el crate).

**El dry-run falla por orden de dependencias.** La lista del workflow está
en orden topológico (core → storage → embedder → search → chat → sync →
engram-import → project → setup → eval → http → mcp → tui → cli). Un crate
nuevo entra DESPUÉS de todo lo que importa y ANTES de lo que lo importa.

**El tag no coincide con la versión del manifest.** El primer step lo
corta con error explícito; corregir versión o tag y re-tagear (borrar un
tag publicado: `git push origin :refs/tags/vX.Y.Z` — solo si el release
aún no salió).
