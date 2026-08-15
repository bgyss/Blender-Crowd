"""The deliberately narrow M1 project panel."""

import bpy
from bpy.types import Panel, UIList


class CROWD_UL_diagnostics(UIList):
    """Short, keyboard-navigable diagnostic history."""

    def draw_item(self, _context, layout, _data, item, _icon, _active_data, _active_propname, _index):
        icon = {"ERROR": "ERROR", "WARNING": "ERROR", "INFO": "INFO"}.get(item.severity, "QUESTION")
        layout.label(text="{}: {}".format(item.severity.title(), item.summary), icon=icon)


class CROWD_UL_m4_layers(UIList):
    """Dense layer rows expose composition order and affect scope at a glance."""

    def draw_item(self, _context, layout, _data, item, _icon, _active_data, _active_propname, _index):
        icon = "HIDE_ON" if item.muted else "RESTRICT_VIEW_OFF"
        layout.label(text="{} · {} · {}".format(item.kind, item.layer_id, item.target_summary), icon=icon)


class CROWD_PT_workflow(Panel):
    bl_label = "Crowd Workflow"
    bl_idname = "CROWD_PT_workflow"
    bl_space_type = "PROPERTIES"
    bl_region_type = "WINDOW"
    bl_context = "scene"

    def draw(self, context):
        layout = self.layout
        props = context.scene.crowd_project
        layout.label(text="Stage: {}".format(props.current_stage), icon="WORKSPACE")
        layout.label(text="Selection: {}".format(props.selection_context), icon="RESTRICT_SELECT_OFF")
        layout.label(text="Next: {}".format(props.next_action), icon="FORWARD")
        if props.operation_estimate:
            layout.label(text=props.operation_estimate, icon="TIME")
            layout.prop(props, "operation_progress", slider=True, text="Progress")
        layout.separator()
        row = layout.row(align=True)
        row.operator("crowd.create_reference_project", text="Create")
        row.operator("crowd.validate_project", text="Validate")
        row.operator("crowd.bake_cache", text="Bake")
        row.operator("crowd.cancel_bake", text="Cancel")
        layout.separator()
        layout.prop(props, "cache_path")
        row = layout.row(align=True)
        row.operator("crowd.inspect_cache_health", text="Inspect Health")
        row.operator("crowd.attach_cache", text="Attach Complete Cache")
        box = layout.box()
        box.label(text="Cache: {}".format(props.cache_status))
        if props.cache_source_hash:
            box.label(text="Source: {}".format(props.cache_source_hash[:12]))
        box.label(text="Readable ticks: {}".format(props.cache_readable_range))
        if props.cache_disk_size:
            box.label(text="Measured size: {}".format(props.cache_disk_size))
        box.label(text=props.cache_recovery_hint)
        if props.cache_resolved_path:
            box.label(text="Artifact: {}".format(props.cache_resolved_path))
        layout.separator()
        layout.label(text="Diagnostic History", icon="CONSOLE")
        layout.template_list(
            "CROWD_UL_diagnostics", "", props, "diagnostics", props, "active_diagnostic_index", rows=4
        )
        if props.diagnostics and props.active_diagnostic_index < len(props.diagnostics):
            item = props.diagnostics[props.active_diagnostic_index]
            detail = layout.box()
            detail.label(text=item.detail or "No further detail")
            if item.filepath:
                detail.label(text="File: {}".format(item.filepath))
            if item.object_name:
                detail.label(text="Object: {}".format(item.object_name))
            detail.label(text="Help: {}".format(item.documentation))
        layout.operator("crowd.write_support_bundle", text="Write Safe Support Bundle", icon="FILE_TEXT")


