#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_PATH=""
EXPECTED_ARCH=""
EXPECTED_VERSION=""
TEMP_ROOT=""

usage() {
  cat <<EOF
Usage: $0 --app PATH --arch arm64|x86_64 --version VERSION

Prove the unsigned release gate rejects representative bundle and DMG
corruption. All mutations happen in a temporary copy.
EOF
}

fail() {
  echo "Error: $*" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --app)
      [[ $# -ge 2 ]] || fail "--app requires a value"
      APP_PATH="$2"
      shift 2
      ;;
    --arch)
      [[ $# -ge 2 ]] || fail "--arch requires a value"
      EXPECTED_ARCH="$2"
      shift 2
      ;;
    --version)
      [[ $# -ge 2 ]] || fail "--version requires a value"
      EXPECTED_VERSION="${2#v}"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      fail "unknown option: $1"
      ;;
  esac
done

[[ -d "$APP_PATH" ]] || fail "app bundle not found: $APP_PATH"
case "$EXPECTED_ARCH" in
  arm64|x86_64) ;;
  *) fail "--arch must be arm64 or x86_64" ;;
esac
[[ "$EXPECTED_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "invalid --version: $EXPECTED_VERSION"

cleanup() {
  [[ -n "$TEMP_ROOT" ]] && rm -rf "$TEMP_ROOT"
}
trap cleanup EXIT

TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/termy-release-regressions.XXXXXX")"
CASE_APP="$TEMP_ROOT/Termy.app"
OUTPUT_FILE="$TEMP_ROOT/output.txt"

fresh_app() {
  rm -rf "$CASE_APP"
  cp -R "$APP_PATH" "$CASE_APP"
}

expect_failure() {
  local label="$1"
  local expected="$2"
  shift 2
  if "$@" >"$OUTPUT_FILE" 2>&1; then
    fail "$label unexpectedly passed"
  fi
  grep -F "$expected" "$OUTPUT_FILE" >/dev/null || {
    cat "$OUTPUT_FILE" >&2
    fail "$label failed without the expected diagnostic: $expected"
  }
  echo "Rejected as expected: $label"
}

readiness_for_app() {
  "$SCRIPT_DIR/check-release-readiness.sh" \
    --app "$CASE_APP" \
    --arch "$EXPECTED_ARCH" \
    --version "$EXPECTED_VERSION" \
    --skip-launch
}

fresh_app
rm "$CASE_APP/Contents/MacOS/termy-cli"
expect_failure "missing CLI helper" "missing executable CLI helper" readiness_for_app

fresh_app
cp /usr/bin/true "$CASE_APP/Contents/MacOS/termy-cli"
chmod +x "$CASE_APP/Contents/MacOS/termy-cli"
expect_failure "wrong CLI architecture" "CLI architecture" readiness_for_app

fresh_app
install_name_tool -change \
  "@rpath/libtermy_ffi.dylib" \
  "/tmp/target/release/libtermy_ffi.dylib" \
  "$CASE_APP/Contents/MacOS/Termy"
expect_failure "absolute FFI dependency" "app must link exactly @rpath/libtermy_ffi.dylib" readiness_for_app

fresh_app
rm "$CASE_APP/Contents/Resources/TermyIcon.png"
expect_failure "missing selectable logo" "missing required bundle resource" readiness_for_app

fresh_app
placeholder_id="com.ex""ample.termy"
/usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $placeholder_id" "$CASE_APP/Contents/Info.plist"
expect_failure "placeholder bundle identifier" "bundle identifier is $placeholder_id" readiness_for_app

fresh_app
BAD_ROOT="$TEMP_ROOT/bad-dmg-root"
BAD_DMG="$TEMP_ROOT/Termy-bad.dmg"
rm -rf "$BAD_ROOT"
mkdir -p "$BAD_ROOT"
cp -R "$CASE_APP" "$BAD_ROOT/Termy.app"
hdiutil create -volname "Termy Regression" -srcfolder "$BAD_ROOT" -ov -fs HFS+ -format UDZO "$BAD_DMG" >/dev/null
expect_failure \
  "DMG missing Applications link" \
  "DMG is missing Applications symlink" \
  "$SCRIPT_DIR/check-release-readiness.sh" \
  --dmg "$BAD_DMG" \
  --arch "$EXPECTED_ARCH" \
  --version "$EXPECTED_VERSION" \
  --skip-launch

echo "Unsigned release corruption regressions passed"
