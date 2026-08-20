#!/usr/bin/env sh
set -eu

artifact_dir="${M6_REFERENCE_SCENE_ARTIFACT_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/blender-crowd-m6-scenes.XXXXXX")}"
cleanup_artifact_dir=0
if [ -z "${M6_REFERENCE_SCENE_ARTIFACT_DIR:-}" ]; then
    cleanup_artifact_dir=1
fi
if [ "$cleanup_artifact_dir" -eq 1 ]; then
    trap 'rm -rf "$artifact_dir"' EXIT HUP INT TERM
else
    mkdir -p "$artifact_dir"
fi

cargo test -p crowd-bench --test m6_acceptance_scenes

for run in first second
do
    cargo run --quiet -p crowd-bench --bin m6-acceptance-scenes -- \
        --fixture assets/reference/m6/acceptance-scenes-v1.json \
        --motion-report docs/benchmarks/2026-08-18-m6-cmu-motion.json \
        --out "$artifact_dir/$run.json"
done

cmp "$artifact_dir/first.json" "$artifact_dir/second.json"
python3 - "$artifact_dir/first.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    report = json.load(handle)
assert report["schema_version"] == 1
assert report["passed"] is True
assert report["hard_safety_passed"] is True
assert report["unrelated_agent_mutations"] == 0
assert len(report["scenes"]) == 6
assert report["motion_source_selection"]["external_candidate"]["status"] == "rejected"
assert report["motion_source_selection"]["baseline"]["license_id"] == "CC0-1.0"
print("M6 reference scenes passed twice with exact hashes and metrics: {}".format(
    report["deterministic_replay_hash"]
))
PY
