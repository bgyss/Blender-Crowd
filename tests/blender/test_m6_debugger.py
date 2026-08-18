"""Blender-process smoke for the M6 trace and graph debugger surface."""

import json
import os
import sys
import tempfile
from pathlib import Path

import addon_utils
import bpy
import blender_crowd_native


EXTENSION = "bl_ext.user_default.blender_crowd"


def all_terminal_templates_library():
    templates = {
        "navigate": {"type": "navigate", "destination_id": "exit_n"},
        "wait": {"type": "wait", "ticks": 1},
        "queue": {"type": "queue", "queue_id": "east_queue"},
        "action": {"type": "action", "action_id": "wave"},
        "follow_lane": {"type": "follow_lane", "lane_id": "east_lane"},
        "hold_position": {"type": "hold_position"},
    }
    return {
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
        "presets": [{
            "id": "all_terminal_templates",
            "subgraph_id": "all_terminal_templates",
            "parameters": {},
        }],
    }


def fail(message):
    print("FAIL: {}".format(message))
    sys.exit(1)


def require(condition, message):
    if not condition:
        fail(message)


def main():
    if os.environ.get("CROWD_SOURCE_ADDON"):
        from addon import blender_crowd

        blender_crowd.register()
        from addon.blender_crowd import behavior_editor, m6_debugger
    else:
        addon_utils.enable(EXTENSION, default_set=True)
        from bl_ext.user_default.blender_crowd import behavior_editor, m6_debugger

    props = bpy.context.scene.crowd_project
    props.selected_agent_id = "7"
    props.m6_debug_tier = "hero"
    trace = {
        "agent_id": 7,
        "tick": 12,
        "event_id": "event-7-12",
        "graph_id": "traceable_brain",
        "graph_node_id": "hold",
        "action_id": "hold_position",
        "motion_clip_id": "idle_ready",
        "contact_id": "right_hand_guard",
        "layer_id": "interaction-pair-7-9",
        "correction_id": "mute-pair-7-9",
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
            {"id": "choose", "type": "utility_selector", "children": ["respond", "travel"]},
            {"id": "respond", "type": "interrupt", "child": "hold"},
            {"id": "hold", "type": "hold_position", "action_id": "hold_position"},
            {"id": "travel", "type": "navigate"},
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
        require(not hasattr(props, "m6_navigation_target_id"), "navigation added a copied-ID field")
        targets = [
            ("node", "hold"),
            ("agent", "7"),
            ("event", "event-7-12"),
            ("action", "hold_position"),
            ("clip", "idle_ready"),
            ("contact", "right_hand_guard"),
            ("layer", "interaction-pair-7-9"),
            ("correction", "mute-pair-7-9"),
        ]
        for target_kind, target_id in targets + list(reversed(targets)):
            props.m6_navigation_target = "{}::{}".format(target_kind, target_id)
            require(
                bpy.ops.crowd.navigate_m6_context() == {"FINISHED"},
                "M6 navigation failed for {}".format(target_kind),
            )
            require(props.selected_agent_id == "7", "navigation lost selected agent")
            require(props.m6_navigation_node == "hold", "navigation lost graph context")
            require(props.m6_navigation_action == "hold_position", "navigation lost action context")
            require(props.m6_navigation_clip == "idle_ready", "navigation lost clip context")
            require(props.m6_navigation_contact == "right_hand_guard", "navigation lost contact context")
            require(props.m6_navigation_layer == "interaction-pair-7-9", "navigation lost layer context")
            require(props.m6_navigation_correction == "mute-pair-7-9", "navigation lost correction context")
        library_path = Path(__file__).resolve().parents[2] / "assets" / "reference" / "m6" / "brain-library-v1.json"
        props.m6_brain_library_path = str(library_path)
        props.m6_brain_preset_id = "guarded_exit"
        props.m6_brain_instance_id = "north"
        props.m6_brain_parameters_json = '{"destination":"exit_n"}'
        require(bpy.ops.crowd.apply_m6_brain_preset() == {"FINISHED"}, "M6 preset operator failed")
        serialized = behavior_editor.graph_from_tree()
        require(serialized["entry_id"] == "north::root", "preset entry did not serialize through the behavior editor")
        require(
            {node["id"] for node in serialized["nodes"]} == {"north::root", "north::leave", "north::hold"},
            "preset graph did not remain bounded and namespaced",
        )
        compiled = blender_crowd_native.compile_behavior_graph(json.dumps(serialized, sort_keys=True))
        require(compiled["node_count"] == 3, "preset graph did not compile through Rust")
        all_templates_path = os.path.join(directory, "all-terminal-templates.json")
        with open(all_templates_path, "w", encoding="utf-8") as handle:
            json.dump(all_terminal_templates_library(), handle)
        props.m6_brain_library_path = all_templates_path
        props.m6_brain_preset_id = "all_terminal_templates"
        props.m6_brain_instance_id = "all"
        props.m6_brain_parameters_json = "{}"
        require(
            bpy.ops.crowd.apply_m6_brain_preset() == {"FINISHED"},
            "all supported M6 action templates did not apply",
        )
        all_serialized = behavior_editor.graph_from_tree()
        all_compiled = blender_crowd_native.compile_behavior_graph(json.dumps(all_serialized, sort_keys=True))
        require(all_compiled["node_count"] == 7, "all supported action templates did not compile through Rust")
    print("M6 debugger Blender smoke: PASS")
    bpy.ops.wm.quit_blender()


try:
    main()
except SystemExit:
    raise
except Exception as error:
    fail("unexpected {}: {}".format(type(error).__name__, error))
