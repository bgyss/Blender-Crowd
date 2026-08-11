"""Reference-project creation and typed Blender-to-ProjectIrV1 extraction."""

import copy
import json
from pathlib import Path

import bpy
from mathutils import Vector


PROJECT_TEXT_NAME = "CrowdProjectIrV1"
SEMANTIC_COLLECTION_NAME = "Crowd Project Semantics"
SEMANTIC_BOX_MESH_ID = "crowd_semantic_unit_box"
REFERENCE_PROJECT_FILE = "concourse-project-v1.json"
BEHAVIOR_GRAPH_TEXT_NAME = "CrowdBehaviorGraphV1"
REFERENCE_BEHAVIOR_GRAPH_FILE = "leave-concourse-v1.json"
REFERENCE_AUTHORING_FILE = "concourse-authoring-v2.json"

_BOUNDED_TYPES = {
    "walkable": "bounds",
    "blocked": "bounds",
    "spawn": "bounds",
    "destination": "capacity_bounds",
}
_IR_ARRAYS = {
    "walkable": "walkable",
    "blocked": "blocked",
    "spawn": "spawns",
    "destination": "destinations",
    "portal": "portals",
}


def _reference_path(filename):
    return Path(__file__).with_name("reference") / filename


def load_reference_project():
    with _reference_path(REFERENCE_PROJECT_FILE).open(encoding="utf-8") as handle:
        return json.load(handle)


def load_reference_behavior_graph():
    with _reference_path(REFERENCE_BEHAVIOR_GRAPH_FILE).open(encoding="utf-8") as handle:
        return json.load(handle)


def load_reference_authoring_semantics():
    with _reference_path(REFERENCE_AUTHORING_FILE).open(encoding="utf-8") as handle:
        return json.load(handle)


def behavior_graph_json():
    """Return the artist-editable graph text, never a per-agent Python callback."""
    text = bpy.data.texts.get(BEHAVIOR_GRAPH_TEXT_NAME)
    if text is None:
        raise ValueError("create a reference project or behavior graph first")
    return text.as_string()


def _semantic_collection(scene):
    collection = bpy.data.collections.get(SEMANTIC_COLLECTION_NAME)
    if collection is None:
        collection = bpy.data.collections.new(SEMANTIC_COLLECTION_NAME)
        collection["crowd_logical_id"] = "reference_concourse_semantics"
    if collection.name not in scene.collection.children:
        scene.collection.children.link(collection)
    return collection


def _unit_box_mesh():
    for mesh in bpy.data.meshes:
        if mesh.get("crowd_logical_id") == SEMANTIC_BOX_MESH_ID:
            return mesh
    mesh = bpy.data.meshes.new("Crowd Semantic Unit Box")
    mesh.from_pydata(
        [
            (-1.0, -1.0, -1.0),
            (1.0, -1.0, -1.0),
            (1.0, 1.0, -1.0),
            (-1.0, 1.0, -1.0),
            (-1.0, -1.0, 1.0),
            (1.0, -1.0, 1.0),
            (1.0, 1.0, 1.0),
            (-1.0, 1.0, 1.0),
        ],
        [],
        [
            (0, 1, 2, 3),
            (4, 7, 6, 5),
            (0, 4, 5, 1),
            (1, 5, 6, 2),
            (2, 6, 7, 3),
            (4, 0, 3, 7),
        ],
    )
    mesh["crowd_logical_id"] = SEMANTIC_BOX_MESH_ID
    return mesh


def _find_semantic_object(entity_type, logical_id):
    for obj in bpy.data.objects:
        if (
            obj.get("crowd_entity_type") == entity_type
            and obj.get("crowd_logical_id") == logical_id
        ):
            return obj
    return None


def _bounded_object(collection, entity_type, item, bounds_key):
    logical_id = item["id"]
    obj = _find_semantic_object(entity_type, logical_id)
    if obj is None:
        obj = bpy.data.objects.new(
            "Crowd {} {}".format(entity_type.title(), logical_id), _unit_box_mesh()
        )
        collection.objects.link(obj)
    elif obj.name not in collection.objects:
        collection.objects.link(obj)

    bounds = item[bounds_key]
    center = [
        (bounds["min"][axis] + bounds["max"][axis]) * 0.5 for axis in range(2)
    ]
    half_size = [
        (bounds["max"][axis] - bounds["min"][axis]) * 0.5 for axis in range(2)
    ]
    obj.location = (center[0], center[1], 0.0)
    obj.rotation_euler = (0.0, 0.0, 0.0)
    obj.scale = (half_size[0], half_size[1], 0.05)
    obj.display_type = "WIRE"
    obj.hide_render = True
    obj["crowd_entity_type"] = entity_type
    obj["crowd_logical_id"] = logical_id
    obj["crowd_ir_fields"] = json.dumps(
        {key: value for key, value in item.items() if key != bounds_key},
        sort_keys=True,
        separators=(",", ":"),
    )
    return obj


