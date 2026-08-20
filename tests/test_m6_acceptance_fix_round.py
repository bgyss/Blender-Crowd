#!/usr/bin/env python3
"""Fix-round regressions for the fail-closed M6 acceptance audit."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKS_MODULE = REPO_ROOT / "scripts" / "m6_acceptance_checks.py"
PUBLIC_RUNNER = REPO_ROOT / "scripts" / "m6-acceptance.sh"
STATUS_HARNESS = REPO_ROOT / "tests" / "m6_acceptance_status_harness.py"
ACCEPTANCE_REPORT = REPO_ROOT / "docs/benchmarks/2026-08-20-m6-acceptance.md"

FRESH_GATES = {
    "foundation": "PASS",
    "debugger_library": "PASS",
    "motion_source": "OPEN",
    "reference_scenes": "PASS",
    "blender": "PASS",
    "mixed_tier": "PASS",
    "extension_examples": "PASS",
    "release_workspace": "PASS",
    "clippy": "PASS",
    "format": "PASS",
    "python": "PASS",
}

MOTION_FILES = (
    "assets/reference/m6/cmu-motion-source-v1.json",
    "assets/reference/m6/motion-thresholds-v1.json",
    "assets/reference/m6/motion-provenance-v1.json",
    "assets/reference/m6/motion-database-input-v1.json",
    "docs/benchmarks/2026-08-18-m6-cmu-motion.json",
)

REPORT_FILES = MOTION_FILES + (
    "assets/reference/m6/acceptance-scenes-v1.json",
    "schemas/m6-acceptance-scenes-v1.schema.json",
    "docs/benchmarks/2026-08-20-m6-acceptance.md",
    "docs/benchmarks/2026-08-20-m6-criterion-5-deferral.md",
    "docs/benchmarks/2026-08-19-m6-acceptance.md",
    "docs/milestones/M9-neural-animation-operator-validation.md",
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
    "CLAUDE.md",
)


def load_checks():
    spec = importlib.util.spec_from_file_location("m6_acceptance_checks", CHECKS_MODULE)
    checks = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(checks)
    return checks


def copy_files(root, paths):
    for relative in paths:
        source = REPO_ROOT / relative
        destination = root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


class MotionSourceStatusTests(unittest.TestCase):
    def test_current_rejected_candidate_is_open_from_evidence(self):
        ruling = load_checks().check_motion_source(REPO_ROOT)
        self.assertEqual(ruling["gate_status"], "OPEN")
        self.assertEqual(ruling["candidate_status"], "rejected")
        self.assertEqual(ruling["joint_limit_violations"], 3_587)
        self.assertEqual(ruling["joint_limit"], 0)

    def test_future_candidate_that_meets_unchanged_hard_limits_can_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            copy_files(root, MOTION_FILES)
            report_path = root / "docs/benchmarks/2026-08-18-m6-cmu-motion.json"
            threshold_path = root / "assets/reference/m6/motion-thresholds-v1.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            thresholds = json.loads(threshold_path.read_text(encoding="utf-8"))
            for clip in report["clip_metrics"]:
                clip["joint_limit_violations"] = 0
            report["quality_metrics"]["joint_limit_violations"] = 0
            report["hard_limit_observations"]["joint_limit_violations"] = 0
            report["hard_limit_evidence"]["joint_limit_violations"]["observed"] = 0
            thresholds["hard_limits"]["joint_limit_violations"]["baseline"] = 0
            write_json(report_path, report)
            write_json(threshold_path, thresholds)

            ruling = load_checks().check_motion_source(root)
            self.assertEqual(ruling["gate_status"], "PASS")
            self.assertEqual(ruling["candidate_status"], "accepted")
            completed = subprocess.run(
                [
                    sys.executable,
                    str(CHECKS_MODULE),
                    "motion-source",
                    "--repo-root",
                    str(root),
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
            self.assertIn("candidate accepted", completed.stdout)
            self.assertNotIn("remains rejected", completed.stdout)

    def test_malformed_candidate_hash_relationship_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            copy_files(root, MOTION_FILES)
            report_path = root / "docs/benchmarks/2026-08-18-m6-cmu-motion.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["source_hashes"].pop("35_01_walk")
            write_json(report_path, report)
            with self.assertRaisesRegex(ValueError, "source hashes"):
                load_checks().check_motion_source(root)

    def test_future_candidate_cannot_pass_by_loosening_unchanged_limits(self):
        mutations = (
            ("hard_limits", "joint_limit_violations", 3_587),
            ("soft_limits", "max_foot_slide_millimeters", 22),
        )
        for section, metric, loosened_limit in mutations:
            with self.subTest(metric=metric), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                copy_files(root, MOTION_FILES)
                threshold_path = root / "assets/reference/m6/motion-thresholds-v1.json"
                thresholds = json.loads(threshold_path.read_text(encoding="utf-8"))
                thresholds[section][metric]["limit"] = loosened_limit
                write_json(threshold_path, thresholds)

                with self.assertRaisesRegex(ValueError, "unchanged"):
                    load_checks().check_motion_source(root)


class AcceptanceStatusHarnessTests(unittest.TestCase):
    def run_harness(self, *arguments):
        return subprocess.run(
            [sys.executable, str(STATUS_HARNESS), *arguments],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_open_motion_gate_is_deferred_to_m9_and_never_reported_as_passing(self):
        completed = self.run_harness("--motion", "OPEN")
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        result = json.loads(completed.stdout)
        self.assertEqual(result["audit_status"], "PASS")
        self.assertEqual(result["criteria"]["5"], "DEFERRED TO M9")
        self.assertNotIn("criterion 5", result["remaining"])

    def test_failed_motion_gate_still_fails_m6_closed(self):
        closed = self.run_harness("--motion", "FAILED")
        acknowledged = self.run_harness("--motion", "FAILED", "--allow-open")
        self.assertEqual(closed.returncode, 2, closed.stdout + closed.stderr)
        self.assertEqual(acknowledged.returncode, 2, acknowledged.stdout + acknowledged.stderr)
        result = json.loads(closed.stdout)
        self.assertEqual(result["audit_status"], "FAILED")
        self.assertEqual(result["criteria"]["5"], "FAILED")

    def test_a_non_deferred_open_gate_still_holds_m6_open(self):
        closed = self.run_harness("--open", "blender")
        acknowledged = self.run_harness("--open", "blender", "--allow-open")
        self.assertEqual(closed.returncode, 2, closed.stdout + closed.stderr)
        self.assertEqual(acknowledged.returncode, 0, acknowledged.stdout + acknowledged.stderr)
        self.assertEqual(json.loads(closed.stdout)["audit_status"], "OPEN")

    def test_criterion_nine_requires_foundation_and_extension_contracts(self):
        extension_failed = self.run_harness("--fail", "extension_examples")
        foundation_failed = self.run_harness("--fail", "foundation")
        self.assertEqual(json.loads(extension_failed.stdout)["criteria"]["9"], "FAILED")
        self.assertEqual(json.loads(foundation_failed.stdout)["criteria"]["9"], "FAILED")
        self.assertEqual(extension_failed.returncode, 2)
        self.assertEqual(foundation_failed.returncode, 2)

    def test_allow_open_never_masks_a_failed_gate(self):
        completed = self.run_harness(
            "--motion",
            "OPEN",
            "--fail",
            "extension_examples",
            "--allow-open",
        )
        self.assertEqual(completed.returncode, 2, completed.stdout + completed.stderr)
        self.assertEqual(json.loads(completed.stdout)["audit_status"], "FAILED")

    def test_public_runner_environment_cannot_bypass_failed_foundation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            scripts = root / "scripts"
            scripts.mkdir()
            for name in (
                "m6-acceptance.sh",
                "m6_acceptance_checks.py",
                "m6_acceptance_status.py",
            ):
                source = REPO_ROOT / "scripts" / name
                if source.exists():
                    shutil.copy2(source, scripts / name)
            for name, exit_code in (
                ("m6-foundation-test.sh", 41),
                ("m6-reference-scenes-test.sh", 0),
                ("m6-blender-test.sh", 0),
                ("m6-performance-test.sh", 0),
                ("m6-extension-examples-test.sh", 0),
            ):
                path = scripts / name
                path.write_text("#!/bin/sh\nexit {}\n".format(exit_code), encoding="utf-8")
                path.chmod(path.stat().st_mode | stat.S_IXUSR)
            report = root / "docs/benchmarks/2026-08-20-m6-acceptance.md"
            report.parent.mkdir(parents=True)
            report.write_text("test report\n", encoding="utf-8")
            fake_bin = root / "bin"
            fake_bin.mkdir()
            cargo = fake_bin / "cargo"
            cargo.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            cargo.chmod(cargo.stat().st_mode | stat.S_IXUSR)
            python = fake_bin / "python3"
            python.write_text(
                "#!/bin/sh\n"
                "case \"$1\" in\n"
                "  */m6_acceptance_status.py) exec \"{}\" \"$@\" ;;\n"
                "  */m6_acceptance_checks.py)\n"
                "    previous=''\n"
                "    for argument in \"$@\"; do\n"
                "      if [ \"$previous\" = '--json-out' ]; then\n"
                "        printf '%s\\n' '{{\"gate_status\":\"OPEN\",\"candidate_status\":\"rejected\",\"candidate_id\":\"fixture\",\"failure_reasons\":[\"joint limits\"],\"baseline_database_id\":\"cc0\",\"baseline_license\":\"CC0-1.0\"}}' > \"$argument\"\n"
                "      fi\n"
                "      previous=\"$argument\"\n"
                "    done\n"
                "    exit 0 ;;\n"
                "  *) exit 0 ;;\n"
                "esac\n".format(sys.executable),
                encoding="utf-8",
            )
            python.chmod(python.stat().st_mode | stat.S_IXUSR)
            env = os.environ.copy()
            env.update(
                {
                    "PATH": str(fake_bin) + os.pathsep + env["PATH"],
                    "M6_ACCEPTANCE_TEST_MODE": "1",
                    "M6_RUN_BLENDER": "1",
                    "M6_ALLOW_OPEN": "1",
                }
            )
            completed = subprocess.run(
                [str(scripts / "m6-acceptance.sh")],
                cwd=root,
                env=env,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(completed.returncode, 2, completed.stdout + completed.stderr)
            self.assertIn("deterministic foundation: FAILED", completed.stdout)
            self.assertIn("M6 acceptance audit: FAILED", completed.stdout)


class AcceptanceReportCheckerTests(unittest.TestCase):
    def copy_report_tree(self, root):
        copy_files(root, REPORT_FILES)

    def check(self, root, gates=None):
        return load_checks().check_acceptance_report(
            root / "docs/benchmarks/2026-08-20-m6-acceptance.md",
            root,
            gates or FRESH_GATES,
        )

    def test_current_report_hashes_evidence_and_executed_gate_labels_validate(self):
        ruling = load_checks().check_acceptance_report(
            ACCEPTANCE_REPORT,
            REPO_ROOT,
            FRESH_GATES,
        )
        self.assertEqual(ruling["milestone_status"], "pass")
        self.assertEqual(ruling["criteria"][5], "DEFERRED TO M9")
        self.assertEqual(len(ruling["sha256"]), 6)

    def test_report_checker_cli_consumes_the_fresh_gate_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            fresh = Path(directory) / "fresh.json"
            write_json(fresh, FRESH_GATES)
            completed = subprocess.run(
                [
                    sys.executable,
                    str(CHECKS_MODULE),
                    "acceptance-report",
                    "--repo-root",
                    str(REPO_ROOT),
                    "--report",
                    str(ACCEPTANCE_REPORT),
                    "--fresh-status",
                    str(fresh),
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
            self.assertIn("milestone status: PASS", completed.stdout)

    def test_changed_hashed_input_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_report_tree(root)
            target = root / "assets/reference/m6/acceptance-scenes-v1.json"
            target.write_bytes(target.read_bytes() + b"\n")
            with self.assertRaisesRegex(ValueError, "SHA-256"):
                self.check(root)

    def test_missing_referenced_evidence_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_report_tree(root)
            (root / "docs/benchmarks/2026-08-18-m6-blender-layers.md").unlink()
            with self.assertRaisesRegex(ValueError, "evidence"):
                self.check(root)

    def test_report_path_must_be_the_canonical_checked_in_report(self):
        with tempfile.TemporaryDirectory() as directory:
            outside = Path(directory) / "substitute-report.md"
            shutil.copy2(ACCEPTANCE_REPORT, outside)
            with self.assertRaisesRegex(ValueError, "canonical"):
                load_checks().check_acceptance_report(outside, REPO_ROOT, FRESH_GATES)

    def test_stale_or_fabricated_expected_gate_status_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_report_tree(root)
            path = root / "docs/benchmarks/2026-08-20-m6-acceptance.md"
            text = path.read_text(encoding="utf-8")
            text = text.replace("PASS (expected current run)", "PASS (recorded)", 1)
            path.write_text(text, encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "expected"):
                self.check(root)

            self.copy_report_tree(root)
            text = path.read_text(encoding="utf-8")
            text = text.replace("| DEFERRED TO M9 |", "| PASS |", 1)
            path.write_text(text, encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "criterion statuses"):
                self.check(root)

    def test_private_path_forms_are_rejected_but_contract_blender_command_is_allowed(self):
        rejected = (
            "~/private",
            "/Volumes/private/evidence",
            "/Applications/Other.app/tool",
            r"C:\Users\person\evidence",
            "$HOME/private",
            "%USERPROFILE%\\private",
        )
        for private_path in rejected:
            with self.subTest(private_path=private_path), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.copy_report_tree(root)
                path = root / "docs/benchmarks/2026-08-20-m6-acceptance.md"
                path.write_text(
                    path.read_text(encoding="utf-8") + "\n" + private_path + "\n",
                    encoding="utf-8",
                )
                with self.assertRaisesRegex(ValueError, "contributor-local"):
                    self.check(root)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_report_tree(root)
            path = root / "docs/benchmarks/2026-08-20-m6-acceptance.md"
            path.write_text(
                path.read_text(encoding="utf-8")
                + "\n```sh\nBLENDER=/Applications/Blender.app/Contents/MacOS/Blender "
                + "scripts/m6-blender-test.sh\n```\n",
                encoding="utf-8",
            )
            self.check(root)


if __name__ == "__main__":
    unittest.main()
