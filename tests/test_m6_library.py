import copy
import importlib.util
import json
from pathlib import Path
import unittest


ROOT = Path(__file__).parents[1]
MODULE = ROOT / "addon" / "blender_crowd" / "m6_library.py"
SPEC = importlib.util.spec_from_file_location("m6_library", MODULE)
m6_library = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(m6_library)


LIBRARY = {
    "schema_version": 1,
    "id": "m6-reference-library",
    "actions": [
        {
            "id": "hold_position",
            "channel": "locomotion",
            "parameters": [],
            "node": {"type": "hold_position"},
        },
        {
            "id": "leave",
            "channel": "navigation",
            "parameters": [{"id": "destination", "type": "stable_id"}],
            "node": {"type": "navigate", "destination_id": "$destination"},
        },
    ],
    "subgraphs": [
        {
            "id": "guarded_exit",
            "entry_id": "root",
            "parameters": [{"id": "destination", "type": "stable_id"}],
            "nodes": [
                {"id": "root", "type": "selector", "children": ["leave", "hold"]},
                {
                    "id": "leave",
                    "type": "navigate",
                    "action_id": "leave",
                    "parameters": {"destination": "$destination"},
                },
                {"id": "hold", "type": "hold_position", "action_id": "hold_position"},
            ],
        }
    ],
    "presets": [
        {
            "id": "guarded_exit",
            "subgraph_id": "guarded_exit",
            "parameters": {"destination": "exit_n"},
        }
    ],
}


class M6LibraryTest(unittest.TestCase):
    def test_action_template_vocabulary_has_only_representable_node_kinds(self):
        self.assertEqual(
            m6_library.ACTION_TEMPLATE_TYPES,
            {"navigate", "wait", "queue", "action", "follow_lane", "hold_position"},
        )
        invalid = copy.deepcopy(LIBRARY)
        invalid["actions"][0]["node"] = {
            "type": "reserve",
            "resource_id": "seat_a",
            "priority": 0,
        }
        with self.assertRaisesRegex(ValueError, "unsupported node template reserve"):
            m6_library.validate_library(invalid)

    def test_every_action_template_lowers_to_a_canonical_terminal_node(self):
        templates = {
            "navigate": {"type": "navigate", "destination_id": "exit_n"},
            "wait": {"type": "wait", "ticks": 1},
            "queue": {"type": "queue", "queue_id": "east_queue"},
            "action": {"type": "action", "action_id": "wave"},
            "follow_lane": {"type": "follow_lane", "lane_id": "east_lane"},
            "hold_position": {"type": "hold_position"},
        }
        library = {
            "schema_version": 1,
            "id": "all-terminal-templates",
            "actions": [
                {"id": action_id, "channel": "test", "parameters": [], "node": node}
                for action_id, node in templates.items()
            ],
            "subgraphs": [{
                "id": "all_terminal_templates",
                "entry_id": "root",
                "parameters": [],
                "nodes": [
                    {"id": "root", "type": "selector", "children": list(templates)},
                    *[
                        {"id": action_id, "type": "action", "action_id": action_id}
                        for action_id in templates
                    ],
                ],
            }],
            "presets": [{"id": "all_terminal_templates", "subgraph_id": "all_terminal_templates", "parameters": {}}],
        }
        graph = m6_library.instantiate_preset(
            library, "all_terminal_templates", "all", {}
        )
        nodes = {node["id"]: node for node in graph["nodes"]}
        self.assertEqual(nodes["all::root"]["children"], ["all::{}".format(name) for name in templates])
        for action_id, template in templates.items():
            self.assertEqual(nodes["all::{}".format(action_id)], {"id": "all::{}".format(action_id), **template})

    def test_preset_instantiation_is_namespaced_and_deterministic(self):
        first = m6_library.instantiate_preset(
            LIBRARY, "guarded_exit", "north", {"destination": "exit_n"}
        )
        second = m6_library.instantiate_preset(
            LIBRARY, "guarded_exit", "north", {"destination": "exit_n"}
        )
        self.assertEqual(first, second)
        self.assertEqual(first["entry_id"], "north::root")
        self.assertEqual(
            {node["id"] for node in first["nodes"]},
            {"north::root", "north::leave", "north::hold"},
        )
        self.assertEqual([node["id"] for node in first["nodes"]], ["north::hold", "north::leave", "north::root"])
        self.assertEqual([action["id"] for action in first["actions"]], ["north::hold_position", "north::leave"])
        nodes = {node["id"]: node for node in first["nodes"]}
        self.assertEqual(nodes["north::root"], {
            "id": "north::root",
            "type": "selector",
            "children": ["north::leave", "north::hold"],
        })
        self.assertEqual(nodes["north::hold"], {"id": "north::hold", "type": "hold_position"})
        self.assertEqual(nodes["north::leave"], {
            "id": "north::leave",
            "type": "navigate",
            "destination_id": "exit_n",
        })

    def test_library_rejects_unbounded_or_undeclared_data(self):
        cases = [
            ("duplicate action", lambda value: value["actions"].append(copy.deepcopy(value["actions"][0]))),
            ("missing reference", lambda value: value["subgraphs"][0]["nodes"][0].update(children=["missing"])),
            ("unknown parameter", lambda value: value["presets"][0]["parameters"].update(extra="nope")),
            ("unsupported node", lambda value: value["subgraphs"][0]["nodes"][0].update(type="python")),
            ("source-code", lambda value: value["subgraphs"][0]["nodes"][0].update(source_code="print('no')")),
            ("runtime callback", lambda value: value["actions"][0].update(callback="run")),
        ]
        for label, mutate in cases:
            with self.subTest(label=label):
                invalid = copy.deepcopy(LIBRARY)
                mutate(invalid)
                with self.assertRaises(ValueError):
                    m6_library.validate_library(invalid)

    def test_instantiation_rejects_unknown_parameters_and_namespace_collisions(self):
        with self.assertRaisesRegex(ValueError, "unknown parameter"):
            m6_library.instantiate_preset(LIBRARY, "guarded_exit", "north", {"extra": "nope"})
        with self.assertRaisesRegex(ValueError, "instance ID"):
            m6_library.instantiate_preset(LIBRARY, "guarded_exit", "", {"destination": "exit_n"})
        with self.assertRaisesRegex(ValueError, "namespace collision"):
            m6_library.instantiate_preset(
                LIBRARY, "guarded_exit", "north::hold", {"destination": "exit_n"}
            )

    def test_checked_reference_fixture_validates(self):
        value = json.loads(
            (ROOT / "assets" / "reference" / "m6" / "brain-library-v1.json").read_text(
                encoding="utf-8"
            )
        )
        m6_library.validate_library(value)


if __name__ == "__main__":
    unittest.main()
