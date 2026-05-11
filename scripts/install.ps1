# scripts/install.ps1 — SEELE Windows installer.
#
# Usage (PowerShell):
#   irm https://raw.githubusercontent.com/orlando-vazquez-career/seele/main/scripts/install.ps1 | iex
#   $env:SEELE_VERSION = 'v0.1.0'; irm <url> | iex
#
# Honors:
#   $env:SEELE_VERSION     — pin a specific version (default: latest stable).
#   $env:SEELE_INSTALL_DIR — install destination (default: $env:USERPROFILE\.seele\bin).
#
# Mirrors install.sh: detect arch → download zip + sha256 → verify → extract.

$ErrorActionPreference = 'Stop'

$repo = 'orlando-vazquez-career/seele'
$installDir = if ($env:SEELE_INSTALL_DIR) { $env:SEELE_INSTALL_DIR } else { Join-Path $env:USERPROFILE '.seele\bin' }
$version = $env:SEELE_VERSION

# --- Detect arch. v0.1 ships x86_64 only on Windows. ---
$arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
if ($arch -ne 'X64') {
    Write-Error "seele-install: unsupported Windows arch: $arch (x86_64 only in v0.1)"
}
$target = 'x86_64-pc-windows-msvc'

# --- Resolve version ---
if (-not $version) {
    Write-Host 'seele-install: resolving latest release...'
    $latest = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
    $version = $latest.tag_name
    if (-not $version) { Write-Error 'seele-install: could not resolve latest release tag.' }
}
$versionBare = $version.TrimStart('v')

$asset    = "seele-$versionBare-$target.zip"
$shaAsset = "$asset.sha256"
$baseUrl  = "https://github.com/$repo/releases/download/$version"

# --- Download + verify ---
$tmp = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "seele-install-$(Get-Random)")
try {
    Write-Host "seele-install: downloading $asset ..."
    Invoke-WebRequest -Uri "$baseUrl/$asset"    -OutFile (Join-Path $tmp $asset)
    Invoke-WebRequest -Uri "$baseUrl/$shaAsset" -OutFile (Join-Path $tmp $shaAsset)

    Write-Host 'seele-install: verifying sha256 ...'
    # sha256 sidecar is `<hex>  <filename>` on Linux/Mac and may have
    # different whitespace on Windows-produced files; -split '\s+' is
    # the robust way to grab the hex regardless of which builder wrote
    # it.
    $expected = ((Get-Content (Join-Path $tmp $shaAsset) -Raw).Trim() -split '\s+')[0].ToLower()
    $got = (Get-FileHash -Algorithm SHA256 (Join-Path $tmp $asset)).Hash.ToLower()
    if ($expected -ne $got) {
        Write-Error "seele-install: sha256 mismatch — expected $expected, got $got"
    }

    # --- Extract + place ---
    if (-not (Test-Path $installDir)) {
        New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    }
    Expand-Archive -Path (Join-Path $tmp $asset) -DestinationPath $tmp -Force
    Move-Item -Path (Join-Path $tmp 'seele.exe') -Destination (Join-Path $installDir 'seele.exe') -Force
    Write-Host "seele-install: installed seele $version → $installDir\seele.exe"

    # --- Path hint ---
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (-not $userPath.Split(';').Contains($installDir)) {
        Write-Host "seele-install: note: $installDir is not on your user PATH."
        Write-Host "seele-install: to add it permanently, run from a new shell:"
        Write-Host "      [Environment]::SetEnvironmentVariable('Path', `"`$env:Path;$installDir`", 'User')"
    }

    & (Join-Path $installDir 'seele.exe') '--version'
}
finally {
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
