#!/usr/bin/env bash
set -euo pipefail

APP_NAME="Termy"
DMG_NAME="termy"
RELEASE_DIR="target/release"
BUNDLE_DIR="$RELEASE_DIR/bundle/osx"
APP_PATH="$BUNDLE_DIR/$APP_NAME.app"
DMG_ROOT="target/dmg-root"
RW_DMG="$RELEASE_DIR/$DMG_NAME-rw.dmg"
OUTPUT_DMG="$RELEASE_DIR/$DMG_NAME.dmg"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is not installed"
  exit 1
fi

if ! command -v hdiutil >/dev/null 2>&1; then
  echo "hdiutil is not available (this script must run on macOS)"
  exit 1
fi

if ! command -v osascript >/dev/null 2>&1; then
  echo "osascript is not available (required to arrange DMG window)"
  exit 1
fi

if ! cargo bundle --version >/dev/null 2>&1; then
  echo "cargo-bundle not found. Install it with: cargo install cargo-bundle"
  exit 1
fi

echo "Building macOS app bundle..."
cargo bundle --release --format osx

if [ ! -d "$APP_PATH" ]; then
  echo "Expected app bundle not found at: $APP_PATH"
  exit 1
fi

echo "Preparing DMG staging folder..."
rm -rf "$DMG_ROOT"
mkdir -p "$DMG_ROOT"
cp -R "$APP_PATH" "$DMG_ROOT/"
ln -s /Applications "$DMG_ROOT/Applications"

echo "Creating unsigned DMG..."
rm -f "$RW_DMG" "$OUTPUT_DMG"

hdiutil create \
  -volname "$APP_NAME" \
  -srcfolder "$DMG_ROOT" \
  -ov \
  -format UDRW \
  "$RW_DMG"

MOUNT_INFO="$(hdiutil attach -readwrite -noverify -noautoopen "$RW_DMG")"
DEVICE="$(printf '%s\n' "$MOUNT_INFO" | awk '/\/Volumes\// {print $1; exit}')"
MOUNT_POINT="/Volumes/$APP_NAME"

if [ -z "$DEVICE" ] || [ ! -d "$MOUNT_POINT" ]; then
  echo "Failed to mount temporary DMG"
  exit 1
fi

cleanup() {
  if [ -n "${DEVICE:-}" ]; then
    hdiutil detach "$DEVICE" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

osascript <<EOF
tell application "Finder"
  tell disk "$APP_NAME"
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    set bounds of container window to {120, 120, 660, 440}
    set opts to the icon view options of container window
    set arrangement of opts to not arranged
    set icon size of opts to 128
    set text size of opts to 12
    set position of item "$APP_NAME.app" to {150, 180}
    set position of item "Applications" to {390, 180}
    close
    open
    update without registering applications
    delay 1
  end tell
end tell
EOF

hdiutil detach "$DEVICE"
DEVICE=""

hdiutil convert "$RW_DMG" -format UDZO -imagekey zlib-level=9 -o "$OUTPUT_DMG"
rm -f "$RW_DMG"

echo "Done: $OUTPUT_DMG"
