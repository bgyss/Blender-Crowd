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
    "graph_id": "traceable_brain",
    "decisive_node": "hold",
    "visited_nodes": ["choose", "respond", "hold"],
    "observations": ["Hearing", "GroupExtent"],
    "utility_scores": [["respond", 900000], ["travel", 100000]],
    "blackboard_changes": [{"key": "threat_visible", "value": True}],
    "interrupts": ["respond"],
    "group_context": {"group_id": "pair-7-9", "role": "responder"},
    "contact_diagnostics": ["touch contact accepted"],
    "layer_ownership": "interaction-pair-7-9",
    "degraded_evidence": None,
}


class M6DebuggerTest(unittest.TestCase):
    def test_summary_keeps_trace_to_node_and_cross_agent_context_readable(self):
        summary = m6_debugger.build_trace_summary(TRACE, selected_agent_id=7, tier="hero")
        self.assertEqual(summary["agent_id"], 7)
        self.assertEqual(summary["decisive_node"], "hold")
        self.assertEqual(summary["utility_scores"][0], {"option": "respond", "score": 900000})
        self.assertEqual(summary["group_context"]["group_id"], "pair-7-9")
        self.assertEqual(summary["layer_ownership"], "interaction-pair-7-9")
        self.assertEqual(summary["degraded_evidence"], "full evidence")

    def test_lower_tier_summary_states_which_evidence_is_unavailable(self):
        trace = dict(TRACE)
        trace["degraded_evidence"] = "observation budget reduced M6 evidence for this tier"
        summary = m6_debugger.build_trace_summary(trace, selected_agent_id=7, tier="distant")
        self.assertIn("reduced", summary["degraded_evidence"])
        self.assertIn("utility scores", " ".join(summary["unavailable_evidence"]))

    def test_graph_search_returns_stable_node_matches_and_highlight_path(self):
        graph = {
            "nodes": [
                {"id": "choose", "kind": "utility", "children": ["respond", "travel"]},
                {"id": "respond", "kind": "interrupt", "child": "hold"},
                {"id": "hold", "kind": "action"},
                {"id": "travel", "kind": "action"},
            ],
        }
        result = m6_debugger.search_graph(graph, "resp")
        self.assertEqual(result["matches"], ["respond"])
        self.assertEqual(result["highlight_path"], ["choose", "respond", "hold"])


if __name__ == "__main__":
    unittest.main()
