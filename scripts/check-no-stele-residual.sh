#!/usr/bin/env bash
# scripts/check-no-stele-residual.sh
#
# Static check: ningún archivo del repo debe contener referencias a 'stele'
# (lowercase) o 'STELE' (uppercase) excepto los esperados (allowlist).
#
# Cierra observación [NIT] de Cloven 2026-05-10:
#   "asegurarse que en sprint-01 ni un solo archivo Rust generado tenga
#    `stele` en algún import o env var".
#
# El allowlist es por path relativo (al root del repo), no por basename —
# se sincroniza con el script PowerShell hermano. El bug previo de
# filtrar por basename via `--exclude=$(basename ...)` quedó cerrado en
# Sprint-04 post-cierre (2026-05-11) cuando el job CI pwsh detectó
# residuos legítimos que el bash silenciaba por accidente.
#
# Portabilidad de `\b`: el regex usa `\b` (word boundary) bajo `grep -E`,
# que GNU grep y BSD grep modernos (ubuntu-latest + macos-latest CI)
# soportan sin problema. NO es portable a busybox grep (alpine docker
# minimal): allá `\b` se interpreta como literal-backslash-b y el match
# falla silencioso. Si Sprint-05+ agrega un runner alpine al CI matrix,
# reemplazar el patron por algo que use `[[:alnum:]]` lookaheads o
# escalar a `grep -P` (no portable a BSD). Cloven 2026-05-11 [NIT].
#
# Uso:
#   bash scripts/check-no-stele-residual.sh
#
# Exit code:
#   0 — no residuos
#   1 — encontró STELE/stele fuera de los allowlist

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Exact relative paths. Must mirror $allowlistFiles in the .ps1 sibling.
ALLOWLIST_FILES=(
  "genesis/plans/estrategia/03-naming-options.md"
  "genesis/plans/tactica/00-INDEX.md"
  ".github/workflows/ci.yml"
  "scripts/check-no-stele-residual.sh"
  "scripts/check-no-stele-residual.ps1"
  "CHANGELOG.md"
  "CLAUDE.md"
  "docs/INDEX.md"
)

# Directory prefixes (any file under these is allowlisted). Must mirror
# $allowlistDirs in the .ps1 sibling.
ALLOWLIST_DIRS=(
  "docs/aegis/devlogs/"
  "genesis/plans/executed/tactica/"
  # In-flight sprint plans (sprint-NN/) live here until their cierre
  # moves them to `executed/`. They legitimately mention the legacy
  # name when describing CI/static-check coverage.
  "genesis/plans/tactica/"
  # Post-v0.1 plans live under docs/plans/ per CLAUDE.md. Same allowance
  # as genesis/plans: in-flight and executed táctica may mention the
  # legacy name when documenting CI / static-check coverage.
  "docs/plans/tactica/"
  "docs/plans/executed/tactica/"
  # v0.3: estrategia plans + the architecture compendium legitimately
  # reference the legacy name (naming history + documenting the check itself).
  "docs/plans/estrategia/"
  "docs/plans/executed/estrategia/"
  "docs/compendium/"
)

is_allowlisted() {
  local rel="$1"
  for f in "${ALLOWLIST_FILES[@]}"; do
    if [ "$rel" = "$f" ]; then
      return 0
    fi
  done
  for d in "${ALLOWLIST_DIRS[@]}"; do
    case "$rel" in
      "$d"*) return 0 ;;
    esac
  done
  return 1
}

violations=""
while IFS=: read -r path lineno content; do
  rel="${path#./}"
  if is_allowlisted "$rel"; then
    continue
  fi
  violations+="${rel}:${lineno}: ${content}"$'\n'
done < <(grep -rn -E '\b(STELE|stele)\b' \
  --include='*.md' --include='*.rs' --include='*.toml' --include='*.yaml' \
  --include='*.yml' --include='*.json' --include='*.sh' --include='*.ps1' \
  --exclude-dir=target --exclude-dir=.git --exclude-dir=node_modules \
  . 2>/dev/null || true)

if [ -n "$violations" ]; then
  echo "ERROR: Found STELE/stele residuals outside allowlist:"
  printf '%s' "$violations"
  echo ""
  echo "If a new file legitimately needs 'stele' (e.g., new historical doc),"
  echo "add it to ALLOWLIST_FILES (exact path) or ALLOWLIST_DIRS (prefix)"
  echo "and re-run."
  exit 1
fi

echo "OK: no STELE/stele residuals found outside allowlist."
