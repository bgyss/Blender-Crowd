"""Generate the redistributable M1 proxy commuters from literal JSON data."""

import json
import math
from pathlib import Path

import bpy


ASSET_FILE = "commuter-assets-v1.json"
ASSET_COLLECTION_NAME = "Crowd Reference Assets"


def load_reference_asset_manifest():
    path = Path(__file__).with_name("reference") / ASSET_FILE
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def _find_by_logical_id(blocks, logical_id):
    return next(
        (item for item in blocks if item.get("crowd_logical_id") == logical_id), None
    )


def _asset_collection(scene):
    collection = bpy.data.collections.get(ASSET_COLLECTION_NAME)
    if collection is None:
        collection = bpy.data.collections.new(ASSET_COLLECTION_NAME)
        collection["crowd_logical_id"] = "commuter_reference_assets"
    if collection.name not in scene.collection.children:
        scene.collection.children.link(collection)
    return collection


def _ensure_material(spec):
    material = _find_by_logical_id(bpy.data.materials, spec["id"])
    if material is None:
        material = bpy.data.materials.new("Crowd Material {}".format(spec["id"]))
        material["crowd_logical_id"] = spec["id"]
    material.diffuse_color = spec["base_color"]
    material.use_nodes = True
    principled = material.node_tree.nodes.get("Principled BSDF")
    if principled is not None:
        principled.inputs["Base Color"].default_value = spec["base_color"]
        principled.inputs["Roughness"].default_value = 0.72
    return material


def _append_box(vertices, faces, center, size):
    start = len(vertices)
    half = [axis * 0.5 for axis in size]
    for x, y, z in (
        (-1, -1, -1),
        (1, -1, -1),
        (1, 1, -1),
        (-1, 1, -1),
        (-1, -1, 1),
        (1, -1, 1),
        (1, 1, 1),
        (-1, 1, 1),
    ):
        vertices.append(
            (
                center[0] + x * half[0],
                center[1] + y * half[1],
                center[2] + z * half[2],
            )
        )
    faces.extend(
        tuple(start + index for index in face)
        for face in (
            (0, 1, 2, 3),
            (4, 7, 6, 5),
            (0, 4, 5, 1),
            (1, 5, 6, 2),
            (2, 6, 7, 3),
            (4, 0, 3, 7),
        )
    )


def _prototype_geometry(spec):
    height = float(spec["height_m"])
    width = float(spec["shoulder_width_m"])
    depth = float(spec["body_depth_m"])
    head = float(spec["head_radius_m"])
    vertices = []
    faces = []
    _append_box(vertices, faces, (0.0, 0.0, height * 0.57), (width, depth, height * 0.42))
    _append_box(vertices, faces, (0.0, 0.0, height - head), (head * 2.0, head * 2.0, head * 2.0))
    _append_box(vertices, faces, (-width * 0.22, 0.0, height * 0.19), (width * 0.28, depth * 0.72, height * 0.38))
    _append_box(vertices, faces, (width * 0.22, 0.0, height * 0.19), (width * 0.28, depth * 0.72, height * 0.38))
    _append_box(vertices, faces, (-width * 0.62, 0.0, height * 0.58), (width * 0.20, depth * 0.66, height * 0.38))
    _append_box(vertices, faces, (width * 0.62, 0.0, height * 0.58), (width * 0.20, depth * 0.66, height * 0.38))
    return vertices, faces


def _ensure_prototype(spec, materials):
    logical_id = spec["id"]
    obj = _find_by_logical_id(bpy.data.objects, logical_id)
    mesh = _find_by_logical_id(bpy.data.meshes, logical_id)
    if mesh is None:
        mesh = bpy.data.meshes.new("Crowd Prototype Mesh {}".format(logical_id))
        vertices, faces = _prototype_geometry(spec)
        mesh.from_pydata(vertices, [], faces)
        mesh.update()
        mesh["crowd_logical_id"] = logical_id
    if obj is None:
        obj = bpy.data.objects.new("Crowd Prototype {}".format(logical_id), mesh)
        obj["crowd_logical_id"] = logical_id
        # The prototype remains an unlinked data block. Geometry Nodes may
        # instance it, but it cannot appear as an accidental scene object.
    elif obj.data is not mesh:
        obj.data = mesh
    obj["crowd_asset_kind"] = "prototype"
    obj["crowd_height_m"] = float(spec["height_m"])
    material = materials[spec["material_id"]]
    if not mesh.materials:
        mesh.materials.append(material)
    else:
        mesh.materials[0] = material
    return obj


