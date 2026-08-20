import importlib.util
import tempfile
from pathlib import Path
import unittest


MODULE = Path(__file__).parents[1] / "addon" / "blender_crowd" / "m6_physics.py"
SPEC = importlib.util.spec_from_file_location("m6_physics", MODULE)
m6_physics = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(m6_physics)


HASH = "a" * 64


class M6PhysicsBoundaryTest(unittest.TestCase):
    def test_transition_layer_declares_solver_cache_recovery_and_failure_policy(self):
        layer = m6_physics.new_transition_layer(
            "hero-recovery-7", HASH, [7], 20, 30, "deterministic-kinematic-reference", "resume-walk", "fallback"
        )
        self.assertEqual(layer["cache_hash"], HASH)
        self.assertEqual(layer["recovery"], "resume-walk")
        self.assertEqual(layer["failure_policy"], "fallback")

    def test_cross_cache_and_hidden_solver_ownership_are_rejected(self):
        layer = m6_physics.new_transition_layer(
            "hero-recovery-7", HASH, [7], 20, 30, "solver", "recover", "fallback"
        )
        with self.assertRaisesRegex(ValueError, "another base cache"):
            m6_physics.validate_transition(layer, "b" * 64)
        layer["solver"] = ""
        with self.assertRaisesRegex(ValueError, "solver"):
            m6_physics.validate_transition(layer, HASH)

    def test_atomic_round_trip_preserves_transition_identity(self):
        layer = m6_physics.new_transition_layer(
            "hero-recovery-7", HASH, [7], 20, 30, "solver", "recover", "fallback"
        )
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "physics.json"
            m6_physics.write_transition(path, layer)
            self.assertEqual(m6_physics.load_transition(path, HASH), layer)


if __name__ == "__main__":
    unittest.main()
