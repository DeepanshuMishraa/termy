#!/usr/bin/env bash
# Deterministic native render-performance regression gate.
set -euo pipefail

MACOS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_DIR="$(cd "$MACOS_DIR/.." && pwd)"

P95_CEILING_MICROS="${TERMY_RENDER_P95_CEILING_MICROS:-2000}"
MIN_SAMPLES="${TERMY_RENDER_MIN_SAMPLES:-100}"
SKIP_BUILD=0

if [[ "${1:-}" == "--skip-build" ]]; then
    SKIP_BUILD=1
    shift
fi
[[ $# -eq 0 ]] || { echo "Usage: $0 [--skip-build]" >&2; exit 2; }

cd "$ROOT_DIR"
if [[ "$SKIP_BUILD" == "0" ]]; then
    cargo build -p termy_ffi >/dev/null
    swift build --package-path "$MACOS_DIR" >/dev/null
fi

BINARY="${TERMY_RENDER_BENCHMARK_BINARY:-$MACOS_DIR/.build/debug/TermySwift}"
[[ -x "$BINARY" ]] || { echo "render-perf: benchmark binary not found at $BINARY" >&2; exit 1; }

if ! OUTPUT="$("$BINARY" --benchmark 2>&1)"; then
    echo "render-perf: benchmark command failed" >&2
    printf '%s\n' "$OUTPUT" >&2
    exit 1
fi
RESULTS="$(printf '%s\n' "$OUTPUT" | sed -n 's/^native-benchmark-result //p')"

[[ -n "$RESULTS" ]] || {
    echo "render-perf: benchmark did not emit scenario results" >&2
    printf '%s\n' "$OUTPUT" >&2
    exit 1
}

P95_CEILING="$P95_CEILING_MICROS" MIN_SAMPLES="$MIN_SAMPLES" RESULTS="$RESULTS" python3 - <<'PY'
import json, os, sys

ceiling = float(os.environ["P95_CEILING"])
minimum = int(os.environ["MIN_SAMPLES"])
results = [json.loads(line) for line in os.environ["RESULTS"].splitlines() if line.strip()]
expected = {
    "idle-cursor-blink",
    "continuous-bulk-output",
    "large-scrollback-navigation",
    "fullscreen-tui-redraw",
    "rapid-resize",
    "split-panes-2",
    "split-panes-4",
    "split-panes-8",
    "tmux-control-output",
    "search-many-matches",
}

by_name = {result["scenario"]: result for result in results}
errors = []
missing = sorted(expected - by_name.keys())
unexpected = sorted(by_name.keys() - expected)
if missing:
    errors.append(f"missing scenarios: {', '.join(missing)}")
if unexpected:
    errors.append(f"unexpected scenarios: {', '.join(unexpected)}")

for name in sorted(expected & by_name.keys()):
    result = by_name[name]
    metrics = result["metrics"]
    times = result["buildTimes"]
    presented = metrics["presentedFrames"]
    samples = times["samples"]
    p95 = times["p95Micros"]
    if presented < minimum:
        errors.append(f"{name}: only {presented} presented frames; require {minimum}")
    if samples < minimum:
        errors.append(f"{name}: only {samples} render-plan samples; require {minimum}")
    if p95 > ceiling:
        errors.append(f"{name}: render-plan p95 {p95:.2f}us exceeds {ceiling:.2f}us")

bulk = by_name.get("continuous-bulk-output")
if bulk:
    metrics = bulk["metrics"]
    presented = metrics["presentedFrames"]
    full = metrics["fullRenderPlanRebuilds"]
    full_cap = max(2, int(presented * 0.1))
    if full > full_cap:
        errors.append(
            f"continuous-bulk-output: full render-plan rebuilds {full} exceed cap {full_cap} of {presented} presents"
        )

if errors:
    for error in errors:
        print(f"render-perf FAIL: {error}", file=sys.stderr)
    sys.exit(1)

print(f"render-perf OK: {len(results)} scenarios, >= {minimum} presents each, p95 <= {ceiling:.0f}us")
for name in sorted(by_name):
    result = by_name[name]
    metrics = result["metrics"]
    times = result["buildTimes"]
    print(
        f"  {name}: presents={metrics['presentedFrames']} skipped={metrics['skippedPresents']} "
        f"full/partial={metrics['fullRenderPlanRebuilds']}/{metrics['partialRenderPlanRebuilds']} "
        f"patched={metrics['patchedCells']} p50/p95/p99="
        f"{times['p50Micros']:.2f}/{times['p95Micros']:.2f}/{times['p99Micros']:.2f}us"
    )
PY
