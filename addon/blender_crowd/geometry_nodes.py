"""Build the crowd instancing node group.

Built in Python rather than shipped as a .blend so it stays reviewable in
diffs and needs no binary fixture. This layer is presentation only: it reads
attributes the Rust side baked and never decides anything.
"""

import bpy

NODE_GROUP_NAME = "CrowdInstances"
MODIFIER_NAME = "CrowdInstances"
CACHE_NODE_GROUP_NAME = "CrowdCacheInstancesV1"
CACHE_MODIFIER_NAME = "CrowdCacheInstancesV1"
CACHE_PROTOTYPE_COLLECTION = "CrowdCachePrototypesV1"

# Share of a clip's limb-swing amplitude that the instanced body leans through.
# A walking person's torso moves; it does not swing as far as their leg.
BODY_LEAN_FRACTION = 0.15


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


def _prototype_collection(prototypes):
    collection = bpy.data.collections.get(CACHE_PROTOTYPE_COLLECTION)
    if collection is None:
        collection = bpy.data.collections.new(CACHE_PROTOTYPE_COLLECTION)
        collection["crowd_logical_id"] = "cache_prototypes_v1"
    for prototype in prototypes:
        if prototype.name not in collection.objects:
            collection.objects.link(prototype)
    return collection


def _named_attribute(nodes, name, data_type, location):
    node = nodes.new("GeometryNodeInputNamedAttribute")
    node.data_type = data_type
    node.inputs["Name"].default_value = name
    node.location = location
    return node


def clip_swing_amplitudes(clips):
    """Per-clip swing amplitudes in radians, indexed by clip ID."""
    return [float(clip["swing_radians"]) for clip in clips]


