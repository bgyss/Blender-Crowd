#!/usr/bin/env sh
# M6 R0 foundation: versioned contracts, deterministic interaction validation,
# out-of-process paired-clip worker, removable cache layers, and Python-side
# persistence. This is not full M6 acceptance or the independent-user UI gate.
set -eu

for fixture in \
    assets/reference/m6/activity-v1.json \
    assets/reference/m6/brain-v1.json \
    assets/reference/m6/contact-v1.json \
    assets/reference/m6/formation-v1.json \
    assets/reference/m6/hero-integration-v1.json \
    assets/reference/m6/interaction-animation-layer-v1.json \
    assets/reference/m6/interaction-motion-v1.json \
    assets/reference/m6/interaction-request-v1.json \
    assets/reference/m6/motion-database-input-v1.json \
    assets/reference/m6/mixed-tier-v1.json \
    assets/reference/m6/motion-provenance-v1.json \
    assets/reference/m6/perception-v1.json \
    assets/reference/m6/physics-transition-v1.json \
    assets/reference/m6/retarget-profile-v1.json \
    assets/reference/m6/terrain-motion-v1.json \
    assets/reference/m6/trajectory-v1.json
do
    test -s "$fixture"
done

cargo test -p crowd-core --test m6_schema_fixtures
cargo test -p crowd-core --test m6_interaction --test m6_interaction_invalid
cargo test -p crowd-core --test m6_interaction_scheduler
cargo test -p crowd-core --test m6_perception --test m6_blackboard
cargo test -p crowd-core --test m6_brain_runtime --test m6_runtime_perception
cargo test -p crowd-core --test m6_action_library
cargo test -p crowd-core --test m6_action_node
cargo test -p crowd-core --test m6_activity --test m6_activity_behavior
cargo test -p crowd-core --test m6_activity_rich
cargo test -p crowd-core --test m6_formations
cargo test -p crowd-core --test m6_motion_matching --test m6_motion_feedback
cargo test -p crowd-core --test m6_metrics
cargo test -p crowd-core --test m6_physics_recovery --test m6_extensions
cargo test -p crowd-cache --test interaction_layers
cargo test -p crowd-bench --test m6_worker
python3 -m unittest -q tests/test_m6_extensions.py tests/test_m6_interaction_layers.py tests/test_m6_debugger.py tests/test_m6_motion_database.py tests/test_m6_motion_evaluation.py tests/test_m6_physics_boundaries.py
python3 -m py_compile scripts/m6_motion_build.py scripts/m6_motion_evaluate.py tests/blender/test_m6_debugger.py addon/blender_crowd/m6_debugger.py addon/blender_crowd/m6_extensions.py addon/blender_crowd/m6_interaction.py addon/blender_crowd/m6_physics.py addon/blender_crowd/operators.py addon/blender_crowd/panels.py addon/blender_crowd/properties.py

artifact_dir="${M6_ARTIFACT_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/blender-crowd-m6-r0.XXXXXX")}"
cleanup_artifact_dir=0
if [ -z "${M6_ARTIFACT_DIR:-}" ]; then
    cleanup_artifact_dir=1
fi
if [ "$cleanup_artifact_dir" -eq 1 ]; then
    trap 'rm -rf "$artifact_dir"' EXIT HUP INT TERM
else
    mkdir -p "$artifact_dir"
fi

cargo run --quiet -p crowd-bench --bin m6-interaction-worker -- \
    --request assets/reference/m6/interaction-request-v1.json \
    --out "$artifact_dir/interaction-motion-v1.json"

test -s "$artifact_dir/interaction-motion-v1.json"
echo "M6 R0 foundation passed; motion artifact: $artifact_dir/interaction-motion-v1.json"
