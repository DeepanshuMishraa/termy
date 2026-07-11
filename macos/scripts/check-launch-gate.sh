#!/usr/bin/env bash
set -euo pipefail

APP_PATH=""
OUTPUT_PATH=""
MAX_STARTUP_MS=5000
MAX_ONE_TAB_RSS_MIB=200
MAX_MULTI_PANE_RSS_MIB=350
MAX_IDLE_CPU_PERCENT=15
MAX_IDLE_CPU_P95_PERCENT=40
SAMPLE_COUNT=10
SAMPLE_INTERVAL_SECONDS=0.5
SETTLE_SECONDS=5
MULTI_PANE_COUNT=8
TEMP_ROOT=""
PID=""

usage() {
  cat <<EOF
Usage: $0 --app PATH [options]

Launch a staged native app from isolated state, wait for a usable AppKit
window, then gate settled multi-sample CPU and one-tab/eight-pane RSS.

Options:
  --app PATH                     Staged Termy.app path
  --output PATH                  Write the JSON measurement report
  --max-startup-ms N             Usable-window budget (default: 5000)
  --max-one-tab-rss-mib N        One-tab RSS budget (default: 200)
  --max-multi-pane-rss-mib N     Multi-pane RSS budget (default: 350)
  --max-idle-cpu-percent N       Settled mean CPU budget (default: 15)
  --max-idle-cpu-p95-percent N   Settled p95 CPU budget (default: 40)
  --sample-count N               Samples per launch (default: 10)
  --sample-interval-seconds N    Delay between samples (default: 0.5)
  --settle-seconds N             Settle delay before samples (default: 3)
  --multi-pane-count N           Pane count for memory run (default: 8)
  --help                         Show this help message
EOF
}

fail() {
  echo "Error: $*" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --app) APP_PATH="${2:-}"; shift 2 ;;
    --output) OUTPUT_PATH="${2:-}"; shift 2 ;;
    --max-startup-ms) MAX_STARTUP_MS="${2:-}"; shift 2 ;;
    --max-one-tab-rss-mib) MAX_ONE_TAB_RSS_MIB="${2:-}"; shift 2 ;;
    --max-multi-pane-rss-mib) MAX_MULTI_PANE_RSS_MIB="${2:-}"; shift 2 ;;
    --max-idle-cpu-percent) MAX_IDLE_CPU_PERCENT="${2:-}"; shift 2 ;;
    --max-idle-cpu-p95-percent) MAX_IDLE_CPU_P95_PERCENT="${2:-}"; shift 2 ;;
    --sample-count) SAMPLE_COUNT="${2:-}"; shift 2 ;;
    --sample-interval-seconds) SAMPLE_INTERVAL_SECONDS="${2:-}"; shift 2 ;;
    --settle-seconds) SETTLE_SECONDS="${2:-}"; shift 2 ;;
    --multi-pane-count) MULTI_PANE_COUNT="${2:-}"; shift 2 ;;
    --help|-h) usage; exit 0 ;;
    *) echo "Error: unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
done

[[ -n "$APP_PATH" ]] || { usage >&2; exit 2; }
[[ -d "$APP_PATH" ]] || fail "app bundle not found: $APP_PATH"
APP_PATH="$(cd "$(dirname "$APP_PATH")" && pwd -P)/$(basename "$APP_PATH")"
APP_BINARY="$APP_PATH/Contents/MacOS/Termy"
[[ -x "$APP_BINARY" ]] || fail "app binary not found: $APP_BINARY"
[[ "$SAMPLE_COUNT" =~ ^[1-9][0-9]*$ ]] || fail "sample count must be positive"
[[ "$MULTI_PANE_COUNT" =~ ^[2-8]$ ]] || fail "multi-pane count must be between 2 and 8"

now_ms() {
  perl -MTime::HiRes=time -e 'printf "%.0f\n", time * 1000'
}

