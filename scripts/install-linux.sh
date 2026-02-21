#!/usr/bin/env bash
set -euo pipefail

# Termy Linux Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/lassejlv/termy/main/scripts/install-linux.sh | bash

REPO="lassejlv/termy"
INSTALL_DIR="${TERMY_INSTALL_DIR:-$HOME/.local/bin}"

die() {
  echo "Error: $*" >&2
  exit 1
}

log() {
  echo "==> $*"
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "'$1' is required but not found"
}

detect_arch() {
  local arch
  arch="$(uname -m)"
  case "$arch" in
    x86_64|amd64) echo "x86_64" ;;
    aarch64|arm64) echo "aarch64" ;;
    *) die "Unsupported architecture: $arch" ;;
  esac
}

# Check dependencies
require_cmd curl
require_cmd tar
require_cmd grep

log "Detecting system architecture..."
ARCH="$(detect_arch)"
log "Architecture: $ARCH"

log "Fetching latest release from GitHub..."
RELEASE_JSON="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest")"

TAG="$(echo "$RELEASE_JSON" | grep -oP '"tag_name":\s*"\K[^"]+')"
if [[ -z "$TAG" ]]; then
  die "Could not determine latest release tag"
fi
log "Latest version: $TAG"

# Find the Linux tarball for our architecture
DOWNLOAD_URL="$(echo "$RELEASE_JSON" | grep -oP '"browser_download_url":\s*"\K[^"]+' | grep -E "linux.*${ARCH}.*\.tar\.gz$" | head -n1)"

if [[ -z "$DOWNLOAD_URL" ]]; then
  # Try fallback: any Linux tarball
  DOWNLOAD_URL="$(echo "$RELEASE_JSON" | grep -oP '"browser_download_url":\s*"\K[^"]+' | grep -E "linux.*\.tar\.gz$" | head -n1)"
fi

if [[ -z "$DOWNLOAD_URL" ]]; then
  die "Could not find Linux tarball for architecture '$ARCH' in release $TAG"
fi

log "Download URL: $DOWNLOAD_URL"

# Create temp directory
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT

TARBALL_PATH="$TEMP_DIR/termy.tar.gz"

log "Downloading Termy $TAG..."
curl -fsSL "$DOWNLOAD_URL" -o "$TARBALL_PATH"

log "Extracting..."
tar -xzf "$TARBALL_PATH" -C "$TEMP_DIR"

# Find the binary
BINARY_PATH=""
if [[ -f "$TEMP_DIR/termy/termy" ]]; then
  BINARY_PATH="$TEMP_DIR/termy/termy"
elif [[ -f "$TEMP_DIR/termy" ]]; then
  BINARY_PATH="$TEMP_DIR/termy"
else
  # Search for it
  BINARY_PATH="$(find "$TEMP_DIR" -name "termy" -type f -executable 2>/dev/null | head -n1)"
fi

if [[ -z "$BINARY_PATH" || ! -f "$BINARY_PATH" ]]; then
  die "Could not find termy binary in downloaded tarball"
fi

# Create install directory if needed
mkdir -p "$INSTALL_DIR"

log "Installing to $INSTALL_DIR/termy..."
cp "$BINARY_PATH" "$INSTALL_DIR/termy"
chmod +x "$INSTALL_DIR/termy"

log "Termy $TAG installed successfully!"

# Check if install dir is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
  echo ""
  echo "NOTE: $INSTALL_DIR is not in your PATH."
  echo "Add it to your shell config:"
  echo ""
  echo "  # For bash (~/.bashrc):"
  echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
  echo ""
  echo "  # For zsh (~/.zshrc):"
  echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
  echo ""
  echo "  # For fish (~/.config/fish/config.fish):"
  echo "  set -gx PATH \$HOME/.local/bin \$PATH"
  echo ""
fi

echo ""
echo "Run 'termy' to start the terminal."
