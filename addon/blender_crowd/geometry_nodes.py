"""Build the crowd instancing node group.

Built in Python rather than shipped as a .blend so it stays reviewable in
diffs and needs no binary fixture. This layer is presentation only: it reads
attributes the Rust side baked and never decides anything.
"""

import bpy

NODE_GROUP_NAME = "CrowdInstances"
MODIFIER_NAME = "CrowdInstances"


def ensure_crowd_node_group():
    """Return the crowd instancing node group, creating it if absent."""
    existing = bpy.data.node_groups.get(NODE_GROUP_NAME)
    if existing is not None:
        return existing

    group = bpy.data.node_groups.new(NODE_GROUP_NAME, "GeometryNodeTree")
    group.interface.new_socket(
        "Geometry", in_out="INPUT", socket_type="NodeSocketGeometry"
    )
    group.interface.new_socket(
        "Geometry", in_out="OUTPUT", socket_type="NodeSocketGeometry"
    )

    nodes = group.nodes
    links = group.links

    group_in = nodes.new("NodeGroupInput")
    group_in.location = (-600, 0)
    group_out = nodes.new("NodeGroupOutput")
    group_out.location = (600, 0)

    # A cone is a stand-in for a character: it has an obvious facing
    # direction, so a wrong orientation is visible at a glance.
    cone = nodes.new("GeometryNodeMeshCone")
    cone.location = (-300, -200)
    cone.inputs["Radius Bottom"].default_value = 0.25
    cone.inputs["Depth"].default_value = 1.7

    instance = nodes.new("GeometryNodeInstanceOnPoints")
    instance.location = (0, 0)

    orientation = nodes.new("GeometryNodeInputNamedAttribute")
    orientation.location = (-300, 200)
    orientation.data_type = "FLOAT"
    orientation.inputs["Name"].default_value = "orientation"

    combine = nodes.new("ShaderNodeCombineXYZ")
    combine.location = (-150, 200)

    # flags == 0 means the slot has no agent yet (agents spawn gradually, so
    # early ticks are mostly unspawned/padded slots). Per the trace format
    # contract in crowd-trace, such records must be hidden rather than
    # rendered as an agent standing at the origin.
    flags = nodes.new("GeometryNodeInputNamedAttribute")
    flags.location = (-300, 400)
    flags.data_type = "INT"
    flags.inputs["Name"].default_value = "flags"

    is_spawned = nodes.new("FunctionNodeCompare")
    is_spawned.location = (-150, 400)
    is_spawned.data_type = "INT"
    is_spawned.operation = "NOT_EQUAL"
    is_spawned.inputs["B"].default_value = 0

    links.new(group_in.outputs[0], instance.inputs["Points"])
    links.new(cone.outputs["Mesh"], instance.inputs["Instance"])
    links.new(orientation.outputs["Attribute"], combine.inputs["Z"])
    links.new(combine.outputs["Vector"], instance.inputs["Rotation"])
    links.new(flags.outputs["Attribute"], is_spawned.inputs["A"])
    links.new(is_spawned.outputs["Result"], instance.inputs["Selection"])
    links.new(instance.outputs["Instances"], group_out.inputs[0])

    return group


def attach(obj):
    """Attach the crowd node group to `obj`, reusing an existing modifier."""
    modifier = obj.modifiers.get(MODIFIER_NAME)
    if modifier is None:
        modifier = obj.modifiers.new(MODIFIER_NAME, "NODES")
    modifier.node_group = ensure_crowd_node_group()
    return modifier
