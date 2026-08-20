#!/usr/bin/env python3
"""Focused artifact checks used by the M6 acceptance runner."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re


UNCHANGED_HARD_THRESHOLD_CONTRACT = {
    "root_teleportations": {"limit": 0, "evidence_status": "not_applicable"},
    "undeclared_contacts": {"limit": 0, "evidence_status": "not_applicable"},
    "source_hash_drift": {"limit": 0, "evidence_status": "measured"},
    "cross_cache_mutations": {"limit": 0, "evidence_status": "not_applicable"},
    "joint_limit_violations": {"limit": 0, "evidence_status": "measured"},
}

UNCHANGED_SOFT_LIMITS = {
    "max_foot_slide_millimeters": 21,
    "max_trajectory_deviation_millimeters": 3,
    "max_turn_discontinuity_microradians": 60_005,
    "rejected_frame_rate_ppm": 0,
}


def load_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def resolve_repo_path(repo_root, relative, label):
    if not isinstance(relative, str) or not relative:
        raise ValueError("{} must be a non-empty repository-relative path".format(label))
    repo_root = Path(repo_root).resolve()
    path = (repo_root / relative).resolve()
    if repo_root not in path.parents:
        raise ValueError("{} escapes the repository".format(label))
    if not path.is_file():
        raise ValueError("{} does not exist: {}".format(label, relative))
    return path


def check_motion_source(repo_root):
    repo_root = Path(repo_root).resolve()
    threshold_path = resolve_repo_path(
        repo_root,
        "assets/reference/m6/motion-thresholds-v1.json",
        "motion threshold contract",
    )
    limits = load_json(threshold_path)
    if limits.get("schema_version") != 1 or not limits.get("threshold_id"):
        raise ValueError("motion threshold contract is malformed")

    report_path = resolve_repo_path(
        repo_root, limits.get("baseline_report"), "motion candidate report"
    )
    manifest_path = resolve_repo_path(
        repo_root, limits.get("source_manifest"), "motion source manifest"
    )
    candidate = load_json(report_path)
    manifest = load_json(manifest_path)
    if candidate.get("schema_version") != 1 or manifest.get("schema_version") != 1:
        raise ValueError("motion candidate or source manifest version is malformed")
    dataset_id = manifest.get("dataset_id")
    if not dataset_id or candidate.get("database_id") != dataset_id:
        raise ValueError("motion candidate database identity does not match its manifest")
    if candidate.get("source_manifest_id") != dataset_id:
        raise ValueError("motion candidate source-manifest identity does not match")
    profile_id = manifest.get("retarget_profile", {}).get("profile_id")
    if not profile_id or candidate.get("retarget_profile_id") != profile_id:
        raise ValueError("motion candidate retarget identity does not match its manifest")

    files = manifest.get("files")
    if not isinstance(files, list) or not files:
        raise ValueError("motion source manifest has no files")
    expected_hashes = {}
    for item in files:
        identity = item.get("id") if isinstance(item, dict) else None
        digest = item.get("sha256") if isinstance(item, dict) else None
        if (
            not identity
            or not isinstance(digest, str)
            or len(digest) != 64
            or re.fullmatch(r"[0-9a-f]{64}", digest) is None
            or identity in expected_hashes
        ):
            raise ValueError("motion source manifest contains malformed source hashes")
        expected_hashes[identity] = digest
    if candidate.get("source_hashes") != expected_hashes:
        raise ValueError("motion candidate source hashes do not match its manifest")
    if limits.get("source_hashes") != expected_hashes:
        raise ValueError("motion threshold source hashes do not match the candidate manifest")
    if re.fullmatch(r"[0-9a-f]{64}", candidate.get("source_hash", "")) is None:
        raise ValueError("motion candidate aggregate source hash is malformed")

    hard_limits = limits.get("hard_limits")
    observations = candidate.get("hard_limit_observations")
    evidence = candidate.get("hard_limit_evidence")
    quality = candidate.get("quality_metrics")
    if not all(isinstance(value, dict) for value in (hard_limits, observations, evidence, quality)):
        raise ValueError("motion hard-limit evidence is malformed")
    actual_hard_contract = {
        metric: {
            "limit": contract.get("limit") if isinstance(contract, dict) else None,
            "evidence_status": (
                contract.get("evidence_status") if isinstance(contract, dict) else None
            ),
        }
        for metric, contract in hard_limits.items()
    }
    if actual_hard_contract != UNCHANGED_HARD_THRESHOLD_CONTRACT:
        raise ValueError("motion hard thresholds no longer match the unchanged M6 contract")

    failures = []
    for metric, contract in hard_limits.items():
        if not isinstance(contract, dict) or not isinstance(contract.get("limit"), int):
            raise ValueError("motion hard limit {} is malformed".format(metric))
        status = contract.get("evidence_status")
        if status == "measured":
            observed = observations.get(metric)
            if not isinstance(observed, int) or observed < 0:
                raise ValueError("motion hard observation {} is malformed".format(metric))
            if contract.get("baseline") != observed or quality.get(metric) != observed:
                raise ValueError("motion hard observation {} disagrees with its baseline".format(metric))
            if evidence.get(metric) != {"status": "measured", "observed": observed}:
                raise ValueError("motion hard evidence {} is malformed".format(metric))
            if observed > contract["limit"]:
                failures.append(
                    "{} observed {} > limit {}".format(metric, observed, contract["limit"])
                )
        elif status == "not_applicable":
            if not contract.get("reason"):
                raise ValueError("motion hard limit {} lacks a not-applicable reason".format(metric))
        else:
            raise ValueError("motion hard limit {} has an unknown evidence status".format(metric))

    clip_metrics = candidate.get("clip_metrics")
    if not isinstance(clip_metrics, list) or not clip_metrics:
        raise ValueError("motion candidate clip evidence is malformed")
    if sum(item.get("joint_limit_violations", -1) for item in clip_metrics) != observations.get(
        "joint_limit_violations"
    ):
        raise ValueError("motion joint-limit clip evidence does not reconcile")

    soft_limits = limits.get("soft_limits")
    baseline_metrics = candidate.get("threshold_baseline")
    if not isinstance(soft_limits, dict) or not isinstance(baseline_metrics, dict):
        raise ValueError("motion soft-limit evidence is malformed")
    actual_soft_limits = {
        metric: contract.get("limit") if isinstance(contract, dict) else None
        for metric, contract in soft_limits.items()
    }
    if actual_soft_limits != UNCHANGED_SOFT_LIMITS:
        raise ValueError("motion soft thresholds no longer match the unchanged M6 contract")
    if limits.get("policy") != {
        "soft_limit_rule": "integer-ceiling-of-maximum-observed-baseline",
        "additional_headroom": 0,
        "loosening_requires_new_dated_adjudication": True,
    }:
        raise ValueError("motion threshold policy no longer matches the unchanged M6 contract")
    for metric, contract in soft_limits.items():
        observed = baseline_metrics.get(metric)
        if (
            not isinstance(contract, dict)
            or not isinstance(observed, int)
            or contract.get("baseline") != observed
            or not isinstance(contract.get("limit"), int)
        ):
            raise ValueError("motion soft-limit evidence {} is malformed".format(metric))
        if observed > contract["limit"]:
            failures.append(
                "{} observed {} > limit {}".format(metric, observed, contract["limit"])
            )

    provenance_path = resolve_repo_path(
        repo_root,
        "assets/reference/m6/motion-provenance-v1.json",
        "accepted authored motion provenance",
    )
    baseline = load_json(provenance_path)
    if baseline.get("schema_version") != 1:
        raise ValueError("accepted authored motion provenance version is malformed")
    if baseline.get("license_id") != "CC0-1.0" or baseline.get("redistribution_allowed") is not True:
        raise ValueError("accepted authored motion baseline lost checked CC0 provenance")
    source_uri = baseline.get("source_uri", "")
    if not source_uri.startswith("repo://"):
        raise ValueError("accepted authored motion source URI is malformed")
    database_path = resolve_repo_path(repo_root, source_uri[len("repo://") :], "authored motion database")
    database = load_json(database_path)
    if database.get("database_id") != "reference-humanoid-motion":
        raise ValueError("accepted authored motion database identity changed")

    observed_joint_limits = observations.get("joint_limit_violations")
    joint_limit = hard_limits["joint_limit_violations"]["limit"]
    gate_status = "PASS" if not failures else "OPEN"
    return {
        "gate_status": gate_status,
        "candidate_status": "accepted" if gate_status == "PASS" else "rejected",
        "candidate_id": dataset_id,
        "candidate_report": str(report_path.relative_to(repo_root)),
        "threshold_id": limits["threshold_id"],
        "joint_limit_violations": observed_joint_limits,
        "joint_limit": joint_limit,
        "failure_reasons": failures,
        "baseline_status": "accepted",
        "baseline_license": baseline["license_id"],
        "baseline_database_id": database["database_id"],
    }


HASHED_INPUTS = (
    "assets/reference/m6/motion-thresholds-v1.json",
    "docs/benchmarks/2026-08-18-m6-cmu-motion.json",
    "assets/reference/m6/motion-database-input-v1.json",
    "assets/reference/m6/motion-provenance-v1.json",
    "assets/reference/m6/acceptance-scenes-v1.json",
    "schemas/m6-acceptance-scenes-v1.schema.json",
)

REQUIRED_EVIDENCE = (
    "docs/benchmarks/2026-08-20-m6-criterion-5-deferral.md",
    "docs/benchmarks/2026-08-18-m6-foundation.md",
    "docs/benchmarks/2026-08-18-m6-cmu-motion.md",
    "docs/benchmarks/2026-08-18-m6-reference-scenes.md",
    "docs/benchmarks/2026-08-18-m6-blender-layers.md",
    "docs/benchmarks/2026-08-18-m6-mixed-tier.md",
    "examples/m6-extension-rust.rs",
    "examples/m6_extension_python.py",
    "scripts/m6-foundation-test.sh",
    "scripts/m6-reference-scenes-test.sh",
    "scripts/m6-blender-test.sh",
    "scripts/m6-performance-test.sh",
    "scripts/m6-extension-examples-test.sh",
)

FRESH_GATE_IDS = (
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
)


def load_status_module():
    path = Path(__file__).with_name("m6_acceptance_status.py")
    spec = importlib.util.spec_from_file_location("m6_acceptance_status", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def parse_criterion_statuses(text):
    criteria = {}
    for line in text.splitlines():
        columns = [column.strip() for column in line.strip().strip("|").split("|")]
        if len(columns) == 4 and columns[0].isdigit():
            criterion = int(columns[0])
            if criterion in criteria:
                raise ValueError("M6 acceptance report repeats criterion {}".format(criterion))
            criteria[criterion] = columns[-1]
    return criteria


def parse_expected_gate_statuses(text):
    statuses = {}
    pattern = re.compile(
        r"^\| `([a-z_]+)` \| (PASS|OPEN|FAILED) \(expected current run\) \|$",
        re.MULTILINE,
    )
    for match in pattern.finditer(text):
        gate, status = match.groups()
        if gate in statuses:
            raise ValueError("M6 acceptance report repeats expected gate {}".format(gate))
        statuses[gate] = status
    return statuses


def check_private_paths(text, repo_root):
    private_pattern = re.compile(
        r"/Users/|/home/|\.codex|/private/tmp|/Volumes/|"
        r"(?i:[A-Z]:\\Users\\)|(?:^|\s)~[/\\]|"
        r"\$\{?HOME\}?|%USERPROFILE%",
        flags=re.MULTILINE,
    )
    if private_pattern.search(text):
        raise ValueError("M6 acceptance report contains a contributor-local path")

    contract_path = Path(repo_root) / "CLAUDE.md"
    contract = contract_path.read_text(encoding="utf-8") if contract_path.is_file() else ""
    allowed_blender = "/Applications/Blender.app/Contents/MacOS/Blender"
    for line in text.splitlines():
        if "/Applications/" not in line:
            continue
        allowed_line = (
            "BLENDER={}".format(allowed_blender) in line
            and "scripts/m6-blender-test.sh" in line
            and allowed_blender in contract
        )
        if not allowed_line or line.count("/Applications/") != 1:
            raise ValueError("M6 acceptance report contains a contributor-local path")


def check_acceptance_report(path, repo_root, fresh_gate_statuses):
    repo_root = Path(repo_root).resolve()
    expected_report = resolve_repo_path(
        repo_root,
        "docs/benchmarks/2026-08-20-m6-acceptance.md",
        "canonical M6 acceptance report",
    )
    path = Path(path).resolve()
    if path != expected_report:
        raise ValueError("M6 acceptance report path is not the canonical checked-in report")
    text = path.read_text(encoding="utf-8")
    required_headings = (
        "# M6 requirement-level acceptance — 2026-08-20",
        "## Result",
        "## Environment",
        "## Inputs and hashes",
        "## Criterion-by-criterion adjudication",
        "## Known failures and rejected inputs",
        "## Unsupported claims",
        "## Criterion 5 deferral to M9",
        "## M9 deferrals",
        "## Verification",
    )
    for heading in required_headings:
        if heading not in text:
            raise ValueError("M6 acceptance report missing heading: {}".format(heading))

    reported_hashes = {
        relative: digest
        for relative, digest in re.findall(
            r"^\| `([^`]+)` \| `([0-9a-f]{64})` \|", text, flags=re.MULTILINE
        )
    }
    if set(reported_hashes) != set(HASHED_INPUTS):
        raise ValueError("M6 acceptance report SHA-256 input set is incomplete")
    actual_hashes = {}
    for relative in HASHED_INPUTS:
        evidence_path = resolve_repo_path(repo_root, relative, "hashed acceptance input")
        actual_hashes[relative] = hashlib.sha256(evidence_path.read_bytes()).hexdigest()
    if reported_hashes != actual_hashes:
        raise ValueError("M6 acceptance report SHA-256 values are stale or fabricated")

    for relative in REQUIRED_EVIDENCE:
        resolve_repo_path(repo_root, relative, "required M6 evidence")
        if relative not in text:
            raise ValueError("M6 acceptance report does not reference required evidence: {}".format(relative))

    for target in re.findall(r"\]\(([^)#]+)(?:#[^)]*)?\)", text):
        if "://" not in target:
            linked = (path.parent / target).resolve()
            if not linked.is_file():
                raise ValueError("M6 acceptance report references missing evidence: {}".format(target))

    check_private_paths(text, repo_root)

    if set(fresh_gate_statuses) != set(FRESH_GATE_IDS):
        raise ValueError("fresh M6 gate status set is incomplete")
    for gate, status in fresh_gate_statuses.items():
        if status not in {"PASS", "OPEN", "FAILED"}:
            raise ValueError("fresh M6 gate {} has invalid status {}".format(gate, status))
    reported_expected = parse_expected_gate_statuses(text)
    if reported_expected != fresh_gate_statuses:
        raise ValueError(
            "M6 acceptance report expected gate labels disagree with executed gates: {}".format(
                reported_expected
            )
        )

    status_module = load_status_module()
    adjudication_gates = dict(fresh_gate_statuses)
    adjudication_gates["acceptance_report"] = "PASS"
    expected_result = status_module.adjudicate(adjudication_gates)
    expected_statuses = {
        int(criterion): status for criterion, status in expected_result["criteria"].items()
    }
    criteria = parse_criterion_statuses(text)
    if criteria != expected_statuses:
        raise ValueError(
            "M6 acceptance report criterion statuses changed: {}".format(criteria)
        )

    for phrase in (
        "DEFERRED TO M9",
        "docs/benchmarks/2026-08-20-m6-criterion-5-deferral.md",
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
    milestone_status = expected_result["audit_status"].lower()
    result_line = re.search(r"^\*\*(PASS|OPEN|FAILED) —", text, flags=re.MULTILINE)
    if result_line is None or result_line.group(1).lower() != milestone_status:
        raise ValueError("M6 acceptance report result label is stale or fabricated")

    return {
        "milestone_status": milestone_status,
        "criteria": criteria,
        "sha256": actual_hashes,
        "executed_gates": dict(fresh_gate_statuses),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("check", choices=("motion-source", "acceptance-report"))
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--report", type=Path)
    parser.add_argument("--fresh-status", type=Path)
    parser.add_argument("--json-out", type=Path)
    args = parser.parse_args()

    if args.check == "motion-source":
        ruling = check_motion_source(args.repo_root)
        detail = "; ".join(ruling["failure_reasons"]) or "all unchanged thresholds satisfied"
        print(
            "Motion source candidate {}: {} ({})".format(
                ruling["candidate_status"], ruling["candidate_id"], detail
            )
        )
        print(
            "Accepted motion baseline: checked {} authored data".format(
                ruling["baseline_license"]
            )
        )
        if args.json_out is not None:
            args.json_out.write_text(
                json.dumps(ruling, indent=2, sort_keys=True) + "\n", encoding="utf-8"
            )
        return

    if args.report is None or args.fresh_status is None:
        parser.error("acceptance-report requires --report and --fresh-status")
    fresh_gate_statuses = load_json(args.fresh_status)
    ruling = check_acceptance_report(args.report, args.repo_root, fresh_gate_statuses)
    print(
        "M6 acceptance report structure: PASS; milestone status: {}".format(
            ruling["milestone_status"].upper()
        )
    )


if __name__ == "__main__":
    main()
