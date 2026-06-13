#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-run}"
BUNDLE_ID="com.lassevestergaard.termy"
MACOS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_DIR="$(cd "$MACOS_DIR/.." && pwd)"
shift || true

case "$MODE" in
  run) COMMAND="run" ;;
  --debug|debug) COMMAND="debug" ;;
  --logs|logs) COMMAND="logs" ;;
  --telemetry|telemetry) COMMAND="telemetry" ;;
  --verify|verify) COMMAND="verify" ;;
  *)
    echo "usage: $0 [run|--debug|--logs|--telemetry|--verify]" >&2
    exit 2
    ;;
esac

cd "$ROOT_DIR"
exec cargo macos "$COMMAND" "$@"
