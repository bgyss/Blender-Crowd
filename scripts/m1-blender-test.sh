#!/usr/bin/env bash
# Install the extension from scratch and exercise the M1 Blender workflow.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ONLY="project"

while [ "$#" -gt 0 ]; do
    case "$1" in
        --only)
            [ "$#" -ge 2 ] || { echo "--only requires a test name" >&2; exit 2; }
            ONLY="$2"
            shift 2
            ;;
        *)
            echo "unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

case "$ONLY" in
    project)
        TEST_PATH="tests/blender/test_m1_project.py"
        ;;
    cache-playback)
        TEST_PATH="tests/blender/test_m1_cache_playback.py"
        ;;
    *)
        echo "unknown M1 Blender test: $ONLY" >&2
        exit 2
        ;;
esac

if [ "$ONLY" = "cache-playback" ]; then
    if [ -n "${CROWD_M1_CACHE_PATH:-}" ]; then
        CACHE_PATH="$CROWD_M1_CACHE_PATH"
    else
        TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/blender-crowd-m1-playback.XXXXXX")"
        trap 'rm -rf "$TEMP_ROOT"' EXIT
        CACHE_PATH="$TEMP_ROOT/cache"
        cargo run --release -p crowd-bench -- m1 bake --cache "$CACHE_PATH"
    fi
    CROWD_M1_CACHE_PATH="$CACHE_PATH" \
        "$REPO_ROOT/scripts/blender-install-test.sh" --python "$TEST_PATH"
else
    "$REPO_ROOT/scripts/blender-install-test.sh" --python "$TEST_PATH"
fi
