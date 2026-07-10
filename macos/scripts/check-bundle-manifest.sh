#!/usr/bin/env bash
set -euo pipefail

APP_PATH=""
EXPECTED_ARCH=""
EXPECTED_BUNDLE_ID=""

usage() {
  cat <<EOF
Usage: $0 --app PATH [--arch arm64|x86_64] [--bundle-id IDENTIFIER]

Validate the runtime manifest shared by development and release Termy.app
bundles. The app, CLI, and FFI library must be executable Mach-O files with
matching architectures, and the app must link only the bundled FFI dylib.
EOF
}

fail() {
  echo "Error: $*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "'$1' is required"
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
    --bundle-id)
      [[ $# -ge 2 ]] || fail "--bundle-id requires a value"
      EXPECTED_BUNDLE_ID="$2"
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

[[ -n "$APP_PATH" ]] || fail "--app is required"
[[ -d "$APP_PATH" ]] || fail "app bundle not found: $APP_PATH"

case "$EXPECTED_ARCH" in
  ""|arm64|x86_64) ;;
  *) fail "unsupported architecture: $EXPECTED_ARCH" ;;
esac

require_cmd lipo
require_cmd otool

INFO_PLIST="$APP_PATH/Contents/Info.plist"
APP_BINARY="$APP_PATH/Contents/MacOS/Termy"
CLI_BINARY="$APP_PATH/Contents/MacOS/termy-cli"
FFI_DYLIB="$APP_PATH/Contents/Frameworks/libtermy_ffi.dylib"

[[ -f "$INFO_PLIST" ]] || fail "missing Info.plist: $INFO_PLIST"
[[ -x "$APP_BINARY" ]] || fail "missing executable app binary: $APP_BINARY"
[[ -x "$CLI_BINARY" ]] || fail "missing executable CLI helper: $CLI_BINARY"
[[ -x "$FFI_DYLIB" ]] || fail "missing executable FFI library: $FFI_DYLIB"

/usr/bin/plutil -lint "$INFO_PLIST" >/dev/null
if [[ -n "$EXPECTED_BUNDLE_ID" ]]; then
  actual_bundle_id="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$INFO_PLIST")"
  [[ "$actual_bundle_id" == "$EXPECTED_BUNDLE_ID" ]] \
    || fail "bundle identifier is $actual_bundle_id, expected $EXPECTED_BUNDLE_ID"
fi

normalize_archs() {
  tr ' ' '\n' | sed '/^$/d' | sort | tr '\n' ' ' | sed 's/ $//'
}

architectures() {
  local path="$1"
  local output
  output="$(lipo -archs "$path" 2>&1)" || fail "not a valid Mach-O file: $path ($output)"
  printf '%s' "$output" | normalize_archs
}

app_archs="$(architectures "$APP_BINARY")"
cli_archs="$(architectures "$CLI_BINARY")"
ffi_archs="$(architectures "$FFI_DYLIB")"

[[ "$cli_archs" == "$app_archs" ]] \
  || fail "CLI architecture ($cli_archs) differs from app ($app_archs): $CLI_BINARY"
[[ "$ffi_archs" == "$app_archs" ]] \
  || fail "FFI architecture ($ffi_archs) differs from app ($app_archs): $FFI_DYLIB"
if [[ -n "$EXPECTED_ARCH" ]]; then
  [[ "$app_archs" == "$EXPECTED_ARCH" ]] \
    || fail "app architecture is $app_archs, expected $EXPECTED_ARCH: $APP_BINARY"
fi

linked_ffi="$(otool -L "$APP_BINARY" | awk '/libtermy_ffi\.dylib/ {print $1}')"
[[ "$linked_ffi" == "@rpath/libtermy_ffi.dylib" ]] || {
  otool -L "$APP_BINARY" >&2
  fail "app must link exactly @rpath/libtermy_ffi.dylib (found: ${linked_ffi:-none})"
}

ffi_id="$(otool -D "$FFI_DYLIB" | sed -n '2p')"
[[ "$ffi_id" == "@rpath/libtermy_ffi.dylib" ]] \
  || fail "FFI install name is ${ffi_id:-missing}, expected @rpath/libtermy_ffi.dylib"

while IFS= read -r rpath; do
  case "$rpath" in
    */target/*|*/.build/*)
      fail "app contains build-directory rpath: $rpath"
      ;;
  esac
done < <(otool -l "$APP_BINARY" | awk '
  $1 == "cmd" && $2 == "LC_RPATH" { in_rpath = 1; next }
  in_rpath && $1 == "path" { print $2; in_rpath = 0 }
')

echo "Bundle manifest passed: $APP_PATH ($app_archs)"
