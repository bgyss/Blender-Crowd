import importlib.util
import json
import math
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
        "source_manifest_id": "cmu-test-v1",
        "source_hashes": {"skeleton": "a" * 64, "walk-a": "b" * 64, "walk-b": "c" * 64},
        "clips": [
            {
                "id": "walk-a",
                "samples": [
                    {"tick": 0, "velocity_millimeters_per_second": [1000, 0], "contact": "left_foot", "slope_millionths": 0},
                    {"tick": 1, "velocity_millimeters_per_second": [1200, 0], "contact": "right_foot", "slope_millionths": 10000},
                ],
                "metrics": {
                    "max_root_speed_error_millimeters_per_second": 1,
                    "max_foot_slide_millimeters": 7,
                    "max_trajectory_deviation_millimeters": 3,
                    "max_turn_discontinuity_microradians": 900,
                    "joint_limit_violations": 0,
                    "rejected_frames": 1,
                    "parsed_frames": 9,
                    "rejected_frame_rate_ppm": math.ceil(1_000_000 / 9),
                    "source_hash_drift": 0,
                },
                "evidence": {
                    "retarget_failures": {"status": "not_applicable", "reason": "no retarget"},
                    "root_teleportations": {"status": "not_applicable", "reason": "no runtime transition"},
                    "undeclared_contacts": {"status": "not_applicable", "reason": "no independent contacts"},
                    "cross_cache_mutations": {"status": "not_applicable", "reason": "no cache"},
                },
            },
            {
                "id": "walk-b",
                "samples": [
                    {"tick": 0, "velocity_millimeters_per_second": [800, 0], "contact": "left_foot", "slope_millionths": 0},
                ],
                "metrics": {
                    "max_root_speed_error_millimeters_per_second": 2,
                    "max_foot_slide_millimeters": 5,
                    "max_trajectory_deviation_millimeters": 4,
                    "max_turn_discontinuity_microradians": 800,
                    "joint_limit_violations": 0,
                    "rejected_frames": 0,
                    "parsed_frames": 4,
                    "rejected_frame_rate_ppm": 0,
                    "source_hash_drift": 0,
                },
                "evidence": {
                    "retarget_failures": {"status": "not_applicable", "reason": "no retarget"},
                    "root_teleportations": {"status": "not_applicable", "reason": "no runtime transition"},
                    "undeclared_contacts": {"status": "not_applicable", "reason": "no independent contacts"},
                    "cross_cache_mutations": {"status": "not_applicable", "reason": "no cache"},
                },
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

    def test_evaluation_rejects_numeric_undeclared_contact_evidence(self):
        payload = database()
        for clip in payload["clips"]:
            clip["metrics"]["undeclared_contacts"] = 0
        with self.assertRaisesRegex(ValueError, "undeclared_contacts"):
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

    def test_evaluation_records_exact_hashes_and_observed_threshold_baseline(self):
        report = m6_motion_evaluate.evaluate_database(database())
        self.assertEqual(report["source_hashes"]["walk-a"], "b" * 64)
        self.assertEqual(report["quality_metrics"]["max_foot_slide_millimeters"], 7)
        self.assertEqual(report["quality_metrics"]["max_trajectory_deviation_millimeters"], 4)
        self.assertEqual(report["quality_metrics"]["rejected_frame_rate_ppm"], 111112)
        self.assertEqual(report["hard_limit_observations"]["joint_limit_violations"], 0)
        self.assertEqual(
            report["threshold_baseline"],
            {
                "max_foot_slide_millimeters": 7,
                "max_trajectory_deviation_millimeters": 4,
                "max_turn_discontinuity_microradians": 900,
                "rejected_frame_rate_ppm": 111112,
            },
        )


if __name__ == "__main__":
    unittest.main()
