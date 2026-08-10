#!/usr/bin/env bash
# Bake a fresh strict cache, destroy that process, then render it in Blender.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/blender-crowd-m1-render.XXXXXX")"
trap 'rm -rf "$TEMP_ROOT"' EXIT
OUTPUT_DIR="$TEMP_ROOT/render"

while [ "$#" -gt 0 ]; do
    case "$1" in
        --out)
            [ "$#" -ge 2 ] || { echo "--out requires a path" >&2; exit 2; }
            OUTPUT_DIR="$2"
            shift 2
            ;;
        *)
            echo "unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

if [ -n "${CROWD_M1_CACHE_PATH:-}" ]; then
    CACHE_PATH="$CROWD_M1_CACHE_PATH"
else
    CACHE_PATH="$TEMP_ROOT/cache"
    cargo run --release -p crowd-bench -- m1 bake --cache "$CACHE_PATH"
fi

mkdir -p "$OUTPUT_DIR"
CROWD_M1_CACHE_PATH="$CACHE_PATH" CROWD_M1_RENDER_DIR="$OUTPUT_DIR" \
    "$REPO_ROOT/scripts/blender-install-test.sh" \
    --python tests/blender/test_m1_render.py

python3 -m json.tool "$OUTPUT_DIR/m1-render-metrics.json"
echo "render test: PASS"
