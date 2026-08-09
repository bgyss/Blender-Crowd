#!/usr/bin/env bash
# Record baked-trace playback in Blender to the clip checked in under docs/media/.
#
# This is a rendered recording, not a screen capture: every frame comes from
# `scripts/render_playback.py` driving the shipped extension headlessly, so
# anyone with Blender and a trace can regenerate it byte-for-byte rather than
# having to trust a video of someone's desktop.
#
# The clip is a visualisation, not a measurement. Frames are rendered one at a
# time with a sync in between, so nothing about its length or frame rate says
# anything about playback speed. The playback cost is measured separately by
# scripts/blender-playback-test.sh and reported in docs/benchmarks/.
#
# Usage: scripts/make-blender-recording.sh [SCENE] [AGENTS]
set -euo pipefail

SCENE="${1:-crossing}"
AGENTS="${2:-1000}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLENDER="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"
REPORT_DIR="$REPO_ROOT/benchmarks/reports"
TRACE="$REPORT_DIR/$SCENE-$AGENTS.crowdtrace"
FRAME_DIR="$REPORT_DIR/render-$SCENE-$AGENTS"
OUT_DIR="$REPO_ROOT/docs/media"
STEM="blender-playback-$SCENE-$AGENTS"

# Every Nth tick becomes a frame. At 30 ticks/second of simulation, step 10
# played at 30 fps runs the crowd at 10x real time, which fits the whole run
# into a clip short enough to watch.
TICK_STEP="${TICK_STEP:-10}"
RES_X="${RES_X:-960}"
FPS="${FPS:-30}"
GIF_WIDTH="${GIF_WIDTH:-440}"
GIF_FPS="${GIF_FPS:-10}"

command -v ffmpeg >/dev/null || { echo "ffmpeg is required" >&2; exit 1; }
[ -x "$BLENDER" ] || { echo "Blender not found at $BLENDER" >&2; exit 1; }

if [ ! -f "$TRACE" ]; then
    echo "== baking trace (no $TRACE) =="
    # Not a throughput measurement: --trace writes every tick to disk.
    cargo run --release -p crowd-bench -- run \
        --scene "$SCENE" --agents "$AGENTS" --trace --out "$REPORT_DIR"
fi

echo "== rendering frames =="
rm -rf "$FRAME_DIR"
mkdir -p "$FRAME_DIR" "$OUT_DIR"
CROWD_TRACE_PATH="$TRACE" \
CROWD_FRAME_DIR="$FRAME_DIR" \
CROWD_TICK_STEP="$TICK_STEP" \
CROWD_RES_X="$RES_X" \
    "$BLENDER" -b --factory-startup --python "$REPO_ROOT/scripts/render_playback.py"

echo "== encoding =="
# H.264 for anywhere video is accepted; it carries the full frame rate and
# resolution at a fraction of the GIF's size.
ffmpeg -y -loglevel error -framerate "$FPS" -i "$FRAME_DIR/frame-%05d.png" \
    -c:v libx264 -pix_fmt yuv420p -crf 23 -movflags +faststart \
    "$OUT_DIR/$STEM.mp4"

# Two-pass GIF, same reasoning as scripts/make-gif.sh: a single-pass encode
# picks its palette from the first frame, which here is a nearly empty arena
# and would posterise every frame after it.
PALETTE="$(mktemp -t crowd-palette.XXXXXX).png"
trap 'rm -f "$PALETTE"' EXIT

ffmpeg -y -loglevel error -framerate "$FPS" -i "$FRAME_DIR/frame-%05d.png" \
    -vf "fps=$GIF_FPS,scale=$GIF_WIDTH:-2:flags=lanczos,palettegen=stats_mode=diff:max_colors=32" \
    "$PALETTE"

# No dithering, for the same reason make-gif.sh gives: dither scatters noise
# across a background that is otherwise one flat colour, and GIF's LZW cannot
# compress noise. Here it costs roughly 40% of the file for no visible gain.
ffmpeg -y -loglevel error -framerate "$FPS" -i "$FRAME_DIR/frame-%05d.png" -i "$PALETTE" \
    -lavfi "fps=$GIF_FPS,scale=$GIF_WIDTH:-2:flags=lanczos[x];[x][1:v]paletteuse=dither=none" \
    -loop 0 "$OUT_DIR/$STEM.gif"

echo "wrote $OUT_DIR/$STEM.mp4 ($(du -h "$OUT_DIR/$STEM.mp4" | cut -f1))"
echo "wrote $OUT_DIR/$STEM.gif ($(du -h "$OUT_DIR/$STEM.gif" | cut -f1))"
