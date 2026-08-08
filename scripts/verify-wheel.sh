#!/usr/bin/env bash
# Build a trace, install the wheel into a throwaway venv, and check the bridge.
#
# A plain CPython, not Blender: this proves the module itself, so that when
# Blender fails later the addon is the only remaining suspect. The venv's
# interpreter is deliberately whatever `python3` is rather than 3.11 -- the
# wheel is abi3 for >= 3.11, and installing it on a newer CPython is the
# whole point of building it that way.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WHEEL_DIR="$REPO_ROOT/addon/blender_crowd/wheels"
WORK_DIR="$REPO_ROOT/target/wheel-check"
AGENTS="${1:-50}"

shopt -s nullglob
wheels=("$WHEEL_DIR"/*.whl)
shopt -u nullglob
if [ ${#wheels[@]} -ne 1 ]; then
    echo "expected exactly one wheel in $WHEEL_DIR; run scripts/build-wheel.sh" >&2
    exit 1
fi

# Every directory made explicitly: a missing one must not turn into a bare
# Errno 2 halfway through.
mkdir -p "$WORK_DIR"

cargo run --release -p crowd-bench -- \
    run --scene crossing --agents "$AGENTS" --trace --out "$WORK_DIR"

rm -rf "$WORK_DIR/venv"
python3 -m venv "$WORK_DIR/venv"
"$WORK_DIR/venv/bin/pip" install --quiet --force-reinstall "${wheels[0]}"
"$WORK_DIR/venv/bin/python" "$REPO_ROOT/scripts/verify_wheel.py" \
    "$WORK_DIR/crossing-$AGENTS.crowdtrace"
