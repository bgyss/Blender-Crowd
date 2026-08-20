#!/usr/bin/env python3
"""Execution tests for the public M6 extension examples and audit runner."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
PYTHON_EXAMPLE = REPO_ROOT / "examples" / "m6_extension_python.py"

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
        first = subprocess.run(
            ["python3", str(PYTHON_EXAMPLE)],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        second = subprocess.run(
            ["python3", str(PYTHON_EXAMPLE)],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(first.stdout, second.stdout)
        records = parse_example_output(first)
        self.assert_example_contract(records)
        self.assertEqual(records[0]["value"], {"gaze_offset": [0, 0, 0]})
        self.assertEqual(records[1]["value"], {"gaze_offset": [0, 0, 0]})
        self.assertIsNone(records[2]["value"])
        self.assertIsNone(records[3]["value"])

    def test_rust_example_executes_every_extension_outcome(self) -> None:
        command = [
            "cargo",
            "run",
            "--quiet",
            "-p",
            "crowd-core",
            "--example",
            "m6-extension-rust",
        ]
        first = subprocess.run(
            command,
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        second = subprocess.run(
            command,
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(first.stdout, second.stdout)
        records = parse_example_output(first)
        self.assert_example_contract(records)
        self.assertEqual(records[0]["value"], {"gaze_offset": [0, 0, 0]})
        self.assertEqual(records[1]["value"], {"gaze_offset": [0, 0, 0]})
        self.assertIsNone(records[2]["value"])
        self.assertIsNone(records[3]["value"])

    def test_rust_and_python_examples_have_the_same_deterministic_replay(self) -> None:
        rust = subprocess.run(
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
        python = subprocess.run(
            ["python3", str(PYTHON_EXAMPLE)],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(rust.returncode, 0, rust.stderr)
        self.assertEqual(python.returncode, 0, python.stderr)
        self.assertEqual(rust.stdout, python.stdout)
        self.assertEqual(
            hashlib.sha256(rust.stdout.encode("utf-8")).hexdigest(),
            "7132ecd92ab0feb0efc7592fdb144fd625727769b5abecad8d869726d73f83fc",
        )
if __name__ == "__main__":
    unittest.main()
