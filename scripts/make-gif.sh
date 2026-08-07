#!/usr/bin/env bash
# Render one scene to the animated GIF checked in under docs/media/.
#
# The frames themselves come from `crowd-bench run --frames`, which is
# dependency-free and part of the workspace. Only the GIF encode needs an
# external tool, and ffmpeg is the one assumed here.
#
# Usage: scripts/make-gif.sh [SCENE] [AGENTS]
set -euo pipefail

SCENE="${1:-crossing}"
AGENTS="${2:-600}"
FPS="${FPS:-20}"
WIDTH="${WIDTH:-560}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRAME_DIR="$REPO_ROOT/benchmarks/reports/frames-$SCENE-$AGENTS"
OUT_DIR="$REPO_ROOT/docs/media"
OUT="$OUT_DIR/$SCENE-$AGENTS.gif"

command -v ffmpeg >/dev/null || { echo "ffmpeg is required" >&2; exit 1; }

# A frame-recorded run samples and writes per tick, so its reported
# ticks_per_second is not a performance measurement. Never quote it.
cargo run --release -p crowd-bench -- run \
    --scene "$SCENE" --agents "$AGENTS" --frames \
    --out "$REPO_ROOT/benchmarks/reports"

mkdir -p "$OUT_DIR"

# Two passes: palettegen over the whole sequence, then paletteuse. A
# single-pass GIF encode picks its palette from the first frame alone, which
# banks the colours of a near-empty opening frame and posterises the rest.
#
# Nearest-neighbour scaling and no dithering are deliberate, and worth ~8x in
# file size: the frames are flat discs on a flat background, so interpolation
# and dithering invent thousands of near-identical colours that GIF's LZW
# cannot compress and that add nothing to a chart of coloured dots.
PALETTE="$(mktemp -t crowd-palette.XXXXXX).png"
trap 'rm -f "$PALETTE"' EXIT

ffmpeg -y -loglevel error -framerate "$FPS" -i "$FRAME_DIR/frame-%05d.ppm" \
    -vf "scale=$WIDTH:-2:flags=neighbor,palettegen=stats_mode=diff:max_colors=16" "$PALETTE"

ffmpeg -y -loglevel error -framerate "$FPS" -i "$FRAME_DIR/frame-%05d.ppm" -i "$PALETTE" \
    -lavfi "scale=$WIDTH:-2:flags=neighbor[x];[x][1:v]paletteuse=dither=none" \
    -loop 0 "$OUT"

echo "wrote $OUT ($(du -h "$OUT" | cut -f1))"
