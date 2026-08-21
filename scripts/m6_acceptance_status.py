#!/usr/bin/env python3
"""Pure M6 gate-to-criterion adjudication shared by the public runner and tests."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys


VALID_STATUSES = {"PASS", "OPEN", "FAILED"}
REQUIRED_GATES = (
    "foundation",
    "debugger_library",
    "motion_source",
    "reference_scenes",
    "blender",
    "mixed_tier",
    "extension_examples",
    "release_workspace",
    "clippy",
    "format",
    "python",
    "acceptance_report",
)

CRITERION_DEPENDENCIES = {
    1: ("foundation", "debugger_library", "blender"),
    2: ("foundation",),
    3: ("foundation", "reference_scenes"),
    4: ("foundation", "reference_scenes"),
    5: ("motion_source", "reference_scenes", "mixed_tier"),
    6: ("foundation", "reference_scenes", "mixed_tier"),
    7: ("foundation", "reference_scenes", "blender"),
    8: ("foundation", "blender"),
    9: ("foundation", "extension_examples"),
    10: ("foundation", "reference_scenes", "blender"),
}

GLOBAL_GATES = ("acceptance_report", "release_workspace", "clippy", "format", "python")

# Criterion 5 (production motion matching against an accepted clip-state
# baseline) was rescoped out of M6 on 2026-08-20. No conforming production
# motion corpus is available: the CMU candidate is rejected at 3,587 measured
# joint-limit violations against the hard limit of zero, and acquiring a
# licensed replacement has no schedule. The criterion, its unchanged measured
# thresholds, and its evidence contract move to M9 rather than being weakened,
# hidden, or asserted from the narrow CC0 fixture baseline. See
# docs/benchmarks/2026-08-20-m6-criterion-5-deferral.md.
DEFERRED_CRITERIA = {5: "M9"}

# motion_source keeps running and still fails M6 closed when its evidence is
# malformed or inconsistent, because the accepted CC0 fixture it validates is
# consumed by criteria 3, 4, and 6. Only its OPEN outcome -- "no production
# candidate meets the unchanged thresholds" -- is deferred with criterion 5.
DEFERRED_GATES = {"motion_source": "M9"}


def deferred_label(milestone):
    return "DEFERRED TO {}".format(milestone)


def combine_statuses(statuses):
    if "FAILED" in statuses:
        return "FAILED"
    if "OPEN" in statuses:
        return "OPEN"
    return "PASS"


def adjudicate(gates):
    if set(gates) != set(REQUIRED_GATES):
        missing = sorted(set(REQUIRED_GATES) - set(gates))
        extra = sorted(set(gates) - set(REQUIRED_GATES))
        raise ValueError("M6 gate set mismatch: missing={} extra={}".format(missing, extra))
    for gate, status in gates.items():
        if status not in VALID_STATUSES:
            raise ValueError("M6 gate {} has invalid status {}".format(gate, status))

    criteria = {}
    for criterion, dependencies in CRITERION_DEPENDENCIES.items():
        status = combine_statuses([gates[gate] for gate in dependencies])
        if criterion in DEFERRED_CRITERIA and status != "FAILED":
            status = deferred_label(DEFERRED_CRITERIA[criterion])
        criteria[str(criterion)] = status

    # A deferred gate's OPEN outcome no longer blocks M6; its FAILED still does.
    blocking = [
        status
        for gate, status in gates.items()
        if not (gate in DEFERRED_GATES and status == "OPEN")
    ]
    audit_status = combine_statuses(blocking)
    remaining = [
        "criterion {}".format(criterion)
        for criterion, status in criteria.items()
        if status != "PASS" and not status.startswith("DEFERRED")
    ]
    remaining.extend(gate.replace("_", " ") for gate in GLOBAL_GATES if gates[gate] != "PASS")
    return {
        "audit_status": audit_status,
        "criteria": criteria,
        "gates": dict(gates),
        "remaining": remaining,
    }


def render_summary(result, motion):
    gates = result["gates"]
    lines = [
        "M6 acceptance audit: {}".format(result["audit_status"]),
        "  deterministic foundation: {}".format(gates["foundation"]),
        "  debugger navigation/library: {}".format(gates["debugger_library"]),
    ]
    if gates["motion_source"] == "FAILED":
        lines.extend(
            (
                "  motion source candidate: UNRESOLVED",
                "  accepted motion baseline: FAILED",
            )
        )
    else:
        lines.extend(
            (
                "  motion source candidate: {} ({}; {})".format(
                    motion["candidate_status"].upper(),
                    motion["candidate_id"],
                    "; ".join(motion["failure_reasons"]) or "all thresholds satisfied",
                ),
                "  accepted motion baseline: PASS ({}; {})".format(
                    motion["baseline_database_id"], motion["baseline_license"]
                ),
            )
        )
    lines.extend(
        (
            "  integrated reference scenes: {}".format(gates["reference_scenes"]),
            "  host Blender layer/debugger proof: {}".format(gates["blender"]),
            "  mixed-tier performance: {}".format(gates["mixed_tier"]),
            "  extension examples/contracts (Rust/Python): {}".format(
                gates["extension_examples"]
            ),
            "  full release workspace: {}".format(gates["release_workspace"]),
            "  clippy warnings denied: {}".format(gates["clippy"]),
            "  Rust formatting: {}".format(gates["format"]),
            "  full Python suite: {}".format(gates["python"]),
            "  requirement-level acceptance report: {}".format(gates["acceptance_report"]),
        )
    )
    for criterion in range(1, 11):
        lines.append("  criterion {}: {}".format(criterion, result["criteria"][str(criterion)]))
    lines.extend(
        (
            "  production motion corpus (criterion 5): {}".format(
                deferred_label(DEFERRED_CRITERIA[5])
            ),
            "  R1-R4 neural animation: DEFERRED TO M9",
            "  independent-user verification: DEFERRED TO M9",
            "  remaining M6 gates: {}".format(
                ", ".join(result["remaining"]) if result["remaining"] else "none"
            ),
        )
    )
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--gates", type=Path, required=True)
    parser.add_argument("--motion", type=Path, required=True)
    parser.add_argument("--allow-open", action="store_true")
    args = parser.parse_args()

    gates = json.loads(args.gates.read_text(encoding="utf-8"))
    motion = json.loads(args.motion.read_text(encoding="utf-8"))
    result = adjudicate(gates)
    print(render_summary(result, motion))
    if result["audit_status"] == "PASS":
        return 0
    if result["audit_status"] == "OPEN" and args.allow_open:
        return 0
    return 2


if __name__ == "__main__":
    sys.exit(main())
