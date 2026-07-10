#!/usr/bin/env bash
set -euo pipefail

APP_PATH=""
TIMEOUT_SECONDS=10
TEMP_ROOT=""
PID=""

usage() {
  cat <<EOF
Usage: $0 --app PATH [--timeout-seconds N]

Launch a native Termy.app with an isolated HOME/config directory and require a
probe from a visible AppKit window with non-empty content.
EOF
}

fail() {
  echo "Error: $*" >&2
  if [[ -n "$TEMP_ROOT" && -s "$TEMP_ROOT/app.log" ]]; then
    echo "--- app output ---" >&2
    cat "$TEMP_ROOT/app.log" >&2
  fi
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --app)
      [[ $# -ge 2 ]] || fail "--app requires a value"
      APP_PATH="$2"
      shift 2
      ;;
    --timeout-seconds)
      [[ $# -ge 2 ]] || fail "--timeout-seconds requires a value"
      TIMEOUT_SECONDS="$2"
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
[[ "$TIMEOUT_SECONDS" =~ ^[1-9][0-9]*$ ]] || fail "timeout must be a positive integer"
APP_PATH="$(cd "$(dirname "$APP_PATH")" && pwd -P)/$(basename "$APP_PATH")"
APP_BINARY="$APP_PATH/Contents/MacOS/Termy"
[[ -x "$APP_BINARY" ]] || fail "missing executable app binary: $APP_BINARY"

cleanup() {
  if [[ -n "$PID" ]]; then
    kill "$PID" >/dev/null 2>&1 || true
    wait "$PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "$TEMP_ROOT" ]]; then
    rm -rf "$TEMP_ROOT"
  fi
}
trap cleanup EXIT

TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/termy-launch-ready.XXXXXX")"
ISOLATED_HOME="$TEMP_ROOT/home"
CONFIG_HOME="$TEMP_ROOT/config"
WORKING_DIR="$TEMP_ROOT/workspace"
PROBE_FILE="$TEMP_ROOT/window-ready.txt"
mkdir -p "$ISOLATED_HOME" "$CONFIG_HOME" "$WORKING_DIR"

echo "==> Launching with isolated HOME and config: $APP_PATH"
env -i \
  HOME="$ISOLATED_HOME" \
  XDG_CONFIG_HOME="$CONFIG_HOME" \
  PATH="/usr/bin:/bin:/usr/sbin:/sbin" \
  SHELL="/bin/zsh" \
  TMPDIR="$TEMP_ROOT" \
  TERMY_LAUNCH_PROBE_FILE="$PROBE_FILE" \
  "$APP_BINARY" --working-directory "$WORKING_DIR" \
  >"$TEMP_ROOT/app.log" 2>&1 &
PID=$!

attempts=$((TIMEOUT_SECONDS * 20))
for ((attempt = 0; attempt < attempts; attempt++)); do
  if [[ -s "$PROBE_FILE" ]]; then
    break
  fi
  if ! kill -0 "$PID" >/dev/null 2>&1; then
    fail "app exited before presenting a usable window"
  fi
  sleep 0.05
done

[[ -s "$PROBE_FILE" ]] || fail "usable window did not appear within ${TIMEOUT_SECONDS}s"

probe_value() {
  local key="$1"
  awk -F= -v key="$key" '$1 == key { print $2; exit }' "$PROBE_FILE"
}

probe_pid="$(probe_value pid)"
visible="$(probe_value visible)"
window_number="$(probe_value window_number)"
content_width="$(probe_value content_width)"
content_height="$(probe_value content_height)"

[[ "$probe_pid" == "$PID" ]] || fail "probe PID $probe_pid differs from launched PID $PID"
[[ "$visible" == "true" ]] || fail "probe did not report a visible window"
[[ "$window_number" =~ ^[1-9][0-9]*$ ]] || fail "invalid window number: ${window_number:-missing}"
[[ "$content_width" =~ ^[1-9][0-9]*$ ]] || fail "invalid content width: ${content_width:-missing}"
[[ "$content_height" =~ ^[1-9][0-9]*$ ]] || fail "invalid content height: ${content_height:-missing}"

echo "Usable native window passed: ${content_width}x${content_height} (pid $PID)"
