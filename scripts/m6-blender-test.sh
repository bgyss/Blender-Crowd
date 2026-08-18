#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLENDER="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"

if [[ ! -x "$BLENDER" ]]; then
    echo "M6 Blender smoke unavailable: Blender 5.2 LTS was not found at $BLENDER" >&2
    exit 2
fi

CROWD_REPO_ROOT="$REPO_ROOT" "$BLENDER" --background --factory-startup \
    --python "$REPO_ROOT/tests/blender/test_m6_debugger.py"
