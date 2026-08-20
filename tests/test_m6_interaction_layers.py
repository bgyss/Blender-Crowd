import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


MODULE = Path(__file__).parents[1] / "addon" / "blender_crowd" / "m6_interaction.py"
SPEC = importlib.util.spec_from_file_location("m6_interaction", MODULE)
m6_interaction = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(m6_interaction)

HASH = "a" * 64


class M6InteractionLayersTest(unittest.TestCase):
    def test_new_layer_has_explicit_targets_fallback_and_provenance(self):
        layer = m6_interaction.new_animation_layer(
            "pair-layer",
            "pair-7-9",
            HASH,
            [7, 9],
            10,
            20,
            edits=[
                {"agent_id": 7, "tick": 15, "clip_id": 42, "phase_millionths": 500000},
                {"agent_id": 9, "tick": 15, "clip_id": 43, "phase_millionths": 500000},
            ],
        )
        self.assertEqual(layer["base_cache_hash"], HASH)
        self.assertEqual(layer["target_agent_ids"], [7, 9])
        self.assertEqual(layer["fallback"]["clip_id"], "walk")
        self.assertEqual(layer["provenance"], "authored-paired-clip-v1")

    def test_layer_round_trip_preserves_edits_and_rejects_another_base_cache(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "interaction-pair.json"
            layer = m6_interaction.new_animation_layer(
                "pair-layer", "pair-7-9", HASH, [7, 9], 10, 20,
                edits=[{"agent_id": 7, "tick": 15, "clip_id": 42, "phase_millionths": 500000}],
            )
            m6_interaction.write_layer(path, layer)
            self.assertEqual(m6_interaction.load_layer(path, HASH), layer)
            with self.assertRaisesRegex(ValueError, "another base cache"):
                m6_interaction.load_layer(path, "b" * 64)

    def test_remove_layer_is_explicit_and_does_not_remove_another_layer(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "layers.json"
            first = m6_interaction.new_animation_layer(
                "first", "pair-7-9", HASH, [7, 9], 10, 20,
                edits=[{"agent_id": 7, "tick": 15, "clip_id": 42, "phase_millionths": 0}],
            )
            second = m6_interaction.new_animation_layer(
                "second", "pair-11-12", HASH, [11, 12], 10, 20,
                edits=[{"agent_id": 11, "tick": 15, "clip_id": 44, "phase_millionths": 0}],
            )
            m6_interaction.write_layer_stack(path, [first, second])
            remaining = m6_interaction.remove_layer(path, "first")
            self.assertEqual([item["layer_id"] for item in remaining], ["second"])
            self.assertEqual(json.loads(path.read_text())[0]["layer_id"], "second")

    def test_fallback_layer_is_deterministic_and_targets_the_whole_group(self):
        first = m6_interaction.fallback_layer("pair-layer", "pair-7-9", HASH, [7, 9], 10, 20)
        second = m6_interaction.fallback_layer("pair-layer", "pair-7-9", HASH, [7, 9], 10, 20)
        self.assertEqual(first, second)
        self.assertEqual(first["fallback"]["reason"], "interaction validation or worker failure")
        self.assertEqual(first["target_agent_ids"], [7, 9])


if __name__ == "__main__":
    unittest.main()