def ensure_cache_node_group(prototypes, amplitudes):
    """Build the versioned cache-only instancing contract."""
    existing = bpy.data.node_groups.get(CACHE_NODE_GROUP_NAME)
    if existing is not None:
        return existing

    group = bpy.data.node_groups.new(CACHE_NODE_GROUP_NAME, "GeometryNodeTree")
    group["crowd_contract_version"] = 1
    group.interface.new_socket(
        "Geometry", in_out="INPUT", socket_type="NodeSocketGeometry"
    )
    group.interface.new_socket(
        "Geometry", in_out="OUTPUT", socket_type="NodeSocketGeometry"
    )
    nodes = group.nodes
    links = group.links
    group_in = nodes.new("NodeGroupInput")
    group_in.location = (-1050, 0)
    group_out = nodes.new("NodeGroupOutput")
    group_out.location = (850, 0)

    collection_info = nodes.new("GeometryNodeCollectionInfo")
    collection_info.name = "M1 Procedural Commuter Prototypes"
    collection_info.location = (-400, -320)
    collection_info.inputs["Collection"].default_value = _prototype_collection(prototypes)
    collection_info.inputs["Separate Children"].default_value = True
    collection_info.inputs["Reset Children"].default_value = False

    visible = _named_attribute(
        nodes, "crowd_visible", "INT", (-1000, 420)
    )
    is_visible = nodes.new("FunctionNodeCompare")
    is_visible.data_type = "INT"
    is_visible.operation = "NOT_EQUAL"
    is_visible.inputs["B"].default_value = 0
    is_visible.location = (-800, 420)
    links.new(visible.outputs["Attribute"], is_visible.inputs["A"])

    tier = _named_attribute(nodes, "crowd_render_tier", "INT", (-1000, 260))
    supported_tier = nodes.new("FunctionNodeCompare")
    supported_tier.data_type = "INT"
    supported_tier.operation = "LESS_EQUAL"
    supported_tier.inputs["B"].default_value = 2
    supported_tier.location = (-800, 260)
    links.new(tier.outputs["Attribute"], supported_tier.inputs["A"])

    selection = nodes.new("FunctionNodeBooleanMath")
    selection.operation = "AND"
    selection.location = (-580, 360)
    links.new(is_visible.outputs["Result"], selection.inputs[0])
    links.new(supported_tier.outputs["Result"], selection.inputs[1])

    phase = _named_attribute(nodes, "crowd_clip_phase", "FLOAT", (-1000, 80))
    phase_angle = nodes.new("ShaderNodeMath")
    phase_angle.operation = "MULTIPLY"
    phase_angle.inputs[1].default_value = 6.283185307179586
    phase_angle.location = (-800, 80)
    links.new(phase.outputs["Attribute"], phase_angle.inputs[0])
    phase_sine = nodes.new("ShaderNodeMath")
    phase_sine.name = "M1 Clip Phase Sine"
    phase_sine.operation = "SINE"
    phase_sine.location = (-600, 80)
    links.new(phase_angle.outputs[0], phase_sine.inputs[0])

    clip = _named_attribute(nodes, "crowd_clip_id", "INT", (-1000, -80))
    # Each clip declares its own swing amplitude in radians, and idle declares
    # zero, so the amplitude comes from the manifest rather than from a
    # moving/not-moving test with an implied amplitude of one radian.
    clip_index = nodes.new("ShaderNodeMath")
    clip_index.operation = "MINIMUM"
    clip_index.inputs[1].default_value = float(max(len(amplitudes) - 1, 0))
    clip_index.location = (-870, -80)
    links.new(clip.outputs["Attribute"], clip_index.inputs[0])
    clip_floor = nodes.new("ShaderNodeMath")
    clip_floor.operation = "MAXIMUM"
    clip_floor.inputs[1].default_value = 0.0
    clip_floor.location = (-700, -80)
    links.new(clip_index.outputs[0], clip_floor.inputs[0])

    amplitude = nodes.new("GeometryNodeIndexSwitch")
    amplitude.name = "M1 Clip Swing Amplitude"
    amplitude.data_type = "FLOAT"
    amplitude.location = (-540, -80)
    while len(amplitude.index_switch_items) < len(amplitudes):
        amplitude.index_switch_items.new()
    links.new(clip_floor.outputs[0], amplitude.inputs["Index"])
    for offset, radians in enumerate(amplitudes):
        amplitude.inputs[offset + 1].default_value = radians

    swing = nodes.new("ShaderNodeMath")
    swing.name = "M1 Walk Jog Proxy Swing"
    swing.operation = "MULTIPLY"
    swing.location = (-380, 60)
    links.new(phase_sine.outputs[0], swing.inputs[0])
    links.new(amplitude.outputs[0], swing.inputs[1])

    store_swing = nodes.new("GeometryNodeStoreNamedAttribute")
    store_swing.domain = "POINT"
    store_swing.data_type = "FLOAT"
    store_swing.inputs["Name"].default_value = "crowd_proxy_swing"
    store_swing.location = (-300, 300)
    links.new(group_in.outputs[0], store_swing.inputs["Geometry"])
    links.new(swing.outputs[0], store_swing.inputs["Value"])

    variant = _named_attribute(nodes, "crowd_variant_id", "INT", (-390, -160))
    variant_index = nodes.new("ShaderNodeMath")
    variant_index.operation = "MODULO"
    variant_index.inputs[1].default_value = float(max(len(prototypes), 1))
    variant_index.location = (-180, -160)
    links.new(variant.outputs["Attribute"], variant_index.inputs[0])

    orientation = _named_attribute(
        nodes, "crowd_orientation", "FLOAT", (-390, -500)
    )
    # The manifest amplitude is a limb swing: the canonical rig swings a limb
    # through it, and crowd_proxy_swing carries it for anything that wants the
    # same value. Leaning a whole body through it instead would tip a walking
    # commuter 32 degrees and a jogging one 52, so the body gets a fraction of
    # it, which is what reads as gait at instance scale.
    body_lean = nodes.new("ShaderNodeMath")
    body_lean.name = "M1 Proxy Body Lean"
    body_lean.operation = "MULTIPLY"
    body_lean.inputs[1].default_value = BODY_LEAN_FRACTION
    body_lean.location = (-300, -430)
    links.new(swing.outputs[0], body_lean.inputs[0])

    rotation = nodes.new("ShaderNodeCombineXYZ")
    rotation.location = (-120, -430)
    links.new(body_lean.outputs[0], rotation.inputs["X"])
    links.new(orientation.outputs["Attribute"], rotation.inputs["Z"])

    scale_value = _named_attribute(nodes, "crowd_scale", "FLOAT", (-390, -650))
    scale = nodes.new("ShaderNodeCombineXYZ")
    scale.location = (-120, -610)
    for axis in ("X", "Y", "Z"):
        links.new(scale_value.outputs["Attribute"], scale.inputs[axis])

    instance = nodes.new("GeometryNodeInstanceOnPoints")
    instance.name = "M1 Variant Instances"
    instance.location = (220, 100)
    instance.inputs["Pick Instance"].default_value = True
    links.new(store_swing.outputs["Geometry"], instance.inputs["Points"])
    links.new(selection.outputs["Boolean"], instance.inputs["Selection"])
    links.new(collection_info.outputs["Instances"], instance.inputs["Instance"])
    links.new(variant_index.outputs[0], instance.inputs["Instance Index"])
    links.new(rotation.outputs["Vector"], instance.inputs["Rotation"])
    links.new(scale.outputs["Vector"], instance.inputs["Scale"])
    links.new(instance.outputs["Instances"], group_out.inputs[0])
    return group


def attach_cache(obj, prototypes, clips):
    modifier = obj.modifiers.get(CACHE_MODIFIER_NAME)
    if modifier is None:
        modifier = obj.modifiers.new(CACHE_MODIFIER_NAME, "NODES")
    modifier.node_group = ensure_cache_node_group(
        prototypes, clip_swing_amplitudes(clips)
    )
    return modifier
