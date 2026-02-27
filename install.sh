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
url="https://github.com/${REPO}/releases/download/${VERSION}/${archive}"

echo "Installing mdink ${VERSION} (${target}) → ${BIN_DIR}/mdink"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

curl -fsSL "$url" | tar xz -C "$tmp"

# Create BIN_DIR if it doesn't exist (e.g. ~/.local/bin on a fresh system)
mkdir -p "$BIN_DIR"
install -m 755 "$tmp/mdink" "$BIN_DIR/mdink"

echo "Done. Run: mdink --version"
