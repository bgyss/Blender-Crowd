#!/usr/bin/env bash
# Install the extension from scratch and exercise the M1 Blender workflow.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ONLY="all"

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
    all)
        TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/blender-crowd-m1-all.XXXXXX")"
        trap 'rm -rf "$TEMP_ROOT"' EXIT
        if [ -n "${CROWD_M1_CACHE_PATH:-}" ]; then
            CACHE_PATH="$CROWD_M1_CACHE_PATH"
        else
            CACHE_PATH="$TEMP_ROOT/cache"
            cargo run --release -p crowd-bench -- m1 bake --cache "$CACHE_PATH"
        fi
        "$REPO_ROOT/scripts/blender-install-test.sh" \
            --python tests/blender/test_m1_project.py
        for TEST_PATH in \
            tests/blender/test_m1_cache_playback.py \
            tests/blender/test_m1_override.py \
            tests/blender/test_m1_render.py
        do
            CROWD_M1_CACHE_PATH="$CACHE_PATH" CROWD_M1_RENDER_DIR="$TEMP_ROOT/render" \
                "$REPO_ROOT/scripts/blender-install-test.sh" --python "$TEST_PATH"
        done
        echo "M1 Blender suite: PASS"
        exit 0
        ;;
    project)
        TEST_PATH="tests/blender/test_m1_project.py"
        ;;
    cache-playback)
        TEST_PATH="tests/blender/test_m1_cache_playback.py"
        ;;
    override)
        TEST_PATH="tests/blender/test_m1_override.py"
        ;;
    render)
        exec "$REPO_ROOT/scripts/m1-render-test.sh"
        ;;
    *)
        echo "unknown M1 Blender test: $ONLY" >&2
        exit 2
        ;;
esac

if [ "$ONLY" = "cache-playback" ] || [ "$ONLY" = "override" ]; then
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
