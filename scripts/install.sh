#!/usr/bin/env bash
# scripts/install.sh — SEELE Unix installer.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/orlando-vazquez-career/seele/main/scripts/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/orlando-vazquez-career/seele/main/scripts/install.sh | bash -s -- v0.1.0
#
# Honors:
#   $SEELE_VERSION   — pin a specific version (default: latest stable release).
#   $SEELE_INSTALL_DIR — install destination (default: $HOME/.local/bin).
#
# What it does:
#   1. Detects OS + arch → release asset name.
#   2. Downloads tar.gz + .sha256 sidecar.
#   3. Verifies SHA256.
#   4. Extracts binary, places it in $SEELE_INSTALL_DIR, chmods +x.
#   5. Hints if $SEELE_INSTALL_DIR is not on $PATH.

set -euo pipefail

REPO="orlando-vazquez-career/seele"
INSTALL_DIR="${SEELE_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${SEELE_VERSION:-${1:-}}"

log()  { printf '%s\n' "seele-install: $*"; }
die()  { printf 'seele-install error: %s\n' "$*" >&2; exit 1; }

# --- Detect platform ---
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux)  target_os="unknown-linux-gnu" ;;
  Darwin) target_os="apple-darwin" ;;
  *)      die "unsupported OS: $os. Use install.ps1 for Windows; build from source for $os." ;;
esac
case "$arch" in
  x86_64|amd64) target_arch="x86_64" ;;
  aarch64|arm64) target_arch="aarch64" ;;
  *) die "unsupported arch: $arch (supported: x86_64, aarch64)" ;;
esac
target="${target_arch}-${target_os}"

# --- Resolve version ---
if [ -z "$VERSION" ]; then
  log "resolving latest release..."
  VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep -E '^\s*"tag_name"' | head -1 | sed -E 's/.*"([^"]+)".*/\1/')"
  [ -n "$VERSION" ] || die "could not resolve latest release tag — check repo or pass a version explicitly."
fi
# Strip leading 'v' for asset naming; tag remains as-is for the URL.
version_bare="${VERSION#v}"

asset="seele-${version_bare}-${target}.tar.gz"
sha_asset="${asset}.sha256"
base_url="https://github.com/${REPO}/releases/download/${VERSION}"

# --- Download + verify ---
tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT
log "downloading $asset ..."
curl -fsSL "${base_url}/${asset}" -o "${tmpdir}/${asset}"
curl -fsSL "${base_url}/${sha_asset}" -o "${tmpdir}/${sha_asset}"

log "verifying sha256 ..."
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$tmpdir" && sha256sum -c "${sha_asset}") || die "sha256 mismatch — refusing to install."
elif command -v shasum >/dev/null 2>&1; then
  expected="$(awk '{print $1}' < "${tmpdir}/${sha_asset}")"
  got="$(shasum -a 256 "${tmpdir}/${asset}" | awk '{print $1}')"
  [ "$expected" = "$got" ] || die "sha256 mismatch: expected $expected, got $got"
else
  die "no sha256 tool found (sha256sum or shasum); refusing to install."
fi

# --- Extract + place ---
mkdir -p "$INSTALL_DIR"
tar -xzf "${tmpdir}/${asset}" -C "$tmpdir"
mv "${tmpdir}/seele" "${INSTALL_DIR}/seele"
chmod +x "${INSTALL_DIR}/seele"
log "installed seele ${VERSION} → ${INSTALL_DIR}/seele"

case ":$PATH:" in
  *":${INSTALL_DIR}:"*) ;;
  *) log "note: ${INSTALL_DIR} is not on \$PATH. Add this to your shell rc:"
     log "      export PATH=\"${INSTALL_DIR}:\$PATH\""
     ;;
esac

"${INSTALL_DIR}/seele" --version
