#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MACOS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$MACOS_DIR/.." && pwd)"
APP_PATH=""
APP_PID=""
TEMP_ROOT=""

usage() {
  cat <<EOF
Usage: $0 --app PATH

Exercise the bundled CLI installer with an isolated HOME, PATH, shell profile,
and working directory. The developer's real shell configuration is never read
or modified.
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
APP_PATH="$(cd "$(dirname "$APP_PATH")" && pwd)/$(basename "$APP_PATH")"
CLI_BINARY="$APP_PATH/Contents/MacOS/termy-cli"
APP_BINARY="$APP_PATH/Contents/MacOS/Termy"

[[ -x "$CLI_BINARY" ]] || fail "missing executable CLI helper: $CLI_BINARY"
[[ -x "$APP_BINARY" ]] || fail "missing executable app binary: $APP_BINARY"

cleanup() {
  if [[ -n "$APP_PID" ]]; then
    kill "$APP_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "$TEMP_ROOT" ]]; then
    rm -rf "$TEMP_ROOT"
  fi
}
trap cleanup EXIT

TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/termy-cli-smoke.XXXXXX")"
ISOLATED_HOME="$TEMP_ROOT/home"
WORKING_DIR="$TEMP_ROOT/workspace"
mkdir -p "$ISOLATED_HOME" "$WORKING_DIR"
CANONICAL_WORKING_DIR="$(cd "$WORKING_DIR" && pwd -P)"

echo "==> Building isolated CLI installer helper"
(cd "$REPO_ROOT" && cargo build -p termy_cli_install_core --example install_cli)
INSTALL_HELPER="$REPO_ROOT/target/debug/examples/install_cli"
[[ -x "$INSTALL_HELPER" ]] || fail "installer helper was not built: $INSTALL_HELPER"

ISOLATED_PATH="$ISOLATED_HOME/.local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
echo "==> Installing bundled CLI into isolated HOME"
env -i \
  HOME="$ISOLATED_HOME" \
  PATH="$ISOLATED_PATH" \
  SHELL="/bin/zsh" \
  TMPDIR="$TEMP_ROOT" \
  "$INSTALL_HELPER" "$CLI_BINARY" "$ISOLATED_HOME" "/bin/zsh"

INSTALLED_CLI="$ISOLATED_HOME/.local/bin/termy"
[[ -L "$INSTALLED_CLI" ]] || fail "installer did not create symlink: $INSTALLED_CLI"
[[ "$(readlink "$INSTALLED_CLI")" == "$CLI_BINARY" ]] \
  || fail "installed CLI does not point at bundled helper: $INSTALLED_CLI"
[[ -f "$ISOLATED_HOME/.zshrc" ]] || fail "installer did not create isolated .zshrc"

echo "==> Running installed CLI version and config inspection"
env -i HOME="$ISOLATED_HOME" PATH="$ISOLATED_PATH" SHELL="/bin/zsh" \
  "$INSTALLED_CLI" --version >/dev/null
env -i HOME="$ISOLATED_HOME" PATH="$ISOLATED_PATH" SHELL="/bin/zsh" \
  "$INSTALLED_CLI" -show-config >/dev/null

echo "==> Opening native app through installed CLI"
env -i HOME="$ISOLATED_HOME" PATH="$ISOLATED_PATH" SHELL="/bin/zsh" \
  "$INSTALLED_CLI" --working-directory "$WORKING_DIR"

for _ in {1..50}; do
  APP_PID="$(pgrep -f "$APP_BINARY" 2>/dev/null \
    | awk 'NR == 1 { pid = $1 } END { if (pid) print pid }' || true)"
  [[ -n "$APP_PID" ]] && break
  sleep 0.1
done
[[ -n "$APP_PID" ]] || fail "installed CLI did not launch $APP_BINARY"
APP_COMMAND="$(ps -p "$APP_PID" -o command=)"
[[ "$APP_COMMAND" == *"--working-directory $CANONICAL_WORKING_DIR"* ]] \
  || fail "native app did not receive working directory: $APP_COMMAND"

echo "CLI install smoke passed: $APP_PATH"