class CROWD_PT_project(Panel):
    bl_label = "Crowd Authoring (Advanced)"
    bl_idname = "CROWD_PT_project"
    bl_space_type = "PROPERTIES"
    bl_region_type = "WINDOW"
    bl_context = "scene"
    bl_options = {"DEFAULT_CLOSED"}

    def draw(self, context):
        layout = self.layout
        props = context.scene.crowd_project
        layout.label(text=props.status)
        layout.prop(props, "project_uuid")
        row = layout.row(align=True)
        row.prop(props, "seed")
        row.prop(props, "ticks_per_second")
        layout.prop(props, "cache_path")
        layout.operator("crowd.create_reference_project")
        layout.operator("crowd.validate_project")
        layout.operator("crowd.validate_behavior_graph")
        layout.operator("crowd.validate_authorable_project")
        layout.separator()
        layout.label(text="M2 Populations")
        row = layout.row(align=True)
        row.operator("crowd.add_population", text="Add Population")
        row.operator("crowd.remove_population", text="Remove Last")
        for population in props.populations:
            box = layout.box()
            box.prop(population, "logical_id")
            box.prop(population, "count")
            box.prop(population, "emission_interval_ticks")
            box.prop(population, "spawn_source_ids_json")
            box.prop(population, "destinations_json")
            box.prop(population, "archetypes_json")
            box.prop(population, "appearances_json")
        layout.separator()
        layout.label(text="M2 Assets, Retargeting, and Variation")
        row = layout.row(align=True)
        for entity_type, label in (
            ("retarget_profile", "Retarget"),
            ("clip", "Clip"),
            ("variation", "Variation"),
        ):
            add = row.operator("crowd.add_m2_asset", text="Add " + label)
            add.entity_type = entity_type
            remove = row.operator("crowd.remove_m2_asset", text="-")
            remove.entity_type = entity_type
        for profile in props.retarget_profiles:
            box = layout.box()
            box.label(text="Retarget Profile")
            box.prop(profile, "logical_id")
            box.prop(profile, "source_rig_id")
            box.prop(profile, "root_bone")
            box.prop(profile, "forward_axis")
            box.prop(profile, "scale_millimeters")
            box.prop(profile, "bone_map_json")
        for clip in props.clips:
            box = layout.box()
            box.label(text="Locomotion Clip")
            box.prop(clip, "logical_id")
            box.prop(clip, "retarget_profile_id")
            box.prop(clip, "duration_ticks")
            box.prop(clip, "loop_start_tick")
            box.prop(clip, "loop_end_tick")
            box.prop(clip, "average_root_speed_mmps")
            box.prop(clip, "left_foot_contacts_json")
            box.prop(clip, "right_foot_contacts_json")
        for variation in props.variations:
            box = layout.box()
            box.label(text="Weighted Variation")
            box.prop(variation, "logical_id")
            box.prop(variation, "bodies_json")
            box.prop(variation, "clothing_json")
            box.prop(variation, "materials_json")
            box.prop(variation, "props_json")
            box.prop(variation, "clips_json")
        layout.separator()
        layout.label(text="M2 Layouts")
        row = layout.row(align=True)
        for entity_type, label in (
            ("region", "Region"),
            ("curve", "Curve"),
            ("formation", "Formation"),
            ("seating", "Seating"),
        ):
            add = row.operator("crowd.add_layout", text="Add " + label)
            add.entity_type = entity_type
        row = layout.row(align=True)
        row.operator("crowd.remove_layout", text="Remove Last")
        row.operator("crowd.materialize_layout_guides")
        for item in props.layouts:
            box = layout.box()
            box.prop(item, "logical_id")
            box.prop(item, "kind")
            box.prop(item, "population_id")
            box.prop(item, "source_id")
            box.prop(item, "rows")
            box.prop(item, "columns")
            box.prop(item, "spacing_x_m")
            box.prop(item, "spacing_y_m")
            box.prop(item, "points_json")
        layout.separator()
        layout.label(text="M2 Environment")
        row = layout.row(align=True)
        for entity_type, label in (("queue", "Queue"), ("lane", "Lane"), ("cost_region", "Region")):
            add = row.operator("crowd.add_m2_semantic", text="Add " + label)
            add.entity_type = entity_type
            remove = row.operator("crowd.remove_m2_semantic", text="-")
            remove.entity_type = entity_type
        for queue in props.queues:
            box = layout.box()
            box.prop(queue, "logical_id")
            box.prop(queue, "portal_id")
            box.prop(queue, "admission_capacity")
            box.prop(queue, "slots_json")
        for lane in props.lanes:
            box = layout.box()
            box.prop(lane, "logical_id")
            box.prop(lane, "strength_millionths")
            box.prop(lane, "points_json")
        for region in props.cost_regions:
            box = layout.box()
            box.prop(region, "logical_id")
            box.prop(region, "walkable_id")
            box.prop(region, "kind")
            box.prop(region, "weight_millionths")
            box.prop(region, "bounds_json")
        layout.separator()
        layout.label(text="M2 Social Groups")
        row = layout.row(align=True)
        row.operator("crowd.add_group", text="Add Group")
        row.operator("crowd.remove_group", text="Remove Last")
        for group in props.groups:
            box = layout.box()
            box.prop(group, "logical_id")
            box.prop(group, "kind")
            box.prop(group, "member_agent_ids_json")
            box.prop(group, "leader_agent_id")
            box.prop(group, "shared_destination_id")
            box.prop(group, "max_separation_millimeters")
            box.prop(group, "bottleneck_policy")
        row = layout.row(align=True)
        row.operator("crowd.bake_cache")
        row.operator("crowd.cancel_bake")
        layout.operator("crowd.attach_cache")
        layout.prop(props, "terrain_object")
        layout.prop(props, "terrain_max_slope_degrees")
        layout.operator("crowd.apply_terrain_presentation")
        layout.separator()
        layout.label(text="Selected Agent Debug")
        layout.prop(props, "selected_agent_id")
        box = layout.box()
        box.label(text="Inspect to refresh cached evidence")
        box.prop(props, "selected_agent_tick")
        box.prop(props, "selected_agent_behavior_state")
        box.prop(props, "selected_agent_decision_reason")
        box.prop(props, "selected_agent_graph_id")
        box.prop(props, "selected_agent_decisive_node")
        box.prop(props, "selected_agent_event_count")
        row = layout.row(align=True)
        row.prop(props, "override_tick_start")
        row.prop(props, "override_tick_end")
        layout.prop(props, "override_enabled")
        layout.operator("crowd.inspect_agent")
        layout.operator("crowd.pin_selected_agent")
        layout.operator("crowd.render_reference_frame")


