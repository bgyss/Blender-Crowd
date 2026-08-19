#!/usr/bin/env bash
# Record the 100,000-agent M5 scale fixture to an mp4.
#
# This is a visualisation, not a measurement. Frames are rendered one at a
# time, so nothing about the clip's length or frame rate says anything about
# playback or simulation speed. The measured figures are in
# docs/benchmarks/2026-08-18-m5-100k.md.
#
# It is also a TRUNCATED run: --max-ticks stops well before the scene's 142,302
# ticks, so the report it writes beside the trace is not gate evidence and its
# completion rate is meaningless — agents that would arrive later never get the
# chance. Nothing here should be quoted as a result.
#
# Why the trace is subsampled: a trace is 34 bytes per agent per tick, so
# 142,302 ticks x 100,000 agents is 484 GB at every tick. TRACE_INTERVAL writes
# every Nth tick instead. The simulation still steps every tick; only the
# recording is sparse.
#
# Multi-hour. Run it under tmux, or with nohup, in its own window.
#
#   tmux new -s m5-recording
#   scripts/make-m5-100k-recording.sh
#
# Environment overrides:
#   AGENTS          population (default 100000)
#   MAX_TICKS       stop after this many ticks (default 42000, just before the
#                   first arrivals, so the whole population is on screen)
#   TRACE_INTERVAL  write every Nth tick (default 48, ~875 frames = ~29 s clip)
#   RES_X           horizontal resolution (default 1280)
#   STREAM_SPLIT    y that divides the two lane blocks (default: scene midpoint)
#   FPS             output frame rate (default 30)
#   OUT_DIR         where the mp4 lands (default ~/blender-crowd-m5/recording)
#   BLENDER         path to the Blender binary
set -euo pipefail

AGENTS="${AGENTS:-100000}"
MAX_TICKS="${MAX_TICKS:-42000}"
TRACE_INTERVAL="${TRACE_INTERVAL:-48}"
RES_X="${RES_X:-1280}"
FPS="${FPS:-30}"
OUT_DIR="${OUT_DIR:-$HOME/blender-crowd-m5/recording}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLENDER="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"
SCENE="m5_city_flow"
# m5_city_flow stacks its eastbound and westbound lane blocks along y, with a
# gap between them. Scene height is 40 * scale and scale is sqrt(agents/100),
# so the midpoint is 20 * sqrt(agents/100); the two blocks colour correctly
# either side of it.
STREAM_SPLIT="${STREAM_SPLIT:-$(python3 -c "import math,sys; print(20*math.sqrt(int(sys.argv[1])/100))" "$AGENTS")}"
TRACE="$OUT_DIR/$SCENE-$AGENTS.crowdtrace"
FRAME_DIR="$OUT_DIR/frames"
STEM="m5-city-flow-$AGENTS"

command -v ffmpeg >/dev/null || { echo "ffmpeg is required" >&2; exit 1; }
[ -x "$BLENDER" ] || { echo "Blender not found at $BLENDER" >&2; exit 1; }

mkdir -p "$OUT_DIR" "$FRAME_DIR"
cd "$REPO_ROOT"

FRAMES=$(( MAX_TICKS / TRACE_INTERVAL ))
BYTES=$(( FRAMES * AGENTS * 34 ))
printf '=== plan ===\n'
printf 'agents           %s\n' "$AGENTS"
printf 'ticks simulated  %s (of the scene'"'"'s 142302)\n' "$MAX_TICKS"
printf 'trace interval   every %s ticks\n' "$TRACE_INTERVAL"
printf 'frames           ~%s  -> ~%s s of video at %s fps\n' "$FRAMES" "$(( FRAMES / FPS ))" "$FPS"
printf 'trace size       ~%s GB\n' "$(( BYTES / 1000000000 ))"
printf 'output           %s/%s.mp4\n\n' "$OUT_DIR" "$STEM"

if [ ! -f "$TRACE" ]; then
    printf '=== baking trace (this is the long stage) ===\n'
    cargo run --release -p crowd-bench -- run \
        --scene "$SCENE" --agents "$AGENTS" \
        --trace --trace-interval "$TRACE_INTERVAL" --max-ticks "$MAX_TICKS" \
        --out "$OUT_DIR"
else
    printf '=== reusing existing trace %s ===\n' "$TRACE"
fi

printf '\n=== rendering frames ===\n'
# The trace is already subsampled, so the renderer takes every tick in it.
CROWD_TRACE_PATH="$TRACE" \
CROWD_FRAME_DIR="$FRAME_DIR" \
CROWD_TICK_STEP=1 \
CROWD_RES_X="$RES_X" \
CROWD_STREAM_AXIS=y \
CROWD_STREAM_SPLIT="$STREAM_SPLIT" \
    "$BLENDER" -b --factory-startup --python "$REPO_ROOT/scripts/render_playback.py"

printf '\n=== encoding ===\n'
ffmpeg -y -loglevel error -framerate "$FPS" -i "$FRAME_DIR/frame-%05d.png" \
    -c:v libx264 -pix_fmt yuv420p -crf 21 -movflags +faststart \
    "$OUT_DIR/$STEM.mp4"

printf 'wrote %s/%s.mp4 (%s)\n' "$OUT_DIR" "$STEM" "$(du -h "$OUT_DIR/$STEM.mp4" | cut -f1)"
printf '\nReminder: a visualisation, not a measurement, and a truncated run.\n'
