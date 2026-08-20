"""Behavior tests for Blender-facing M6 interaction and hero layer artifacts."""

import copy
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).parents[1]


def load_module(name):
    spec = importlib.util.spec_from_file_location(
        name, ROOT / "addon" / "blender_crowd" / "{}.py".format(name)
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


m6_physics = load_module("m6_physics")
sys.modules["m6_physics"] = m6_physics
m6_interaction = load_module("m6_interaction")


BASE_HASH = "b2c74ec5a6038dc1761afdcb727f756b092ad64113aeeed3a9c5e14611c138d7"


class M6LayerBundleTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="blender-crowd-m6-layers-")
        self.root = Path(self.directory.name)
        self.layer = m6_interaction.new_animation_layer(
            "pair-7-9",
            "request-7-9",
            BASE_HASH,
            [9, 7],
            10,
            20,
            edits=[
                {"agent_id": 7, "tick": 15, "clip_id": 42, "phase_millionths": 500_000},
                {"agent_id": 9, "tick": 15, "clip_id": 43, "phase_millionths": 500_000},
            ],
        )
        self.motion = {
            "schema_version": 1,
            "request_id": "request-7-9",
            "participants": [
                {
                    "agent_id": 7,
                    "root_samples": [
                        {"tick": 10, "translation": [0.0, 0.0, 0.0], "yaw": 0.0},
                        {"tick": 15, "translation": [0.25, 0.0, 0.0], "yaw": 0.0},
                        {"tick": 20, "translation": [0.5, 0.0, 0.0], "yaw": 0.0},
                    ],
                    "skeletal_channels": [],
                },
                {
                    "agent_id": 9,
                    "root_samples": [
                        {"tick": 10, "translation": [1.0, 0.0, 0.0], "yaw": 3.141592653589793},
                        {"tick": 15, "translation": [0.75, 0.0, 0.0], "yaw": 3.141592653589793},
                        {"tick": 20, "translation": [0.5, 0.0, 0.0], "yaw": 3.141592653589793},
                    ],
                    "skeletal_channels": [],
                },
            ],
            "contacts": [{
                "contact_id": "touch-7-9",
                "label": "touch",
                "owner_agent_id": 7,
                "other_agent_id": 9,
                "tick": 15,
                "distance_m": 0.0,
            }],
            "provenance": {
                "backend": "authored-paired-clip",
                "model_hash": None,
                "seed": 2026,
                "config_hash": "reference-v1",
            },
            "diagnostics": [],
            "fallback": {
                "clip_set_id": "pedestrian_basic",
                "clip_id": "walk",
                "reason": "deterministic baseline",
            },
        }
        self.transition = m6_physics.new_transition_layer(
            "hero-recovery-7",
            BASE_HASH,
            [7],
            20,
            30,
            "deterministic-kinematic-reference",
            "resume-walk",
            "fallback",
        )
        self.hero = {
            "integration_id": "hero-cloth-7",
            "solver": "blender-cloth",
            "cache_policy": "adjacent-layer",
            "supported_render_tiers": ["hero"],
            "failure_policy": "fallback-to-cached-body",
        }

    def tearDown(self):
        self.directory.cleanup()

    def _write(self, name, value):
        path = self.root / name
        path.write_text(json.dumps(value), encoding="utf-8")
        return path

    def test_bundle_exposes_ownership_contacts_provenance_and_hero_boundaries(self):
        bundle = m6_interaction.load_layer_bundle(
            self._write("interaction.json", self.layer),
            self._write("motion.json", self.motion),
            self._write("physics.json", self.transition),
            self._write("hero.json", self.hero),
            BASE_HASH,
        )

        self.assertEqual(bundle["base_cache_hash"], BASE_HASH)
        self.assertEqual(bundle["owner_agent_ids"], [7, 9])
        self.assertEqual(bundle["interaction_interval"], [10, 20])
        self.assertEqual(bundle["physics_interval"], [20, 30])
        self.assertEqual(bundle["contacts"][0]["contact_id"], "touch-7-9")
        self.assertEqual(bundle["interaction_provenance"], "authored-paired-clip-v1")
        self.assertEqual(bundle["motion_provenance"]["backend"], "authored-paired-clip")
        self.assertEqual(bundle["physics_solver"], "deterministic-kinematic-reference")
        self.assertEqual(bundle["recovery"], "resume-walk")
        self.assertEqual(bundle["physics_failure_policy"], "fallback")
        self.assertEqual(bundle["hero_boundary"]["supported_render_tiers"], ["hero"])
        self.assertEqual(bundle["hero_execution_status"], "declaration_only_unsupported")
        self.assertEqual(
            bundle["physics_binding"],
            {
                "base_cache_hash": BASE_HASH,
                "target_agent_ids": [7],
                "tick_start": 20,
                "tick_end": 30,
            },
        )
        self.assertEqual(
            bundle["hero_binding"],
            {
                "base_cache_hash": BASE_HASH,
                "target_agent_ids": [7],
                "tick_start": 20,
                "tick_end": 30,
                "execution_status": "declaration_only_unsupported",
                "attachment_status": "not_attached",
            },
        )
        self.assertEqual(bundle["interaction_motion"], self.motion)

    def test_bundle_rejects_cross_cache_and_cross_interaction_artifacts(self):
        wrong_cache = copy.deepcopy(self.transition)
        wrong_cache["cache_hash"] = "a" * 64
        with self.assertRaisesRegex(ValueError, "another base cache"):
            m6_interaction.load_layer_bundle(
                self._write("interaction.json", self.layer),
                self._write("motion.json", self.motion),
                self._write("physics.json", wrong_cache),
                self._write("hero.json", self.hero),
                BASE_HASH,
            )

        wrong_motion = copy.deepcopy(self.motion)
        wrong_motion["request_id"] = "another-request"
        with self.assertRaisesRegex(ValueError, "does not match interaction"):
            m6_interaction.load_layer_bundle(
                self._write("interaction.json", self.layer),
                self._write("motion.json", wrong_motion),
                self._write("physics.json", self.transition),
                self._write("hero.json", self.hero),
                BASE_HASH,
            )

        malformed_motion = copy.deepcopy(self.motion)
        malformed_motion["participants"] = [None, {"agent_id": 9}]
        with self.assertRaisesRegex(ValueError, "participant"):
            m6_interaction.load_layer_bundle(
                self._write("interaction.json", self.layer),
                self._write("motion.json", malformed_motion),
                self._write("physics.json", self.transition),
                self._write("hero.json", self.hero),
                BASE_HASH,
            )

    def test_layout_conversion_is_sparse_mutable_and_keeps_source_artifacts_unchanged(self):
        bundle = m6_interaction.load_layer_bundle(
            self._write("interaction.json", self.layer),
            self._write("motion.json", self.motion),
            self._write("physics.json", self.transition),
            self._write("hero.json", self.hero),
            BASE_HASH,
        )
        original = copy.deepcopy(bundle)
        physics_samples = [
            {"tick": tick, "position": [float(tick), 0.0, 0.0], "velocity": [1.0, 0.0, 0.0]}
            for tick in range(20, 31)
        ]

        layers = m6_interaction.build_layout_layers(bundle, physics_samples, muted=False)
        self.assertEqual(len(layers), 3)
        self.assertEqual([layer["target"]["agent_ids"] for layer in layers[:2]], [[7], [9]])
        self.assertEqual([layer["target"]["tick_start"] for layer in layers[:2]], [15, 15])
        self.assertEqual([layer["edits"][0]["clip_id"] for layer in layers[:2]], [42, 43])
        self.assertEqual(layers[2]["kind"], "physics")
        self.assertEqual(layers[2]["target"]["agent_ids"], [7])
        self.assertEqual(
            [sample["tick"] for sample in layers[2]["edits"][0]["cached_samples"]],
            list(range(20, 31)),
        )
        self.assertTrue(all(not layer["muted"] for layer in layers))

        with self.assertRaisesRegex(ValueError, "complete 20..30 interval"):
            m6_interaction.build_layout_layers(bundle, physics_samples[:-1], muted=False)

        muted = m6_interaction.build_layout_layers(bundle, physics_samples, muted=True)
        self.assertTrue(all(layer["muted"] for layer in muted))
        self.assertEqual(bundle, original, "mute conversion mutated the authoritative artifacts")


if __name__ == "__main__":
    unittest.main()
