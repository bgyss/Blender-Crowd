"""Reference-project creation and typed Blender-to-ProjectIrV1 extraction."""

import copy
import json
from pathlib import Path

import bpy
from mathutils import Vector

from . import behavior_editor, layout_editor


PROJECT_TEXT_NAME = "CrowdProjectIrV1"
SEMANTIC_COLLECTION_NAME = "Crowd Project Semantics"
SEMANTIC_BOX_MESH_ID = "crowd_semantic_unit_box"
REFERENCE_PROJECT_FILE = "concourse-project-v1.json"
BEHAVIOR_GRAPH_TEXT_NAME = "CrowdBehaviorGraphV1"
REFERENCE_BEHAVIOR_GRAPH_FILE = "leave-concourse-v1.json"
REFERENCE_AUTHORING_FILE = "concourse-authoring-v2.json"
REFERENCE_AUTHORABLE_ASSETS_FILE = "commuter-authorable-assets-v1.json"
REFERENCE_LAYOUTS_FILE = "concourse-layouts-v1.json"

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


def load_reference_authorable_assets():
    with _reference_path(REFERENCE_AUTHORABLE_ASSETS_FILE).open(encoding="utf-8") as handle:
        return json.load(handle)


def load_reference_layouts():
    with _reference_path(REFERENCE_LAYOUTS_FILE).open(encoding="utf-8") as handle:
        return json.load(handle)


def _set_authoring_semantics(scene, semantics):
    """Materialize M2 semantic inputs in saveable Blender PropertyGroups."""
    props = scene.crowd_project
    props.queues.clear()
    props.lanes.clear()
    props.cost_regions.clear()
    for item in semantics.get("queues", []):
        queue = props.queues.add()
        queue.logical_id = item["id"]
        queue.portal_id = item["portal_id"]
        queue.admission_capacity = item["admission_capacity"]
        queue.slots_json = json.dumps(item["slots"], separators=(",", ":"))
    for item in semantics.get("lanes", []):
        lane = props.lanes.add()
        lane.logical_id = item["id"]
        lane.strength_millionths = item["strength_millionths"]
        lane.points_json = json.dumps(item["points"], separators=(",", ":"))
    for item in semantics.get("cost_regions", []):
        region = props.cost_regions.add()
        region.logical_id = item["id"]
        region.walkable_id = item["walkable_id"]
        region.kind = item["kind"]
        region.weight_millionths = item["weight_millionths"]
        region.bounds_json = json.dumps(item["bounds"], separators=(",", ":"))


def _set_populations(scene, populations):
    props = scene.crowd_project
    props.populations.clear()
    for item in populations:
        population = props.populations.add()
        population.logical_id = item["id"]
        population.count = item["count"]
        population.emission_interval_ticks = item["emission_interval_ticks"]
        for field in ("spawn_source_ids", "destinations", "archetypes", "appearances"):
            setattr(population, "{}_json".format(field), json.dumps(item[field], separators=(",", ":")))


def _set_authorable_assets(scene, assets):
    """Materialize asset, retarget, and variation contracts in Blender data."""
    props = scene.crowd_project
    props.retarget_profiles.clear()
    props.clips.clear()
    props.variations.clear()
    for item in assets.get("retarget_profiles", []):
        profile = props.retarget_profiles.add()
        profile.logical_id = item["id"]
        profile.source_rig_id = item["source_rig_id"]
        profile.root_bone = item["root_bone"]
        profile.forward_axis = item["forward_axis"]
        profile.scale_millimeters = item["scale_millimeters"]
        profile.bone_map_json = json.dumps(item["bone_map"], separators=(",", ":"))
    for item in assets.get("clips", []):
        clip = props.clips.add()
        clip.logical_id = item["id"]
        clip.retarget_profile_id = item["retarget_profile_id"]
        clip.duration_ticks = item["duration_ticks"]
        clip.loop_start_tick = item["loop_start_tick"]
        clip.loop_end_tick = item["loop_end_tick"]
        clip.average_root_speed_mmps = item["average_root_speed_mmps"]
        clip.left_foot_contacts_json = json.dumps(item["left_foot_contacts"], separators=(",", ":"))
        clip.right_foot_contacts_json = json.dumps(item["right_foot_contacts"], separators=(",", ":"))
    for item in assets.get("variations", []):
        variation = props.variations.add()
        variation.logical_id = item["id"]
        for field in ("bodies", "clothing", "materials", "props", "clips"):
            setattr(variation, "{}_json".format(field), json.dumps(item[field], separators=(",", ":")))


def _set_layouts(scene, layouts):
    props = scene.crowd_project
    props.layouts.clear()
    for item in layouts:
        layout = props.layouts.add()
        layout.logical_id = item["id"]
        layout.kind = item["kind"]
        layout.population_id = item["population_id"]
        layout.source_id = item["source_id"]
        layout.rows = item["rows"]
        layout.columns = item["columns"]
        layout.spacing_x_m = item["spacing_x_m"]
        layout.spacing_y_m = item["spacing_y_m"]
        layout.points_json = json.dumps(item["points"], separators=(",", ":"))