def _portal_object(collection, item):
    logical_id = item["id"]
    obj = _find_semantic_object("portal", logical_id)
    if obj is None:
        obj = bpy.data.objects.new("Crowd Portal {}".format(logical_id), None)
        collection.objects.link(obj)
    elif obj.name not in collection.objects:
        collection.objects.link(obj)
    obj.empty_display_type = "CUBE"
    obj.empty_display_size = max(float(item["width_m"]) * 0.5, 0.25)
    obj.location = (item["center"][0], item["center"][1], 0.0)
    obj["crowd_entity_type"] = "portal"
    obj["crowd_logical_id"] = logical_id
    obj["crowd_ir_fields"] = json.dumps(
        {key: value for key, value in item.items() if key != "center"},
        sort_keys=True,
        separators=(",", ":"),
    )
    return obj


def create_reference_project(scene):
    """Populate `scene` from the packaged reference fixture, idempotently."""
    ir = load_reference_project()
    props = scene.crowd_project
    props.project_uuid = ir["project_id"]
    props.seed = ir["seed"]
    props.ticks_per_second = ir["clock"]["ticks_per_second"]
    props.reference_fixture_version = "concourse-project-v1"
    if not props.cache_path:
        props.cache_path = str(Path(bpy.app.tempdir) / "crowd-cache")
    props.status = "Reference project created"

    scene.frame_start = ir["clock"]["frame_start"]
    scene.frame_end = ir["clock"]["frame_end"]
    scene.render.fps = ir["clock"]["frames_per_second"]

    text = bpy.data.texts.get(PROJECT_TEXT_NAME)
    if text is None:
        text = bpy.data.texts.new(PROJECT_TEXT_NAME)
    text.clear()
    text.write(json.dumps(ir, sort_keys=True, separators=(",", ":")))
    text["crowd_logical_id"] = "reference_project_ir_v1"

    graph_text = bpy.data.texts.get(BEHAVIOR_GRAPH_TEXT_NAME)
    if graph_text is None:
        graph_text = bpy.data.texts.new(BEHAVIOR_GRAPH_TEXT_NAME)
    graph_text.clear()
    graph_text.write(json.dumps(load_reference_behavior_graph(), sort_keys=True, separators=(",", ":")))
    graph_text["crowd_logical_id"] = "reference_behavior_graph_v1"

    collection = _semantic_collection(scene)
    semantics = ir["semantics"]
    for item in semantics["walkable"]:
        _bounded_object(collection, "walkable", item, "bounds")
    for item in semantics["blocked"]:
        _bounded_object(collection, "blocked", item, "bounds")
    for item in semantics["spawns"]:
        _bounded_object(collection, "spawn", item, "bounds")
    for item in semantics["destinations"]:
        _bounded_object(collection, "destination", item, "capacity_bounds")
    for item in semantics["portals"]:
        _portal_object(collection, item)
    bpy.context.view_layer.update()
    return ir


def _world_bounds_2d(obj):
    corners = [obj.matrix_world @ Vector(corner) for corner in obj.bound_box]
    return {
        "min": [min(point.x for point in corners), min(point.y for point in corners)],
        "max": [max(point.x for point in corners), max(point.y for point in corners)],
    }


def _semantic_map(scene):
    mapped = {}
    for obj in scene.objects:
        entity_type = obj.get("crowd_entity_type")
        logical_id = obj.get("crowd_logical_id")
        if entity_type in _IR_ARRAYS and logical_id:
            key = (entity_type, logical_id)
            if key in mapped:
                raise ValueError(
                    "duplicate semantic object {}:{}".format(entity_type, logical_id)
                )
            mapped[key] = obj
    return mapped


def extract_ir(scene):
    """Extract plain ProjectIrV1 data without deriving meaning from object names."""
    text = bpy.data.texts.get(PROJECT_TEXT_NAME)
    if text is None:
        raise ValueError("reference project has not been created")
    ir = copy.deepcopy(json.loads(text.as_string()))
    props = scene.crowd_project
    ir["project_id"] = props.project_uuid
    ir["seed"] = props.seed
    ir["clock"]["ticks_per_second"] = props.ticks_per_second
    ir["clock"]["frame_start"] = scene.frame_start
    ir["clock"]["frame_end"] = scene.frame_end
    ir["clock"]["frames_per_second"] = scene.render.fps

    semantic_objects = _semantic_map(scene)
    for entity_type, array_name in _IR_ARRAYS.items():
        extracted = []
        for template_item in ir["semantics"][array_name]:
            logical_id = template_item["id"]
            obj = semantic_objects.get((entity_type, logical_id))
            if obj is None:
                raise ValueError(
                    "missing semantic object {}:{}".format(entity_type, logical_id)
                )
            item = json.loads(obj["crowd_ir_fields"])
            item["id"] = logical_id
            if entity_type in _BOUNDED_TYPES:
                item[_BOUNDED_TYPES[entity_type]] = _world_bounds_2d(obj)
            else:
                item["center"] = [obj.location.x, obj.location.y]
            extracted.append(item)
        ir["semantics"][array_name] = extracted
    return ir
