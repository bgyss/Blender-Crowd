"""M4 cache-only layout, conflict, flatten, USD, and save/reload proof."""

import hashlib
import json
import os
import sys
import tempfile
import time

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
    if os.environ.get("CROWD_SOURCE_ADDON"):
        from addon import blender_crowd

        blender_crowd.register()
        from addon.blender_crowd import m4_layout, operators, project, render_workflow
    else:
        addon_utils.enable(EXTENSION, default_set=True)
        from bl_ext.user_default.blender_crowd import m4_layout, operators, project, render_workflow
    import blender_crowd_native

    scene = bpy.context.scene
    require(bpy.ops.crowd.create_reference_project() == {"FINISHED"}, "reference project failed")
    scene.frame_end = scene.frame_start + 4_999
    cache_dir = os.path.join(tempfile.mkdtemp(prefix="blender-crowd-m4-cache-"), "cache")
    compiled = blender_crowd_native.compile_project(json.dumps(project.extract_ir(scene)))
    # M4's directing UI must stay procedural at the product's 1,000-agent
    # reference scale; a seven-agent correction is deliberately scoped within
    # that full cache, not substituted for it.
    session = compiled.create_session(agent_count=1000)
    outcome = session.bake(cache_dir, 5_000, blender_crowd_native.CancelToken())
    require(outcome["status"] == "complete", "M4 base cache did not bake")
    manifest_path = os.path.join(cache_dir, "manifest.json")
    before_hash = hashlib.sha256(open(manifest_path, "rb").read()).hexdigest()
    scene_object_count_before_attach = len(scene.objects)
    playback = operators.attach_cache_path(scene, cache_dir)
    scene_object_count_after_attach = len(scene.objects)
    require(playback.agent_count == 1000, "M4 fixture did not attach the 1,000-agent cache")
    base_identity = playback.base_cache_hash
    agent_ids = compiled.agent_ids()[:7]
    props = scene.crowd_project
    captures_dir = os.environ.get("M4_ARTIFACT_DIR") or tempfile.mkdtemp(prefix="blender-crowd-m4-captures-")
    os.makedirs(captures_dir, exist_ok=True)
    render_workflow.configure_reference_scene(scene)
    scene.render.engine = render_workflow._eevee_engine_identifier(scene)
    scene.frame_set(scene.frame_start + 5)
    before_selected_position = tuple(playback.object.data.attributes["crowd_position"].data[0].vector)
    before_capture = os.path.join(captures_dir, "before-seven-agent-correction.png")
    scene.render.filepath = before_capture
    started = time.perf_counter()
    bpy.ops.render.render(write_still=True)
    before_capture_seconds = time.perf_counter() - started
    require(os.path.isfile(before_capture) and os.path.getsize(before_capture) > 0, "before correction capture was not written")
    scene.cursor.location = playback.object.matrix_world @ playback.object.data.attributes["crowd_position"].data[0].vector
    require(bpy.ops.crowd.select_m4_nearest_agent() == {"FINISHED"}, "cursor selection failed")
    require(int(props.selected_agent_id) in compiled.agent_ids(), "cursor selection did not resolve a stable cache ID")
    props.m4_layer_id = "seven-agent-layout"
    props.m4_layer_kind = "layout"
    props.m4_target_agent_ids = ",".join(str(value) for value in agent_ids)
    props.m4_tick_start = 5
    props.m4_tick_end = 25
    props.m4_offset_x = 2.0
    props.m4_offset_y = 0.0
    props.m4_offset_z = 0.0
    props.layout_layers_path = "//m4-layout-layers.json"
    blend_path = os.path.join(tempfile.mkdtemp(prefix="blender-crowd-m4-blend-"), "m4.blend")
    bpy.ops.wm.save_as_mainfile(filepath=blend_path, check_existing=False)
    require(bpy.ops.crowd.add_m4_transform_layer() == {"FINISHED"}, "seven-agent correction failed")
    require(len(props.m4_layers) == 1, "layer list did not reflect correction")
    target_summary = props.m4_layers[0].target_summary
    require("7 ID(s)" in target_summary, "seven-agent target scope was lost")
    require("ticks 5..25" in target_summary, "seven-agent tick scope was lost")
    # Mute/solo are durable layer states, not presentation-only UI toggles.
    require(bpy.ops.crowd.toggle_m4_layer_mute() == {"FINISHED"}, "layer mute failed")
    require(m4_layout.load_layer_stack(bpy.path.abspath(props.layout_layers_path))[0]["muted"], "mute was not persisted")
    require(bpy.ops.crowd.toggle_m4_layer_mute() == {"FINISHED"}, "layer unmute failed")
    require(bpy.ops.crowd.toggle_m4_layer_solo() == {"FINISHED"}, "layer solo failed")
    require(m4_layout.load_layer_stack(bpy.path.abspath(props.layout_layers_path))[0]["solo"], "solo was not persisted")
    require(bpy.ops.crowd.toggle_m4_layer_solo() == {"FINISHED"}, "layer unsolo failed")
    scene.frame_set(scene.frame_start + 5)
    after_selected_position = tuple(playback.object.data.attributes["crowd_position"].data[0].vector)
    require(
        abs(after_selected_position[0] - (before_selected_position[0] + 2.0)) < 1e-4,
        "seven-agent correction did not move the selected cache point by its authored X offset",
    )
    after_capture = os.path.join(captures_dir, "after-seven-agent-correction.png")
    scene.render.filepath = after_capture
    started = time.perf_counter()
    bpy.ops.render.render(write_still=True)
    after_capture_seconds = time.perf_counter() - started
    require(os.path.isfile(after_capture) and os.path.getsize(after_capture) > 0, "after correction capture was not written")
    require(
        hashlib.sha256(open(before_capture, "rb").read()).digest()
        != hashlib.sha256(open(after_capture, "rb").read()).digest(),
        "before/after captures did not show a changed composed result",
    )

    # Inject a deterministic same-channel conflict and verify it is visible.
    layer_path = bpy.path.abspath(props.layout_layers_path)
    conflict = m4_layout.new_transform_layer(
        "director-conflict", "shot", base_identity, [agent_ids[0]], 5, 25, (3, 0, 0), order=20
    )
    layers = m4_layout.append_layer(layer_path, conflict)
    playback.set_layout_layers(layers)
    m4_layout.sync_layer_summaries(scene, layers)
    scene.frame_set(5)
    require(playback.current_tick == 5, "conflict inspection did not reach the overlapping tick")
    require(bpy.ops.crowd.inspect_m4_layout() == {"FINISHED"}, "layout inspection failed")
    require("1 conflict" in props.m4_layout_status, "injected conflict was not displayed")

    # Exercise the remaining scoped directing actions against this baked cache.
    props.m4_layer_id = "region-density-west"
    props.m4_region_id = "concourse-west"
    props.m4_density_millionths = 700_000
    require(bpy.ops.crowd.add_m4_region_density() == {"FINISHED"}, "region-density correction failed")
    props.m4_layer_id = "curve-retime-exit"
    props.m4_curve_id = "exit-curve"
    props.m4_curve_offset_ticks = 3
    require(bpy.ops.crowd.add_m4_curve_retiming() == {"FINISHED"}, "curve-retiming correction failed")
    props.m4_layer_id = "local-resim-agent"
    props.m4_resim_target_x = 8.0
    props.m4_resim_target_y = 4.0
    props.m4_resim_target_z = 0.0
    require(bpy.ops.crowd.add_m4_local_resimulation() == {"FINISHED"}, "local resimulation failed")
    props.m4_layer_id = "physics-handoff-agent"
    props.m4_physics_masks = "crowd,hero_props"
    require(bpy.ops.crowd.add_m4_physics_handoff() == {"FINISHED"}, "physics handoff failed")
    persisted_layers = m4_layout.load_layer_stack(layer_path)
    edit_types = {layer["edits"][0]["type"] for layer in persisted_layers}
    require({"region_density", "curve_retiming", "physics_handoff"}.issubset(edit_types), "scoped M4 edits were not persisted")
    local_resim = next(layer for layer in persisted_layers if layer["layer_id"] == "local-resim-agent")
    require(local_resim["local_resimulation"]["affected_agent_ids"] == [int(props.selected_agent_id)], "local resimulation scope was not persisted")
    physics = next(layer for layer in persisted_layers if layer["layer_id"] == "physics-handoff-agent")
    require(physics["edits"][0]["cached_samples"], "physics handoff did not cache interval samples")

    # At a populated tick the full cache must remain Geometry Nodes instances,
    # rather than one Blender object per agent.  Keep the image as a durable
    # render-capture proof in addition to this evaluated-scene assertion.
    scene.frame_set(scene.frame_end)
    bpy.context.view_layer.update()
    depsgraph = bpy.context.evaluated_depsgraph_get()
    procedural_instance_count = sum(1 for item in depsgraph.object_instances if item.is_instance)
    require(
        procedural_instance_count >= 600,
        "populated M4 cache evaluated only {} procedural instances at tick {}".format(
            procedural_instance_count, playback.current_tick
        ),
    )
    require(
        scene_object_count_after_attach - scene_object_count_before_attach < 10,
        "M4 cache attach added {} persistent objects for {} agents".format(
            scene_object_count_after_attach - scene_object_count_before_attach,
            playback.agent_count,
        ),
    )
    scale_capture = os.path.join(captures_dir, "procedural-1000-agent-cache.png")
    scene.render.filepath = scale_capture
    bpy.ops.render.render(write_still=True)
    require(os.path.isfile(scale_capture) and os.path.getsize(scale_capture) > 0, "procedural scale capture was not written")

    props.layout_flatten_path = "//m4-flattened.json"
    props.layout_export_path = "//m4-layout.usda"
    require(bpy.ops.crowd.flatten_m4_layout() == {"FINISHED"}, "reversible flatten preview failed")
    require(bpy.ops.crowd.export_m4_usd() == {"FINISHED"}, "M4 USD export failed")
    flattened = json.load(open(bpy.path.abspath(props.layout_flatten_path), encoding="utf-8"))
    usda = open(bpy.path.abspath(props.layout_export_path), encoding="utf-8").read()
    require(flattened["source_base_hash"] == base_identity, "flattened preview lost base identity")
    require("PointInstancer" in usda and base_identity in usda, "USD profile lost identity or instancer")
    require(hashlib.sha256(open(manifest_path, "rb").read()).hexdigest() == before_hash, "M4 mutated base cache")

    bpy.ops.wm.save_as_mainfile(filepath=blend_path, check_existing=False)
    bpy.ops.wm.open_mainfile(filepath=blend_path)
    scene = bpy.context.scene
    props = scene.crowd_project
    require(len(props.m4_layers) == 6, "layer editor state was lost after save/reload")
    playback = operators.attach_cache_path(scene, cache_dir)
    require(bpy.ops.crowd.apply_m4_layers() == {"FINISHED"}, "saved M4 stack could not be reapplied")
    require(playback.base_cache_hash == base_identity, "reload changed immutable cache identity")
    print("M4 Blender layout: PASS {}".format(json.dumps({
        "layers": len(props.m4_layers), "base": base_identity,
        "captures": [before_capture, after_capture, scale_capture],
        "capture_seconds": {"before": round(before_capture_seconds, 3), "after": round(after_capture_seconds, 3)},
        "procedural_instance_count": procedural_instance_count,
        "scene_object_count": len(scene.objects),
        "scene_object_count_before_attach": scene_object_count_before_attach,
        "scene_object_count_after_attach": scene_object_count_after_attach,
    }, sort_keys=True)))
    bpy.ops.wm.quit_blender()


try:
    main()
except SystemExit:
    raise
except Exception as error:
    fail("unexpected {}: {}".format(type(error).__name__, error))