class CROWD_PT_m5_scale_profile(Panel):
    bl_label = "M5 Scale and Profiling"
    bl_idname = "CROWD_PT_m5_scale_profile"
    bl_space_type = "PROPERTIES"
    bl_region_type = "WINDOW"
    bl_context = "scene"
    bl_options = {"DEFAULT_CLOSED"}

    def draw(self, context):
        layout = self.layout
        props = context.scene.crowd_project
        layout.label(text="Aggregate tiers; no per-agent list", icon="SEQ_STRIP_META")
        layout.label(text=props.m5_profile_status)
        box = layout.box()
        box.label(text="Declared S0-S3")
        for name in ("m5_s0_count", "m5_s1_count", "m5_s2_count", "m5_s3_count"):
            box.prop(props, name)
        box.label(text="Declared R0-R4")
        for name in ("m5_r0_count", "m5_r1_count", "m5_r2_count", "m5_r3_count", "m5_r4_count"):
            box.prop(props, name)
        box.prop(props, "m5_backend")
        box.prop(props, "m5_fallback")
        estimates = layout.box()
        estimates.label(text="Estimates (not measurements)", icon="INFO")
        estimates.operator("crowd.estimate_m5_preflight", icon="PREVIEW_RANGE")
        estimates.prop(props, "m5_estimated_memory")
        estimates.prop(props, "m5_estimated_cache")
        estimates.prop(props, "m5_estimated_extract")

        measured = layout.box()
        measured.label(text="Measured evidence", icon="CHECKMARK")
        measured.prop(props, "m5_report_path")
        measured.prop(props, "m5_adjudication_path")
        measured.operator("crowd.load_m5_report", icon="IMPORT")
        measured.prop(props, "m5_measured_summary")
        measured.prop(props, "m5_gate_result")
        measured.prop(props, "m5_bottleneck")
        measured.prop(props, "m5_animation_scheduling")

        playback = layout.box()
        playback.label(text="Attached cache", icon="SEQUENCE")
        playback.operator("crowd.summarize_m5_playback", icon="SEQ_STRIP_META")
        playback.prop(props, "m5_playback_tiers")


