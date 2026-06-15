#!/usr/bin/env bash
# Native render-performance regression gate (P5/P6 native side).
#
# Runs the headless `--benchmark` workload and enforces two invariants:
#   1. The partial render-plan path holds — a bulk-scroll workload must not force
#      a full render-plan rebuild on (almost) every presented frame.
#   2. Render-plan build time stays under a generous p95 ceiling.
#
# Thresholds are deliberately loose: this catches gross regressions, not noise.
set -euo pipefail

MACOS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_DIR="$(cd "$MACOS_DIR/.." && pwd)"

P95_CEILING_MICROS="${TERMY_RENDER_P95_CEILING_MICROS:-2000}"

cd "$ROOT_DIR"
# Ensure the FFI dylib and the Swift binary are built.
cargo build -p termy_ffi >/dev/null
swift build --package-path "$MACOS_DIR" >/dev/null

BINARY="$MACOS_DIR/.build/debug/TermySwift"
[[ -x "$BINARY" ]] || { echo "render-perf: built binary not found at $BINARY" >&2; exit 1; }

if ! OUTPUT="$("$BINARY" --benchmark 2>&1)"; then
    echo "render-perf: benchmark command failed" >&2
    printf '%s\n' "$OUTPUT" >&2
    exit 1
fi
METRICS_LINE="$(printf '%s\n' "$OUTPUT" | grep '^native-render-metrics ' | sed 's/^native-render-metrics //')"
TIMES_LINE="$(printf '%s\n' "$OUTPUT" | grep '^native-build-times ' | sed 's/^native-build-times //')"

[[ -n "$METRICS_LINE" && -n "$TIMES_LINE" ]] || {
    echo "render-perf: benchmark did not emit metrics" >&2
    printf '%s\n' "$OUTPUT" >&2
    exit 1
}

P95_CEILING="$P95_CEILING_MICROS" \
METRICS="$METRICS_LINE" TIMES="$TIMES_LINE" python3 - <<'PY'
import json, os, sys

metrics = json.loads(os.environ["METRICS"])
times = json.loads(os.environ["TIMES"])
ceiling = float(os.environ["P95_CEILING"])

presented = metrics["presentedFrames"]
full = metrics["fullRenderPlanRebuilds"]
p95 = times["p95Micros"]

errors = []
# Partial path must dominate: full rebuilds capped at ~10% of presents (+1 for
# the forced seed).
full_cap = max(2, int(presented * 0.1))
if full > full_cap:
    errors.append(f"full render-plan rebuilds {full} exceeds cap {full_cap} of {presented} presents")
if p95 > ceiling:
    errors.append(f"render-plan build p95 {p95}us exceeds ceiling {ceiling}us")

if errors:
    for e in errors:
        print(f"render-perf FAIL: {e}", file=sys.stderr)
    sys.exit(1)

print(f"render-perf OK: {full}/{presented} full rebuilds, p95 {p95}us (ceiling {ceiling}us)")
PY
