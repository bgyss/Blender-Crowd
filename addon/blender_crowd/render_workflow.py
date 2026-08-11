"""Deterministic M1 cache-only Eevee/Cycles reference render workflow."""

import hashlib
import json
from pathlib import Path
import resource
import time

import bpy
from mathutils import Vector

from . import cache_playback


IMAGE_SIZE = (320, 180)
REFERENCE_TICK = 4999
_LAST_METRICS = None


def last_metrics():
    return _LAST_METRICS


def _find_object(logical_id):
    return next(
        (item for item in bpy.data.objects if item.get("crowd_render_id") == logical_id),
        None,
    )


def _material(logical_id, color):
    material = next(
        (
            item
            for item in bpy.data.materials
            if item.get("crowd_render_id") == logical_id
        ),
        None,
    )
    if material is None:
        material = bpy.data.materials.new("Crowd Render {}".format(logical_id))
        material["crowd_render_id"] = logical_id
    material.diffuse_color = color
    material.use_nodes = True
    principled = material.node_tree.nodes.get("Principled BSDF")
    if principled is not None:
        principled.inputs["Base Color"].default_value = color
        principled.inputs["Roughness"].default_value = 0.82
    return material


def _ensure_ground(scene):
    obj = _find_object("concourse_ground")
    if obj is None:
        mesh = bpy.data.meshes.new("M1 Concourse Ground")
        mesh.from_pydata(
            [(0.0, 0.0, 0.0), (60.0, 0.0, 0.0), (60.0, 20.0, 0.0), (0.0, 20.0, 0.0)],
            [],
            [(0, 1, 2, 3)],
        )
        mesh.update()
        obj = bpy.data.objects.new("M1 Concourse Ground", mesh)
        obj["crowd_render_id"] = "concourse_ground"
        scene.collection.objects.link(obj)
    obj.data.materials.clear()
    obj.data.materials.append(_material("ground", (0.16, 0.18, 0.22, 1.0)))
    return obj


def _look_at(obj, target):
    direction = Vector(target) - obj.location
    obj.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()


def _ensure_camera(scene):
    camera = _find_object("reference_camera")
    if camera is None:
        data = bpy.data.cameras.new("M1 Reference Camera")
        camera = bpy.data.objects.new("M1 Reference Camera", data)
        camera["crowd_render_id"] = "reference_camera"
        scene.collection.objects.link(camera)
    camera.location = (30.0, -42.0, 35.0)
    camera.data.lens = 46.0
    _look_at(camera, (30.0, 10.0, 1.0))
    scene.camera = camera
    return camera


def _ensure_light(scene, logical_id, light_type, location, energy, size=5.0):
    obj = _find_object(logical_id)
    if obj is None:
        data = bpy.data.lights.new("M1 {}".format(logical_id), light_type)
        obj = bpy.data.objects.new("M1 {}".format(logical_id), data)
        obj["crowd_render_id"] = logical_id
        scene.collection.objects.link(obj)
    obj.location = location
    obj.data.energy = energy
    if light_type == "AREA":
        obj.data.shape = "DISK"
        obj.data.size = size
        _look_at(obj, (30.0, 10.0, 0.0))
    else:
        obj.rotation_euler = (0.45, -0.35, -0.55)
    return obj


def configure_reference_scene(scene):
    for obj in scene.objects:
        if obj.name in {"Cube", "Camera", "Light"} and "crowd_render_id" not in obj:
            obj.hide_render = True
    _ensure_ground(scene)
    _ensure_camera(scene)
    _ensure_light(scene, "key_area", "AREA", (30.0, 4.0, 28.0), 1800.0, 14.0)
    _ensure_light(scene, "fill_sun", "SUN", (0.0, 0.0, 20.0), 1.8)
    scene.world.color = (0.025, 0.035, 0.06)
    scene.render.resolution_x = IMAGE_SIZE[0]
    scene.render.resolution_y = IMAGE_SIZE[1]
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.film_transparent = False


def _measure_armature(scene):
    rig = next(
        (
            item
            for item in bpy.data.objects
            if item.get("crowd_logical_id") == "commuter_canonical_rig"
        ),
        None,
    )
    action = next(
        (
            item
            for item in bpy.data.actions
            if item.get("crowd_logical_id") == "walk"
        ),
        None,
    )
    if rig is None or action is None:
        raise ValueError("canonical armature and walk action are required")
    previous_frame = scene.frame_current
    rig.animation_data_create()
    previous_action = rig.animation_data.action
    rig.animation_data.action = action
    # Stepping the timeline would otherwise decode 31 cache ticks, which both
    # moves playback off the reference tick and charges cache reads to the
    # armature measurement this function exists to isolate.
    with cache_playback.suspended_frame_sync():
        started = time.perf_counter()
        for frame in range(31):
            scene.frame_set(frame)
            bpy.context.view_layer.update()
        elapsed = time.perf_counter() - started
        rig.animation_data.action = previous_action
        scene.frame_set(previous_frame)
    return elapsed


