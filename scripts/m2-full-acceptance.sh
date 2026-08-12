#!/usr/bin/env bash
# Full 1K M2 authorable bake -> cache-only replay -> debug -> render proof.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_ROOT=""
ARCHIVE=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --out)
            [ "$#" -ge 2 ] || { echo "--out requires a directory" >&2; exit 2; }
            OUTPUT_ROOT="$2"
            shift 2
            ;;
        --archive)
            [ "$#" -ge 2 ] || { echo "--archive requires a zip path" >&2; exit 2; }
            ARCHIVE="$2"
            shift 2
            ;;
        *)
            echo "unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

if [ -z "$OUTPUT_ROOT" ]; then
    OUTPUT_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/blender-crowd-m2-full.XXXXXX")"
    trap 'rm -rf "$OUTPUT_ROOT"' EXIT
else
    mkdir -p "$OUTPUT_ROOT"
fi

CACHE_PATH="$OUTPUT_ROOT/cache"
RENDER_DIR="$OUTPUT_ROOT/render"
REPORT_PATH="$OUTPUT_ROOT/m2-full-acceptance.json"

INSTALL_ARGS=()
if [ -n "$ARCHIVE" ]; then
    INSTALL_ARGS+=(--archive "$ARCHIVE")
fi
CROWD_M2_CACHE_PATH="$CACHE_PATH" \
CROWD_M2_RENDER_DIR="$RENDER_DIR" \
CROWD_M2_ACCEPTANCE_REPORT="$REPORT_PATH" \
    "$REPO_ROOT/scripts/blender-install-test.sh" \
    "${INSTALL_ARGS[@]}" \
    --python tests/blender/test_m2_full_acceptance.py

python3 -m json.tool "$REPORT_PATH"
echo "M2 full acceptance subgate: PASS $REPORT_PATH"
