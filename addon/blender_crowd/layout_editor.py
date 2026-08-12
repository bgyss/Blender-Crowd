"""Persisted M2 population layouts and deterministic viewport guide objects."""

import json

import bpy


LAYOUT_COLLECTION_NAME = "Crowd Layout Guides"
_KINDS = {"region", "curve", "formation", "seating"}


def _collection(scene):
    collection = bpy.data.collections.get(LAYOUT_COLLECTION_NAME)
    if collection is None:
        collection = bpy.data.collections.new(LAYOUT_COLLECTION_NAME)
        collection["crowd_logical_id"] = "crowd_layout_guides"
    if collection.name not in scene.collection.children:
        scene.collection.children.link(collection)
    return collection


def extract_layouts(scene):
    """Validate and serialize the UI-owned layout contracts.

    Layouts are authoring guides, not a second simulation: the population and
    semantic source IDs point at the same M2 project data used by the native
    compiler.
    """
    props = scene.crowd_project
    population_ids = {item.logical_id for item in props.populations}
    source_ids = {item.logical_id for item in props.lanes}
    source_ids.update(
        obj.get("crowd_logical_id")
        for obj in scene.objects
        if obj.get("crowd_entity_type") in {"walkable", "spawn", "destination"}
    )
    layouts = []
    seen = set()
    for item in props.layouts:
        if not item.logical_id or item.logical_id in seen:
            raise ValueError("layout needs a unique non-empty ID: {}".format(item.logical_id))
        seen.add(item.logical_id)
        if item.kind not in _KINDS:
            raise ValueError("layout {} has unsupported kind {}".format(item.logical_id, item.kind))
        if item.population_id not in population_ids:
            raise ValueError(
                "layout {} references unknown population {}".format(item.logical_id, item.population_id)
            )
        if item.kind in {"region", "curve"} and item.source_id not in source_ids:
            raise ValueError(
                "layout {} references unknown region or lane {}".format(item.logical_id, item.source_id)
            )
        try:
            points = json.loads(item.points_json)
        except json.JSONDecodeError as error:
            raise ValueError("layout {} has invalid points JSON: {}".format(item.logical_id, error)) from error
        if not isinstance(points, list):
            raise ValueError("layout {} points must be a JSON array".format(item.logical_id))
        layouts.append(
            {
                "id": item.logical_id,
                "kind": item.kind,
                "population_id": item.population_id,
                "source_id": item.source_id,
                "rows": item.rows,
                "columns": item.columns,
                "spacing_x_m": item.spacing_x_m,
                "spacing_y_m": item.spacing_y_m,
                "points": points,
            }
        )
    return layouts


def _positions(layout):
    if layout["kind"] in {"formation", "curve"} and layout["points"]:
        return [(float(point[0]), float(point[1]), 0.0) for point in layout["points"]]
    rows, columns = layout["rows"], layout["columns"]
    return [
        (column * layout["spacing_x_m"], row * layout["spacing_y_m"], 0.0)
        for row in range(rows)
        for column in range(columns)
    ]


def materialize_guides(scene):
    """Create saveable empties for every authored seat or formation point."""
    layouts = extract_layouts(scene)
    collection = _collection(scene)
    wanted = set()
    for layout in layouts:
        for index, position in enumerate(_positions(layout)):
            logical_id = "{}:{}".format(layout["id"], index)
            wanted.add(logical_id)
            guide = next(
                (obj for obj in collection.objects if obj.get("crowd_layout_guide_id") == logical_id),
                None,
            )
            if guide is None:
                guide = bpy.data.objects.new("Crowd Layout {}".format(logical_id), None)
                collection.objects.link(guide)
            guide.empty_display_type = "CUBE"
            guide.empty_display_size = 0.2
            guide.location = position
            guide["crowd_layout_guide_id"] = logical_id
            guide["crowd_layout_id"] = layout["id"]
            guide["crowd_population_id"] = layout["population_id"]
            guide["crowd_layout_kind"] = layout["kind"]
    for guide in list(collection.objects):
        if guide.get("crowd_layout_guide_id") not in wanted:
            bpy.data.objects.remove(guide, do_unlink=True)
    return len(wanted)
