"""M4 layer-stack artifacts are portable JSON adjacent to, never inside, Cache v1."""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


MODULE = Path(__file__).parents[1] / "addon" / "blender_crowd" / "m4_layout.py"
SPEC = importlib.util.spec_from_file_location("m4_layout", MODULE)
m4_layout = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(m4_layout)


class M4LayoutArtifactsTest(unittest.TestCase):
    def test_layer_stack_round_trip_is_adjacent_and_sorted_json(self):
        with tempfile.TemporaryDirectory() as directory:
            cache_path = Path(directory) / "base-cache"
            path = Path(m4_layout.default_layer_stack_path(cache_path))
            layers = [{"layer_id": "shot", "base_cache_hash": "a" * 64}]
            m4_layout.write_layer_stack(path, layers)
            self.assertEqual(path, cache_path / "layers" / "layout-layers-v1.json")
            self.assertEqual(m4_layout.load_layer_stack(path), layers)
            self.assertEqual(json.loads(path.read_text()), layers)

    def test_layer_stack_rejects_non_array_documents(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "layers.json"
            path.write_text("{}")
            with self.assertRaisesRegex(ValueError, "JSON array"):
                m4_layout.load_layer_stack(path)

    def test_transform_layer_has_provenance_and_immutable_base_identity(self):
        layer = m4_layout.new_transform_layer(
            "fix-seven", "layout", "a" * 64, [1, 2, 3, 4, 5, 6, 7], 10, 20, (1, 2, 3)
        )
        self.assertEqual(layer["base_cache_hash"], "a" * 64)
        self.assertEqual(layer["target"]["agent_ids"], [1, 2, 3, 4, 5, 6, 7])
        self.assertEqual(layer["edits"][0]["type"], "transform")
        self.assertEqual(layer["edits"][0]["samples"][0]["translation"], [1.0, 2.0, 3.0])

    def test_mute_and_solo_are_persisted_without_rewriting_edits(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "layers.json"
            original = m4_layout.new_transform_layer("director-fix", "layout", "a" * 64, [7], 10, 20, (1, 2, 3))
            m4_layout.write_layer_stack(path, [original])
            muted = m4_layout.set_layer_enabled_state(path, 0, "muted", True)
            soloed = m4_layout.set_layer_enabled_state(path, 0, "solo", True)
            self.assertTrue(muted[0]["muted"])
            self.assertTrue(soloed[0]["solo"])
            self.assertEqual(soloed[0]["edits"], original["edits"])
            self.assertTrue(m4_layout.load_layer_stack(path)[0]["muted"])

    def test_layer_state_rejects_unknown_controls_and_bad_rows(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "layers.json"
            m4_layout.write_layer_stack(path, [])
            with self.assertRaisesRegex(ValueError, "muted or solo"):
                m4_layout.set_layer_enabled_state(path, 0, "visible", True)
            with self.assertRaisesRegex(ValueError, "choose an M4 layer row"):
                m4_layout.set_layer_enabled_state(path, 0, "muted", True)


if __name__ == "__main__":
    unittest.main()
