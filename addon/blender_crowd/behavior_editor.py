"""Blender node-tree authoring for the bounded M2 behavior graph language."""

import json

import bpy
from bpy.props import EnumProperty, StringProperty
from bpy.types import Node, NodeTree


TREE_NAME = "CrowdBehaviorGraph"
TREE_ID = "CrowdBehaviorTree"
NODE_ID = "CrowdBehaviorNode"

_NODE_TYPES = (
    ("selector", "Selector", "First child with an action"),
    ("sequence", "Sequence", "Run children in order after completion"),
    ("fallback", "Fallback", "Try children in order"),
    ("utility_selector", "Utility Selector", "Choose the highest score"),
    ("state_switch", "State Switch", "Explicit finite state branch"),
    ("interrupt", "Interrupt", "Typed boolean interruption"),
    ("timer", "Timer", "Release child after ticks"),
    ("probability", "Probability", "Deterministic probability branch"),
    ("event", "Event", "Typed event branch"),
    ("blackboard_compare", "Blackboard Compare", "Typed boolean branch"),
    ("navigate", "Navigate", "Set a named destination"),
    ("wait", "Wait", "Hold for ticks"),
    ("queue", "Queue", "Reserve an authored queue slot"),
    ("follow_lane", "Follow Lane", "Follow an authored lane"),
    ("hold_position", "Hold Position", "Stop desired velocity"),
)


class CrowdBehaviorTree(NodeTree):
    bl_idname = TREE_ID
    bl_label = "Crowd Behavior Graph"
    bl_icon = "NODETREE"


class CrowdBehaviorNode(Node):
    bl_idname = NODE_ID
    bl_label = "Crowd Behavior"
    bl_icon = "NODE"

    node_type: EnumProperty(name="Type", items=_NODE_TYPES, default="hold_position")
    logical_id: StringProperty(name="Node ID", default="node")
    payload_json: StringProperty(name="Typed Fields (JSON)", default="{}")

    def init(self, context):
        child = self.inputs.new("NodeSocketFloat", "Children")
        child.link_limit = 4095
        self.outputs.new("NodeSocketFloat", "Flow")

    def draw_buttons(self, context, layout):
        layout.prop(self, "node_type")
        layout.prop(self, "logical_id")
        layout.prop(self, "payload_json")


def _node_payload(spec):
    return {
        key: value
        for key, value in spec.items()
        if key not in {"type", "id", "children"}
    }


def ensure_reference_tree(graph):
    """Create or replace the stable, editable node tree from a graph JSON value."""
    tree = bpy.data.node_groups.get(TREE_NAME)
    if tree is None:
        tree = bpy.data.node_groups.new(TREE_NAME, TREE_ID)
    if tree.bl_idname != TREE_ID:
        raise ValueError("{} exists but is not a Crowd behavior tree".format(TREE_NAME))
    tree.nodes.clear()
    tree.links.clear()
    tree["crowd_graph_id"] = graph["id"]
    tree["crowd_entry_id"] = graph["entry_id"]
    nodes = {}
    for index, spec in enumerate(graph["nodes"]):
        node = tree.nodes.new(NODE_ID)
        node.name = spec["id"]
        node.label = spec["id"]
        node.node_type = spec["type"]
        node.logical_id = spec["id"]
        node.payload_json = json.dumps(_node_payload(spec), sort_keys=True, separators=(",", ":"))
        node.location = (index % 4 * 260, -(index // 4) * 180)
        nodes[spec["id"]] = node
    for spec in graph["nodes"]:
        parent = nodes[spec["id"]]
        for child_id in spec.get("children", []):
            tree.links.new(
                nodes[child_id].outputs["Flow"],
                parent.inputs["Children"],
                verify_limits=False,
            )
    return tree


def graph_from_tree(tree=None):
    """Serialize only bounded node data; Rust remains the authoritative compiler."""
    tree = tree or bpy.data.node_groups.get(TREE_NAME)
    if tree is None:
        raise ValueError("create a Crowd Behavior Graph first")
    if tree.bl_idname != TREE_ID:
        raise ValueError("selected node tree is not a Crowd Behavior Graph")
    graph = {
        "id": tree.get("crowd_graph_id", "behavior_graph"),
        "entry_id": tree.get("crowd_entry_id", ""),
        "nodes": [],
    }
    for node in sorted(tree.nodes, key=lambda item: item.logical_id):
        if node.bl_idname != NODE_ID:
            continue
        try:
            spec = json.loads(node.payload_json)
        except json.JSONDecodeError as error:
            raise ValueError("node {} has invalid typed fields: {}".format(node.logical_id, error)) from error
        spec["type"] = node.node_type
        spec["id"] = node.logical_id
        children = [link.from_node.logical_id for link in node.inputs["Children"].links]
        if children:
            spec["children"] = children
        graph["nodes"].append(spec)
    return graph


def highlight_node(logical_id, tree=None):
    """Select one bounded graph node when an M6 navigation record resolves it."""
    tree = tree or bpy.data.node_groups.get(TREE_NAME)
    if tree is None or tree.bl_idname != TREE_ID:
        return False
    matched = False
    for node in tree.nodes:
        node.select = node.bl_idname == NODE_ID and node.logical_id == logical_id
        if node.select:
            tree.nodes.active = node
            matched = True
    return matched


_CLASSES = (CrowdBehaviorTree, CrowdBehaviorNode)


def register():
    for cls in _CLASSES:
        bpy.utils.register_class(cls)


def unregister():
    for cls in reversed(_CLASSES):
        bpy.utils.unregister_class(cls)
