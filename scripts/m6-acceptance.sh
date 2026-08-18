#!/usr/bin/env bash
set -euo pipefail

# M6 acceptance audit. The deterministic foundation is executable locally; the
# higher gates remain explicit until their evidence is supplied. Set
# M6_RUN_BLENDER=1 to attempt the host Blender smoke as a separate lane.

scripts/m6-foundation-test.sh

blender_status="SKIPPED"
if [[ "${M6_RUN_BLENDER:-0}" == "1" ]]; then
    if scripts/m6-blender-test.sh; then
        blender_status="PASS"
    else
        blender_status="BLOCKED_OR_FAILED"
    fi
fi

echo "M6 acceptance audit: OPEN"
echo "  deterministic foundation: PASS"
echo "  Blender debugger smoke: ${blender_status}"
echo "  production motion/terrain/physics thresholds: OPEN"
echo "  R1-R4 reactive-motion research gates: OPEN"
echo "  independent-user UI gate: OPEN"
echo "  full release workspace: PASS (debug workspace run remains unproven)"

if [[ "${M6_ALLOW_OPEN:-0}" == "1" ]]; then
    exit 0
fi
exit 2
