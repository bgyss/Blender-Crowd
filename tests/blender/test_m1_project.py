"""Exercise the self-contained M1 project workflow in a clean Blender process."""

import json
import os
import sys
import tempfile

import addon_utils
import bpy


EXTENSION = "bl_ext.user_default.blender_crowd"
EXPECTED_PHASES = [0.0, 0.25, 0.5, 0.75, 1.0]


def fail(message):
    print("FAIL: {}".format(message))
    sys.exit(1)


def require(condition, message):
    if not condition:
        fail(message)


def stable_datablock_counts():
    return {
        "objects": sum(1 for item in bpy.data.objects if "crowd_logical_id" in item),
        "meshes": sum(1 for item in bpy.data.meshes if "crowd_logical_id" in item),
        "materials": sum(
            1 for item in bpy.data.materials if "crowd_logical_id" in item
        ),
        "armatures": sum(
            1 for item in bpy.data.armatures if "crowd_logical_id" in item
        ),
        "actions": sum(1 for item in bpy.data.actions if "crowd_logical_id" in item),
    }


def assert_reference_contract(project_ir, assets):
    require(project_ir["schema_version"] == 1, "wrong project schema version")
    require(len(project_ir["populations"]) == 1, "expected one population")
    require(project_ir["populations"][0]["count"] == 1000, "expected 1,000 agents")
    require(len(project_ir["semantics"]["spawns"]) == 2, "expected two spawns")
    require(
        len(project_ir["semantics"]["destinations"]) == 3,
        "expected three destinations",
    )
    require(len(project_ir["semantics"]["portals"]) == 2, "expected two doors")
    require(
        {item["id"] for item in project_ir["semantics"]["portals"]}
        == {"east_gate", "west_gate"},
        "door logical IDs changed",
    )
    require(len(project_ir["archetypes"]) == 3, "expected three archetypes")
    require(
        {item["id"] for item in assets["clips"]} == {"idle", "walk", "jog"},
        "reference clip logical IDs changed",
    )
    require(not assets.get("external_paths"), "reference assets contain external paths")


def main():
    addon_utils.enable(EXTENSION, default_set=True)
    try:
        from bl_ext.user_default.blender_crowd import operators, project, reference_assets
    except ImportError as error:
        fail("M1 project modules did not import: {}".format(error))

    if not hasattr(bpy.context.scene, "crowd_project"):
        fail("Scene.crowd_project is not registered")

    try:
        result = bpy.ops.crowd.create_reference_project()
    except AttributeError as error:
        fail("crowd.create_reference_project is not registered: {}".format(error))
    require(result == {"FINISHED"}, "reference project operator did not finish")

    project_ir = project.extract_ir(bpy.context.scene)
    assets = reference_assets.load_reference_asset_manifest()
    assert_reference_contract(project_ir, assets)
    repo_root = os.environ.get("CROWD_REPO_ROOT")
    if repo_root:
        with open(
            os.path.join(repo_root, "assets/reference/concourse-project-v1.json"),
            encoding="utf-8",
        ) as handle:
            require(
                json.load(handle) == project.load_reference_project(),
                "packaged project fixture drifted from the source fixture",
            )
        with open(
            os.path.join(repo_root, "assets/reference/commuter-assets-v1.json"),
            encoding="utf-8",
        ) as handle:
            require(
                json.load(handle) == assets,
                "packaged asset fixture drifted from the source fixture",
            )

    import blender_crowd_native

    compiled = blender_crowd_native.compile_project(
        json.dumps(project_ir, sort_keys=True, separators=(",", ":"))
    )
    require(compiled.agent_count == 1000, "native compiler did not produce 1,000 agents")

    semantic_objects = [
        item for item in bpy.data.objects if "crowd_entity_type" in item
    ]
    require(len(semantic_objects) == 11, "expected 11 typed semantic objects")
    require(
        all(item.get("crowd_logical_id") for item in semantic_objects),
        "a semantic object lacks a stable logical ID",
    )

    first_counts = stable_datablock_counts()
    require(
        bpy.ops.crowd.create_reference_project() == {"FINISHED"},
        "second reference project creation did not finish",
    )
    require(
        stable_datablock_counts() == first_counts,
        "reference generation duplicated stable data blocks",
    )

    for clip_id in ("idle", "walk", "jog"):
        action = next(
            (
                item
                for item in bpy.data.actions
                if item.get("crowd_logical_id") == clip_id
            ),
            None,
        )
        require(action is not None, "missing {} action".format(clip_id))
        require(
            list(action.get("crowd_normalized_phases", [])) == EXPECTED_PHASES,
            "{} action has wrong normalized phases".format(clip_id),
        )
        require(
            int(action.get("crowd_keyframe_count", 0)) >= len(EXPECTED_PHASES),
            "{} action has no authored keyframes".format(clip_id),
        )
        require(
            action.frame_range[1] >= int(action["crowd_duration_frames"]),
            "{} action does not contain the authored frame range".format(clip_id),
        )

    cache_path = os.path.join(tempfile.mkdtemp(prefix="blender-crowd-m1-"), "cache")
    bpy.context.scene.crowd_project.cache_path = cache_path
    bake_result = bpy.ops.crowd.bake_cache()
    require(
        bake_result in ({"RUNNING_MODAL"}, {"FINISHED"}),
        "bake operator did not start",
    )
    require(bpy.ops.crowd.cancel_bake() == {"FINISHED"}, "cancel operator failed")
    outcome = operators.wait_for_bake(timeout=60.0)
    require(outcome is not None, "bake worker did not finish")
    require(outcome.get("status") == "canceled", "bake did not report canceled")
    inspection = blender_crowd_native.inspect_cache(cache_path)
    require(inspection["status"] == "canceled", "cache is not marked canceled")

    print("project source hash: {}".format(compiled.source_hash))
    print("stable data blocks: {}".format(first_counts))
    print("canceled cache: {}".format(cache_path))
    print("PASS: self-contained M1 project workflow")


main()
