"""Contract tests for the fixed M3 release-budget gate."""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("m3_budget_audit", ROOT / "scripts" / "m3_budget_audit.py")
BUDGET = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUDGET)


class M3BudgetAuditTests(unittest.TestCase):
    def fixture(self, root, bake_seconds=60.0):
        archive = root / "release.zip"
        archive.write_bytes(b"release")
        reference = root / "reference"
        (reference / "cache").mkdir(parents=True)
        (reference / "cache" / "chunk").write_bytes(b"cache")
        (reference / "render").mkdir()
        (reference / "m2-full-acceptance.json").write_text(
            json.dumps(
                {
                    "authorable_bake_seconds": bake_seconds,
                    "debug_inspection_seconds_per_query": 0.01,
                    "sequential_cache_ticks_per_second": 500.0,
                }
            )
        )
        (reference / "render" / "m1-render-metrics.json").write_text(
            json.dumps(
                {
                    "peak_resident_bytes": 1000,
                    "point_upload_seconds": 0.01,
                    "armature_evaluation_seconds": 0.01,
                    "renders": {"eevee": {"seconds": 0.1}, "cycles": {"seconds": 0.1}},
                }
            )
        )
        return archive, reference

    def test_accepts_measurements_inside_every_fixed_budget(self):
        with tempfile.TemporaryDirectory() as directory:
            archive, reference = self.fixture(Path(directory))
            self.assertTrue(BUDGET.audit(archive, reference)["passed"])

    def test_rejects_any_measurement_over_a_fixed_budget(self):
        with tempfile.TemporaryDirectory() as directory:
            archive, reference = self.fixture(Path(directory), bake_seconds=121.0)
            report = BUDGET.audit(archive, reference)
            self.assertFalse(report["passed"])
            failed = [item["metric"] for item in report["checks"] if not item["passed"]]
            self.assertEqual(failed, ["authorable_bake_seconds"])


if __name__ == "__main__":
    unittest.main()
