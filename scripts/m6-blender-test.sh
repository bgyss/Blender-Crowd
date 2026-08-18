#!/usr/bin/env bash
# M6 trace debugger and graph-search smoke. Like the M4/M5 runners, this uses
# the current source add-on and freshly built native wheel instead of a possibly
# stale extension installed in the user's Blender profile.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLENDER="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"

if [[ ! -x "$BLENDER" ]]; then
    echo "M6 Blender smoke unavailable: Blender 5.2 LTS was not found at $BLENDER" >&2
    exit 2
fi

"$REPO_ROOT/scripts/build-wheel.sh"
WHEEL="$(ls "$REPO_ROOT"/addon/blender_crowd/wheels/blender_crowd_native-*.whl | tail -n 1)"
SITE_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/blender-crowd-m6-wheel.XXXXXX")"
trap 'rm -rf "$SITE_ROOT"' EXIT
unzip -q "$WHEEL" -d "$SITE_ROOT"

CROWD_REPO_ROOT="$REPO_ROOT" \
CROWD_SOURCE_ADDON=1 \
PYTHONPATH="$REPO_ROOT:$SITE_ROOT${PYTHONPATH:+:$PYTHONPATH}" \
    "$BLENDER" --background --factory-startup --python-use-system-env \
    --python "$REPO_ROOT/tests/blender/test_m6_debugger.py"
