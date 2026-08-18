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


if __name__ == "__main__":
    unittest.main()
