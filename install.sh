#!/usr/bin/env bash
# Install mdink — terminal markdown renderer
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/mdink-rs/mdink/main/install.sh | bash
#   VERSION=v0.2.0 bash install.sh          # pin a version
#   BIN_DIR=$HOME/.local/bin bash install.sh # install for current user only

set -euo pipefail

REPO="mdink-rs/mdink"
BIN_DIR="${BIN_DIR:-/usr/local/bin}"

# ── Detect OS and architecture ────────────────────────────────────────
os=$(uname -s | tr '[:upper:]' '[:lower:]')
arch=$(uname -m)

case "$os" in
  linux)
    case "$arch" in
      x86_64)          target="linux-x86_64"  ;;
      aarch64 | arm64) target="linux-aarch64" ;;
      *) echo "error: unsupported architecture: $arch" >&2; exit 1 ;;
    esac
    ;;
  darwin)
    case "$arch" in
      x86_64) target="macos-x86_64"  ;;
      arm64)  target="macos-aarch64" ;;
      *) echo "error: unsupported architecture: $arch" >&2; exit 1 ;;
    esac
    ;;
  *)
    echo "error: unsupported OS: $os (use the .zip on Windows)" >&2
    exit 1
    ;;
esac

# ── Resolve version ───────────────────────────────────────────────────
if [ -z "${VERSION:-}" ]; then
  VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' \
    | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')
fi

if [ -z "$VERSION" ]; then
  echo "error: could not determine latest release version" >&2
  exit 1
fi

# ── Download and install ──────────────────────────────────────────────
archive="mdink-${target}.tar.gz"
base="https://github.com/${REPO}/releases/download/${VERSION}"
url="${base}/${archive}"
sums_url="${base}/SHA256SUMS"

echo "Installing mdink ${VERSION} (${target}) → ${BIN_DIR}/mdink"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# Download the archive to disk (not piped) so we can verify it before extracting.
curl -fsSL "$url" -o "$tmp/$archive"

# ── Verify authenticity (minisign) + integrity (SHA256) ───────────────
# Trust chain, fail-closed at every step:
#   1. A minisign signature proves SHA256SUMS came from the mdink maintainer.
#      The public key is PINNED in this script — the trust anchor delivered
#      alongside it. Without this, SHA256SUMS is only as trustworthy as the
#      release host (an attacker who can swap the archive can swap the sums).
#   2. SHA256SUMS then proves the downloaded archive matches what was signed.
#
# Insecure opt-outs (in increasing order of risk):
#   MDINK_SKIP_MINISIG=1   signature only is skipped → integrity, not authenticity
#   MDINK_SKIP_CHECKSUM=1  skip everything (last resort, e.g. pre-checksum releases)

# Pinned minisign public key (the bare 'RW...' key string from `minisign -G`).
# MAINTAINER: replace the placeholder below with the project's real public key.
MINISIGN_PUBKEY="${MINISIGN_PUBKEY:-RWRmgiu+ux4uarEOjYoIKgDTmXfLmWVNKXVVLguBogC7+tGLW8QArgVD}"

fetch_sums() {
  if ! curl -fsSL "$sums_url" -o "$tmp/SHA256SUMS"; then
    echo "error: could not fetch SHA256SUMS from ${sums_url}" >&2
    echo "       this release may predate checksum publishing; to override (insecure):" >&2
    echo "       MDINK_SKIP_CHECKSUM=1 bash install.sh" >&2
    exit 1
  fi
}

# Verifies $tmp/SHA256SUMS against its detached signature using whichever
# minisign-compatible tool is installed (jedisct1 minisign, or the rsign2 port).
verify_signature() {
  case "$MINISIGN_PUBKEY" in
    *PIN_YOUR_MINISIGN_PUBLIC_KEY*)
      echo "error: install.sh has no real minisign public key pinned." >&2
      echo "       The maintainer must set MINISIGN_PUBKEY to the project's key." >&2
      echo "       To bypass signature checking (insecure): MDINK_SKIP_MINISIG=1 bash install.sh" >&2
      exit 1 ;;
  esac

  if ! curl -fsSL "${sums_url}.minisig" -o "$tmp/SHA256SUMS.minisig"; then
    echo "error: could not fetch SHA256SUMS.minisig from ${sums_url}.minisig" >&2
    echo "       to fall back to checksum-only (weaker): MDINK_SKIP_MINISIG=1 bash install.sh" >&2
    exit 1
  fi

  local ok=1
  if command -v minisign >/dev/null 2>&1; then
    minisign -Vm "$tmp/SHA256SUMS" -x "$tmp/SHA256SUMS.minisig" -P "$MINISIGN_PUBKEY" >/dev/null 2>&1 || ok=0
  elif command -v rsign >/dev/null 2>&1; then
    rsign verify -P "$MINISIGN_PUBKEY" -x "$tmp/SHA256SUMS.minisig" "$tmp/SHA256SUMS" >/dev/null 2>&1 || ok=0
  else
    echo "error: no signature tool found — install 'minisign'" >&2
    echo "       (https://jedisct1.github.io/minisign/) to verify release authenticity," >&2
    echo "       or fall back to checksum-only (weaker): MDINK_SKIP_MINISIG=1 bash install.sh" >&2
    exit 1
  fi

  if [ "$ok" -ne 1 ]; then
    echo "error: minisign signature verification FAILED for SHA256SUMS — refusing to install" >&2
    exit 1
  fi
  echo "Signature verified (minisign)."
}

verify_checksum() {
  local expected actual
  # Pull the expected hash for exactly our archive (field 2 == filename).
  expected=$(awk -v f="$archive" '$2 == f { print $1 }' "$tmp/SHA256SUMS")
  if [ -z "$expected" ]; then
    echo "error: no checksum for ${archive} in SHA256SUMS" >&2
    exit 1
  fi

  # Compute the local hash with whichever tool is available.
  if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$tmp/$archive" | awk '{print $1}')
  elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$tmp/$archive" | awk '{print $1}')
  else
    echo "error: no SHA-256 tool found (need 'sha256sum' or 'shasum')" >&2
    exit 1
  fi

  # Case-insensitive compare of the two hex digests.
  if [ "$(printf '%s' "$expected" | tr 'A-F' 'a-f')" \
     != "$(printf '%s' "$actual"   | tr 'A-F' 'a-f')" ]; then
    echo "error: checksum mismatch for ${archive} — refusing to install" >&2
    echo "  expected: $expected" >&2
    echo "  actual:   $actual" >&2
    exit 1
  fi

  echo "Checksum verified (sha256)."
}

if [ "${MDINK_SKIP_CHECKSUM:-0}" = "1" ]; then
  echo "WARNING: skipping ALL release verification (MDINK_SKIP_CHECKSUM=1)" >&2
else
  fetch_sums
  if [ "${MDINK_SKIP_MINISIG:-0}" = "1" ]; then
    echo "WARNING: skipping signature verification — integrity only (MDINK_SKIP_MINISIG=1)" >&2
  else
    verify_signature
  fi
  verify_checksum
fi

# Extract only after verification passes.
tar xzf "$tmp/$archive" -C "$tmp"

# Create BIN_DIR if it doesn't exist (e.g. ~/.local/bin on a fresh system)
mkdir -p "$BIN_DIR"
install -m 755 "$tmp/mdink" "$BIN_DIR/mdink"

echo "Done. Run: mdink --version"