def set_reference_groups(scene, agent_ids):
    """Assign the reference pair after the base IR has produced stable IDs."""
    if len(agent_ids) < 2:
        raise ValueError("reference project needs two stable agents for its group")
    props = scene.crowd_project
    props.groups.clear()
    group = props.groups.add()
    group.logical_id = "reference_pair"
    group.kind = "couple"
    group.member_agent_ids_json = json.dumps(list(agent_ids[:2]), separators=(",", ":"))
    group.shared_destination_id = "east_exit"
    group.max_separation_millimeters = 2000
    group.bottleneck_policy = "leader_first"


def extract_authoring_semantics(scene):
    """Return UI-authored M2 semantics; malformed fields fail before a bake."""
    props = scene.crowd_project
    try:
        queues = [
            {
                "id": item.logical_id,
                "portal_id": item.portal_id,
                "slots": json.loads(item.slots_json),
                "admission_capacity": item.admission_capacity,
            }
            for item in props.queues
        ]
        lanes = [
            {
                "id": item.logical_id,
                "points": json.loads(item.points_json),
                "strength_millionths": item.strength_millionths,
            }
            for item in props.lanes
        ]
        cost_regions = [
            {
                "id": item.logical_id,
                "walkable_id": item.walkable_id,
                "bounds": json.loads(item.bounds_json),
                "kind": item.kind,
                "weight_millionths": item.weight_millionths,
            }
            for item in props.cost_regions
        ]
    except json.JSONDecodeError as error:
        raise ValueError("M2 semantic JSON field is invalid: {}".format(error)) from error
    return {"queues": queues, "lanes": lanes, "cost_regions": cost_regions}


def extract_authorable_groups(scene):
    """Return persisted social constraints with explicit stable agent IDs."""
    try:
        return [
            {
                "id": item.logical_id,
                "kind": item.kind,
                "member_agent_ids": json.loads(item.member_agent_ids_json),
                "leader_agent_id": int(item.leader_agent_id)
                if item.leader_agent_id.strip()
                else None,
                "shared_destination_id": item.shared_destination_id,
                "max_separation_millimeters": item.max_separation_millimeters,
                "bottleneck_policy": item.bottleneck_policy,
            }
            for item in scene.crowd_project.groups
        ]
    except (TypeError, ValueError, json.JSONDecodeError) as error:
        raise ValueError("M2 group editor contains invalid stable agent IDs: {}".format(error)) from error


def extract_authorable_assets(scene):
    """Return the checked asset library authored in the current Blender scene."""
    props = scene.crowd_project
    try:
        retarget_profiles = [
            {
                "id": item.logical_id,
                "source_rig_id": item.source_rig_id,
                "root_bone": item.root_bone,
                "forward_axis": item.forward_axis,
                "scale_millimeters": item.scale_millimeters,
                "bone_map": json.loads(item.bone_map_json),
            }
            for item in props.retarget_profiles
        ]
        clips = [
            {
                "id": item.logical_id,
                "retarget_profile_id": item.retarget_profile_id,
                "duration_ticks": item.duration_ticks,
                "loop_start_tick": item.loop_start_tick,
                "loop_end_tick": item.loop_end_tick,
                "average_root_speed_mmps": item.average_root_speed_mmps,
                "left_foot_contacts": json.loads(item.left_foot_contacts_json),
                "right_foot_contacts": json.loads(item.right_foot_contacts_json),
            }
            for item in props.clips
        ]
        variations = []
        for item in props.variations:
            variation = {"id": item.logical_id}
            for field in ("bodies", "clothing", "materials", "props", "clips"):
                variation[field] = json.loads(getattr(item, "{}_json".format(field)))
            variations.append(variation)
    except json.JSONDecodeError as error:
        raise ValueError("M2 asset JSON field is invalid: {}".format(error)) from error
    return {
        "retarget_profiles": retarget_profiles,
        "clips": clips,
        "variations": variations,
    }


def behavior_graph_json():
    """Return bounded node-tree data, falling back to the checked graph text."""
    tree = bpy.data.node_groups.get(behavior_editor.TREE_NAME)
    if tree is not None:
        return json.dumps(behavior_editor.graph_from_tree(tree), sort_keys=True, separators=(",", ":"))
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
    _set_authoring_semantics(scene, load_reference_authoring_semantics())
    _set_populations(scene, ir["populations"])
    _set_authorable_assets(scene, load_reference_authorable_assets())
    _set_layouts(scene, load_reference_layouts())

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
    behavior_editor.ensure_reference_tree(load_reference_behavior_graph())

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
    layout_editor.materialize_guides(scene)
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
    population_by_id = {item.logical_id: item for item in props.populations}
    for population in ir["populations"]:
        editor = population_by_id.get(population["id"])
        if editor is None:
            raise ValueError("missing population editor for {}".format(population["id"]))
        population["count"] = editor.count
        population["emission_interval_ticks"] = editor.emission_interval_ticks
        try:
            for field in ("spawn_source_ids", "destinations", "archetypes", "appearances"):
                population[field] = json.loads(getattr(editor, "{}_json".format(field)))
        except json.JSONDecodeError as error:
            raise ValueError("population {} has invalid JSON: {}".format(editor.logical_id, error)) from error

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
