"""Typed Blender properties owned by the narrow M1 project workflow."""

import bpy
from bpy.props import (
    BoolProperty,
    CollectionProperty,
    FloatProperty,
    IntProperty,
    PointerProperty,
    StringProperty,
)
from bpy.types import Object, PropertyGroup


def _signed_u32(value):
    return value if value < (1 << 31) else value - (1 << 32)


def _update_selected_agent_id(self, _context):
    """Keep the UI's decimal stable ID synchronized with the native u32 pair."""
    text = self.selected_agent_id.strip()
    if not text:
        self.selected_agent_id_lo = 0
        self.selected_agent_id_hi = 0
        return
    try:
        value = int(text, 10)
    except ValueError:
        self.status = "Selected Agent ID must be an unsigned decimal integer"
        return
    if not 0 <= value < (1 << 64):
        self.status = "Selected Agent ID must fit an unsigned 64-bit integer"
        return
    self.selected_agent_id_lo = _signed_u32(value & 0xFFFFFFFF)
    self.selected_agent_id_hi = _signed_u32((value >> 32) & 0xFFFFFFFF)


class CrowdQueueProperties(PropertyGroup):
    logical_id: StringProperty(name="Queue ID")
    portal_id: StringProperty(name="Portal")
    admission_capacity: IntProperty(name="Admissions / Tick", min=1, default=1)
    slots_json: StringProperty(name="Slots (JSON)", default="[]")


class CrowdLaneProperties(PropertyGroup):
    logical_id: StringProperty(name="Lane ID")
    strength_millionths: IntProperty(
        name="Strength", min=0, max=1_000_000, default=500_000
    )
    points_json: StringProperty(name="Points (JSON)", default="[]")


class CrowdCostRegionProperties(PropertyGroup):
    logical_id: StringProperty(name="Region ID")
    walkable_id: StringProperty(name="Walkable Region")
    kind: StringProperty(name="Kind", default="interest")
    weight_millionths: IntProperty(
        name="Weight", min=0, max=1_000_000, default=0
    )
    bounds_json: StringProperty(name="Bounds (JSON)", default="{}")


class CrowdPopulationProperties(PropertyGroup):
    logical_id: StringProperty(name="Population ID")
    count: IntProperty(name="Agent Count", min=1, default=1)
    emission_interval_ticks: IntProperty(name="Emission Interval", min=1, default=1)
    spawn_source_ids_json: StringProperty(name="Spawn Sources (JSON)", default="[]")
    destinations_json: StringProperty(name="Destinations (JSON)", default="[]")
    archetypes_json: StringProperty(name="Archetypes (JSON)", default="[]")
    appearances_json: StringProperty(name="Appearances (JSON)", default="[]")


class CrowdRetargetProperties(PropertyGroup):
    logical_id: StringProperty(name="Retarget ID")
    source_rig_id: StringProperty(name="Source Rig")
    root_bone: StringProperty(name="Root Bone")
    forward_axis: StringProperty(name="Forward Axis", default="-Y")
    scale_millimeters: IntProperty(name="Scale (mm)", min=1, default=1000)
    bone_map_json: StringProperty(name="Bone Map (JSON)", default="{}")


class CrowdClipProperties(PropertyGroup):
    logical_id: StringProperty(name="Clip ID")
    retarget_profile_id: StringProperty(name="Retarget Profile")
    duration_ticks: IntProperty(name="Duration (ticks)", min=1, default=30)
    loop_start_tick: IntProperty(name="Loop Start", min=0, default=0)
    loop_end_tick: IntProperty(name="Loop End", min=0, default=29)
    average_root_speed_mmps: IntProperty(name="Root Speed (mm/s)", min=1, default=1350)
    left_foot_contacts_json: StringProperty(name="Left Contacts (JSON)", default="[]")
    right_foot_contacts_json: StringProperty(name="Right Contacts (JSON)", default="[]")


class CrowdVariationProperties(PropertyGroup):
    logical_id: StringProperty(name="Variation ID")
    bodies_json: StringProperty(name="Bodies (JSON)", default="[]")
    clothing_json: StringProperty(name="Clothing (JSON)", default="[]")
    materials_json: StringProperty(name="Materials (JSON)", default="[]")
    props_json: StringProperty(name="Props (JSON)", default="[]")
    clips_json: StringProperty(name="Clips (JSON)", default="[]")


class CrowdLayoutProperties(PropertyGroup):
    logical_id: StringProperty(name="Layout ID")
    kind: StringProperty(name="Layout Kind", default="seating")
    population_id: StringProperty(name="Population")
    source_id: StringProperty(name="Region or Lane", default="")
    rows: IntProperty(name="Rows", min=1, default=1)
    columns: IntProperty(name="Columns", min=1, default=1)
    spacing_x_m: FloatProperty(name="X Spacing (m)", min=0.01, default=0.6)
    spacing_y_m: FloatProperty(name="Y Spacing (m)", min=0.01, default=0.8)
    points_json: StringProperty(name="Points (JSON)", default="[]")


class CrowdGroupProperties(PropertyGroup):
    logical_id: StringProperty(name="Group ID")
    kind: StringProperty(name="Kind", default="couple")
    member_agent_ids_json: StringProperty(name="Stable Agent IDs (JSON)", default="[]")
    leader_agent_id: StringProperty(name="Leader Agent ID (optional)", default="")
    shared_destination_id: StringProperty(name="Shared Destination")
    max_separation_millimeters: IntProperty(
        name="Maximum Separation (mm)", min=1, default=2000
    )
    bottleneck_policy: StringProperty(name="Bottleneck Policy", default="individual")


