"""Selected-agent evidence and simple scene-space path/velocity overlays."""

import json

import bpy


_ACTIVE_EVIDENCE = None


def active_evidence():
    return _ACTIVE_EVIDENCE


def _point3(point):
    if len(point) == 3:
        return tuple(float(value) for value in point)
    return (float(point[0]), float(point[1]), 0.03)


def _ensure_line(logical_id, points):
    mesh = next(
        (
            item
            for item in bpy.data.meshes
            if item.get("crowd_debug_id") == logical_id
        ),
        None,
    )
    if mesh is None:
        mesh = bpy.data.meshes.new("Crowd Debug {}".format(logical_id))
        mesh["crowd_debug_id"] = logical_id
    else:
        mesh.clear_geometry()
    vertices = [_point3(point) for point in points]
    edges = [(index, index + 1) for index in range(max(len(vertices) - 1, 0))]
    mesh.from_pydata(vertices, edges, [])
    mesh.update()

    obj = next(
        (
            item
            for item in bpy.data.objects
            if item.get("crowd_debug_id") == logical_id
        ),
        None,
    )
    if obj is None:
        obj = bpy.data.objects.new("Crowd Debug {}".format(logical_id), mesh)
        obj["crowd_debug_id"] = logical_id
        bpy.context.scene.collection.objects.link(obj)
    elif obj.data is not mesh:
        obj.data = mesh
    obj.show_in_front = True
    obj.hide_render = True
    return obj


def inspect(playback, agent_id, tick):
    global _ACTIVE_EVIDENCE
    evidence = playback.inspect_agent(agent_id, tick)
    trace_json = evidence.pop("decision_trace_json", None)
    if trace_json:
        trace = json.loads(trace_json)
        if isinstance(trace, list):
            evidence["behavior_events"] = trace
            if trace:
                latest = trace[-1]
                evidence["graph_id"] = latest.get("graph_id")
                evidence["decisive_node"] = latest.get("decisive_node")
        elif isinstance(trace, dict):
            evidence.update(trace)
    position = _point3(evidence["position"])
    corridor = evidence.get("corridor_points") or [position]
    _ensure_line("selected_path", corridor)
    desired = evidence.get("desired_velocity", [0.0, 0.0])
    solved = evidence.get("solved_velocity", [0.0, 0.0])
    _ensure_line(
        "desired_velocity",
        [position, (position[0] + desired[0], position[1] + desired[1], position[2])],
    )
    _ensure_line(
        "solved_velocity",
        [position, (position[0] + solved[0], position[1] + solved[1], position[2])],
    )
    _ACTIVE_EVIDENCE = evidence
    return evidence
