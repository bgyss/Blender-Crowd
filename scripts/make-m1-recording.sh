#!/usr/bin/env bash
# Record cache-only M1 concourse playback to the clip checked in under docs/media/.
#
# This is a rendered recording, not a screen capture: every frame comes from
# `scripts/record_m1_playback.py` driving the cleanly installed extension
# headlessly against a completed Cache v1, so anyone with Blender can
# regenerate it rather than having to trust a video of someone's desktop.
#
# The clip is a visualisation, not a measurement. Frames are rendered one at a
# time with a cache sync in between, so nothing about its length or frame rate
# says anything about playback or simulation speed. Those costs are measured
# separately and reported in docs/benchmarks/2026-08-10-m1-vertical-slice.md.
#
# Usage: scripts/make-m1-recording.sh
#   CROWD_M1_CACHE_PATH=DIR  reuse an existing strict cache instead of baking
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="$REPO_ROOT/docs/media"
STEM="m1-concourse-1000"

# Every Nth tick becomes a frame. At 30 ticks/second of simulation, step 20
# played at 30 fps runs the concourse at 20x real time, which fits the whole
# 10,000-tick bake into a clip short enough to watch.
TICK_STEP="${CROWD_TICK_STEP:-20}"
RES_X="${CROWD_RES_X:-960}"
RES_Y="${CROWD_RES_Y:-540}"
FPS="${FPS:-30}"
GIF_WIDTH="${GIF_WIDTH:-440}"
GIF_FPS="${GIF_FPS:-10}"

command -v ffmpeg >/dev/null || { echo "ffmpeg is required" >&2; exit 1; }

TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/blender-crowd-m1-recording.XXXXXX")"
cleanup() { rm -rf "$TEMP_ROOT"; }
trap cleanup EXIT

FRAME_DIR="$TEMP_ROOT/frames"

# A complete strict cache is roughly 560 MB, so it lives in a temp directory and
# is never written into the repository.
if [ -n "${CROWD_M1_CACHE_PATH:-}" ]; then
    CACHE_PATH="$CROWD_M1_CACHE_PATH"
    echo "== reusing cache $CACHE_PATH =="
else
    CACHE_PATH="$TEMP_ROOT/cache"
    echo "== baking strict M1 cache =="
    cargo run --release -p crowd-bench -- m1 bake --cache "$CACHE_PATH"
fi

echo "== rendering frames =="
mkdir -p "$FRAME_DIR" "$OUT_DIR"
CROWD_M1_CACHE_PATH="$CACHE_PATH" \
CROWD_M1_FRAME_DIR="$FRAME_DIR" \
CROWD_TICK_STEP="$TICK_STEP" \
CROWD_RES_X="$RES_X" \
CROWD_RES_Y="$RES_Y" \
    "$REPO_ROOT/scripts/blender-install-test.sh" \
    --python scripts/record_m1_playback.py

echo "== encoding =="
# H.264 for anywhere video is accepted; it carries the full frame rate and
# resolution at a fraction of the GIF's size.
ffmpeg -y -loglevel error -framerate "$FPS" -i "$FRAME_DIR/frame-%05d.png" \
    -c:v libx264 -pix_fmt yuv420p -crf 23 -movflags +faststart \
    "$OUT_DIR/$STEM.mp4"

# Two-pass GIF, same reasoning as scripts/make-gif.sh: a single-pass encode
# picks its palette from the first frame, which here is a nearly empty concourse
# and would posterise every frame after it.
PALETTE="$TEMP_ROOT/palette.png"

ffmpeg -y -loglevel error -framerate "$FPS" -i "$FRAME_DIR/frame-%05d.png" \
    -vf "fps=$GIF_FPS,scale=$GIF_WIDTH:-2:flags=lanczos,palettegen=stats_mode=diff:max_colors=32" \
    "$PALETTE"

# No dithering, for the same reason make-gif.sh gives: dither scatters noise
# across a background that is otherwise one flat colour, and GIF's LZW cannot
# compress noise.
ffmpeg -y -loglevel error -framerate "$FPS" -i "$FRAME_DIR/frame-%05d.png" -i "$PALETTE" \
    -lavfi "fps=$GIF_FPS,scale=$GIF_WIDTH:-2:flags=lanczos[x];[x][1:v]paletteuse=dither=none" \
    -loop 0 "$OUT_DIR/$STEM.gif"

# The sidecar names the exact cache the clip came from; keep it with the media.
cp "$FRAME_DIR/m1-recording.json" "$OUT_DIR/$STEM.json"

echo "wrote $OUT_DIR/$STEM.mp4 ($(du -h "$OUT_DIR/$STEM.mp4" | cut -f1))"
echo "wrote $OUT_DIR/$STEM.gif ($(du -h "$OUT_DIR/$STEM.gif" | cut -f1))"
echo "wrote $OUT_DIR/$STEM.json"
