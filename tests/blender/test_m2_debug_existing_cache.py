"""Verify the installed Blender UI exposes durable M2 cache evidence."""

import json
import os
import sys

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
    cache_path = os.environ["CROWD_M2_DEBUG_CACHE_PATH"]
    addon_utils.enable(EXTENSION, default_set=True)
    from bl_ext.user_default.blender_crowd import debug_overlay

    with open(os.path.join(cache_path, "events", "behavior-v1.json"), encoding="utf-8") as handle:
        event = json.load(handle)["events"][0]
    agent_id = event["agent_id"]

    require(bpy.ops.crowd.attach_cache(filepath=cache_path) == {"FINISHED"}, "cache attach failed")
    scene = bpy.context.scene
    props = scene.crowd_project
    props.selected_agent_id = str(agent_id)
    # The selected agent has evidence at ticks 0 and 3. Inspect at tick 5 to
    # prove the debugger retains the latest causal evidence rather than
    # requiring the user to land on an exact sparse-event frame.
    scene.frame_set(5)
    require(bpy.ops.crowd.inspect_agent() == {"FINISHED"}, "inspect failed")

    evidence = debug_overlay.active_evidence()
    require(evidence and evidence["agent_id"] == agent_id, "wrong selected evidence")
    require(props.selected_agent_graph_id == "leave_concourse", "graph field is blank")
    require(props.selected_agent_decisive_node != "none", "node field is blank")
    require(props.selected_agent_event_count > 0, "event count is blank")
    require(
        any(obj.get("crowd_debug_id") == "selected_agent_marker" for obj in bpy.data.objects),
        "selected agent marker is missing",
    )
    print("M2 cached debug UI: PASS agent={} graph={} node={}".format(
        agent_id, props.selected_agent_graph_id, props.selected_agent_decisive_node
    ))


try:
    main()
except SystemExit:
    raise
except Exception as error:
    fail("unexpected {}: {}".format(type(error).__name__, error))
