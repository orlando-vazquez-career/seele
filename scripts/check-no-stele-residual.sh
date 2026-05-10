#!/usr/bin/env bash
# scripts/check-no-stele-residual.sh
#
# Static check: ningún archivo del repo debe contener referencias a 'stele'
# (lowercase) o 'STELE' (uppercase) excepto los esperados:
#   - genesis/plans/estrategia/03-naming-options.md (contexto histórico de naming)
#   - este script
#   - changelog si lo agregamos en el futuro
#
# Cierra observación [NIT] de Cloven 2026-05-10:
#   "asegurarse que en sprint-01 ni un solo archivo Rust generado tenga
#    `stele` en algún import o env var".
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

ALLOWLIST=(
  "genesis/plans/estrategia/03-naming-options.md"
  "genesis/plans/tactica/00-INDEX.md"
  "genesis/plans/tactica/sprint-01/00-INDEX.md"
  "genesis/plans/tactica/sprint-01/01-bloque-A-workspace-skeleton.md"
  "genesis/plans/tactica/sprint-01/04-bloque-D-tests-integration.md"
  ".github/workflows/ci.yml"
  "scripts/check-no-stele-residual.sh"
  "scripts/check-no-stele-residual.ps1"
  "CHANGELOG.md"
)

# Build grep --exclude args
EXCLUDES=()
for path in "${ALLOWLIST[@]}"; do
  EXCLUDES+=(--exclude="$(basename "$path")")
done

# Search for STELE / stele excluyendo allowlist files
MATCHES=$(grep -rn -E '\b(STELE|stele)\b' \
  --include='*.md' --include='*.rs' --include='*.toml' --include='*.yaml' \
  --include='*.yml' --include='*.json' --include='*.sh' --include='*.ps1' \
  --exclude-dir=target --exclude-dir=.git --exclude-dir=node_modules \
  "${EXCLUDES[@]}" \
  . 2>/dev/null | grep -v -F -f <(printf '%s\n' "${ALLOWLIST[@]}") || true)

if [ -n "$MATCHES" ]; then
  echo "ERROR: Found STELE/stele residuals outside allowlist:"
  echo "$MATCHES"
  echo ""
  echo "If a new file legitimately needs 'stele' (e.g., new historical doc),"
  echo "add it to the ALLOWLIST array in this script and re-run."
  exit 1
fi

echo "OK: no STELE/stele residuals found outside allowlist."
