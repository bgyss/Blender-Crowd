#!/usr/bin/env bash
# Clean-install M3 workflow/recovery proof.  It deliberately uses a canceled
# cache: the test proves the UI retains recovery evidence and never attaches it.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLENDER="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"

CROWD_REPO_ROOT="$REPO_ROOT" "$BLENDER" --factory-startup \
    --python "$REPO_ROOT/tests/blender/test_m3_production.py"
