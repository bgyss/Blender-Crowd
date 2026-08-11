#!/usr/bin/env python3
"""Contract tests for the consolidated M0 acceptance runner."""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
RUNNER = REPO_ROOT / "scripts" / "m0-acceptance.sh"


class M0AcceptanceRunnerTests(unittest.TestCase):
    def test_list_exposes_every_gate_in_required_order(self) -> None:
        completed = subprocess.run(
            [str(RUNNER), "--list"],
            cwd=REPO_ROOT,
            check=True,
            capture_output=True,
            text=True,
        )

        step_ids = [line.split("\t", 1)[0] for line in completed.stdout.splitlines()]
        self.assertEqual(
            step_ids,
            [
                "workspace_tests",
                "release_density",
                "release_two_room",
                "solver_baselines",
                "cache_lifecycle",
                "cache_experiment",
                "wheel_facade",
                "blender_clean_install",
                "blender_playback",
                "static_checks",
            ],
        )

    def test_first_failure_is_preserved_in_machine_readable_summary(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            summary_path = Path(temp_dir) / "summary.json"
            env = os.environ.copy()
            env["M0_ACCEPTANCE_TEST_MODE"] = "1"
            env["M0_ACCEPTANCE_SUMMARY"] = str(summary_path)

            completed = subprocess.run(
                [str(RUNNER)],
                cwd=REPO_ROOT,
                env=env,
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertNotEqual(completed.returncode, 0)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            self.assertEqual(summary["schema_version"], 1)
            self.assertEqual(summary["status"], "failed")
            self.assertEqual(summary["first_failure"], "failure_probe")
            self.assertEqual(
                [(step["id"], step["exit_code"]) for step in summary["steps"]],
                [("success_probe", 0), ("failure_probe", 23)],
            )
            self.assertIn("environment", summary)
            self.assertIn("duration_seconds", summary["steps"][0])


if __name__ == "__main__":
    unittest.main()
