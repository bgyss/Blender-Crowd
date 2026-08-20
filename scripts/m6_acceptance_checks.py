#!/usr/bin/env python3
"""Focused artifact checks used by the M6 acceptance runner."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re


def load_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def check_motion_source(repo_root):
    repo_root = Path(repo_root)
    cmu = load_json(repo_root / "docs/benchmarks/2026-08-18-m6-cmu-motion.json")
    limits = load_json(repo_root / "assets/reference/m6/motion-thresholds-v1.json")
    baseline = load_json(repo_root / "assets/reference/m6/motion-provenance-v1.json")

    observed = cmu["hard_limit_observations"]["joint_limit_violations"]
    limit = limits["hard_limits"]["joint_limit_violations"]["limit"]
    if (observed, limit) != (3_587, 0):
        raise ValueError(
            "CMU source ruling changed: observed={} limit={}".format(observed, limit)
        )
    if cmu["hard_limit_evidence"]["joint_limit_violations"] != {
        "observed": 3_587,
        "status": "measured",
    }:
        raise ValueError("CMU joint-limit evidence is no longer measured at 3,587")
    if limits["hard_limits"]["joint_limit_violations"] != {
        "baseline": 3_587,
        "evidence_status": "measured",
        "limit": 0,
    }:
        raise ValueError("CMU hard-zero threshold contract changed")
    if baseline["license_id"] != "CC0-1.0" or baseline["redistribution_allowed"] is not True:
        raise ValueError("accepted authored motion baseline lost checked CC0 provenance")

    return {
        "candidate_status": "rejected",
        "joint_limit_violations": observed,
        "joint_limit": limit,
        "baseline_status": "accepted",
        "baseline_license": baseline["license_id"],
    }


def check_acceptance_report(path):
    path = Path(path)
    text = path.read_text(encoding="utf-8")
    required_headings = (
        "# M6 requirement-level acceptance — 2026-08-19",
        "## Result",
        "## Environment",
        "## Inputs and hashes",
        "## Criterion-by-criterion adjudication",
        "## Known failures and rejected inputs",
        "## Unsupported claims",
        "## M9 deferrals",
        "## Verification",
    )
    for heading in required_headings:
        if heading not in text:
            raise ValueError("M6 acceptance report missing heading: {}".format(heading))

    expected_statuses = {criterion: "PASS" for criterion in range(1, 11)}
    expected_statuses[5] = "OPEN"
    criteria = {}
    for line in text.splitlines():
        columns = [column.strip() for column in line.strip().strip("|").split("|")]
        if columns and columns[0].isdigit():
            criteria[int(columns[0])] = columns[-1]
    if criteria != expected_statuses:
        raise ValueError(
            "M6 acceptance report criterion statuses changed: {}".format(criteria)
        )

    for phrase in (
        "M6 remains unaccepted",
        "3,587",
        "hard limit of zero",
        "CC0",
        "cloth/hair/Geometry Nodes deformation",
        "rigid-body parity",
        "GPU",
        "arbitrary-scene",
        "long-duration",
        "visual quality",
        "R1–R4",
        "independent-user verification",
    ):
        if phrase not in text:
            raise ValueError("M6 acceptance report missing boundary: {}".format(phrase))
    if re.search(r"/Users/|/home/|\.codex|/private/tmp", text):
        raise ValueError("M6 acceptance report contains a contributor-local path")

    return {"milestone_status": "open", "criteria": criteria}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("check", choices=("motion-source", "acceptance-report"))
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()

    if args.check == "motion-source":
        ruling = check_motion_source(args.repo_root)
        print(
            "CMU source candidate remains rejected: {} joint-limit violations > hard limit {}".format(
                ruling["joint_limit_violations"], ruling["joint_limit"]
            )
        )
        print(
            "Accepted motion baseline: checked {} authored data".format(
                ruling["baseline_license"]
            )
        )
        return

    if args.report is None:
        parser.error("acceptance-report requires --report")
    ruling = check_acceptance_report(args.report)
    print(
        "M6 acceptance report structure: PASS; milestone status: {}".format(
            ruling["milestone_status"].upper()
        )
    )


if __name__ == "__main__":
    main()
