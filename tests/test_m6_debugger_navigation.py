import importlib.util
from pathlib import Path
import unittest


MODULE = Path(__file__).parents[1] / "addon" / "blender_crowd" / "m6_debugger.py"
SPEC = importlib.util.spec_from_file_location("m6_debugger", MODULE)
m6_debugger = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(m6_debugger)


TRACE = {
    "agent_id": 7,
    "tick": 12,
    "event_id": "event-7-12",
    "graph_node_id": "hold",
    "action_id": "hold_position",
    "motion_clip_id": "idle_ready",
    "contact_id": "right_hand_guard",
    "layer_id": "interaction-pair-7-9",
    "correction_id": "mute-pair-7-9",
}

GRAPH = {
    "nodes": [
        {"id": "root", "type": "utility_selector", "children": ["hold"]},
        {"id": "hold", "type": "hold_position", "action_id": "hold_position"},
    ]
}


class M6DebuggerNavigationTest(unittest.TestCase):
    def test_navigation_resolves_every_context_without_copied_ids(self):
        index = m6_debugger.build_navigation_index(TRACE, GRAPH)
        node = m6_debugger.resolve_navigation(index, "node", "hold")
        self.assertEqual(
            node,
            {
                "target_kind": "node",
                "target_id": "hold",
                "agent_id": 7,
                "tick": 12,
                "graph_node_id": "hold",
                "action_id": "hold_position",
                "motion_clip_id": "idle_ready",
                "contact_id": "right_hand_guard",
                "layer_id": "interaction-pair-7-9",
                "correction_id": "mute-pair-7-9",
            },
        )
        self.assertEqual(
            {(record["target_kind"], record["target_id"]) for record in index},
            {
                ("agent", "7"),
                ("event", "event-7-12"),
                ("node", "hold"),
                ("action", "hold_position"),
                ("clip", "idle_ready"),
                ("contact", "right_hand_guard"),
                ("layer", "interaction-pair-7-9"),
                ("correction", "mute-pair-7-9"),
            },
        )
        for target_kind, target_id in index_keys(index):
            record = m6_debugger.resolve_navigation(index, target_kind, target_id)
            self.assertEqual(record["agent_id"], 7)
            self.assertEqual(record["tick"], 12)
            self.assertEqual(record["graph_node_id"], "hold")

    def test_navigation_rejects_unknown_target(self):
        index = m6_debugger.build_navigation_index(TRACE, GRAPH)
        with self.assertRaisesRegex(ValueError, "unknown M6 navigation target"):
            m6_debugger.resolve_navigation(index, "node", "missing")


def index_keys(index):
    return [(record["target_kind"], record["target_id"]) for record in index]


if __name__ == "__main__":
    unittest.main()
