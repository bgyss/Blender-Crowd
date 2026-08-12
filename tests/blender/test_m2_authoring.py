"""Automated M2 editor persistence, undo, and native-validation coverage."""

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
    from bl_ext.user_default.blender_crowd import (
        behavior_editor,
        debug_overlay,
        layout_editor,
        operators,
        overrides,
        project,
    )

    scene = bpy.context.scene
    require(
        bpy.ops.crowd.create_reference_project() == {"FINISHED"},
        "reference authoring project was not created",
    )
    props = scene.crowd_project
    require(len(props.queues) == 1, "reference queue editor did not materialize")
    require(len(props.lanes) == 1, "reference lane editor did not materialize")
    require(
        len(props.cost_regions) == 1,
        "reference cost-region editor did not materialize",
    )
    require(
        len(props.retarget_profiles) == 1 and len(props.clips) == 2 and len(props.variations) == 1,
        "reference asset editor did not materialize the typed asset library",
    )
    require(len(props.layouts) == 1, "reference seating layout did not materialize")
    require(len(props.groups) == 1, "reference social group did not materialize")
    require(
        json.loads(props.groups[0].member_agent_ids_json),
        "reference social group has no stable agent members",
    )
    selected_agent_id = json.loads(props.groups[0].member_agent_ids_json)[0]
    props.selected_agent_id = str(selected_agent_id)
    require(
        overrides.selected_agent_id(props) == selected_agent_id,
        "decimal selected-agent field did not populate the stable agent ID",
    )
    debug_overlay.record_evidence(
        props,
        {
            "tick": 42,
            "behavior_state": 3,
            "decision_reason": 9,
            "graph_id": "leave_concourse",
            "decisive_node": "queue_exit",
            "behavior_events": [{"kind": "decision"}, {"kind": "queue_requested"}],
        },
    )
    require(props.selected_agent_tick == 42, "debug panel did not retain the inspected tick")
    require(
        props.selected_agent_graph_id == "leave_concourse",
        "debug panel did not retain graph evidence",
    )
    require(
        props.selected_agent_decisive_node == "queue_exit",
        "debug panel did not retain decisive-node evidence",
    )
    require(props.selected_agent_event_count == 2, "debug panel did not retain event count")
    graph = behavior_editor.graph_from_tree()
    require(graph["id"] == "leave_concourse", "behavior node tree did not materialize")
    require(
        any(node["type"] == "queue" for node in graph["nodes"]),
        "behavior node tree lost the queue action",
    )

    props.queues[0].admission_capacity = 2
    props.populations[0].emission_interval_ticks = 3
    props.clips[0].average_root_speed_mmps = 1400
    require(
        bpy.ops.crowd.validate_authorable_project() == {"FINISHED"},
        "edited authorable project did not validate",
    )
    semantics = project.extract_authoring_semantics(scene)
    require(
        semantics["queues"][0]["admission_capacity"] == 2,
        "queue editor change was not compiled into M2 semantics",
    )
    require(
        project.extract_ir(scene)["populations"][0]["emission_interval_ticks"] == 3,
        "population editor change was not compiled into base IR",
    )
    require(
        project.extract_authorable_assets(scene)["clips"][0]["average_root_speed_mmps"] == 1400,
        "clip editor change was not compiled into the M2 asset library",
    )
    require(
        project.extract_authorable_groups(scene)[0]["bottleneck_policy"] == "leader_first",
        "group editor change was not compiled into the M2 project",
    )

    empty_cache_path = os.path.join(tempfile.mkdtemp(prefix="blender-crowd-m2-cache-"), "cache")
    os.mkdir(empty_cache_path)
    props.cache_path = empty_cache_path
    bake_start = bpy.ops.crowd.bake_cache()
    require(
        bake_start in ({"FINISHED"}, {"RUNNING_MODAL"}),
        "an empty cache directory was rejected: {} ({})".format(bake_start, props.status),
    )
    require(
        bpy.ops.crowd.cancel_bake() == {"FINISHED"},
        "empty-directory bake could not be canceled",
    )
    outcome = operators.wait_for_bake(timeout=60.0)
    require(outcome and outcome["status"] == "canceled", "empty-directory bake did not cancel safely")

    require(
        bpy.ops.ed.undo_push(message="Initialize M2 headless undo") == {"FINISHED"},
        "headless undo system did not initialize",
    )
    before = len(props.queues)
    add = bpy.ops.crowd.add_m2_semantic(entity_type="queue")
    require(add == {"FINISHED"}, "queue add operator failed")
    require(len(props.queues) == before + 1, "queue add did not mutate scene")
    require(
        bpy.ops.ed.undo_push(message="Add M2 queue") == {"FINISHED"},
        "queue add undo checkpoint failed",
    )
    require(bpy.ops.ed.undo() == {"FINISHED"}, "queue add is not undoable")
    scene = bpy.context.scene
    props = scene.crowd_project
    require(len(props.queues) == before, "undo did not revert queue add")

    population_before = len(props.populations)
    add_population = bpy.ops.crowd.add_population()
    require(add_population == {"FINISHED"}, "population add operator failed")
    require(
        len(props.populations) == population_before + 1,
        "population add did not mutate scene",
    )
    require(
        props.populations[len(props.populations) - 1].logical_id == "new_population_2",
        "population add did not assign a stable editable ID",
    )
    require(
        bpy.ops.ed.undo_push(message="Add M2 population") == {"FINISHED"},
        "population add undo checkpoint failed",
    )
    require(bpy.ops.ed.undo() == {"FINISHED"}, "population add is not undoable")
    scene = bpy.context.scene
    props = scene.crowd_project
    require(
        len(props.populations) == population_before,
        "undo did not revert population add",
    )

    clips_before = len(props.clips)
    require(
        bpy.ops.crowd.add_m2_asset(entity_type="clip") == {"FINISHED"},
        "clip add operator failed",
    )
    require(len(props.clips) == clips_before + 1, "clip add did not mutate scene")
    require(
        bpy.ops.ed.undo_push(message="Add M2 clip") == {"FINISHED"},
        "clip add undo checkpoint failed",
    )
    require(bpy.ops.ed.undo() == {"FINISHED"}, "clip add is not undoable")
    scene = bpy.context.scene
    props = scene.crowd_project
    require(len(props.clips) == clips_before, "undo did not revert clip add")

    layouts_before = len(props.layouts)
    require(
        bpy.ops.crowd.add_layout(entity_type="seating") == {"FINISHED"},
        "seating layout add operator failed",
    )
    require(len(props.layouts) == layouts_before + 1, "layout add did not mutate scene")
    require(
        bpy.ops.crowd.materialize_layout_guides() == {"FINISHED"},
        "layout guide materialization failed",
    )
    require(
        len(bpy.data.collections[layout_editor.LAYOUT_COLLECTION_NAME].objects) == 100,
        "seating layout did not materialize all guide positions",
    )

    blend_path = os.path.join(tempfile.mkdtemp(prefix="blender-crowd-m2-"), "authoring.blend")
    bpy.ops.wm.save_as_mainfile(filepath=blend_path, check_existing=False)
    bpy.ops.wm.open_mainfile(filepath=blend_path)
    scene = bpy.context.scene
    props = scene.crowd_project
    require(props.queues[0].admission_capacity == 2, "queue edit was lost after reload")
    require(
        props.populations[0].emission_interval_ticks == 3,
        "population edit was lost after reload",
    )
    require(
        props.clips[0].average_root_speed_mmps == 1400,
        "clip edit was lost after reload",
    )
    require(len(props.lanes) == 1, "lane editor data was lost after reload")
    require(len(props.cost_regions) == 1, "region editor data was lost after reload")
    require(len(props.layouts) == 2, "layout editor data was lost after reload")
    require(len(props.groups) == 1, "group editor data was lost after reload")
    require(
        len(bpy.data.collections[layout_editor.LAYOUT_COLLECTION_NAME].objects) == 100,
        "layout guide objects were lost after reload",
    )
    require(
        bpy.ops.crowd.validate_authorable_project() == {"FINISHED"},
        "reloaded authorable project did not validate",
    )
    print("M2 authoring save/reload: PASS {}".format(json.dumps(project.extract_authoring_semantics(scene), sort_keys=True)))
    bpy.ops.wm.quit_blender()


try:
    main()
except SystemExit:
    raise
except Exception as error:
    fail("unexpected {}: {}".format(type(error).__name__, error))
