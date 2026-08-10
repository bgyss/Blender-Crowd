#!/usr/bin/env bash
# Build, install, and load the extension in a clean Blender 5.2 session.
#
# Automates M0 acceptance criterion 5. Every step is headless so this can run
# unattended.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLENDER="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"
DIST_DIR="$REPO_ROOT/dist"
PKG="user_default.blender_crowd"
EXTRA_TEST=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --python)
            [ "$#" -ge 2 ] || { echo "--python requires a path" >&2; exit 2; }
            EXTRA_TEST="$2"
            shift 2
            ;;
        *)
            echo "unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

command -v "$BLENDER" >/dev/null 2>&1 || [ -x "$BLENDER" ] || {
    echo "Blender not found at $BLENDER (override with BLENDER=...)" >&2
    exit 1
}

"$REPO_ROOT/scripts/build-wheel.sh"

WHEEL_NAME="$(basename "$(ls "$REPO_ROOT/addon/blender_crowd/wheels/"*.whl)")"
# Keep the manifest's wheel entry in step with what was just built.
python3 - "$REPO_ROOT/addon/blender_crowd/blender_manifest.toml" "$WHEEL_NAME" <<'PY'
import re
import sys

path, wheel = sys.argv[1], sys.argv[2]
with open(path) as handle:
    text = handle.read()
text = re.sub(r'wheels = \["\./wheels/[^"]*"\]',
              'wheels = ["./wheels/{}"]'.format(wheel), text)
with open(path, "w") as handle:
    handle.write(text)
print("manifest wheel entry: {}".format(wheel))
PY

# `extension build` fails with a bare Errno 2 rather than creating this.
mkdir -p "$DIST_DIR"
rm -f "$DIST_DIR"/blender_crowd-*.zip

"$BLENDER" --command extension validate "$REPO_ROOT/addon/blender_crowd"
"$BLENDER" --command extension build \
    --source-dir "$REPO_ROOT/addon/blender_crowd" \
    --output-dir "$DIST_DIR"

ZIP="$(ls "$DIST_DIR"/blender_crowd-*.zip)"

# Remove any prior install so this is genuinely a clean-install test.
# `extension remove` takes repo.pkg_id as ONE positional, not a --repo flag.
"$BLENDER" --command extension remove "$PKG" >/dev/null 2>&1 || true

"$BLENDER" --command extension install-file --repo user_default --enable "$ZIP"

CROWD_REPO_ROOT="$REPO_ROOT" "$BLENDER" -b --python "$REPO_ROOT/tests/blender/test_install.py"

if [ -n "$EXTRA_TEST" ]; then
    case "$EXTRA_TEST" in
        /*) TEST_PATH="$EXTRA_TEST" ;;
        *) TEST_PATH="$REPO_ROOT/$EXTRA_TEST" ;;
    esac
    [ -f "$TEST_PATH" ] || { echo "test script not found: $TEST_PATH" >&2; exit 2; }
    CROWD_REPO_ROOT="$REPO_ROOT" "$BLENDER" -b --python "$TEST_PATH"
fi

echo "install test: PASS"
