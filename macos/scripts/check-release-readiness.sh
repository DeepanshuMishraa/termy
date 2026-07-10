#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MACOS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$MACOS_DIR/.." && pwd)"
APP_PATH=""
DMG_PATH=""
EXPECTED_ARCH=""
EXPECTED_VERSION=""
SKIP_LAUNCH=0
MOUNT_DEVICE=""
TEMP_ROOT=""

usage() {
  cat <<EOF
Usage: $0 [options]

Validate an unsigned native macOS release candidate. With no artifact option,
run static source/packaging parity checks only.

Options:
  --app PATH            Validate and launch a staged Termy.app
  --dmg PATH            Verify, mount read-only, validate, and launch a DMG
  --arch ARCH           Require arm64 or x86_64 across every Mach-O file
  --version VERSION     Require this semantic app/build version (leading v accepted)
  --skip-launch         Skip the usable-window probe (corruption tests only)
  --help                Show this help

Apple trust validation is intentionally out of scope here and remains Task 10.
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
    --dmg)
      [[ $# -ge 2 ]] || fail "--dmg requires a value"
      DMG_PATH="$2"
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
    --skip-launch)
      SKIP_LAUNCH=1
      shift
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

[[ -z "$APP_PATH" || -z "$DMG_PATH" ]] || fail "use either --app or --dmg, not both"
case "$EXPECTED_ARCH" in
  ""|arm64|x86_64) ;;
  *) fail "unsupported architecture: $EXPECTED_ARCH" ;;
esac

cleanup() {
  if [[ -n "$MOUNT_DEVICE" ]]; then
    hdiutil detach "$MOUNT_DEVICE" -quiet >/dev/null 2>&1 || true
  fi
  if [[ -n "$TEMP_ROOT" ]]; then
    rm -rf "$TEMP_ROOT"
  fi
}
trap cleanup EXIT

require_cmd awk
require_cmd file
require_cmd lipo
require_cmd otool
require_cmd codesign