class CrowdDiagnosticProperties(PropertyGroup):
    sequence: IntProperty(name="Sequence", default=0, min=0)
    severity: StringProperty(name="Severity", default="INFO")
    summary: StringProperty(name="Summary")
    detail: StringProperty(name="Detail")
    filepath: StringProperty(
        name="Affected File",
        subtype="FILE_PATH",
        options={"PATH_SUPPORTS_BLEND_RELATIVE"},
    )
    object_name: StringProperty(name="Affected Object")
    documentation: StringProperty(name="Recovery Documentation")


class CrowdProjectProperties(PropertyGroup):
    project_uuid: StringProperty(name="Project UUID")
    seed: IntProperty(name="Seed", min=0, default=2026)
    ticks_per_second: IntProperty(name="Ticks per Second", min=1, default=30)
    cache_path: StringProperty(
        name="Cache Path",
        subtype="DIR_PATH",
        options={"PATH_SUPPORTS_BLEND_RELATIVE"},
    )
    status: StringProperty(name="Status", default="Not created")
    current_stage: StringProperty(name="Workflow Stage", default="Create or open a Crowd project")
    selection_context: StringProperty(name="Selection Context", default="No Crowd selection")
    next_action: StringProperty(name="Next Action", default="Create Reference Concourse")
    operation_estimate: StringProperty(name="Operation Estimate")
    playback_buffer_estimate: StringProperty(name="Minimum Playback Buffer")
    operation_progress: FloatProperty(name="Operation Progress", min=0.0, max=1.0, default=0.0)
    cache_status: StringProperty(name="Cache Health", default="not_inspected")
    cache_source_hash: StringProperty(name="Cache Source Hash")
    cache_resolved_path: StringProperty(name="Resolved Cache Path", subtype="DIR_PATH")
    cache_readable_range: StringProperty(name="Readable Tick Range", default="none")
    cache_last_complete_tick: IntProperty(name="Last Complete Tick", min=0, default=0)
    cache_valid_chunk_count: IntProperty(name="Valid Cache Chunks", min=0, default=0)
    cache_recovery_hint: StringProperty(name="Cache Recovery", default="Choose a cache to inspect.")
    cache_disk_size: StringProperty(name="Measured Cache Size")
    cache_attached: BoolProperty(name="Authoritative Cache Attached", default=False)
    diagnostics: CollectionProperty(type=CrowdDiagnosticProperties)
    active_diagnostic_index: IntProperty(default=0, min=0)
    diagnostic_sequence: IntProperty(default=0, min=0)
    selected_agent_id: StringProperty(
        name="Selected Agent ID",
        description="Paste the decimal stable agent ID from behavior-v1.json",
        update=_update_selected_agent_id,
    )
    selected_agent_tick: IntProperty(name="Inspected Tick", default=0, min=0)
    selected_agent_behavior_state: StringProperty(name="Behavior State")
    selected_agent_decision_reason: StringProperty(name="Decision Reason")
    selected_agent_graph_id: StringProperty(name="Behavior Graph")
    selected_agent_decisive_node: StringProperty(name="Decisive Node")
    selected_agent_event_count: IntProperty(name="Cached Events at Tick", default=0, min=0)
    selected_agent_id_lo: IntProperty(name="Selected Agent ID Low", default=0)
    selected_agent_id_hi: IntProperty(name="Selected Agent ID High", default=0)
    reference_fixture_version: StringProperty(name="Reference Fixture Version")
    override_tick_start: IntProperty(name="Override Start", min=0, default=30)
    override_tick_end: IntProperty(name="Override End", min=0, default=60)
    override_enabled: BoolProperty(name="Override Enabled", default=True)
    terrain_object: PointerProperty(name="Presentation Terrain", type=Object)
    terrain_max_slope_degrees: FloatProperty(
        name="Maximum Terrain Slope", min=0.0, max=89.9, default=30.0
    )
    queues: CollectionProperty(type=CrowdQueueProperties)
    lanes: CollectionProperty(type=CrowdLaneProperties)
    cost_regions: CollectionProperty(type=CrowdCostRegionProperties)
    populations: CollectionProperty(type=CrowdPopulationProperties)
    retarget_profiles: CollectionProperty(type=CrowdRetargetProperties)
    clips: CollectionProperty(type=CrowdClipProperties)
    variations: CollectionProperty(type=CrowdVariationProperties)
    layouts: CollectionProperty(type=CrowdLayoutProperties)
    groups: CollectionProperty(type=CrowdGroupProperties)
    active_queue_index: IntProperty(default=0, min=0)
    active_lane_index: IntProperty(default=0, min=0)
    active_cost_region_index: IntProperty(default=0, min=0)


_CLASSES = (
    CrowdQueueProperties,
    CrowdLaneProperties,
    CrowdCostRegionProperties,
    CrowdPopulationProperties,
    CrowdRetargetProperties,
    CrowdClipProperties,
    CrowdVariationProperties,
    CrowdLayoutProperties,
    CrowdGroupProperties,
    CrowdDiagnosticProperties,
    CrowdProjectProperties,
)


def register():
    for cls in _CLASSES:
        bpy.utils.register_class(cls)
    bpy.types.Scene.crowd_project = PointerProperty(type=CrowdProjectProperties)


def unregister():
    if hasattr(bpy.types.Scene, "crowd_project"):
        del bpy.types.Scene.crowd_project
    for cls in reversed(_CLASSES):
        bpy.utils.unregister_class(cls)