stop_app() {
  if [[ -n "$PID" ]]; then
    kill "$PID" >/dev/null 2>&1 || true
    wait "$PID" >/dev/null 2>&1 || true
    PID=""
  fi
}

cleanup() {
  stop_app
  [[ -z "$TEMP_ROOT" ]] || rm -rf "$TEMP_ROOT"
}
trap cleanup EXIT

TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/termy-launch-perf.XXXXXX")"

run_case() {
  local name="$1"
  local pane_count="$2"
  local case_root="$TEMP_ROOT/$name"
  local probe_file="$case_root/window-ready.txt"
  local pane_ready_file="$case_root/panes-ready.txt"
  local sample_file="$case_root/samples.tsv"
  local startup_file="$case_root/startup-ms.txt"
  mkdir -p "$case_root/home" "$case_root/config" "$case_root/tmp" "$case_root/workspace"

  local start_ms current_ms elapsed_ms attempt
  start_ms="$(now_ms)"
  /usr/bin/env -i \
    HOME="$case_root/home" \
    XDG_CONFIG_HOME="$case_root/config" \
    PATH="/usr/bin:/bin:/usr/sbin:/sbin" \
    SHELL="/bin/zsh" \
    TMPDIR="$case_root/tmp" \
    TERMY_LAUNCH_PROBE_FILE="$probe_file" \
    TERMY_PERFORMANCE_PANE_COUNT="$pane_count" \
    TERMY_PERFORMANCE_PANE_READY_FILE="$pane_ready_file" \
    "$APP_BINARY" --working-directory "$case_root/workspace" \
    >"$case_root/app.log" 2>&1 &
  PID=$!

  for ((attempt = 0; attempt < 200; attempt++)); do
    if [[ -s "$probe_file" ]]; then
      break
    fi
    kill -0 "$PID" >/dev/null 2>&1 || fail "$name app exited before presenting a usable window"
    current_ms="$(now_ms)"
    elapsed_ms=$((current_ms - start_ms))
    (( elapsed_ms <= MAX_STARTUP_MS )) || fail "$name usable window exceeded ${MAX_STARTUP_MS}ms"
    sleep 0.05
  done
  [[ -s "$probe_file" ]] || fail "$name usable window probe was not written"
  current_ms="$(now_ms)"
  elapsed_ms=$((current_ms - start_ms))
  printf '%s\n' "$elapsed_ms" >"$startup_file"

  if (( pane_count > 1 )); then
    for ((attempt = 0; attempt < 100; attempt++)); do
      if [[ -s "$pane_ready_file" ]]; then
        break
      fi
      kill -0 "$PID" >/dev/null 2>&1 || fail "$name app exited while creating panes"
      sleep 0.05
    done
    [[ -s "$pane_ready_file" ]] || fail "$name pane setup did not complete"
    actual_pane_count="$(tr -d '[:space:]' <"$pane_ready_file")"
    [[ "$actual_pane_count" == "$pane_count" ]] || \
      fail "$name created ${actual_pane_count:-unknown} panes; expected $pane_count"
  fi

  sleep "$SETTLE_SECONDS"
  : >"$sample_file"
  local sample cpu rss_kib
  for ((sample = 0; sample < SAMPLE_COUNT; sample++)); do
    kill -0 "$PID" >/dev/null 2>&1 || fail "$name app exited during sampling"
    cpu="$(ps -o %cpu= -p "$PID" | awk '{print $1}')"
    rss_kib="$(ps -o rss= -p "$PID" | awk '{print $1}')"
    [[ -n "$cpu" && -n "$rss_kib" ]] || fail "$name produced an empty process sample"
    printf '%s\t%s\n' "$cpu" "$rss_kib" >>"$sample_file"
    if (( sample + 1 < SAMPLE_COUNT )); then
      sleep "$SAMPLE_INTERVAL_SECONDS"
    fi
  done
  stop_app
}

echo "==> Measuring usable launch and one-tab idle state"
run_case one-tab 1
echo "==> Measuring ${MULTI_PANE_COUNT}-pane idle state"
run_case multi-pane "$MULTI_PANE_COUNT"