def _ensure_rig(scene, spec):
    logical_id = spec["id"]
    armature = _find_by_logical_id(bpy.data.armatures, logical_id)
    rig = _find_by_logical_id(bpy.data.objects, logical_id)
    collection = _asset_collection(scene)
    if armature is None:
        armature = bpy.data.armatures.new("Crowd Canonical Rig")
        armature["crowd_logical_id"] = logical_id
    if rig is None:
        rig = bpy.data.objects.new("Crowd Canonical Rig", armature)
        rig["crowd_logical_id"] = logical_id
    if rig.name not in collection.objects:
        collection.objects.link(rig)
    rig.hide_render = True

    if not armature.bones:
        previous_active = bpy.context.view_layer.objects.active
        previous_mode = bpy.context.mode
        for obj in bpy.context.selected_objects:
            obj.select_set(False)
        rig.select_set(True)
        bpy.context.view_layer.objects.active = rig
        bpy.ops.object.mode_set(mode="EDIT")
        bone_layout = {
            "root": ((0.0, 0.0, 0.0), (0.0, 0.0, 0.8)),
            "spine": ((0.0, 0.0, 0.8), (0.0, 0.0, 1.45)),
            "head": ((0.0, 0.0, 1.45), (0.0, 0.0, 1.75)),
            "arm_l": ((0.0, 0.0, 1.35), (-0.65, 0.0, 0.95)),
            "arm_r": ((0.0, 0.0, 1.35), (0.65, 0.0, 0.95)),
            "leg_l": ((-0.12, 0.0, 0.8), (-0.12, 0.0, 0.05)),
            "leg_r": ((0.12, 0.0, 0.8), (0.12, 0.0, 0.05)),
        }
        for bone_name in spec["bones"]:
            bone = armature.edit_bones.new(bone_name)
            bone.head, bone.tail = bone_layout[bone_name]
        bpy.ops.object.mode_set(mode="OBJECT")
        rig.select_set(False)
        if previous_active is not None:
            bpy.context.view_layer.objects.active = previous_active
        if previous_mode != "OBJECT" and previous_active is not None:
            try:
                bpy.ops.object.mode_set(mode=previous_mode)
            except RuntimeError:
                pass
    return rig


def _ensure_action(rig, clip, phases):
    logical_id = clip["id"]
    action = _find_by_logical_id(bpy.data.actions, logical_id)
    if action is None:
        action = bpy.data.actions.new("Crowd Clip {}".format(logical_id.title()))
        action["crowd_logical_id"] = logical_id
        rig.animation_data_create()
        rig.animation_data.action = action
        duration = int(clip["duration_frames"])
        amplitude = float(clip["swing_radians"])
        for phase in phases:
            rig["crowd_proxy_swing"] = amplitude * math.sin(phase * math.tau)
            rig.keyframe_insert(
                data_path='["crowd_proxy_swing"]', frame=phase * duration
            )
        rig.animation_data.action = None
    action["crowd_normalized_phases"] = phases
    action["crowd_keyframe_count"] = len(phases)
    action["crowd_duration_frames"] = int(clip["duration_frames"])
    action["crowd_swing_radians"] = float(clip["swing_radians"])
    return action


def ensure_reference_assets(scene):
    """Create or reuse every M1 proxy asset by stable logical ID."""
    manifest = load_reference_asset_manifest()
    if manifest.get("external_paths"):
        raise ValueError("reference fixture must not contain external paths")
    materials = {
        spec["id"]: _ensure_material(spec) for spec in manifest["materials"]
    }
    prototypes = [
        _ensure_prototype(spec, materials) for spec in manifest["prototypes"]
    ]
    rig = _ensure_rig(scene, manifest["rig"])
    phases = [float(value) for value in manifest["normalized_phases"]]
    actions = [_ensure_action(rig, clip, phases) for clip in manifest["clips"]]
    return {
        "manifest": manifest,
        "materials": materials,
        "prototypes": prototypes,
        "rig": rig,
        "actions": actions,
    }
