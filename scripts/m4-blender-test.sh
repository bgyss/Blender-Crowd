#!/usr/bin/env bash
# M4 layout editor and cache-only interchange proof. This uses source-addon
# mode after building the native wheel. Blender ignores PYTHONPATH unless
# --python-use-system-env is enabled explicitly.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLENDER="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"
[ -x "$BLENDER" ] || { echo "Blender not found at $BLENDER" >&2; exit 1; }
"$REPO_ROOT/scripts/build-wheel.sh"
WHEEL="$(ls "$REPO_ROOT"/addon/blender_crowd/wheels/blender_crowd_native-*.whl | tail -n 1)"
SITE_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/blender-crowd-m4-wheel.XXXXXX")"
trap 'rm -rf "$SITE_ROOT"' EXIT
unzip -q "$WHEEL" -d "$SITE_ROOT"
M4_ARTIFACT_DIR="${M4_ARTIFACT_DIR:-}" CROWD_SOURCE_ADDON=1 PYTHONPATH="$REPO_ROOT:$SITE_ROOT${PYTHONPATH:+:$PYTHONPATH}" \
    "$BLENDER" -b --factory-startup --python-use-system-env \
    --python "$REPO_ROOT/tests/blender/test_m4_layout.py"
