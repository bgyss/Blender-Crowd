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
echo "  bidirectional debugger navigation without copied IDs: OPEN"
echo "  reusable subgraphs/actions and presets: OPEN"
echo "  complete activity/group/motion/interaction reference scenes: OPEN"
echo "  licensed deterministic motion data and measured motion/terrain thresholds: OPEN"
echo "  Blender interaction/ragdoll/recovery/hero layer proof: OPEN"
echo "  external extension examples for every claimed API language: OPEN"
echo "  mixed-tier performance and scalability report: OPEN"
echo "  requirement-level M6 acceptance report: OPEN"
echo "  R1-R4 neural animation: DEFERRED TO M9"
echo "  independent-user verification: DEFERRED TO M9"
echo "  full release workspace: PASS (debug workspace run remains unproven)"

if [[ "${M6_ALLOW_OPEN:-0}" == "1" ]]; then
    exit 0
fi
exit 2
