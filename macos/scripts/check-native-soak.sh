#!/usr/bin/env bash
set -euo pipefail

APP_PATH=""
DURATION_SECONDS=7200
OUTPUT_PATH=""
MAX_RSS_GROWTH_MIB=120
MINIMUM_CYCLES=100
PID=""
TEMP_ROOT=""

usage() {
  cat <<EOF
Usage: $0 --app PATH [options]

Run repeated PTY output, window resize, pane split/close, and native-tab
create/close cycles, then validate the app's machine-readable report.

Options:
  --app PATH                 Staged Termy.app bundle
  --duration-seconds N       Soak duration (default: 7200)
  --output PATH              Persist JSON report (default: temporary)
  --max-rss-growth-mib N     Final-minus-initial RSS budget (default: 120)
  --minimum-cycles N         Required completed cycles (default: 100)
  --help                     Show this help
EOF
}

fail() {
  echo "Error: $*" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --app) APP_PATH="${2:-}"; shift 2 ;;
    --duration-seconds) DURATION_SECONDS="${2:-}"; shift 2 ;;
    --output) OUTPUT_PATH="${2:-}"; shift 2 ;;
    --max-rss-growth-mib) MAX_RSS_GROWTH_MIB="${2:-}"; shift 2 ;;
    --minimum-cycles) MINIMUM_CYCLES="${2:-}"; shift 2 ;;
    --help|-h) usage; exit 0 ;;
    *) echo "Error: unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
done

[[ -n "$APP_PATH" ]] || { usage >&2; exit 2; }
[[ -d "$APP_PATH" ]] || fail "app bundle not found: $APP_PATH"
[[ "$DURATION_SECONDS" =~ ^[1-9][0-9]*$ ]] || fail "duration must be a positive integer"
[[ "$MINIMUM_CYCLES" =~ ^[1-9][0-9]*$ ]] || fail "minimum cycles must be positive"
[[ "$MAX_RSS_GROWTH_MIB" =~ ^[0-9]+([.][0-9]+)?$ ]] || fail "RSS growth budget must be numeric"

APP_PATH="$(cd "$(dirname "$APP_PATH")" && pwd -P)/$(basename "$APP_PATH")"
APP_BINARY="$APP_PATH/Contents/MacOS/Termy"
[[ -x "$APP_BINARY" ]] || fail "app binary not found: $APP_BINARY"

cleanup() {
  if [[ -n "$PID" ]]; then
    kill "$PID" >/dev/null 2>&1 || true
    wait "$PID" >/dev/null 2>&1 || true
  fi
  [[ -z "$TEMP_ROOT" ]] || rm -rf "$TEMP_ROOT"
}
trap cleanup EXIT

TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/termy-native-soak.XXXXXX")"
CASE_ROOT="$TEMP_ROOT/case"
REPORT_PATH="$CASE_ROOT/report.json"
LOG_PATH="$CASE_ROOT/app.log"
mkdir -p "$CASE_ROOT/home" "$CASE_ROOT/config" "$CASE_ROOT/tmp" "$CASE_ROOT/workspace"

/usr/bin/env -i \
  HOME="$CASE_ROOT/home" \
  XDG_CONFIG_HOME="$CASE_ROOT/config" \
  PATH="/usr/bin:/bin:/usr/sbin:/sbin" \
  SHELL="/bin/zsh" \
  TMPDIR="$CASE_ROOT/tmp" \
  TERMY_NATIVE_SOAK_DURATION_SECS="$DURATION_SECONDS" \
  TERMY_NATIVE_SOAK_REPORT_FILE="$REPORT_PATH" \
  "$APP_BINARY" --working-directory "$CASE_ROOT/workspace" \
  >"$LOG_PATH" 2>&1 &
PID=$!

deadline=$((SECONDS + DURATION_SECONDS + 90))
while [[ ! -s "$REPORT_PATH" ]]; do
  kill -0 "$PID" >/dev/null 2>&1 || fail "native app exited before writing the soak report"
  (( SECONDS < deadline )) || fail "native soak exceeded its timeout"
  sleep 0.5
done

wait "$PID" || fail "native app exited unsuccessfully after the soak"
PID=""

python3 - "$REPORT_PATH" "$DURATION_SECONDS" "$MINIMUM_CYCLES" "$MAX_RSS_GROWTH_MIB" <<'PY'
import json, pathlib, sys

path = pathlib.Path(sys.argv[1])
requested_duration = float(sys.argv[2])
minimum_cycles = int(sys.argv[3])
max_rss_growth = float(sys.argv[4])
report = json.loads(path.read_text())
errors = list(report.get("errors", []))

if report.get("durationSeconds", 0) < requested_duration * 0.95:
    errors.append(f"duration {report.get('durationSeconds', 0):.2f}s is shorter than requested {requested_duration:.0f}s")
if report.get("cycles", 0) < minimum_cycles:
    errors.append(f"cycles {report.get('cycles', 0)} is below minimum {minimum_cycles}")
if report.get("outputLines", 0) < report.get("cycles", 0) * 200:
    errors.append("output line accounting is incomplete")
if report.get("tabsOpened", 0) < 1:
    errors.append("no native tabs were opened")
if report.get("finalWindowCount", 0) > report.get("initialWindowCount", 0):
    errors.append("terminal windows remained after cleanup")
if report.get("rssGrowthMiB", 0) > max_rss_growth:
    errors.append(
        f"RSS growth {report.get('rssGrowthMiB', 0):.2f} MiB exceeds {max_rss_growth:.2f} MiB"
    )

print(json.dumps(report, indent=2, sort_keys=True))
if errors:
    for error in errors:
        print(f"Error: {error}", file=sys.stderr)
    raise SystemExit(1)
PY

if [[ -n "$OUTPUT_PATH" ]]; then
  mkdir -p "$(dirname "$OUTPUT_PATH")"
  cp "$REPORT_PATH" "$OUTPUT_PATH"
  cp "$LOG_PATH" "${OUTPUT_PATH%.json}.log"
  echo "Native soak report: $OUTPUT_PATH"
fi

echo "Native soak gate passed"