read_cargo_version() {
  awk '
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ && in_package { exit }
    in_package && $1 == "version" {
      gsub(/"/, "", $3)
      print $3
      exit
    }
  ' "$REPO_ROOT/crates/desktop_app/Cargo.toml"
}

source_bundle_id="$(awk -F'"' '/static let bundleIdentifier/ { print $2; exit }' "$MACOS_DIR/Sources/TermySwift/App/TermySwiftApp.swift")"
run_bundle_id="$(awk -F'"' '/^BUNDLE_ID=/ { print $2; exit }' "$MACOS_DIR/scripts/build_and_run.sh")"
xtask_bundle_id="$(awk -F'"' '/const BUNDLE_ID: &str =/ { print $2; exit }' "$REPO_ROOT/crates/xtask/src/macos.rs")"
dmg_bundle_id="$(awk -F'"' '/^BUNDLE_ID=/ { print $2; exit }' "$MACOS_DIR/scripts/build-dmg.sh")"
xtask_minimum="$(awk -F'"' '/const MIN_SYSTEM_VERSION: &str =/ { print $2; exit }' "$REPO_ROOT/crates/xtask/src/macos.rs")"
dmg_minimum="$(awk -F'"' '/^MIN_SYSTEM_VERSION=/ { print $2; exit }' "$MACOS_DIR/scripts/build-dmg.sh")"
source_version="$(read_cargo_version)"
[[ -n "$EXPECTED_VERSION" ]] || EXPECTED_VERSION="$source_version"

echo "==> Checking native metadata source parity"
[[ -n "$source_bundle_id" ]] || fail "could not read AppMetadata.bundleIdentifier"
[[ "$source_bundle_id" == "$run_bundle_id" ]] || fail "build_and_run.sh bundle ID ($run_bundle_id) differs from source ($source_bundle_id)"
[[ "$source_bundle_id" == "$xtask_bundle_id" ]] || fail "xtask bundle ID ($xtask_bundle_id) differs from source ($source_bundle_id)"
[[ "$source_bundle_id" == "$dmg_bundle_id" ]] || fail "build-dmg.sh bundle ID ($dmg_bundle_id) differs from source ($source_bundle_id)"
[[ "$source_bundle_id" =~ ^[A-Za-z0-9][A-Za-z0-9-]*(\.[A-Za-z0-9][A-Za-z0-9-]*)+$ ]] || fail "bundle ID is not reverse-DNS-like: $source_bundle_id"
[[ -n "$xtask_minimum" && "$xtask_minimum" == "$dmg_minimum" ]] || fail "minimum macOS versions differ: xtask=$xtask_minimum dmg=$dmg_minimum"
[[ "$EXPECTED_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "expected version is not numeric semantic version: $EXPECTED_VERSION"

if grep -R -n -E 'com\.example|PRODUCT_BUNDLE_IDENTIFIER *= *com\.example' \
  "$MACOS_DIR/Sources" "$MACOS_DIR/scripts" "$REPO_ROOT/crates/xtask/src" >/dev/null; then
  grep -R -n -E 'com\.example|PRODUCT_BUNDLE_IDENTIFIER *= *com\.example' \
    "$MACOS_DIR/Sources" "$MACOS_DIR/scripts" "$REPO_ROOT/crates/xtask/src" >&2
  fail "placeholder bundle identifier found in native macOS sources or scripts"
fi

echo "==> Checking signing/notarization hooks without requiring credentials"
require_pattern() {
  local pattern="$1"
  local path="$2"
  local message="$3"
  grep -n -E -- "$pattern" "$path" >/dev/null || fail "$message"
}
require_pattern '--sign-identity' "$MACOS_DIR/scripts/build-dmg.sh" "native DMG script must accept --sign-identity"
require_pattern 'codesign' "$MACOS_DIR/scripts/build-dmg.sh" "native DMG script must support app signing"
require_pattern 'notarytool' "$MACOS_DIR/scripts/build-dmg.sh" "native DMG script must support notarization"
require_pattern 'stapler staple' "$MACOS_DIR/scripts/build-dmg.sh" "native DMG script must staple notarized artifacts"
require_pattern '--options runtime' "$MACOS_DIR/scripts/build-dmg.sh" "native app signing must enable hardened runtime"

plist_value() {
  local plist="$1"
  local key="$2"
  /usr/libexec/PlistBuddy -c "Print :$key" "$plist" 2>/dev/null \
    || fail "missing Info.plist field: $key ($plist)"
}

normalize_archs() {
  tr ' ' '\n' | sed '/^$/d' | sort | tr '\n' ' ' | sed 's/ $//'
}

architectures() {
  local path="$1"
  local output
  output="$(lipo -archs "$path" 2>&1)" || fail "not a valid Mach-O file: $path ($output)"
  printf '%s' "$output" | normalize_archs
}

validate_macho_paths() {
  local app="$1"
  local app_binary="$app/Contents/MacOS/Termy"
  local app_archs
  app_archs="$(architectures "$app_binary")"

  while IFS= read -r binary; do
    file -b "$binary" | grep -q 'Mach-O' || continue
    binary_archs="$(architectures "$binary")"
    [[ "$binary_archs" == "$app_archs" ]] \
      || fail "Mach-O architecture ($binary_archs) differs from app ($app_archs): $binary"

    while IFS= read -r dependency; do
      case "$dependency" in
        /Users/*|/home/*|*/target/*|*/.build/*)
          fail "Mach-O contains build-machine dependency path: $binary -> $dependency"
          ;;
      esac
    done < <(otool -L "$binary" | awk 'NR > 1 { print $1 }')

    while IFS= read -r rpath; do
      case "$rpath" in
        /Users/*|/home/*|*/target/*|*/.build/*)
          fail "Mach-O contains build-machine rpath: $binary -> $rpath"
          ;;
      esac
    done < <(otool -l "$binary" | awk '
      $1 == "cmd" && $2 == "LC_RPATH" { in_rpath = 1; next }
      in_rpath && $1 == "path" { print $2; in_rpath = 0 }
    ')
  done < <(find "$app/Contents" -type f -print)
}

validate_app() {
  local app="$1"
  [[ -d "$app" ]] || fail "app bundle not found: $app"
  [[ -d "$app/Contents/MacOS" ]] || fail "missing bundle directory: $app/Contents/MacOS"
  [[ -d "$app/Contents/Frameworks" ]] || fail "missing bundle directory: $app/Contents/Frameworks"
  [[ -d "$app/Contents/Resources" ]] || fail "missing bundle directory: $app/Contents/Resources"

  manifest_args=(--app "$app" --bundle-id "$source_bundle_id")
  [[ -n "$EXPECTED_ARCH" ]] && manifest_args+=(--arch "$EXPECTED_ARCH")
  "$SCRIPT_DIR/check-bundle-manifest.sh" "${manifest_args[@]}"

  local plist="$app/Contents/Info.plist"
  local executable bundle_id bundle_name package_type short_version build_version minimum icon_file url_name url_scheme
  executable="$(plist_value "$plist" CFBundleExecutable)"
  bundle_id="$(plist_value "$plist" CFBundleIdentifier)"
  bundle_name="$(plist_value "$plist" CFBundleName)"
  package_type="$(plist_value "$plist" CFBundlePackageType)"
  short_version="$(plist_value "$plist" CFBundleShortVersionString)"
  build_version="$(plist_value "$plist" CFBundleVersion)"
  minimum="$(plist_value "$plist" LSMinimumSystemVersion)"
  icon_file="$(plist_value "$plist" CFBundleIconFile)"
  url_name="$(plist_value "$plist" CFBundleURLTypes:0:CFBundleURLName)"
  url_scheme="$(plist_value "$plist" CFBundleURLTypes:0:CFBundleURLSchemes:0)"

  [[ "$executable" == "Termy" ]] || fail "CFBundleExecutable is $executable, expected Termy"
  [[ "$bundle_id" == "$source_bundle_id" ]] || fail "CFBundleIdentifier is $bundle_id, expected $source_bundle_id"
  [[ "$bundle_name" == "Termy" ]] || fail "CFBundleName is $bundle_name, expected Termy"
  [[ "$package_type" == "APPL" ]] || fail "CFBundlePackageType is $package_type, expected APPL"
  [[ "$short_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "invalid semantic CFBundleShortVersionString: $short_version"
  [[ "$short_version" == "$EXPECTED_VERSION" ]] || fail "app version is $short_version, expected $EXPECTED_VERSION"
  [[ "$build_version" == "$EXPECTED_VERSION" ]] || fail "build version is $build_version, expected $EXPECTED_VERSION"
  [[ "$minimum" == "$dmg_minimum" ]] || fail "minimum macOS version is $minimum, expected $dmg_minimum"
  [[ "$url_name" == "$source_bundle_id" ]] || fail "URL type name is $url_name, expected $source_bundle_id"
  [[ "$url_scheme" == "termy" ]] || fail "URL scheme is $url_scheme, expected termy"
  [[ "$icon_file" == "TermyIcon" || "$icon_file" == "TermyIcon.icns" ]] || fail "CFBundleIconFile is $icon_file, expected TermyIcon"

  for resource in TermyIcon.icns TermyIcon.png termy_old_icon.png; do
    [[ -s "$app/Contents/Resources/$resource" ]] || fail "missing required bundle resource: $app/Contents/Resources/$resource"
  done
  /usr/bin/plutil -p "$plist" | grep -q 'com\.example' \
    && fail "placeholder identifier found in staged Info.plist: $plist"

  validate_macho_paths "$app"
  codesign --verify --deep --strict --verbose=2 "$app" 2>&1 \
    || fail "bundle has invalid nested or resource signatures: $app"

  if [[ "$SKIP_LAUNCH" -eq 0 ]]; then
    "$SCRIPT_DIR/check-usable-launch.sh" --app "$app"
  fi
}

validate_dmg() {
  local dmg="$1"
  [[ -f "$dmg" ]] || fail "DMG not found: $dmg"
  require_cmd hdiutil

  echo "==> Verifying DMG integrity: $dmg"
  hdiutil verify "$dmg" >/dev/null || fail "DMG integrity verification failed: $dmg"

  TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/termy-release-ready.XXXXXX")"
  local mount_point="$TEMP_ROOT/mount"
  mkdir -p "$mount_point"
  local attach_info
  attach_info="$(hdiutil attach -readonly -nobrowse -noautoopen -mountpoint "$mount_point" "$dmg")" \
    || fail "could not mount DMG read-only: $dmg"
  MOUNT_DEVICE="$(printf '%s\n' "$attach_info" | awk '/^\/dev\// { print $1; exit }')"
  [[ -n "$MOUNT_DEVICE" && -d "$mount_point" ]] || fail "could not determine mounted DMG device: $attach_info"

  echo "==> Checking mounted DMG root: $mount_point"
  [[ -L "$mount_point/Applications" ]] || fail "DMG is missing Applications symlink: $mount_point/Applications"
  [[ "$(readlink "$mount_point/Applications")" == "/Applications" ]] \
    || fail "DMG Applications link must target /Applications"

  app_count="$(find "$mount_point" -mindepth 1 -maxdepth 1 -type d -name '*.app' | wc -l | tr -d ' ')"
  [[ "$app_count" == "1" ]] || fail "DMG must contain exactly one root app bundle (found $app_count)"
  [[ -d "$mount_point/Termy.app" ]] || fail "DMG root app must be named Termy.app"

  validate_app "$mount_point/Termy.app"
}

if [[ -n "$DMG_PATH" ]]; then
  validate_dmg "$DMG_PATH"
elif [[ -n "$APP_PATH" ]]; then
  echo "==> Checking staged native app bundle"
  validate_app "$APP_PATH"
fi

echo "Native unsigned release readiness checks passed"