class CROWD_PT_m4_layout(Panel):
    bl_label = "M4 Layered Layout and Interchange"
    bl_idname = "CROWD_PT_m4_layout"
    bl_space_type = "PROPERTIES"
    bl_region_type = "WINDOW"
    bl_context = "scene"
    bl_options = {"DEFAULT_CLOSED"}

    def draw(self, context):
        layout = self.layout
        props = context.scene.crowd_project
        layout.label(text="Base cache remains read-only", icon="LOCKED")
        layout.prop(props, "layout_layers_path")
        row = layout.row(align=True)
        row.operator("crowd.apply_m4_layers", icon="MODIFIER")
        row.operator("crowd.inspect_m4_layout", icon="INFO")
        layout.template_list(
            "CROWD_UL_m4_layers", "", props, "m4_layers", props, "active_m4_layer_index", rows=4
        )
        if props.m4_layers and props.active_m4_layer_index < len(props.m4_layers):
            item = props.m4_layers[props.active_m4_layer_index]
            box = layout.box()
            box.label(text="Order {} / Priority {}".format(item.order, item.priority))
            box.label(text=item.provenance or "No provenance")
            box.label(text="Validity: {}".format(item.validity))
            row = box.row(align=True)
            row.operator("crowd.toggle_m4_layer_mute", text="Unmute" if item.muted else "Mute", icon="HIDE_ON" if item.muted else "HIDE_OFF")
            row.operator("crowd.toggle_m4_layer_solo", text="Unsolo" if item.solo else "Solo", icon="SOLO_ON" if item.solo else "SOLO_OFF")
        layout.separator()
        layout.label(text="Sparse viewport correction")
        layout.operator("crowd.select_m4_nearest_agent", icon="RESTRICT_SELECT_OFF")
        layout.label(text="Place the 3D cursor near a visible instance, then select it.")
        layout.prop(props, "m4_layer_id")
        layout.prop(props, "m4_layer_kind")
        row = layout.row(align=True)
        row.prop(props, "m4_order")
        row.prop(props, "m4_priority")
        layout.prop(props, "m4_target_agent_ids")
        row = layout.row(align=True)
        row.prop(props, "m4_tick_start")
        row.prop(props, "m4_tick_end")
        row = layout.row(align=True)
        row.prop(props, "m4_offset_x")
        row.prop(props, "m4_offset_y")
        row.prop(props, "m4_offset_z")
        layout.operator("crowd.add_m4_transform_layer", icon="PLUS")
        layout.separator()
        layout.label(text="Region and curve corrections")
        layout.prop(props, "m4_region_id")
        layout.prop(props, "m4_density_millionths")
        layout.operator("crowd.add_m4_region_density", icon="MOD_VERTEX_WEIGHT")
        layout.prop(props, "m4_curve_id")
        layout.prop(props, "m4_curve_offset_ticks")
        layout.operator("crowd.add_m4_curve_retiming", icon="CURVE_DATA")
        layout.separator()
        layout.label(text="Selected-agent physics handoff")
        layout.prop(props, "m4_physics_masks")
        layout.prop(props, "m4_physics_restitution_millionths")
        layout.operator("crowd.add_m4_physics_handoff", icon="PHYSICS")
        layout.separator()
        layout.label(text="Bounded local re-simulation")
        row = layout.row(align=True)
        row.prop(props, "m4_resim_target_x")
        row.prop(props, "m4_resim_target_y")
        row.prop(props, "m4_resim_target_z")
        layout.prop(props, "m4_resim_max_speed_mps")
        layout.operator("crowd.add_m4_local_resimulation", icon="FILE_REFRESH")
        layout.label(text=props.m4_layout_status, icon="FILE_TICK")
        layout.separator()
        layout.prop(props, "layout_flatten_path")
        layout.operator("crowd.flatten_m4_layout", icon="DUPLICATE")
        layout.prop(props, "layout_export_path")
        layout.operator("crowd.export_m4_usd", icon="EXPORT")
        layout.label(text="Unsupported profile features report as warnings.", icon="INFO")


_CLASSES = (CROWD_UL_diagnostics, CROWD_UL_m4_layers, CROWD_PT_workflow, CROWD_PT_project, CROWD_PT_m5_scale_profile, CROWD_PT_m4_layout)


def register():
    for cls in _CLASSES:
        bpy.utils.register_class(cls)


def unregister():
    for cls in reversed(_CLASSES):
        bpy.utils.unregister_class(cls)
