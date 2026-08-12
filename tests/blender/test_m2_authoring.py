"""Headless M2 editor persistence, undo, and native-validation coverage."""

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
        layout_editor,
        operators,
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

    before = len(props.queues)
    add = bpy.ops.crowd.add_m2_semantic(entity_type="queue")
    require(add == {"FINISHED"}, "queue add operator failed")
    require(len(props.queues) == before + 1, "queue add did not mutate scene")
    require(bpy.ops.ed.undo() == {"FINISHED"}, "queue add is not undoable")
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
    require(bpy.ops.ed.undo() == {"FINISHED"}, "population add is not undoable")
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
    require(bpy.ops.ed.undo() == {"FINISHED"}, "clip add is not undoable")
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
    require(
        len(bpy.data.collections[layout_editor.LAYOUT_COLLECTION_NAME].objects) == 100,
        "layout guide objects were lost after reload",
    )
    require(
        bpy.ops.crowd.validate_authorable_project() == {"FINISHED"},
        "reloaded authorable project did not validate",
    )
    print("M2 authoring save/reload: PASS {}".format(json.dumps(project.extract_authoring_semantics(scene), sort_keys=True)))


try:
    main()
except SystemExit:
    raise
except Exception as error:
    fail("unexpected {}: {}".format(type(error).__name__, error))
