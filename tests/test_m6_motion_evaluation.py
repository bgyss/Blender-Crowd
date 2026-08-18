import importlib.util
import json
import tempfile
from pathlib import Path
import unittest


MODULE = Path(__file__).parents[1] / "scripts" / "m6_motion_evaluate.py"
SPEC = importlib.util.spec_from_file_location("m6_motion_evaluate", MODULE)
m6_motion_evaluate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(m6_motion_evaluate)


def database():
    return {
        "schema_version": 1,
        "database_id": "reference",
        "retarget_profile_id": "reference-humanoid",
        "source_provenance": "redistributable-reference-metadata-v1",
        "clips": [
            {
                "id": "walk-a",
                "samples": [
                    {"tick": 0, "velocity_millimeters_per_second": [1000, 0], "contact": "left_foot", "slope_millionths": 0},
                    {"tick": 1, "velocity_millimeters_per_second": [1200, 0], "contact": "right_foot", "slope_millionths": 10000},
                ],
            },
            {
                "id": "walk-b",
                "samples": [
                    {"tick": 0, "velocity_millimeters_per_second": [800, 0], "contact": "left_foot", "slope_millionths": 0},
                ],
            },
        ],
    }


class M6MotionEvaluationTest(unittest.TestCase):
    def test_checked_reference_motion_input_is_evaluable(self):
        source = Path(__file__).parents[1] / "assets" / "reference" / "m6" / "motion-database-input-v1.json"
        report = m6_motion_evaluate.evaluate_database(json.loads(source.read_text()))
        self.assertEqual(report["clip_count"], 2)
        self.assertEqual(report["sample_count"], 4)

    def test_evaluation_is_order_independent_and_records_fit_metrics(self):
        first = m6_motion_evaluate.evaluate_database(database())
        shuffled = database()
        shuffled["clips"].reverse()
        second = m6_motion_evaluate.evaluate_database(shuffled)
        self.assertEqual(first, second)
        self.assertEqual(first["sample_count"], 3)
        self.assertEqual(first["fitted_profile"]["preferred_speed_mps"], 1.0)
        self.assertEqual(first["source_provenance"], "redistributable-reference-metadata-v1")
        self.assertEqual(len(first["source_hash"]), 64)

    def test_evaluation_rejects_unlicensed_or_missing_provenance(self):
        payload = database()
        payload["source_provenance"] = ""
        with self.assertRaisesRegex(ValueError, "provenance"):
            m6_motion_evaluate.evaluate_database(payload)

    def test_cli_writes_a_reviewable_profile_fitting_report(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "database.json"
            output = Path(directory) / "evaluation.json"
            source.write_text(json.dumps(database()))
            self.assertEqual(m6_motion_evaluate.main([str(source), str(output)]), 0)
            report = json.loads(output.read_text())
            self.assertEqual(report["database_id"], "reference")
            self.assertIn("confidence_millionths", report["fitted_profile"])


if __name__ == "__main__":
    unittest.main()
