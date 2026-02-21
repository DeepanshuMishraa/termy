#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

usage() {
  cat <<EOF
Usage: $0 [--version VERSION] [--arch ARCH] [--target TARGET] [--format FORMAT]

Options:
  --version VERSION   Set version (default: read from Cargo.toml)
  --arch ARCH         Set architecture (x86_64 or aarch64)
  --target TARGET     Set target triple (x86_64-unknown-linux-gnu or aarch64-unknown-linux-gnu)
  --format FORMAT     Output format: tarball (default) or appimage
  --help, -h          Show this help message

Output:
  target/dist/Termy-<version>-linux-<arch>.tar.gz
  or
  target/dist/Termy-<version>-linux-<arch>.AppImage
EOF
}

die() {
  echo "Error: $*" >&2
  exit 1
}

log() {
  echo "==> $*"
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "'$1' is required"
}

read_version_from_cargo_toml() {
  awk '
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ && in_package { exit }
    in_package && $1 == "version" {
      gsub(/"/, "", $3)
      print $3
      exit
    }
  ' "$REPO_ROOT/Cargo.toml"
}

arch_to_target() {
  case "$1" in
    x86_64|amd64) echo "x86_64-unknown-linux-gnu" ;;
    aarch64|arm64) echo "aarch64-unknown-linux-gnu" ;;
    *) return 1 ;;
  esac
}

target_to_arch() {
  case "$1" in
    x86_64-unknown-linux-gnu) echo "x86_64" ;;
    aarch64-unknown-linux-gnu) echo "aarch64" ;;
    *) return 1 ;;
  esac
}

VERSION=""
ARCH=""
TARGET=""
FORMAT="tarball"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      [[ $# -ge 2 ]] || die "--version requires a value"
      VERSION="$2"
      shift 2
      ;;
    --arch)
      [[ $# -ge 2 ]] || die "--arch requires a value"
      ARCH="$2"
      shift 2
      ;;
    --target)
      [[ $# -ge 2 ]] || die "--target requires a value"
      TARGET="$2"
      shift 2
      ;;
    --format)
      [[ $# -ge 2 ]] || die "--format requires a value"
      FORMAT="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      die "Unknown option: $1 (use --help)"
      ;;
  esac
done

if [[ -z "$VERSION" ]]; then
  VERSION="$(read_version_from_cargo_toml)"
  [[ -n "$VERSION" ]] || die "Could not read version from Cargo.toml"
fi

# Strip leading 'v' if present
VERSION="${VERSION#v}"

if [[ -z "$ARCH" && -z "$TARGET" ]]; then
  ARCH="$(uname -m)"
fi

if [[ -n "$ARCH" && -z "$TARGET" ]]; then
  TARGET="$(arch_to_target "$ARCH")" || die "Unsupported architecture: $ARCH"
fi

if [[ -n "$TARGET" && -z "$ARCH" ]]; then
  ARCH="$(target_to_arch "$TARGET")" || die "Unsupported target: $TARGET"
fi

if [[ -n "$ARCH" && -n "$TARGET" ]]; then
  EXPECTED_TARGET="$(arch_to_target "$ARCH")" || die "Unsupported architecture: $ARCH"
  [[ "$EXPECTED_TARGET" == "$TARGET" ]] || die "Mismatched --arch ($ARCH) and --target ($TARGET)"
fi

APP_NAME="Termy"
APP_NAME_LOWER="termy"
OS_NAME="linux"
DIST_DIR="$REPO_ROOT/target/dist"
TARGET_RELEASE_DIR="$REPO_ROOT/target/$TARGET/release"
BINARY_PATH="$TARGET_RELEASE_DIR/$APP_NAME_LOWER"

require_cmd cargo

log "Building $APP_NAME v$VERSION for $ARCH ($TARGET)"
(cd "$REPO_ROOT" && cargo build --release --target "$TARGET")

[[ -f "$BINARY_PATH" ]] || die "Binary not found at $BINARY_PATH"

mkdir -p "$DIST_DIR"

case "$FORMAT" in
  tarball)
    STAGING_DIR="$REPO_ROOT/target/linux-staging"
    TARBALL_NAME="${APP_NAME}-${VERSION}-${OS_NAME}-${ARCH}.tar.gz"
    OUTPUT_PATH="$DIST_DIR/$TARBALL_NAME"

    log "Creating tarball staging directory"
    rm -rf "$STAGING_DIR"
    mkdir -p "$STAGING_DIR/$APP_NAME_LOWER"

    cp "$BINARY_PATH" "$STAGING_DIR/$APP_NAME_LOWER/"

    # Copy assets if they exist
    if [[ -d "$REPO_ROOT/assets" ]]; then
      mkdir -p "$STAGING_DIR/$APP_NAME_LOWER/assets"
      cp -r "$REPO_ROOT/assets/"* "$STAGING_DIR/$APP_NAME_LOWER/assets/" 2>/dev/null || true
    fi

    # Create a simple install script
    cat > "$STAGING_DIR/$APP_NAME_LOWER/install.sh" <<'INSTALL_SCRIPT'
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="${1:-$HOME/.local/bin}"

mkdir -p "$INSTALL_DIR"
cp "$SCRIPT_DIR/termy" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/termy"

echo "Installed termy to $INSTALL_DIR/termy"
echo "Make sure $INSTALL_DIR is in your PATH"
INSTALL_SCRIPT
    chmod +x "$STAGING_DIR/$APP_NAME_LOWER/install.sh"

    log "Creating tarball"
    (cd "$STAGING_DIR" && tar -czvf "$OUTPUT_PATH" "$APP_NAME_LOWER")
    rm -rf "$STAGING_DIR"

    echo "Done: $OUTPUT_PATH"
    ;;

  appimage)
    die "AppImage format not yet implemented"
    ;;

  *)
    die "Unknown format: $FORMAT (use tarball or appimage)"
    ;;
esac
