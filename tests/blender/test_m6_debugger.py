"""Blender-process smoke for the M6 trace and graph debugger surface."""

import json
import os
import sys
import tempfile

import addon_utils
import bpy


EXTENSION = "bl_ext.user_default.blender_crowd"


def fail(message):
    print("FAIL: {}".format(message))
    sys.exit(1)


def require(condition, message):
    if not condition:
        fail(message)


def main():
    addon_utils.enable(EXTENSION, default_set=True)
    from bl_ext.user_default.blender_crowd import m6_debugger

    props = bpy.context.scene.crowd_project
    props.selected_agent_id = "7"
    props.m6_debug_tier = "hero"
    trace = {
        "agent_id": 7,
        "tick": 12,
        "graph_id": "traceable_brain",
        "decisive_node": "hold",
        "visited_nodes": ["choose", "respond", "hold"],
        "observations": ["Hearing"],
        "utility_scores": [["respond", 900000], ["travel", 100000]],
        "blackboard_values": [["threat_visible", "AgentId(9)"]],
        "interrupts": ["respond"],
        "group_context": {"group_id": "pair-7-9", "role": "responder"},
        "contact_diagnostics": ["touch accepted"],
        "layer_ownership": "interaction-pair-7-9",
        "degraded_evidence": None,
    }
    graph = {
        "nodes": [
            {"id": "choose", "kind": "utility", "children": ["respond", "travel"]},
            {"id": "respond", "kind": "interrupt", "child": "hold"},
            {"id": "hold", "kind": "action"},
            {"id": "travel", "kind": "action"},
        ]
    }
    with tempfile.TemporaryDirectory(prefix="blender-crowd-m6-") as directory:
        trace_path = os.path.join(directory, "trace.json")
        graph_path = os.path.join(directory, "graph.json")
        with open(trace_path, "w", encoding="utf-8") as handle:
            json.dump(trace, handle)
        with open(graph_path, "w", encoding="utf-8") as handle:
            json.dump(graph, handle)
        props.m6_trace_path = trace_path
        require(bpy.ops.crowd.inspect_m6_trace() == {"FINISHED"}, "M6 trace operator failed")
        require("decisive node hold" in props.m6_trace_summary, "trace summary lost decisive node")
        props.m6_graph_path = graph_path
        props.m6_graph_search = "resp"
        require(bpy.ops.crowd.search_m6_graph() == {"FINISHED"}, "M6 graph search failed")
        require(props.m6_graph_matches == "respond", "graph search lost matching node")
        require(props.m6_graph_highlight_path == "choose → respond → hold", "graph path was not highlighted")
        summary = m6_debugger.build_trace_summary(trace, 7, "hero")
        require(summary["degraded_evidence"] == "full evidence", "hero evidence was incorrectly degraded")
    print("M6 debugger Blender smoke: PASS")
    bpy.ops.wm.quit_blender()


try:
    main()
except SystemExit:
    raise
except Exception as error:
    fail("unexpected {}: {}".format(type(error).__name__, error))
