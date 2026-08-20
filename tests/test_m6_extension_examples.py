#!/usr/bin/env python3
"""Execution tests for the public M6 extension examples and audit runner."""

from __future__ import annotations

import json
import importlib.util
import os
from pathlib import Path
import subprocess
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
PYTHON_EXAMPLE = REPO_ROOT / "examples" / "m6_extension_python.py"
ACCEPTANCE_RUNNER = REPO_ROOT / "scripts" / "m6-acceptance.sh"
CHECKS_MODULE = REPO_ROOT / "scripts" / "m6_acceptance_checks.py"
ACCEPTANCE_REPORT = (
    REPO_ROOT / "docs" / "benchmarks" / "2026-08-19-m6-acceptance.md"
)

EXPECTED_OUTCOMES = [
    ("accepted_call", "accepted", None),
    ("over_budget_call", "fallback", "cost_budget_exceeded"),
    ("undeclared_channel_call", "rejected", "undeclared_input"),
    ("version_mismatch_call", "rejected", "unsupported_version"),
]


def parse_example_output(completed: subprocess.CompletedProcess[str]) -> list[dict]:
    if completed.returncode != 0:
        raise AssertionError(
            "example failed with exit {}:\nstdout:\n{}\nstderr:\n{}".format(
                completed.returncode,
                completed.stdout,
                completed.stderr,
            )
        )
    return [json.loads(line) for line in completed.stdout.splitlines() if line]


class M6ExtensionExampleTests(unittest.TestCase):
    def assert_example_contract(self, records: list[dict]) -> None:
        self.assertEqual(len(records), len(EXPECTED_OUTCOMES))
        for record, (case, status, reason) in zip(records, EXPECTED_OUTCOMES, strict=True):
            self.assertEqual(record["case"], case)
            self.assertEqual(record["status"], status)
            self.assertEqual(record.get("reason"), reason)
            self.assertEqual(record["schema_version"], 1)
            self.assertEqual(record["channel_version"], 1)
            self.assertEqual(record["inputs"], ["attention_target"])
            self.assertEqual(record["outputs"], ["gaze_offset"])
            self.assertEqual(record["cost_budget_millionths"], 100_000)
            self.assertIs(record["deterministic"], True)
            self.assertIs(record["failure_isolated"], True)

    def test_python_example_executes_every_extension_outcome(self) -> None:
        completed = subprocess.run(
            ["python3", str(PYTHON_EXAMPLE)],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assert_example_contract(parse_example_output(completed))

    def test_rust_example_executes_every_extension_outcome(self) -> None:
        completed = subprocess.run(
            [
                "cargo",
                "run",
                "--quiet",
                "-p",
                "crowd-core",
                "--example",
                "m6-extension-rust",
            ],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assert_example_contract(parse_example_output(completed))


class M6AcceptanceRunnerTests(unittest.TestCase):
    def load_checks(self):
        spec = importlib.util.spec_from_file_location("m6_acceptance_checks", CHECKS_MODULE)
        checks = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(checks)
        return checks

    def test_motion_source_checker_preserves_rejected_and_accepted_sources(self) -> None:
        checks = self.load_checks()
        ruling = checks.check_motion_source(REPO_ROOT)
        self.assertEqual(ruling["candidate_status"], "rejected")
        self.assertEqual(ruling["joint_limit_violations"], 3_587)
        self.assertEqual(ruling["joint_limit"], 0)
        self.assertEqual(ruling["baseline_status"], "accepted")
        self.assertEqual(ruling["baseline_license"], "CC0-1.0")

    def test_acceptance_report_preserves_the_open_motion_gate(self) -> None:
        checks = self.load_checks()
        ruling = checks.check_acceptance_report(ACCEPTANCE_REPORT)
        self.assertEqual(ruling["milestone_status"], "open")
        self.assertEqual(ruling["criteria"][5], "OPEN")
        self.assertEqual(
            [status for status in ruling["criteria"].values() if status == "PASS"],
            ["PASS"] * 9,
        )

    def run_audit(
        self,
        *,
        blender: bool,
        fail_gate: str | None = None,
        allow_open: bool = False,
    ):
        env = os.environ.copy()
        env["M6_ACCEPTANCE_TEST_MODE"] = "1"
        if blender:
            env["M6_RUN_BLENDER"] = "1"
        else:
            env.pop("M6_RUN_BLENDER", None)
        if fail_gate is None:
            env.pop("M6_ACCEPTANCE_FAIL_GATE", None)
        else:
            env["M6_ACCEPTANCE_FAIL_GATE"] = fail_gate
        if allow_open:
            env["M6_ALLOW_OPEN"] = "1"
        else:
            env.pop("M6_ALLOW_OPEN", None)
        return subprocess.run(
            [str(ACCEPTANCE_RUNNER)],
            cwd=REPO_ROOT,
            env=env,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_rejected_cmu_candidate_keeps_motion_criterion_open(self) -> None:
        completed = self.run_audit(blender=True)
        self.assertEqual(completed.returncode, 2, completed.stdout + completed.stderr)
        self.assertIn("M6 acceptance audit: OPEN", completed.stdout)
        self.assertIn("CMU source candidate: REJECTED", completed.stdout)
        self.assertIn("accepted motion baseline: PASS", completed.stdout)
        for criterion in (1, 2, 3, 4, 6, 7, 8, 9, 10):
            self.assertIn("criterion {}: PASS".format(criterion), completed.stdout)
        self.assertIn("criterion 5: OPEN", completed.stdout)
        self.assertIn("R1-R4 neural animation: DEFERRED TO M9", completed.stdout)
        self.assertIn("independent-user verification: DEFERRED TO M9", completed.stdout)
        self.assertIn("remaining M6 gates: criterion 5", completed.stdout)

    def test_open_audit_requires_explicit_override_for_zero_exit(self) -> None:
        completed = self.run_audit(blender=True, allow_open=True)
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        self.assertIn("M6 acceptance audit: OPEN", completed.stdout)
        self.assertIn("criterion 5: OPEN", completed.stdout)

    def test_omitted_blender_gate_stays_open_and_exits_nonzero(self) -> None:
        completed = self.run_audit(blender=False)
        self.assertEqual(completed.returncode, 2, completed.stdout + completed.stderr)
        self.assertIn("M6 acceptance audit: OPEN", completed.stdout)
        self.assertIn("host Blender layer/debugger proof: OPEN", completed.stdout)
        self.assertIn("remaining M6 gates:", completed.stdout)
        self.assertNotIn("host Blender layer/debugger proof: DEFERRED", completed.stdout)

    def test_failed_m6_gate_is_never_converted_to_deferred(self) -> None:
        completed = self.run_audit(
            blender=True,
            fail_gate="extension_examples",
            allow_open=True,
        )
        self.assertEqual(completed.returncode, 2, completed.stdout + completed.stderr)
        self.assertIn("M6 acceptance audit: FAILED", completed.stdout)
        self.assertIn("extension examples (Rust/Python): FAILED", completed.stdout)
        self.assertIn("criterion 9: FAILED", completed.stdout)
        self.assertIn("remaining M6 gates: criterion 5, criterion 9", completed.stdout)
        self.assertNotIn("criterion 9: DEFERRED", completed.stdout)


if __name__ == "__main__":
    unittest.main()