REPORT_PATH="${OUTPUT_PATH:-$TEMP_ROOT/report.json}"
mkdir -p "$(dirname "$REPORT_PATH")"
python3 - "$TEMP_ROOT" "$REPORT_PATH" "$MAX_STARTUP_MS" "$MAX_ONE_TAB_RSS_MIB" \
  "$MAX_MULTI_PANE_RSS_MIB" "$MAX_IDLE_CPU_PERCENT" "$MAX_IDLE_CPU_P95_PERCENT" \
  "$MULTI_PANE_COUNT" <<'PY'
import json, math, pathlib, statistics, sys

root = pathlib.Path(sys.argv[1])
report_path = pathlib.Path(sys.argv[2])
max_startup = float(sys.argv[3])
max_one_rss = float(sys.argv[4])
max_multi_rss = float(sys.argv[5])
max_cpu = float(sys.argv[6])
max_cpu_p95 = float(sys.argv[7])
multi_panes = int(sys.argv[8])

def percentile(values, fraction):
    ordered = sorted(values)
    index = round((len(ordered) - 1) * fraction)
    return ordered[index]

def load_case(name, panes):
    startup_ms = int((root / name / "startup-ms.txt").read_text().strip())
    rows = [line.split() for line in (root / name / "samples.tsv").read_text().splitlines()]
    cpu = [float(row[0]) for row in rows]
    rss_mib = [float(row[1]) / 1024 for row in rows]
    return {
        "panes": panes,
        "startupMs": startup_ms,
        "sampleCount": len(rows),
        "cpuMeanPercent": statistics.fmean(cpu),
        "cpuP95Percent": percentile(cpu, 0.95),
        "rssMaxMiB": max(rss_mib),
    }

report = {
    "oneTab": load_case("one-tab", 1),
    "multiPane": load_case("multi-pane", multi_panes),
    "budgets": {
        "startupMs": max_startup,
        "oneTabRssMiB": max_one_rss,
        "multiPaneRssMiB": max_multi_rss,
        "idleCpuMeanPercent": max_cpu,
        "idleCpuP95Percent": max_cpu_p95,
    },
}
report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")

errors = []
for label, case in (("one-tab", report["oneTab"]), ("multi-pane", report["multiPane"])):
    if case["startupMs"] > max_startup:
        errors.append(f"{label} startup {case['startupMs']}ms exceeds {max_startup:.0f}ms")
    if case["cpuMeanPercent"] > max_cpu:
        errors.append(f"{label} mean idle CPU {case['cpuMeanPercent']:.2f}% exceeds {max_cpu:.2f}%")
    if case["cpuP95Percent"] > max_cpu_p95:
        errors.append(f"{label} p95 idle CPU {case['cpuP95Percent']:.2f}% exceeds {max_cpu_p95:.2f}%")
if report["oneTab"]["rssMaxMiB"] > max_one_rss:
    errors.append(f"one-tab RSS {report['oneTab']['rssMaxMiB']:.2f} MiB exceeds {max_one_rss:.2f} MiB")
if report["multiPane"]["rssMaxMiB"] > max_multi_rss:
    errors.append(f"multi-pane RSS {report['multiPane']['rssMaxMiB']:.2f} MiB exceeds {max_multi_rss:.2f} MiB")

for label, case in (("one-tab", report["oneTab"]), ("multi-pane", report["multiPane"])):
    print(
        f"{label}: usable={case['startupMs']}ms samples={case['sampleCount']} "
        f"CPU mean/p95={case['cpuMeanPercent']:.2f}/{case['cpuP95Percent']:.2f}% "
        f"RSS max={case['rssMaxMiB']:.2f} MiB"
    )
if errors:
    for error in errors:
        print(f"launch-perf FAIL: {error}", file=sys.stderr)
    raise SystemExit(1)
print(f"Native launch performance gates passed; report: {report_path}")
PY
