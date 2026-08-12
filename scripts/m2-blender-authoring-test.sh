#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLENDER="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"

# Clean installation and native loading remain background/headless checks.
"$REPO_ROOT/scripts/blender-install-test.sh"

# Blender disables the undo operator in `-b` mode even after undo_push(). Run
# the editor persistence test unattended with a real window context; the test
# exits Blender itself when it completes.
CROWD_REPO_ROOT="$REPO_ROOT" "$BLENDER" --factory-startup \
    --python "$REPO_ROOT/tests/blender/test_m2_authoring.py"
