#!/usr/bin/env bash
# M5 Blender playback, render, and scale/profiling UI proof.
#
# Needs a measured scale report to populate the panel's measured half. Point
# M5_REPORT at one (and M5_ADJUDICATION at its `crowd-bench m5-gate` output);
# if neither is set, a 1,000-agent confirmation run and its adjudication are
# produced here first, because a panel populated from nothing would prove
# nothing about distinguishing estimates from measurements.
#
# Like the M4 runner this uses source-addon mode after building the native
# wheel, and must retain --python-use-system-env: Blender 5.2 otherwise
# ignores PYTHONPATH.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLENDER="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"
[ -x "$BLENDER" ] || { echo "Blender not found at $BLENDER" >&2; exit 1; }

M5_ARTIFACT_DIR="${M5_ARTIFACT_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/blender-crowd-m5-captures.XXXXXX")}"
mkdir -p "$M5_ARTIFACT_DIR"

if [ -z "${M5_REPORT:-}" ]; then
    echo "M5_REPORT unset; producing a 1,000-agent confirmation report first"
    cargo run --release -p crowd-bench -- run \
        --scene m5_city_flow --agents 1000 --out "$M5_ARTIFACT_DIR"
    M5_REPORT="$M5_ARTIFACT_DIR/m5_city_flow-1000.json"
    M5_ADJUDICATION="$M5_ARTIFACT_DIR/adjudication.json"
    # The gate exits non-zero on a failed adjudication. That is a real result
    # for the report, not a failure of this runner, so record it and continue:
    # the panel must show a FAIL just as clearly as a PASS.
    cargo run --release -p crowd-bench -- m5-gate \
        --report "$M5_REPORT" --out "$M5_ADJUDICATION" || true
fi

"$REPO_ROOT/scripts/build-wheel.sh"
WHEEL="$(ls "$REPO_ROOT"/addon/blender_crowd/wheels/blender_crowd_native-*.whl | tail -n 1)"
SITE_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/blender-crowd-m5-wheel.XXXXXX")"
trap 'rm -rf "$SITE_ROOT"' EXIT
unzip -q "$WHEEL" -d "$SITE_ROOT"

M5_ARTIFACT_DIR="$M5_ARTIFACT_DIR" \
M5_REPORT="$M5_REPORT" \
M5_ADJUDICATION="${M5_ADJUDICATION:-}" \
CROWD_SOURCE_ADDON=1 \
PYTHONPATH="$REPO_ROOT:$SITE_ROOT${PYTHONPATH:+:$PYTHONPATH}" \
    "$BLENDER" -b --factory-startup --python-use-system-env \
    --python "$REPO_ROOT/tests/blender/test_m5_scale.py"

echo "M5 Blender captures: $M5_ARTIFACT_DIR"