def _manifest_hash(cache_path):
    return hashlib.sha256(Path(cache_path, "manifest.json").read_bytes()).hexdigest()


def _scene_hash(scene):
    description = []
    for obj in sorted(scene.objects, key=lambda item: item.name):
        logical_id = obj.get("crowd_render_id") or obj.get("crowd_logical_id")
        if logical_id:
            description.append(
                {
                    "id": logical_id,
                    "location": [float(value) for value in obj.location],
                    "rotation": [float(value) for value in obj.rotation_euler],
                    "scale": [float(value) for value in obj.scale],
                }
            )
    encoded = json.dumps(description, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()


def _render(scene, engine, output_path):
    scene.render.engine = engine
    device = "GPU"
    samples = 16
    if engine == "CYCLES":
        scene.cycles.device = "CPU"
        scene.cycles.samples = 4
        scene.cycles.use_denoising = False
        device = "CPU"
        samples = scene.cycles.samples
    scene.render.filepath = str(output_path)
    started = time.perf_counter()
    bpy.ops.render.render(write_still=True)
    return {
        "engine": engine,
        "device": device,
        "samples": samples,
        "seconds": time.perf_counter() - started,
        "output_path": str(output_path),
    }


def _eevee_engine_identifier(scene):
    supported = {
        item.identifier
        for item in scene.bl_rna.properties["render"].fixed_type.properties[
            "engine"
        ].enum_items
    }
    if "BLENDER_EEVEE_NEXT" in supported:
        return "BLENDER_EEVEE_NEXT"
    if "BLENDER_EEVEE" in supported:
        return "BLENDER_EEVEE"
    raise ValueError("this Blender build has no Eevee render engine")


def render_reference(scene, playback, output_dir):
    global _LAST_METRICS
    target = Path(output_dir)
    target.mkdir(parents=True, exist_ok=True)
    configure_reference_scene(scene)
    reference_tick = max(playback.tick_start, min(playback.tick_end, REFERENCE_TICK))
    started = time.perf_counter()
    playback.sync_to_tick(reference_tick)
    point_upload_seconds = time.perf_counter() - started
    armature_evaluation_seconds = _measure_armature(scene)
    # Position through the scene frame, not sync_to_tick: rendering fires the
    # playback frame handler, which re-syncs to frame_current and would discard
    # a direct sync, drawing the opening tick instead of the reference tick.
    reference_frame = scene.frame_start + (reference_tick - playback.tick_start)
    scene.frame_set(reference_frame)
    if playback.current_tick != reference_tick:
        raise ValueError(
            "frame {} drove playback to tick {}, not the reference tick {}".format(
                reference_frame, playback.current_tick, reference_tick
            )
        )
    bpy.context.view_layer.update()
    depsgraph = bpy.context.evaluated_depsgraph_get()
    proxy_instance_count = sum(
        1 for instance in depsgraph.object_instances if instance.is_instance
    )
    if proxy_instance_count < 600:
        raise ValueError(
            "reference tick produced only {} commuter instances".format(
                proxy_instance_count
            )
        )

    renders = {
        "eevee": _render(scene, _eevee_engine_identifier(scene), target / "m1-eevee.png"),
        "cycles": _render(scene, "CYCLES", target / "m1-cycles.png"),
    }
    # What was drawn, not what was requested. Rendering fires the playback frame
    # handler, so these are measured after the renders rather than trusted.
    bpy.context.view_layer.update()
    post_render_depsgraph = bpy.context.evaluated_depsgraph_get()
    post_render_proxy_instance_count = sum(
        1 for instance in post_render_depsgraph.object_instances if instance.is_instance
    )
    metrics = {
        "schema_version": 1,
        "cache_only": True,
        "blender_version": bpy.app.version_string,
        "agent_count": playback.agent_count,
        "proxy_instance_count": proxy_instance_count,
        "post_render_proxy_instance_count": post_render_proxy_instance_count,
        "reference_tick": reference_tick,
        "rendered_tick": playback.current_tick,
        "image_size": list(IMAGE_SIZE),
        "cache_manifest_hash": _manifest_hash(scene.crowd_project.cache_path),
        "scene_hash": _scene_hash(scene),
        "point_upload_seconds": point_upload_seconds,
        "armature_evaluation_seconds": armature_evaluation_seconds,
        "peak_resident_bytes": int(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss),
        "renders": renders,
    }
    metrics_path = target / "m1-render-metrics.json"
    with metrics_path.open("w", encoding="utf-8") as handle:
        json.dump(metrics, handle, indent=2, sort_keys=True)
        handle.write("\n")
    _LAST_METRICS = metrics
    return metrics
