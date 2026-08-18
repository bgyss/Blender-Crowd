import importlib.util
import json
import tempfile
from pathlib import Path
import unittest


MODULE = Path(__file__).parents[1] / "scripts" / "m6_motion_build.py"
SPEC = importlib.util.spec_from_file_location("m6_motion_build", MODULE)
m6_motion_build = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(m6_motion_build)


class M6MotionDatabaseTest(unittest.TestCase):
    def test_build_is_content_addressed_and_stable_for_sorted_clip_input(self):
        database = {
            "schema_version": 1,
            "database_id": "test",
            "retarget_profile_id": "reference-humanoid",
            "source_provenance": "redistributable-reference-metadata-v1",
            "clips": [
                {"id": "walk-b", "samples": [{"tick": 0, "velocity": [1, 0]}]},
                {"id": "walk-a", "samples": [{"tick": 0, "velocity": [1, 0]}]},
            ],
        }
        first = m6_motion_build.build_database(database)
        database["clips"].reverse()
        second = m6_motion_build.build_database(database)
        self.assertEqual(first, second)
        self.assertEqual(first["clip_ids"], ["walk-a", "walk-b"])
        self.assertEqual(len(first["content_hash"]), 64)

    def test_build_rejects_missing_provenance_and_duplicate_clip_ids(self):
        database = {"schema_version": 1, "database_id": "test", "clips": []}
        with self.assertRaisesRegex(ValueError, "provenance"):
            m6_motion_build.build_database(database)
        database.update(
            {
                "retarget_profile_id": "reference-humanoid",
                "source_provenance": "reference",
                "clips": [{"id": "walk"}, {"id": "walk"}],
            }
        )
        with self.assertRaisesRegex(ValueError, "duplicate"):
            m6_motion_build.build_database(database)

    def test_cli_writes_a_report_without_rewriting_the_input(self):
        database = {
            "schema_version": 1,
            "database_id": "test",
            "retarget_profile_id": "reference-humanoid",
            "source_provenance": "reference",
            "clips": [{"id": "walk", "samples": []}],
        }
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source.json"
            output = Path(directory) / "report.json"
            source.write_text(json.dumps(database))
            self.assertEqual(m6_motion_build.main([str(source), str(output)]), 0)
            self.assertEqual(json.loads(source.read_text()), database)
            self.assertEqual(json.loads(output.read_text())["database_id"], "test")

    def test_build_preserves_source_hashes_and_motion_quality_evidence(self):
        database = {
            "schema_version": 1,
            "database_id": "cmu-test",
            "retarget_profile_id": "cmu-acclaim-humanoid-v1",
            "source_provenance": "CMU-Mocap-Free-All-Uses",
            "source_manifest_id": "cmu-test-v1",
            "source_hashes": {"motion": "a" * 64},
            "clips": [
                {
                    "id": "walk",
                    "samples": [{"tick": 0, "velocity_millimeters_per_second": [0, 0]}],
                    "metrics": {
                        "max_root_speed_error_millimeters_per_second": 1,
                        "max_foot_slide_millimeters": 2,
                        "max_trajectory_deviation_millimeters": 3,
                        "max_turn_discontinuity_microradians": 4,
                        "joint_limit_violations": 0,
                        "retarget_failures": 0,
                        "rejected_frames": 1,
                        "parsed_frames": 10,
                        "rejected_frame_rate_ppm": 100000,
                        "root_teleportations": 0,
                        "undeclared_contacts": 0,
                        "source_hash_drift": 0,
                        "cross_cache_mutations": 0,
                    },
                }
            ],
        }
        report = m6_motion_build.build_database(database)
        self.assertEqual(report["source_hashes"], {"motion": "a" * 64})
        self.assertEqual(report["clips"][0]["metrics"]["max_trajectory_deviation_millimeters"], 3)
        self.assertEqual(report["quality_metrics"]["max_foot_slide_millimeters"], 2)


if __name__ == "__main__":
    unittest.main()
