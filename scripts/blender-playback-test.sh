#!/usr/bin/env bash
# Bake a 1,000-agent trace, then play it back in Blender with the simulation
# process already exited.
#
# Automates M0 acceptance criterion 6. Simulation and playback costs are
# printed separately and must never be summed into one number.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLENDER="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"
OUT_DIR="$REPO_ROOT/benchmarks/reports"
SCENE="${SCENE:-crossing}"
AGENTS="${AGENTS:-1000}"

mkdir -p "$OUT_DIR"

echo "== simulation =="
# Not recorded with --svg or --frames, so this run's timing is a real
# measurement rather than a sampling-inflated one.
SIM_START=$(python3 -c "import time; print(time.perf_counter())")
cargo run --release -p crowd-bench -- run \
    --scene "$SCENE" --agents "$AGENTS" --trace --out "$OUT_DIR"
SIM_END=$(python3 -c "import time; print(time.perf_counter())")
python3 -c "print('simulation_wall_s: {:.4f}'.format($SIM_END - $SIM_START))"

TRACE="$OUT_DIR/$SCENE-$AGENTS.crowdtrace"
[ -f "$TRACE" ] || { echo "trace not written to $TRACE" >&2; exit 1; }
echo "trace_bytes: $(wc -c < "$TRACE" | tr -d ' ')"

echo "== blender playback (simulation process has exited) =="
CROWD_TRACE_PATH="$TRACE" "$BLENDER" -b --python "$REPO_ROOT/tests/blender/test_playback.py"

echo "playback test: PASS"
