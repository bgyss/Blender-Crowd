import importlib.util
from pathlib import Path
import unittest


MODULE = Path(__file__).parents[1] / "addon" / "blender_crowd" / "m6_extensions.py"
SPEC = importlib.util.spec_from_file_location("m6_extensions", MODULE)
m6_extensions = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(m6_extensions)


def manifest():
    return {
        "schema_version": 1,
        "id": "studio-actions",
        "channels": [
            {
                "name": "look_at",
                "version": 1,
                "inputs": ["attention_target"],
                "outputs": ["gaze_offset"],
                "cost_budget_millionths": 100000,
                "deterministic": True,
                "failure_isolated": True,
            }
        ],
    }


class M6ExtensionsTest(unittest.TestCase):
    def test_python_manifest_has_the_same_declared_channel_contract(self):
        m6_extensions.validate_manifest(manifest())
        m6_extensions.validate_call(manifest(), "look_at", ["attention_target"], 50000)

    def test_python_facade_rejects_undeclared_input_budget_version_and_non_isolation(self):
        with self.assertRaisesRegex(ValueError, "undeclared"):
            m6_extensions.validate_call(manifest(), "look_at", ["secret"], 50000)
        with self.assertRaisesRegex(ValueError, "budget"):
            m6_extensions.validate_call(manifest(), "look_at", ["attention_target"], 200000)
        invalid = manifest()
        invalid["schema_version"] = 2
        with self.assertRaisesRegex(ValueError, "version"):
            m6_extensions.validate_manifest(invalid)
        invalid = manifest()
        invalid["channels"][0]["failure_isolated"] = False
        with self.assertRaisesRegex(ValueError, "isolated"):
            m6_extensions.validate_manifest(invalid)

    def test_failure_isolation_converts_extension_exceptions_to_fallback_result(self):
        result = m6_extensions.run_isolated(
            manifest(),
            "look_at",
            ["attention_target"],
            50000,
            lambda: (_ for _ in ()).throw(RuntimeError("worker failed")),
            fallback={"gaze_offset": [0.0, 0.0, 0.0]},
        )
        self.assertEqual(result["status"], "fallback")
        self.assertEqual(result["reason"], "worker failed")
        self.assertEqual(result["value"], {"gaze_offset": [0.0, 0.0, 0.0]})


if __name__ == "__main__":
    unittest.main()
