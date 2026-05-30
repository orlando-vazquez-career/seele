# scripts/check-no-stele-residual.ps1
#
# Static check Windows: ningún archivo del repo debe contener referencias
# a 'stele' o 'STELE' excepto los esperados (allowlist).
#
# Cierra observación [NIT] de Cloven 2026-05-10.
#
# Uso:
#   pwsh scripts/check-no-stele-residual.ps1
#
# Exit code:
#   0 — no residuos
#   1 — encontró STELE/stele fuera de allowlist

$ErrorActionPreference = 'Stop'

$root = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location $root

$allowlistFiles = @(
    'genesis/plans/estrategia/03-naming-options.md',
    'genesis/plans/tactica/00-INDEX.md',
    '.github/workflows/ci.yml',
    'scripts/check-no-stele-residual.sh',
    'scripts/check-no-stele-residual.ps1',
    'CHANGELOG.md',
    'CLAUDE.md',
    'docs/INDEX.md'
)

# Directory prefixes (any file under these is allowlisted). Covers
# historical docs that legitimately reference the legacy "STELE" name:
# devlogs, executed sprint plans, in-flight sprint plans (which move
# to `executed/` only at cierre).
$allowlistDirs = @(
    'docs/aegis/devlogs/',
    'genesis/plans/executed/tactica/',
    'genesis/plans/tactica/',
    # Post-v0.1 plans live under docs/plans/ per CLAUDE.md. Same allowance
    # as genesis/plans: in-flight and executed táctica may mention the
    # legacy name when documenting CI / static-check coverage.
    'docs/plans/tactica/',
    'docs/plans/executed/tactica/',
    # v0.3: estrategia plans + the architecture compendium legitimately
    # reference the legacy name (naming history + documenting the check itself).
    'docs/plans/estrategia/',
    'docs/plans/executed/estrategia/',
    'docs/compendium/'
)

$includeExt = @('*.md', '*.rs', '*.toml', '*.yaml', '*.yml', '*.json', '*.sh', '*.ps1')

# Note on naming: avoid `$matches` here — PowerShell auto-populates
# `$Matches` after every `-match` regex evaluation, so using the same
# identifier as a user variable is legal but confusing for the next
# reader (Cloven 2026-05-11 [NIT]).
$violations = Get-ChildItem -Path . -Recurse -Include $includeExt -File `
    | Where-Object { $_.FullName -notmatch '\\(target|\.git|node_modules)\\' } `
    | ForEach-Object {
        $relPath = (Resolve-Path -Relative $_.FullName) -replace '\\', '/'
        $relPath = $relPath -replace '^\./', ''
        if ($allowlistFiles -contains $relPath) { return }
        foreach ($dir in $allowlistDirs) {
            if ($relPath.StartsWith($dir)) { return }
        }

        $hits = Select-String -Path $_.FullName -Pattern '\b(STELE|stele)\b'
        if ($hits) {
            foreach ($h in $hits) {
                "${relPath}:$($h.LineNumber): $($h.Line.Trim())"
            }
        }
    }

if ($violations) {
    Write-Host 'ERROR: Found STELE/stele residuals outside allowlist:' -ForegroundColor Red
    $violations | ForEach-Object { Write-Host $_ }
    Write-Host ''
    Write-Host 'If a new file legitimately needs "stele" (e.g., new historical doc),'
    Write-Host 'add it to $allowlistFiles (exact path) or $allowlistDirs (prefix) and re-run.'
    exit 1
}

Write-Host 'OK: no STELE/stele residuals found outside allowlist.' -ForegroundColor Green
exit 0
