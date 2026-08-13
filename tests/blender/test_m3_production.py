"""M3 production-workflow persistence and cache-recovery smoke test."""

import json
import os
import shutil
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


def require_rejected(operation, message, expected_text=None):
    """Accept Blender's two Python representations of an operator rejection."""
    try:
        outcome = operation()
    except RuntimeError as error:
        if expected_text is not None:
            require(expected_text.lower() in str(error).lower(), message)
        return
    require(outcome == {"CANCELLED"}, message)


def main():
    if os.environ.get("CROWD_SOURCE_ADDON"):
        from addon import blender_crowd

        blender_crowd.register()
        from addon.blender_crowd import operators
    else:
        addon_utils.enable(EXTENSION, default_set=True)
        from bl_ext.user_default.blender_crowd import operators

    scene = bpy.context.scene
    defaults = scene.crowd_project
    require(defaults.cache_status == "not_inspected", "clean install retained cache preference state")
    require(not defaults.project_uuid, "clean install retained a project identity")
    require(bpy.ops.crowd.create_reference_project() == {"FINISHED"}, "reference project creation failed")
    props = scene.crowd_project
    require(props.current_stage == "Author project", "workflow stage was not persisted")
    require(props.selection_context.startswith("Project:"), "project selection context was not persisted")
    require(props.diagnostics, "reference creation did not record a diagnostic")

    project_dir = tempfile.mkdtemp(prefix="blender-crowd-m3-project-")
    blend_path = os.path.join(project_dir, "recovery.blend")
    bpy.ops.wm.save_as_mainfile(filepath=blend_path, check_existing=False)
    cache_path = os.path.join(project_dir, "cache")
    props.cache_path = "//cache"
    started = bpy.ops.crowd.bake_cache()
    require(started in ({"FINISHED"}, {"RUNNING_MODAL"}), "cache bake did not start")
    require("minimum point attributes" in props.operation_estimate, "bake preflight lacks point-buffer estimate")
    require(bpy.ops.crowd.cancel_bake() == {"FINISHED"}, "cache cancel did not start")
    outcome = operators.wait_for_bake(timeout=60.0)
    require(outcome and outcome["status"] == "canceled", "cache did not enter canceled recovery state")
    require(bpy.ops.crowd.inspect_cache_health() == {"FINISHED"}, "canceled cache was not inspectable")
    require(props.cache_status == "canceled", "health did not expose canceled status")
    require(not props.cache_attached, "canceled cache was marked authoritative")
    require("Do not attach" in props.cache_recovery_hint, "recovery hint is not actionable")
    require_rejected(
        bpy.ops.crowd.attach_cache,
        "canceled cache attached as playback",
        "do not attach",
    )
    support_path = os.path.join(tempfile.mkdtemp(prefix="blender-crowd-m3-support-"), "support.json")
    require(
        bpy.ops.crowd.write_support_bundle(filepath=support_path) == {"FINISHED"},
        "safe support bundle was not written",
    )
    with open(support_path, encoding="utf-8") as handle:
        support_bundle = json.load(handle)
    require(not support_bundle["privacy"]["scene_contents_included"], "support bundle includes scene content")
    require(cache_path not in json.dumps(support_bundle), "support bundle leaked an absolute cache path")

    library_source = os.path.join(project_dir, "linked-source.blend")
    linked_mesh = bpy.data.meshes.new("M3 Linked Source Mesh")
    linked_object = bpy.data.objects.new("M3 Linked Source", linked_mesh)
    bpy.data.libraries.write(library_source, {linked_mesh, linked_object})
    bpy.data.objects.remove(linked_object)
    bpy.data.meshes.remove(linked_mesh)
    with bpy.data.libraries.load(library_source, link=True) as (available, requested):
        requested.objects = ["M3 Linked Source"]
    linked_object = requested.objects[0]
    require(linked_object is not None and linked_object.library is not None, "linked test data was not loaded")
    scene.collection.objects.link(linked_object)
    override_object = linked_object.override_create(remap_local_usages=False)
    require(override_object is not None and override_object.override_library is not None, "library override was not created")
    linked_identity = (linked_object.name, linked_object.library.filepath)
    for _index in range(25):
        bpy.context.view_layer.update()
        tuple(bpy.context.evaluated_depsgraph_get().updates)
    require(
        (linked_object.name, linked_object.library.filepath) == linked_identity,
        "Crowd dependency updates mutated linked source data",
    )

    bpy.ops.wm.save_as_mainfile(filepath=blend_path, check_existing=False)
    moved_parent = tempfile.mkdtemp(prefix="blender-crowd-m3-moved-")
    moved_project = os.path.join(moved_parent, "project")
    shutil.copytree(project_dir, moved_project)
    moved_blend = os.path.join(moved_project, "recovery.blend")
    bpy.ops.wm.open_mainfile(filepath=moved_blend)
    scene = bpy.context.scene
    props = scene.crowd_project
    require(props.cache_status == "canceled", "cache recovery state was lost after reload")
    require(not props.cache_attached, "reload restored a canceled cache as authoritative")
    require(props.diagnostics, "diagnostic history was lost after reload")
    require(any(item.severity == "WARNING" for item in props.diagnostics), "recovery warning was not retained")
    require(
        bpy.path.abspath(props.cache_path).startswith(moved_project + os.sep),
        "moved project did not resolve its relative cache path in the new location",
    )
    manifest_path = os.path.join(moved_project, "cache", "manifest.json")
    with open(manifest_path, encoding="utf-8") as handle:
        manifest = json.load(handle)
    manifest["schema_version"] = 2
    with open(manifest_path, "w", encoding="utf-8") as handle:
        json.dump(manifest, handle)
    require_rejected(
        bpy.ops.crowd.inspect_cache_health,
        "newer cache schema was treated as supported",
        "unsupported",
    )
    require(props.cache_status == "unsupported", "newer cache schema was not labeled unsupported")
    require(not props.cache_attached, "unsupported cache was marked authoritative")
    older_cache = os.path.join(moved_project, "older-cache")
    shutil.copytree(os.path.join(moved_project, "cache"), older_cache)
    older_manifest_path = os.path.join(older_cache, "manifest.json")
    with open(older_manifest_path, encoding="utf-8") as handle:
        older_manifest = json.load(handle)
    older_manifest["schema_version"] = 0
    with open(older_manifest_path, "w", encoding="utf-8") as handle:
        json.dump(older_manifest, handle)
    props.cache_path = older_cache
    require_rejected(
        bpy.ops.crowd.inspect_cache_health,
        "older cache schema was treated as supported",
        "unsupported",
    )
    require(props.cache_status == "unsupported", "older cache schema was not labeled unsupported")
    require(not props.cache_attached, "older cache was marked authoritative")
    print("M3 cache recovery: PASS {} diagnostics".format(len(props.diagnostics)))
    bpy.ops.wm.quit_blender()


try:
    main()
except SystemExit:
    raise
except Exception as error:
    fail("unexpected {}: {}".format(type(error).__name__, error))
