#!/usr/bin/env bash
# Requirement-level M6 acceptance audit. Every M6 gate remains fail-closed.
# M6_ALLOW_OPEN=1 permits an explicitly OPEN audit (for example, Blender not
# requested); it never converts a FAILED deterministic gate into success.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORT="$REPO_ROOT/docs/benchmarks/2026-08-19-m6-acceptance.md"
cd "$REPO_ROOT"

if [[ "${1:-}" == "--list" ]]; then
    cat <<'EOF'
foundation	M6 deterministic foundation
debugger_library	debugger navigation and reusable library
motion_source	CMU ruling and accepted CC0 motion baseline
reference_scenes	integrated deterministic reference scenes
blender	host Blender debugger/layer proof (M6_RUN_BLENDER=1)
mixed_tier	fixed 10K mixed-tier performance
extension_examples	Rust and Python external extension examples
acceptance_report	dated requirement-level report structure
release_workspace	full optimized Rust workspace
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
gate_acceptance_report="OPEN"
gate_release_workspace="OPEN"

run_gate() {
    local id="$1"
    local label="$2"
    shift 2
    local status

    if [[ "${M6_ACCEPTANCE_TEST_MODE:-0}" == "1" ]]; then
        if [[ "${M6_ACCEPTANCE_FAIL_GATE:-}" == "$id" ]]; then
            status="FAILED"
        else
            status="PASS"
        fi
    elif "$@"; then
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

run_open_gate() {
    local id="$1"
    local label="$2"
    shift 2
    local status

    if [[ "${M6_ACCEPTANCE_TEST_MODE:-0}" == "1" ]]; then
        if [[ "${M6_ACCEPTANCE_FAIL_GATE:-}" == "$id" ]]; then
            status="FAILED"
        else
            status="OPEN"
        fi
    elif "$@"; then
        status="OPEN"
    else
        status="FAILED"
    fi
    printf -v "gate_${id}" '%s' "$status"
    printf 'M6 gate %-22s %s\n' "$id" "$status"
    if [[ "$status" == "OPEN" ]]; then
        printf '  open gate: %s\n' "$label"
    elif [[ "$status" == "FAILED" ]]; then
        printf '  failed gate: %s\n' "$label"
    fi
}

check_motion_source_ruling() {
    python3 scripts/m6_acceptance_checks.py motion-source &&
    python3 -m unittest -q \
        tests/test_m6_cmu_motion.py \
        tests/test_m6_motion_database.py \
        tests/test_m6_motion_evaluation.py
}

check_acceptance_report() {
    python3 scripts/m6_acceptance_checks.py acceptance-report --report "$REPORT"
}

run_gate foundation "M6 deterministic foundation" scripts/m6-foundation-test.sh
run_gate debugger_library "debugger navigation and reusable library" \
    python3 -m unittest -q \
        tests/test_m6_debugger.py \
        tests/test_m6_debugger_navigation.py \
        tests/test_m6_library.py
run_open_gate motion_source \
    "CMU candidate fails the hard-zero joint-limit gate; CC0 fixture baseline only" \
    check_motion_source_ruling
run_gate reference_scenes "integrated deterministic reference scenes" scripts/m6-reference-scenes-test.sh

if [[ "${M6_RUN_BLENDER:-0}" == "1" ]]; then
    run_gate blender "host Blender debugger/layer proof" scripts/m6-blender-test.sh
else
    gate_blender="OPEN"
    printf 'M6 gate %-22s %s\n' "blender" "OPEN (set M6_RUN_BLENDER=1)"
fi

run_gate mixed_tier "fixed 10K mixed-tier performance" scripts/m6-performance-test.sh
run_gate extension_examples "Rust and Python external extension examples" \
    python3 -m unittest -q tests/test_m6_extension_examples.py
run_gate acceptance_report "dated requirement-level report structure" check_acceptance_report
run_gate release_workspace "full optimized Rust workspace" cargo test --workspace --release

criterion_status() {
    local status="PASS"
    local dependency
    for dependency in "$@"; do
        if [[ "$dependency" == "FAILED" ]]; then
            status="FAILED"
            break
        fi
        if [[ "$dependency" == "OPEN" ]]; then
            status="OPEN"
        fi
    done
    printf '%s' "$status"
}

criterion_1="$(criterion_status "$gate_foundation" "$gate_debugger_library" "$gate_blender")"
criterion_2="$(criterion_status "$gate_foundation")"
criterion_3="$(criterion_status "$gate_foundation" "$gate_reference_scenes")"
criterion_4="$(criterion_status "$gate_foundation" "$gate_reference_scenes")"
criterion_5="$(criterion_status "$gate_motion_source" "$gate_reference_scenes" "$gate_mixed_tier")"
criterion_6="$(criterion_status "$gate_foundation" "$gate_reference_scenes" "$gate_mixed_tier")"
criterion_7="$(criterion_status "$gate_foundation" "$gate_reference_scenes" "$gate_blender")"
criterion_8="$(criterion_status "$gate_foundation" "$gate_blender")"
criterion_9="$(criterion_status "$gate_extension_examples")"
criterion_10="$(criterion_status "$gate_foundation" "$gate_reference_scenes" "$gate_blender")"

audit_status="PASS"
for gate_status in \
    "$gate_foundation" \
    "$gate_debugger_library" \
    "$gate_motion_source" \
    "$gate_reference_scenes" \
    "$gate_blender" \
    "$gate_mixed_tier" \
    "$gate_extension_examples" \
    "$gate_acceptance_report" \
    "$gate_release_workspace"
do
    if [[ "$gate_status" == "FAILED" ]]; then
        audit_status="FAILED"
        break
    fi
    if [[ "$gate_status" == "OPEN" ]]; then
        audit_status="OPEN"
    fi
done

echo "M6 acceptance audit: $audit_status"
echo "  deterministic foundation: $gate_foundation"
echo "  debugger navigation/library: $gate_debugger_library"
if [[ "$gate_motion_source" == "OPEN" ]]; then
    echo "  CMU source candidate: REJECTED (3,587 raw joint-limit violations > hard limit 0)"
    echo "  accepted motion baseline: PASS (checked CC0 authored data)"
else
    echo "  CMU source candidate: UNRESOLVED"
    echo "  accepted motion baseline: $gate_motion_source"
fi
echo "  integrated reference scenes: $gate_reference_scenes"
echo "  host Blender layer/debugger proof: $gate_blender"
echo "  mixed-tier performance: $gate_mixed_tier"
echo "  extension examples (Rust/Python): $gate_extension_examples"
echo "  requirement-level acceptance report: $gate_acceptance_report"
echo "  full release workspace: $gate_release_workspace"

for criterion in 1 2 3 4 5 6 7 8 9 10; do
    eval "status=\$criterion_${criterion}"
    echo "  criterion ${criterion}: $status"
done

echo "  R1-R4 neural animation: DEFERRED TO M9"
echo "  independent-user verification: DEFERRED TO M9"

remaining=()
for criterion in 1 2 3 4 5 6 7 8 9 10; do
    eval "status=\$criterion_${criterion}"
    if [[ "$status" != "PASS" ]]; then
        remaining+=("criterion $criterion")
    fi
done
if [[ "$gate_acceptance_report" != "PASS" ]]; then
    remaining+=("acceptance report")
fi
if [[ "$gate_release_workspace" != "PASS" ]]; then
    remaining+=("release workspace")
fi

if [[ "${#remaining[@]}" -eq 0 ]]; then
    echo "  remaining M6 gates: none"
else
    printf '  remaining M6 gates: '
    printf '%s' "${remaining[0]}"
    for item in "${remaining[@]:1}"; do
        printf ', %s' "$item"
    done
    printf '\n'
fi

if [[ "$audit_status" == "PASS" ]]; then
    exit 0
fi
if [[ "$audit_status" == "OPEN" && "${M6_ALLOW_OPEN:-0}" == "1" ]]; then
    exit 0
fi
exit 2
