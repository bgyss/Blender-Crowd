"""Pure data model for the M6 brain and motion debugger.

The Blender panel is intentionally a thin renderer of these summaries. Keeping
trace navigation and degraded-evidence rules here makes them testable without
starting Blender and keeps UI code from becoming an alternate runtime.
"""


def build_trace_summary(trace, selected_agent_id, tier):
    if int(trace.get("agent_id", -1)) != int(selected_agent_id):
        raise ValueError("trace belongs to another stable agent")
    utility_scores = [
        {"option": str(option), "score": int(score)}
        for option, score in trace.get("utility_scores", [])
    ]
    unavailable = []
    if tier in {"background", "distant", "S3"}:
        unavailable.extend(["utility scores", "blackboard changes", "contact diagnostics"])
    if tier in {"background", "S3"}:
        unavailable.append("group context")
    degraded = trace.get("degraded_evidence") or "full evidence"
    if trace.get("degraded_evidence") and "degraded evidence" not in unavailable:
        unavailable.append("full evidence")
    return {
        "agent_id": int(trace["agent_id"]),
        "tick": int(trace.get("tick", 0)),
        "current_graph_state": {
            "graph_id": trace.get("graph_id", ""),
            "visited_nodes": list(trace.get("visited_nodes", [])),
        },
        "decisive_node": trace.get("decisive_node"),
        "observations": list(trace.get("observations", [])),
        "utility_scores": utility_scores,
        "blackboard_changes": list(
            trace.get("blackboard_changes", trace.get("blackboard_values", []))
        ),
        "interrupts": list(trace.get("interrupts", [])),
        "group_context": dict(trace.get("group_context", {})),
        "contact_diagnostics": list(trace.get("contact_diagnostics", [])),
        "layer_ownership": trace.get("layer_ownership", "base cache"),
        "degraded_evidence": degraded,
        "unavailable_evidence": sorted(set(unavailable)),
    }


def search_graph(graph, query):
    nodes = list(graph.get("nodes", []))
    normalized = str(query).strip().lower()
    matches = [
        node.get("id", "")
        for node in nodes
        if normalized in str(node.get("id", "")).lower()
        or normalized in str(node.get("kind", "")).lower()
    ]
    if not matches:
        return {"matches": [], "highlight_path": []}
    by_id = {node.get("id"): node for node in nodes}
    entry = nodes[0].get("id")
    path = _path_to_match(by_id, entry, matches[0], [])
    while path:
        node = by_id.get(path[-1], {})
        child_ids = list(node.get("children", []))
        if node.get("child"):
            child_ids.append(node["child"])
        next_child = next((child for child in child_ids if child in by_id), None)
        if not next_child:
            break
        path.append(next_child)
    return {"matches": sorted(matches), "highlight_path": path}


def _path_to_match(by_id, current_id, target_id, path):
    if current_id in path:
        return []
    path = path + [current_id]
    if current_id == target_id:
        return path
    node = by_id.get(current_id, {})
    children = list(node.get("children", []))
    if node.get("child"):
        children.append(node["child"])
    for child in children:
        if child in by_id:
            result = _path_to_match(by_id, child, target_id, path)
            if result:
                return result
    return []
