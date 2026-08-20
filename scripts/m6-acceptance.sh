#!/usr/bin/env bash
# Public requirement-level M6 acceptance audit. Every status comes from a gate
# executed by this process. M6_ALLOW_OPEN=1 acknowledges OPEN only; FAILED
# always exits nonzero.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORT="$REPO_ROOT/docs/benchmarks/2026-08-19-m6-acceptance.md"
ARTIFACT_DIR="$(mktemp -d "${TMPDIR:-/tmp}/blender-crowd-m6-acceptance.XXXXXX")"
MOTION_STATUS="$ARTIFACT_DIR/motion-source.json"
FRESH_STATUS="$ARTIFACT_DIR/fresh-gates.json"
ALL_STATUS="$ARTIFACT_DIR/all-gates.json"
trap 'rm -rf "$ARTIFACT_DIR"' EXIT HUP INT TERM
cd "$REPO_ROOT"

if [[ "${1:-}" == "--list" ]]; then
    cat <<'EOF'
foundation	M6 deterministic foundation
debugger_library	debugger navigation and reusable library
motion_source	active production-candidate evidence and accepted CC0 fixture
reference_scenes	integrated deterministic reference scenes
blender	host Blender debugger/layer proof (M6_RUN_BLENDER=1)
mixed_tier	fixed 10K mixed-tier performance
extension_examples	Rust/Python contract, determinism, and isolation
release_workspace	full optimized Rust workspace
clippy	workspace warnings denied
format	Rust formatting
python	full repository Python suite
acceptance_report	dated report hashes, evidence, and fresh statuses
EOF
    exit 0
fi

gate_foundation="OPEN"
gate_debugger_library="OPEN"
gate_motion_source="OPEN"
gate_reference_scenes="OPEN"
gate_blender="OPEN"
gate_mixed_tier="OPEN"
gate_extension_examples="OPEN"
gate_release_workspace="OPEN"
gate_clippy="OPEN"
gate_format="OPEN"
gate_python="OPEN"
gate_acceptance_report="OPEN"

run_gate() {
    local id="$1"
    local label="$2"
    shift 2
    local status

    if "$@"; then
        status="PASS"
    else
        status="FAILED"
    fi
    printf -v "gate_${id}" '%s' "$status"
    printf 'M6 gate %-22s %s\n' "$id" "$status"
    if [[ "$status" == "FAILED" ]]; then
        printf '  failed gate: %s\n' "$label"
    fi
}

run_motion_gate() {
    local status
    if python3 scripts/m6_acceptance_checks.py motion-source --json-out "$MOTION_STATUS" &&
        python3 -m unittest -q \
            tests/test_m6_cmu_motion.py \
            tests/test_m6_motion_database.py \
            tests/test_m6_motion_evaluation.py
    then
        status="$(python3 - "$MOTION_STATUS" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    print(json.load(handle)["gate_status"])
PY
)"
        if [[ "$status" != "PASS" && "$status" != "OPEN" ]]; then
            status="FAILED"
        fi
    else
        status="FAILED"
    fi
    gate_motion_source="$status"
    printf 'M6 gate %-22s %s\n' "motion_source" "$status"
    if [[ "$status" == "OPEN" ]]; then
        echo "  open gate: active production motion candidate failed one or more unchanged thresholds"
    elif [[ "$status" == "FAILED" ]]; then
        echo "  failed gate: motion candidate evidence is malformed, inconsistent, or unverified"
        cat > "$MOTION_STATUS" <<'EOF'
{
  "baseline_database_id": "unresolved",
  "baseline_license": "unresolved",
  "candidate_id": "unresolved",
  "candidate_status": "unresolved",
  "failure_reasons": ["motion source evidence failed validation"]
}
EOF
    fi
}

write_status_json() {
    local path="$1"
    local include_report="$2"
    cat > "$path" <<EOF
{
  "foundation": "$gate_foundation",
  "debugger_library": "$gate_debugger_library",
  "motion_source": "$gate_motion_source",
  "reference_scenes": "$gate_reference_scenes",
  "blender": "$gate_blender",
  "mixed_tier": "$gate_mixed_tier",
  "extension_examples": "$gate_extension_examples",
  "release_workspace": "$gate_release_workspace",
  "clippy": "$gate_clippy",
  "format": "$gate_format",
  "python": "$gate_python"$(if [[ "$include_report" == "1" ]]; then printf ',\n  "acceptance_report": "%s"' "$gate_acceptance_report"; fi)
}
EOF
}

run_gate foundation "M6 deterministic foundation" scripts/m6-foundation-test.sh
run_gate debugger_library "debugger navigation and reusable library" \
    python3 -m unittest -q \
        tests/test_m6_debugger.py \
        tests/test_m6_debugger_navigation.py \
        tests/test_m6_library.py
run_motion_gate
run_gate reference_scenes "integrated deterministic reference scenes" scripts/m6-reference-scenes-test.sh

if [[ "${M6_RUN_BLENDER:-0}" == "1" ]]; then
    run_gate blender "host Blender debugger/layer proof" scripts/m6-blender-test.sh
else
    gate_blender="OPEN"
    printf 'M6 gate %-22s %s\n' "blender" "OPEN (set M6_RUN_BLENDER=1)"
fi

run_gate mixed_tier "fixed 10K mixed-tier performance" scripts/m6-performance-test.sh
run_gate extension_examples "Rust/Python contract, determinism, and isolation" \
    scripts/m6-extension-examples-test.sh
run_gate release_workspace "full optimized Rust workspace" cargo test --workspace --release
run_gate clippy "workspace warnings denied" cargo clippy --workspace --all-targets -- -D warnings
run_gate format "Rust formatting" cargo fmt --all -- --check
run_gate python "full repository Python suite" \
    python3 -m unittest discover -s tests -p 'test_*.py'

write_status_json "$FRESH_STATUS" 0
run_gate acceptance_report "dated report hashes, evidence, and fresh statuses" \
    python3 scripts/m6_acceptance_checks.py acceptance-report \
        --repo-root "$REPO_ROOT" \
        --report "$REPORT" \
        --fresh-status "$FRESH_STATUS"
write_status_json "$ALL_STATUS" 1

status_args=(
    scripts/m6_acceptance_status.py
    --gates "$ALL_STATUS"
    --motion "$MOTION_STATUS"
)
if [[ "${M6_ALLOW_OPEN:-0}" == "1" ]]; then
    status_args+=(--allow-open)
fi
python3 "${status_args[@]}"
