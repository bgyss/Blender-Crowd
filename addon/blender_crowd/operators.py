"""Blender operators for the trace bridge and narrow M1 project workflow."""

import json
import threading

import bpy
from bpy.props import EnumProperty, FloatProperty, IntProperty, StringProperty
from bpy.types import Operator
from mathutils import Vector

import blender_crowd_native

# Relative import: extensions are imported as `bl_ext.user_default.blender_crowd`,
# so an absolute `from blender_crowd.x import y` fails with "package not found".
from .trace_playback import TracePlayback
from . import (
    behavior_editor,
    cache_playback,
    debug_overlay,
    geometry_nodes,
    health,
    layout_editor,
    m6_debugger,
    m6_interaction,
    m6_library,
    m6_physics,
    m4_layout,
    m5_scale,
    overrides,
    project,
    reference_assets,
    render_workflow,
    support,
)

_ACTIVE = {}
_BAKE_LOCK = threading.Lock()
_BAKE_JOB = None


class CROWD_OT_load_trace(Operator):
    bl_idname = "crowd.load_trace"
    bl_label = "Load Crowd Trace"
    bl_description = "Load a baked crowd trace and bind it to a point cloud"

    filepath: StringProperty(subtype="FILE_PATH")

    def execute(self, context):
        try:
            playback = TracePlayback(self.filepath)
        except OSError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        _ACTIVE["playback"] = playback
        geometry_nodes.attach(playback.object)
        playback.sync_to_tick(0)
        self.report(
            {"INFO"},
            "Loaded {} agents, {} ticks".format(
                playback.agent_count, playback.tick_count
            ),
        )
        return {"FINISHED"}


def active_playback():
    """Return the loaded playback, or None."""
    return _ACTIVE.get("playback")


def active_cache_playback():
    return _ACTIVE.get("cache_playback")


def attach_cache_path(scene, cache_path):
    """Inspect, then attach only a complete cache as authoritative playback."""
    if not cache_path:
        raise ValueError("choose a cache path")
    cache_playback.detach_active_playback()
    _ACTIVE.pop("cache_playback", None)
    path = bpy.path.abspath(cache_path)
    report = health.inspect_cache(scene, path)
    if report is None:
        raise ValueError(scene.crowd_project.cache_recovery_hint)
    if report["status"] != "complete":
        health.set_workflow(scene, "Recover cache", "Rebake the project", progress=0.0)
        raise ValueError(scene.crowd_project.cache_recovery_hint)
    assets = reference_assets.ensure_reference_assets(scene)
    playback = cache_playback.CachePlayback(path)
    if scene.crowd_project.project_uuid:
        current = blender_crowd_native.compile_project(_encode_ir(project.extract_ir(scene)))
        if playback.source_hash != current.source_hash:
            playback.object.hide_viewport = True
            playback.object.hide_render = True
            health.set_workflow(scene, "Stale cache", "Rebake the changed project", progress=0.0)
            health.record(
                scene,
                "ERROR",
                "Cache does not match the current project",
                "Cache source {} does not match project source {}. Rebake before playback or render.".format(
                    playback.source_hash[:12], current.source_hash[:12]
                ),
                path,
                playback.object.name,
            )
            raise ValueError("cache is stale for the current project; rebake before playback")
    geometry_nodes.attach_cache(
        playback.object,
        assets["prototypes"],
        assets["manifest"]["clips"],
        scene.crowd_project.terrain_object,
    )
    cache_playback.set_active(playback)
    _ACTIVE["cache_playback"] = playback
    scene.frame_start = playback.tick_start
    scene.frame_end = playback.tick_end
    playback.sync_to_frame(scene, scene.frame_current)
    props = scene.crowd_project
    props.cache_path = cache_path
    props.cache_resolved_path = path
    props.cache_status = "complete"
    props.cache_source_hash = playback.source_hash
    props.cache_attached = True
    health.set_selection(scene, "Cache playback: {}".format(playback.object.name))
    health.set_workflow(scene, "Playback ready", "Inspect an agent or render the cache")
    health.record(
        scene,
        "INFO",
        "Complete cache attached",
        "{} agents, ticks {}..{}".format(playback.agent_count, playback.tick_start, playback.tick_end),
        path,
        playback.object.name,
    )
    return playback


class CROWD_OT_attach_cache(Operator):
    bl_idname = "crowd.attach_cache"
    bl_label = "Attach Crowd Cache"
    bl_description = "Attach a complete Cache v1 without creating a simulation session"

    filepath: StringProperty(subtype="FILE_PATH")

    def execute(self, context):
        path = self.filepath or context.scene.crowd_project.cache_path
        try:
            playback = attach_cache_path(context.scene, path)
        except (OSError, RuntimeError, TypeError, ValueError) as error:
            context.scene.crowd_project.status = "Attach failed: {}".format(error)
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        context.scene.crowd_project.status = "Cache attached: {} agents".format(
            playback.agent_count
        )
        self.report({"INFO"}, context.scene.crowd_project.status)
        return {"FINISHED"}


class CROWD_OT_inspect_cache_health(Operator):
    bl_idname = "crowd.inspect_cache_health"
    bl_label = "Inspect Cache Health"
    bl_description = "Inspect cache completeness and recovery state without attaching it"

    def execute(self, context):
        path = context.scene.crowd_project.cache_path
        if not path:
            self.report({"ERROR"}, "choose a cache path")
            return {"CANCELLED"}
        report = health.inspect_cache(context.scene, path)
        if report is None:
            self.report({"ERROR"}, context.scene.crowd_project.cache_recovery_hint)
            return {"CANCELLED"}
        if report["status"] == "complete":
            health.record(context.scene, "INFO", "Cache health verified", "Ready for cache-only playback.", bpy.path.abspath(path))
        else:
            health.record(context.scene, "WARNING", "Cache requires recovery", context.scene.crowd_project.cache_recovery_hint, bpy.path.abspath(path))
        self.report({"INFO"}, "Cache status: {}".format(report["status"]))
        return {"FINISHED"}


class CROWD_OT_write_support_bundle(Operator):
    bl_idname = "crowd.write_support_bundle"
    bl_label = "Write Safe Support Bundle"
    bl_description = "Write diagnostics without private scene content or absolute paths"

    filepath: StringProperty(subtype="FILE_PATH", default="//blender-crowd-support.json")

    def execute(self, context):
        try:
            path = support.write_bundle(context.scene, self.filepath)
        except (OSError, ValueError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        health.record(
            context.scene,
            "INFO",
            "Safe support bundle written",
            "Support bundle excludes scene content and absolute paths.",
            str(path),
        )
        self.report({"INFO"}, "Support bundle: {}".format(path))
        return {"FINISHED"}


class CROWD_OT_apply_terrain_presentation(Operator):
    bl_idname = "crowd.apply_terrain_presentation"
    bl_label = "Apply Terrain Presentation"
    bl_description = "Project cache-only instances onto terrain without changing cache truth"
    bl_options = {"REGISTER", "UNDO"}

    def execute(self, context):
        playback = active_cache_playback()
        if playback is None:
            self.report({"ERROR"}, "attach a complete crowd cache first")
            return {"CANCELLED"}
        terrain = context.scene.crowd_project.terrain_object
        if terrain is None:
            self.report({"ERROR"}, "choose a presentation terrain object")
            return {"CANCELLED"}
        try:
            geometry_nodes.set_cache_terrain(playback.object, terrain)
        except (RuntimeError, ValueError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        context.scene.crowd_project.status = "Terrain presentation: {}".format(
            terrain.name
        )
        self.report({"INFO"}, context.scene.crowd_project.status)
        return {"FINISHED"}


class CROWD_OT_inspect_agent(Operator):
    bl_idname = "crowd.inspect_agent"
    bl_label = "Inspect Selected Agent"
    bl_description = "Show cached navigation, velocity, state, and decision evidence"

    def execute(self, context):
        playback = active_cache_playback()
        if playback is None:
            self.report({"ERROR"}, "attach a complete crowd cache first")
            return {"CANCELLED"}
        agent_id = overrides.selected_agent_id(context.scene.crowd_project)
        try:
            tick = playback.sync_to_frame(context.scene, context.scene.frame_current)
            evidence = debug_overlay.inspect(playback, agent_id, tick)
            debug_overlay.record_evidence(context.scene.crowd_project, evidence)
        except (OSError, RuntimeError, TypeError, ValueError) as error:
            context.scene.crowd_project.status = "Inspect failed: {}".format(error)
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        context.scene.crowd_project.status = "Agent {}: {} / {}".format(
            agent_id,
            evidence.get("commuter_state", evidence.get("behavior_state", "unknown")),
            evidence.get("decision_reason", "unknown"),
        )
        health.set_selection(context.scene, "Agent: {}".format(agent_id))
        self.report({"INFO"}, context.scene.crowd_project.status)
        return {"FINISHED"}


class CROWD_OT_pin_selected_agent(Operator):
    bl_idname = "crowd.pin_selected_agent"
    bl_label = "Add/Update Pinned Override"
    bl_description = "Sample the active object's translation into a sparse override layer"

    def execute(self, context):
        playback = active_cache_playback()
        if playback is None:
            self.report({"ERROR"}, "attach a complete crowd cache first")
            return {"CANCELLED"}
        authored_object = context.view_layer.objects.active
        if authored_object is None or authored_object is playback.object:
            self.report({"ERROR"}, "select an authored pin object")
            return {"CANCELLED"}
        try:
            path, layer = overrides.write_pin_layer(
                context.scene, authored_object, playback
            )
        except (OSError, RuntimeError, ValueError) as error:
            context.scene.crowd_project.status = "Pin failed: {}".format(error)
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        context.scene.crowd_project.status = "Pinned agent {}: {}".format(
            layer["target_agent_id"], path
        )
        health.set_selection(context.scene, "Pinned agent: {}".format(layer["target_agent_id"]))
        self.report({"INFO"}, context.scene.crowd_project.status)
        return {"FINISHED"}


class CROWD_OT_apply_m4_layers(Operator):
    bl_idname = "crowd.apply_m4_layers"
    bl_label = "Apply M4 Layer Stack"
    bl_description = "Compose a validated non-destructive M4 stack over the attached cache"

    def execute(self, context):
        playback = active_cache_playback()
        if playback is None:
            self.report({"ERROR"}, "attach a complete crowd cache first")
            return {"CANCELLED"}
        props = context.scene.crowd_project
        path = bpy.path.abspath(props.layout_layers_path or m4_layout.default_layer_stack_path(props.cache_path))
        try:
            layers = m4_layout.attach_layer_stack(playback, path)
        except (OSError, RuntimeError, TypeError, ValueError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        props.layout_layers_path = path
        m4_layout.sync_layer_summaries(context.scene, layers)
        props.status = "Applied {} M4 layers without rebaking".format(len(layers))
        health.record(context.scene, "INFO", "M4 layers composed", props.status, path, playback.object.name)
        self.report({"INFO"}, props.status)
        return {"FINISHED"}


class CROWD_OT_export_m4_usd(Operator):
    bl_idname = "crowd.export_m4_usd"
    bl_label = "Export M4 USD Profile"
    bl_description = "Export the current composed cache as the documented PointInstancer profile"

    def execute(self, context):
        playback = active_cache_playback()
        if playback is None:
            self.report({"ERROR"}, "attach a complete crowd cache first")
            return {"CANCELLED"}
        path = bpy.path.abspath(context.scene.crowd_project.layout_export_path)
        try:
            m4_layout.write_usda(playback, playback.current_tick, path)
        except (OSError, RuntimeError, TypeError, ValueError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        context.scene.crowd_project.status = "Exported M4 USD profile: {}".format(path)
        health.record(context.scene, "INFO", "M4 USD profile exported", context.scene.crowd_project.status, path, playback.object.name)
        self.report({"INFO"}, context.scene.crowd_project.status)
        return {"FINISHED"}


class CROWD_OT_add_m4_transform_layer(Operator):
    bl_idname = "crowd.add_m4_transform_layer"
    bl_label = "Add Transform Correction"
    bl_description = "Add a sparse M4 layout correction for selected stable IDs"
    bl_options = {"REGISTER", "UNDO"}

    def execute(self, context):
        playback = active_cache_playback()
        if playback is None:
            self.report({"ERROR"}, "attach a complete crowd cache first")
            return {"CANCELLED"}
        props = context.scene.crowd_project
        try:
            text = props.m4_target_agent_ids.strip()
            agent_ids = [int(value.strip()) for value in text.split(",") if value.strip()]
            if not agent_ids and props.selected_agent_id.strip():
                agent_ids = [overrides.selected_agent_id(props)]
            path = bpy.path.abspath(props.layout_layers_path or m4_layout.default_layer_stack_path(props.cache_path))
            layer = m4_layout.new_transform_layer(
                props.m4_layer_id,
                props.m4_layer_kind,
                playback.base_cache_hash,
                agent_ids,
                props.m4_tick_start,
                props.m4_tick_end,
                (props.m4_offset_x, props.m4_offset_y, props.m4_offset_z),
                order=props.m4_order,
                priority=props.m4_priority,
            )
            layers = m4_layout.append_layer(path, layer)
            playback.set_layout_layers(layers)
        except (OSError, RuntimeError, TypeError, ValueError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        props.layout_layers_path = path
        m4_layout.sync_layer_summaries(context.scene, layers)
        props.m4_layout_status = "Added {} for {} agent(s), ticks {}..{}".format(layer["layer_id"], len(agent_ids), props.m4_tick_start, props.m4_tick_end)
        health.record(context.scene, "INFO", "M4 correction added", props.m4_layout_status, path, playback.object.name)
        self.report({"INFO"}, props.m4_layout_status)
        return {"FINISHED"}


class CROWD_OT_inspect_m4_layout(Operator):
    bl_idname = "crowd.inspect_m4_layout"
    bl_label = "Inspect M4 Layout"
    bl_description = "Show active layers, conflicts, and explicit interchange warnings"

    def execute(self, context):
        playback = active_cache_playback()
        if playback is None:
            self.report({"ERROR"}, "attach a complete crowd cache first")
            return {"CANCELLED"}
        try:
            report = m4_layout.status(playback)
        except (OSError, RuntimeError, TypeError, ValueError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        conflicts = report.get("conflicts", [])
        props = context.scene.crowd_project
        props.m4_layout_status = "{} active layer(s), {} conflict(s)".format(len(report.get("active_layer_ids", [])), len(conflicts))
        severity = "WARNING" if conflicts else "INFO"
        health.record(context.scene, severity, "M4 layout inspected", "\n".join(conflicts + report.get("warnings", [])), object_name=playback.object.name)
        self.report({"INFO"}, props.m4_layout_status)
        return {"FINISHED"}


class CROWD_OT_select_m4_nearest_agent(Operator):
    bl_idname = "crowd.select_m4_nearest_agent"
    bl_label = "Select Agent Near 3D Cursor"
    bl_description = "Resolve the nearest visible procedural cache point to the 3D cursor as a stable agent ID"

    def execute(self, context):
        playback = active_cache_playback()
        if playback is None:
            self.report({"ERROR"}, "attach a complete crowd cache first")
            return {"CANCELLED"}
        point_cloud = playback.object.data
        position = point_cloud.attributes.get("crowd_position")
        id_lo = point_cloud.attributes.get("crowd_agent_id_lo")
        id_hi = point_cloud.attributes.get("crowd_agent_id_hi")
        visible = point_cloud.attributes.get("crowd_visible")
        if any(attribute is None for attribute in (position, id_lo, id_hi, visible)) or not len(position.data):
            self.report({"ERROR"}, "attached cache has no selectable procedural points")
            return {"CANCELLED"}
        try:
            cursor = context.scene.cursor.location
            nearest_index, nearest_distance = None, None
            for index, item in enumerate(position.data):
                if not visible.data[index].value:
                    continue
                world_position = playback.object.matrix_world @ Vector(item.vector)
                distance = (world_position - cursor).length_squared
                if nearest_distance is None or distance < nearest_distance:
                    nearest_index, nearest_distance = index, distance
            if nearest_index is None:
                raise ValueError("no visible cache agents are selectable at this tick")
            agent_id = (int(id_lo.data[nearest_index].value) & 0xFFFFFFFF) | ((int(id_hi.data[nearest_index].value) & 0xFFFFFFFF) << 32)
            props = context.scene.crowd_project
            props.selected_agent_id = str(agent_id)
            health.set_selection(context.scene, "M4 cursor selection: agent {}".format(agent_id))
            props.m4_layout_status = "Selected stable agent {} near 3D cursor ({:.2f} m²)".format(agent_id, nearest_distance)
        except (RuntimeError, TypeError, ValueError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        self.report({"INFO"}, props.m4_layout_status)
        return {"FINISHED"}


class CROWD_OT_toggle_m4_layer_mute(Operator):
    bl_idname = "crowd.toggle_m4_layer_mute"
    bl_label = "Toggle M4 Layer Mute"
    bl_description = "Persistently mute or unmute the selected layout layer"
    bl_options = {"REGISTER", "UNDO"}

    def execute(self, context):
        playback = active_cache_playback()
        if playback is None:
            self.report({"ERROR"}, "attach a complete crowd cache first")
            return {"CANCELLED"}
        props = context.scene.crowd_project
        try:
            path = bpy.path.abspath(props.layout_layers_path or m4_layout.default_layer_stack_path(props.cache_path))
            row = props.m4_layers[props.active_m4_layer_index]
            layer_id, next_state = row.layer_id, not row.muted
            layers = m4_layout.set_layer_enabled_state(path, props.active_m4_layer_index, "muted", next_state)
            playback.set_layout_layers(layers)
            m4_layout.sync_layer_summaries(context.scene, layers)
        except (IndexError, OSError, RuntimeError, TypeError, ValueError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        props.layout_layers_path = path
        props.m4_layout_status = "{} {}".format(layer_id, "muted" if next_state else "unmuted")
        self.report({"INFO"}, props.m4_layout_status)
        return {"FINISHED"}


class CROWD_OT_toggle_m4_layer_solo(Operator):
    bl_idname = "crowd.toggle_m4_layer_solo"
    bl_label = "Toggle M4 Layer Solo"
    bl_description = "Persistently solo or unsolo the selected layout layer"
    bl_options = {"REGISTER", "UNDO"}

    def execute(self, context):
        playback = active_cache_playback()
        if playback is None:
            self.report({"ERROR"}, "attach a complete crowd cache first")
            return {"CANCELLED"}
        props = context.scene.crowd_project
        try:
            path = bpy.path.abspath(props.layout_layers_path or m4_layout.default_layer_stack_path(props.cache_path))
            row = props.m4_layers[props.active_m4_layer_index]
            layer_id, next_state = row.layer_id, not row.solo
            layers = m4_layout.set_layer_enabled_state(path, props.active_m4_layer_index, "solo", next_state)
            playback.set_layout_layers(layers)
            m4_layout.sync_layer_summaries(context.scene, layers)
        except (IndexError, OSError, RuntimeError, TypeError, ValueError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        props.layout_layers_path = path
        props.m4_layout_status = "{} {}".format(layer_id, "soloed" if next_state else "unsoloed")
        self.report({"INFO"}, props.m4_layout_status)
        return {"FINISHED"}


def _m4_target_ids(props):
    text = props.m4_target_agent_ids.strip()
    ids = [int(value.strip()) for value in text.split(",") if value.strip()]
    return ids or ([overrides.selected_agent_id(props)] if props.selected_agent_id.strip() else [])


class CROWD_OT_add_m4_region_density(Operator):
    bl_idname = "crowd.add_m4_region_density"
    bl_label = "Apply Region Density"
    bl_description = "Deterministically retain the chosen density over explicit region-selected IDs"
    bl_options = {"REGISTER", "UNDO"}

    def execute(self, context):
        playback = active_cache_playback()
        props = context.scene.crowd_project
        if playback is None:
            self.report({"ERROR"}, "attach a complete crowd cache first")
            return {"CANCELLED"}
        try:
            path = bpy.path.abspath(props.layout_layers_path or m4_layout.default_layer_stack_path(props.cache_path))
            layer = m4_layout.new_scoped_layer(
                props.m4_layer_id, "layout", playback.base_cache_hash, _m4_target_ids(props), props.m4_tick_start, props.m4_tick_end,
                {"type": "region_density", "region_id": props.m4_region_id, "density_millionths": props.m4_density_millionths}, props.m4_order, props.m4_priority,
            )
            layers = m4_layout.append_layer(path, layer)
            playback.set_layout_layers(layers)
            m4_layout.sync_layer_summaries(context.scene, layers)
        except (OSError, RuntimeError, TypeError, ValueError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        props.layout_layers_path = path
        props.m4_layout_status = "Applied region density to {} explicit agent(s)".format(len(layer["target"]["agent_ids"]))
        self.report({"INFO"}, props.m4_layout_status)
        return {"FINISHED"}


class CROWD_OT_add_m4_curve_retiming(Operator):
    bl_idname = "crowd.add_m4_curve_retiming"
    bl_label = "Add Curve Retiming"
    bl_description = "Retime explicit curve-selected IDs without changing the base cache"
    bl_options = {"REGISTER", "UNDO"}

    def execute(self, context):
        playback = active_cache_playback()
        props = context.scene.crowd_project
        if playback is None:
            self.report({"ERROR"}, "attach a complete crowd cache first")
            return {"CANCELLED"}
        try:
            path = bpy.path.abspath(props.layout_layers_path or m4_layout.default_layer_stack_path(props.cache_path))
            layer = m4_layout.new_scoped_layer(
                props.m4_layer_id, "layout", playback.base_cache_hash, _m4_target_ids(props), props.m4_tick_start, props.m4_tick_end,
                {"type": "curve_retiming", "curve_id": props.m4_curve_id, "offset_ticks": props.m4_curve_offset_ticks}, props.m4_order, props.m4_priority,
            )
            layers = m4_layout.append_layer(path, layer)
            playback.set_layout_layers(layers)
            m4_layout.sync_layer_summaries(context.scene, layers)
        except (OSError, RuntimeError, TypeError, ValueError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        props.layout_layers_path = path
        props.m4_layout_status = "Applied curve retiming to {} explicit agent(s)".format(len(layer["target"]["agent_ids"]))
        self.report({"INFO"}, props.m4_layout_status)
        return {"FINISHED"}


class CROWD_OT_flatten_m4_layout(Operator):
    bl_idname = "crowd.flatten_m4_layout"
    bl_label = "Write Reversible Flattened Preview"
    bl_description = "Write a composed JSON preview while retaining the base cache and layers"

    def execute(self, context):
        playback = active_cache_playback()
        if playback is None:
            self.report({"ERROR"}, "attach a complete crowd cache first")
            return {"CANCELLED"}
        path = bpy.path.abspath(context.scene.crowd_project.layout_flatten_path)
        try:
            m4_layout.write_flattened(playback, playback.current_tick, path)
        except (OSError, RuntimeError, TypeError, ValueError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        context.scene.crowd_project.m4_layout_status = "Flattened preview written; base cache and layers remain unchanged"
        health.record(context.scene, "INFO", "M4 flattened preview written", path, path, playback.object.name)
        self.report({"INFO"}, context.scene.crowd_project.m4_layout_status)
        return {"FINISHED"}


class CROWD_OT_add_m4_physics_handoff(Operator):
    bl_idname = "crowd.add_m4_physics_handoff"
    bl_label = "Cache Physics Handoff"
    bl_description = "Create a selected-agent deterministic physics cache interval and recovery layer"
    bl_options = {"REGISTER", "UNDO"}

    def execute(self, context):
        playback = active_cache_playback()
        props = context.scene.crowd_project
        if playback is None or not props.selected_agent_id.strip():
            self.report({"ERROR"}, "attach a cache and select one stable agent first")
            return {"CANCELLED"}
        try:
            agent_id = overrides.selected_agent_id(props)
            evidence = playback.inspect_agent(agent_id, playback.current_tick)
            path = bpy.path.abspath(props.layout_layers_path or m4_layout.default_layer_stack_path(props.cache_path))
            layer = m4_layout.new_physics_handoff_layer(
                blender_crowd_native, props.m4_layer_id, playback.base_cache_hash, agent_id,
                props.m4_tick_start, props.m4_tick_end, playback.ticks_per_second,
                evidence["position"], evidence["solved_velocity"], props.m4_physics_masks.split(","),
                props.m4_physics_restitution_millionths,
            )
            layers = m4_layout.append_layer(path, layer)
            playback.set_layout_layers(layers)
            m4_layout.sync_layer_summaries(context.scene, layers)
        except (OSError, RuntimeError, TypeError, ValueError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        props.layout_layers_path = path
        props.m4_layout_status = "Cached physics handoff for agent {} through tick {}".format(agent_id, props.m4_tick_end)
        health.record(context.scene, "INFO", "M4 physics handoff cached", props.m4_layout_status, path, playback.object.name)
        self.report({"INFO"}, props.m4_layout_status)
        return {"FINISHED"}


class CROWD_OT_add_m4_local_resimulation(Operator):
    bl_idname = "crowd.add_m4_local_resimulation"
    bl_label = "Recompute Selected Local Trajectory"
    bl_description = "Write a bounded selected-agent trajectory replacement into a reversible M4 layer"
    bl_options = {"REGISTER", "UNDO"}

    def execute(self, context):
        playback = active_cache_playback()
        props = context.scene.crowd_project
        if playback is None or not props.selected_agent_id.strip():
            self.report({"ERROR"}, "attach a cache and select one stable agent first")
            return {"CANCELLED"}
        try:
            agent_id = overrides.selected_agent_id(props)
            evidence = playback.inspect_agent(agent_id, playback.current_tick)
            path = bpy.path.abspath(props.layout_layers_path or m4_layout.default_layer_stack_path(props.cache_path))
            layer = m4_layout.new_local_resimulation_layer(
                blender_crowd_native, props.m4_layer_id, playback.base_cache_hash, agent_id,
                props.m4_tick_start, props.m4_tick_end, playback.ticks_per_second,
                evidence["position"], evidence["solved_velocity"],
                (props.m4_resim_target_x, props.m4_resim_target_y, props.m4_resim_target_z), props.m4_resim_max_speed_mps,
            )
            layers = m4_layout.append_layer(path, layer)
            playback.set_layout_layers(layers)
            m4_layout.sync_layer_summaries(context.scene, layers)
        except (OSError, RuntimeError, TypeError, ValueError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        props.layout_layers_path = path
        props.m4_layout_status = "Recomputed agent {} locally for ticks {}..{}".format(agent_id, props.m4_tick_start, props.m4_tick_end)
        health.record(context.scene, "INFO", "M4 local trajectory recomputed", props.m4_layout_status, path, playback.object.name)
        self.report({"INFO"}, props.m4_layout_status)
        return {"FINISHED"}


class CROWD_OT_render_reference_frame(Operator):
    bl_idname = "crowd.render_reference_frame"
    bl_label = "Render Reference Frame"
    bl_description = "Render the attached cache with Eevee Next and Cycles CPU"

    output_dir: StringProperty(subtype="DIR_PATH")

    def execute(self, context):
        playback = active_cache_playback()
        if playback is None:
            self.report({"ERROR"}, "attach a complete crowd cache first")
            return {"CANCELLED"}
        output_dir = bpy.path.abspath(self.output_dir or "//m1-render")
        try:
            metrics = render_workflow.render_reference(
                context.scene, playback, output_dir
            )
        except (OSError, RuntimeError, TypeError, ValueError) as error:
            context.scene.crowd_project.status = "Render failed: {}".format(error)
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        context.scene.crowd_project.status = (
            "Rendered: Eevee {:.2f}s, Cycles {:.2f}s".format(
                metrics["renders"]["eevee"]["seconds"],
                metrics["renders"]["cycles"]["seconds"],
            )
        )
        self.report({"INFO"}, context.scene.crowd_project.status)
        return {"FINISHED"}


class CROWD_OT_create_reference_project(Operator):
    bl_idname = "crowd.create_reference_project"
    bl_label = "Create Reference Concourse"
    bl_description = "Create the self-contained M1 concourse and proxy assets"
    bl_options = {"REGISTER", "UNDO"}

    def execute(self, context):
        try:
            project.create_reference_project(context.scene)
            reference_assets.ensure_reference_assets(context.scene)
            ir = project.extract_ir(context.scene)
            compiled = blender_crowd_native.compile_project(_encode_ir(ir))
            project.set_reference_groups(context.scene, compiled.agent_ids())
        except (OSError, RuntimeError, ValueError) as error:
            context.scene.crowd_project.status = "Create failed: {}".format(error)
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        health.set_workflow(context.scene, "Author project", "Validate Project")
        health.set_selection(
            context.scene,
            "Project: {}".format(context.scene.crowd_project.project_uuid),
        )
        health.record(context.scene, "INFO", "Reference project created", "The project is ready for validation.")
        context.scene.crowd_project.status = (
            "Ready: {} agents, {}".format(compiled.agent_count, compiled.source_hash[:12])
        )
        self.report({"INFO"}, "Created the 1,000-agent reference concourse")
        return {"FINISHED"}


class CROWD_OT_validate_project(Operator):
    bl_idname = "crowd.validate_project"
    bl_label = "Validate Project"
    bl_description = "Extract and compile the current Crowd Project"

    def execute(self, context):
        try:
            ir = project.extract_ir(context.scene)
            compiled = blender_crowd_native.compile_project(_encode_ir(ir))
        except (OSError, RuntimeError, ValueError) as error:
            context.scene.crowd_project.status = "Invalid: {}".format(error)
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        context.scene.crowd_project.status = "Valid: {} agents, {}".format(
            compiled.agent_count, compiled.source_hash[:12]
        )
        health.set_workflow(context.scene, "Validated", "Bake Crowd Cache")
        health.record(context.scene, "INFO", "Project validation passed", context.scene.crowd_project.status)
        self.report({"INFO"}, context.scene.crowd_project.status)
        return {"FINISHED"}


class CROWD_OT_validate_behavior_graph(Operator):
    bl_idname = "crowd.validate_behavior_graph"
    bl_label = "Validate Behavior Graph"
    bl_description = "Compile the typed graph in Rust and report the actionable first error"

    def execute(self, context):
        try:
            compiled = blender_crowd_native.compile_behavior_graph(
                project.behavior_graph_json()
            )
        except (OSError, RuntimeError, ValueError) as error:
            context.scene.crowd_project.status = "Graph invalid: {}".format(error)
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        context.scene.crowd_project.status = "Graph valid: {} nodes, entry {}".format(
            compiled["node_count"], compiled["entry_index"]
        )
        self.report({"INFO"}, context.scene.crowd_project.status)
        return {"FINISHED"}


class CROWD_OT_validate_authorable_project(Operator):
    bl_idname = "crowd.validate_authorable_project"
    bl_label = "Validate M2 Authorable Project"
    bl_description = "Migrate the scene IR, attach the authored graph, and validate all M2 references"

    def execute(self, context):
        try:
            base_json = _encode_ir(project.extract_ir(context.scene))
            authorable = _authorable_project(context.scene, base_json)
            compiled = blender_crowd_native.compile_authorable_project(
                _encode_ir(authorable)
            )
        except (OSError, RuntimeError, ValueError) as error:
            context.scene.crowd_project.status = "M2 invalid: {}".format(error)
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        context.scene.crowd_project.status = "M2 valid: {} agents, {} graphs".format(
            compiled["agent_count"], compiled["behavior_program_count"]
        )
        self.report({"INFO"}, context.scene.crowd_project.status)
        return {"FINISHED"}


class CROWD_OT_inspect_m6_trace(Operator):
    bl_idname = "crowd.inspect_m6_trace"
    bl_label = "Inspect M6 Trace"
    bl_description = "Load synchronized M6 brain, motion, contact, and group evidence"

    def execute(self, context):
        props = context.scene.crowd_project
        if not props.m6_trace_path:
            self.report({"ERROR"}, "choose an M6 trace JSON file")
            return {"CANCELLED"}
        try:
            with open(bpy.path.abspath(props.m6_trace_path), encoding="utf-8") as handle:
                trace = json.load(handle)
            agent_id = int(props.selected_agent_id)
            summary = m6_debugger.build_trace_summary(trace, agent_id, props.m6_debug_tier)
            props.m6_trace_summary = "agent {} · tick {} · graph {} · decisive node {}".format(
                summary["agent_id"],
                summary["tick"],
                summary["current_graph_state"]["graph_id"] or "<none>",
                summary["decisive_node"] or "<none>",
            )
            props.m6_trace_timeline = "visited: {} · observations: {} · interrupts: {}".format(
                " → ".join(summary["current_graph_state"]["visited_nodes"]),
                ", ".join(summary["observations"]) or "none",
                ", ".join(summary["interrupts"]) or "none",
            )
            props.m6_unavailable_evidence = "unavailable: {}".format(
                ", ".join(summary["unavailable_evidence"]) or "none"
            )
            if props.m6_graph_path:
                _refresh_m6_navigation(props)
        except (OSError, TypeError, ValueError, json.JSONDecodeError) as error:
            props.m6_trace_summary = "M6 trace invalid: {}".format(error)
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        health.record(
            context.scene,
            "INFO",
            "M6 trace inspected",
            "{}\n{}".format(props.m6_trace_summary, props.m6_unavailable_evidence),
            props.m6_trace_path,
        )
        self.report({"INFO"}, props.m6_trace_summary)
        return {"FINISHED"}


class CROWD_OT_search_m6_graph(Operator):
    bl_idname = "crowd.search_m6_graph"
    bl_label = "Search M6 Graph"
    bl_description = "Find graph nodes and highlight the traceable parent/child path"

    def execute(self, context):
        props = context.scene.crowd_project
        if not props.m6_graph_path:
            self.report({"ERROR"}, "choose an M6 graph JSON file")
            return {"CANCELLED"}
        try:
            with open(bpy.path.abspath(props.m6_graph_path), encoding="utf-8") as handle:
                graph = json.load(handle)
            result = m6_debugger.search_graph(graph, props.m6_graph_search)
            props.m6_graph_matches = ", ".join(result["matches"]) or "No matching nodes"
            props.m6_graph_highlight_path = " → ".join(result["highlight_path"]) or "No traceable path"
            if props.m6_trace_path:
                _refresh_m6_navigation(props)
        except (OSError, TypeError, ValueError, json.JSONDecodeError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        self.report({"INFO"}, props.m6_graph_matches)
        return {"FINISHED"}


def _refresh_m6_navigation(props):
    with open(bpy.path.abspath(props.m6_trace_path), encoding="utf-8") as handle:
        trace = json.load(handle)
    with open(bpy.path.abspath(props.m6_graph_path), encoding="utf-8") as handle:
        graph = json.load(handle)
    index = m6_debugger.build_navigation_index(trace, graph)
    props.m6_navigation_index_json = json.dumps(index, sort_keys=True, separators=(",", ":"))
    if index:
        preferred = next((record for record in index if record["target_kind"] == "node"), index[0])
        props.m6_navigation_target = "{}::{}".format(preferred["target_kind"], preferred["target_id"])
    return index


def _navigation_target_id(index, target_kind):
    record = next((record for record in index if record["target_kind"] == target_kind), None)
    return record["target_id"] if record else "No {} selected".format(target_kind)


class CROWD_OT_navigate_m6_context(Operator):
    bl_idname = "crowd.navigate_m6_context"
    bl_label = "Navigate M6 Context"
    bl_description = "Resolve a derived M6 context selector without copying a stable ID"

    def execute(self, context):
        props = context.scene.crowd_project
        try:
            if not props.m6_navigation_index_json or props.m6_navigation_index_json == "[]":
                index = _refresh_m6_navigation(props)
            else:
                index = json.loads(props.m6_navigation_index_json)
            target_kind, separator, target_id = props.m6_navigation_target.partition("::")
            if not separator or target_kind == "none" or not target_id:
                raise ValueError("choose a derived M6 navigation context")
            record = m6_debugger.resolve_navigation(index, target_kind, target_id)
        except (OSError, TypeError, ValueError, json.JSONDecodeError) as error:
            props.m6_navigation_status = "M6 navigation unavailable: {}".format(error)
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}

        props.selected_agent_id = str(record["agent_id"])
        props.selected_agent_tick = record["tick"]
        props.selected_agent_decisive_node = record["graph_node_id"]
        props.m6_navigation_event = _navigation_target_id(index, "event")
        props.m6_navigation_node = record["graph_node_id"] or "No graph node selected"
        props.m6_navigation_action = record["action_id"] or "No action selected"
        props.m6_navigation_clip = record["motion_clip_id"] or "No clip selected"
        props.m6_navigation_contact = record["contact_id"] or "No contact selected"
        props.m6_navigation_layer = record["layer_id"] or "No layer selected"
        props.m6_navigation_correction = record["correction_id"] or "No correction selected"
        props.m6_graph_highlight_path = record["graph_node_id"] or "No traceable path"
        behavior_editor.highlight_node(record["graph_node_id"])
        props.selection_context = "M6 {} {} for agent {} at tick {}".format(
            record["target_kind"], record["target_id"], record["agent_id"], record["tick"]
        )
        props.m6_navigation_status = "Navigated to {} {}".format(record["target_kind"], record["target_id"])
        self.report({"INFO"}, props.m6_navigation_status)
        return {"FINISHED"}


class CROWD_OT_apply_m6_brain_preset(Operator):
    bl_idname = "crowd.apply_m6_brain_preset"
    bl_label = "Apply M6 Brain Preset"
    bl_description = "Instantiate a checked declarative M6 preset into the bounded behavior editor"

    def execute(self, context):
        props = context.scene.crowd_project
        if not props.m6_brain_library_path:
            self.report({"ERROR"}, "choose an M6 brain library JSON file")
            return {"CANCELLED"}
        if props.m6_brain_preset_id == "none":
            self.report({"ERROR"}, "choose a checked M6 brain preset")
            return {"CANCELLED"}
        try:
            with open(bpy.path.abspath(props.m6_brain_library_path), encoding="utf-8") as handle:
                library = json.load(handle)
            parameters = json.loads(props.m6_brain_parameters_json)
            graph = m6_library.instantiate_preset(
                library,
                props.m6_brain_preset_id,
                props.m6_brain_instance_id,
                parameters,
            )
            behavior_editor.ensure_reference_tree(graph)
            serialized = behavior_editor.graph_from_tree()
            compiled = blender_crowd_native.compile_behavior_graph(
                json.dumps(serialized, sort_keys=True, separators=(",", ":"))
            )
        except (OSError, TypeError, ValueError, json.JSONDecodeError) as error:
            props.status = "M6 preset invalid: {}".format(error)
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        props.status = "M6 preset applied: {} as {} ({} nodes)".format(
            props.m6_brain_preset_id, props.m6_brain_instance_id, compiled["node_count"]
        )
        self.report({"INFO"}, props.status)
        return {"FINISHED"}


def _m6_physics_samples(playback, bundle):
    transition = bundle["physics_transition"]
    owner = transition["agent_ids"][0]
    evidence = playback.inspect_agent(owner, transition["tick_start"])
    position = list(evidence["position"])
    if len(position) == 2:
        position.append(0.5)
    else:
        position[2] = max(float(position[2]), 0.5)
    velocity = list(evidence["solved_velocity"])
    if len(velocity) == 2:
        velocity.append(-2.0)
    else:
        velocity[2] = -2.0
    spec = {
        "tick_start": transition["tick_start"],
        "tick_end": transition["tick_end"],
        "ticks_per_second": playback.ticks_per_second,
        "incoming_position": position,
        "incoming_velocity": velocity,
        "gravity_mps2": -9.81,
        "floor_z": 0.0,
        "restitution_millionths": 0,
        "collision_masks": ["crowd", "ground"],
    }
    return json.loads(
        blender_crowd_native.simulate_physics_handoff(json.dumps(spec, sort_keys=True))
    )


def _set_m6_layer_summaries(props, bundle):
    interaction = bundle["interaction_layer"]
    transition = bundle["physics_transition"]
    hero = bundle["hero_boundary"]
    contacts = bundle["contacts"]
    props.m6_layer_owner = "interaction/physics owners: {}".format(
        ", ".join(str(agent_id) for agent_id in bundle["owner_agent_ids"])
    )
    props.m6_layer_interval = "interaction {}..{} · physics {}..{}".format(
        *bundle["interaction_interval"], *bundle["physics_interval"]
    )
    props.m6_layer_contacts = ", ".join(
        "{} at tick {}".format(contact["contact_id"], contact["tick"])
        for contact in contacts
    ) or "No contacts declared"
    props.m6_layer_provenance = "cache {} · interaction {} · motion {} · physics {}".format(
        bundle["base_cache_hash"],
        bundle["interaction_provenance"],
        bundle["motion_provenance"]["backend"],
        bundle["physics_solver"],
    )
    props.m6_layer_recovery = "{} via {}".format(
        bundle["recovery"], transition["transition_id"]
    )
    props.m6_layer_failure_policy = "interaction {} · motion {} · physics {} · hero {}".format(
        interaction["fallback"]["reason"],
        bundle["motion_fallback"]["reason"],
        bundle["physics_failure_policy"],
        hero["failure_policy"],
    )
    binding = bundle["hero_binding"]
    props.m6_hero_support = (
        "{} · declaration-only unsupported · not attached · requested cache {} · "
        "targets {} · interval {}..{} · solver {} · cache policy {} · tiers {}"
    ).format(
        hero["integration_id"],
        binding["base_cache_hash"],
        ", ".join(str(agent_id) for agent_id in binding["target_agent_ids"]),
        binding["tick_start"],
        binding["tick_end"],
        hero["solver"],
        hero["cache_policy"],
        ", ".join(hero["supported_render_tiers"]),
    )


def _clear_m6_layer_summaries(props):
    props.m6_layer_owner = "No M6 layer loaded"
    props.m6_layer_interval = "No M6 layer loaded"
    props.m6_layer_contacts = "No M6 contact evidence loaded"
    props.m6_layer_provenance = "No M6 provenance loaded"
    props.m6_layer_recovery = "No M6 recovery loaded"
    props.m6_layer_failure_policy = "No M6 failure policy loaded"
    props.m6_hero_support = "No M6 hero boundary loaded"


def _load_m6_bundle(props, playback):
    bundle = m6_interaction.load_layer_bundle(
        bpy.path.abspath(props.m6_interaction_request_path),
        bpy.path.abspath(props.m6_interaction_layer_path),
        bpy.path.abspath(props.m6_interaction_motion_path),
        bpy.path.abspath(props.m6_physics_transition_path),
        bpy.path.abspath(props.m6_hero_boundary_path),
        playback.base_cache_hash,
    )
    validated_motion = json.loads(
        blender_crowd_native.validate_interaction_motion_attachment(
            playback.base_cache_hash,
            json.dumps(bundle["interaction_request"], sort_keys=True, separators=(",", ":")),
            json.dumps(bundle["interaction_layer"], sort_keys=True, separators=(",", ":")),
            json.dumps(bundle["interaction_motion"], sort_keys=True, separators=(",", ":")),
        )
    )
    bundle["interaction_motion"] = validated_motion
    bundle["contacts"] = validated_motion["contacts"]
    bundle["motion_provenance"] = validated_motion["provenance"]
    bundle["motion_fallback"] = validated_motion["fallback"]
    samples = _m6_physics_samples(playback, bundle)
    layers = m6_interaction.build_layout_layers(bundle, samples, props.m6_layers_muted)
    playback.set_m6_layers(layers)
    _set_m6_layer_summaries(props, bundle)
    props.m6_layers_attached = True
    return layers


class CROWD_OT_load_m6_layers(Operator):
    bl_idname = "crowd.load_m6_layers"
    bl_label = "Load M6 Physics/Hero Layers"
    bl_description = "Compose cache-bound interaction and physics artifacts without rebaking"

    def execute(self, context):
        playback = active_cache_playback()
        props = context.scene.crowd_project
        if playback is None:
            self.report({"ERROR"}, "attach a complete crowd cache first")
            return {"CANCELLED"}
        try:
            layers = _load_m6_bundle(props, playback)
        except (OSError, RuntimeError, TypeError, ValueError) as error:
            props.status = "M6 layer attachment failed: {}".format(error)
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        props.status = "Attached {} M6 derived layers to complete cache {}".format(
            len(layers), playback.base_cache_hash[:12]
        )
        health.record(
            context.scene,
            "INFO",
            "M6 physics/hero layers attached",
            props.status,
            props.m6_interaction_layer_path,
            playback.object.name,
        )
        self.report({"INFO"}, props.status)
        return {"FINISHED"}


class CROWD_OT_toggle_m6_layers_mute(Operator):
    bl_idname = "crowd.toggle_m6_layers_mute"
    bl_label = "Mute/Unmute M6 Layers"
    bl_description = "Toggle the attached M6 bundle while retaining its artifacts and base cache"

    def execute(self, context):
        playback = active_cache_playback()
        props = context.scene.crowd_project
        if playback is None or not props.m6_layers_attached:
            self.report({"ERROR"}, "load M6 layers against a complete cache first")
            return {"CANCELLED"}
        props.m6_layers_muted = not props.m6_layers_muted
        try:
            _load_m6_bundle(props, playback)
        except (OSError, RuntimeError, TypeError, ValueError) as error:
            props.m6_layers_muted = not props.m6_layers_muted
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        props.status = "M6 layers {}".format("muted" if props.m6_layers_muted else "unmuted")
        self.report({"INFO"}, props.status)
        return {"FINISHED"}


class CROWD_OT_remove_m6_layers(Operator):
    bl_idname = "crowd.remove_m6_layers"
    bl_label = "Remove M6 Layers"
    bl_description = "Detach M6 overlays while preserving their files and the immutable base cache"

    def execute(self, context):
        playback = active_cache_playback()
        props = context.scene.crowd_project
        if playback is None:
            self.report({"ERROR"}, "attach a complete crowd cache first")
            return {"CANCELLED"}
        playback.clear_m6_layers()
        props.m6_layers_attached = False
        props.m6_layers_muted = False
        _clear_m6_layer_summaries(props)
        props.status = "M6 layers removed; source artifacts and base cache retained"
        self.report({"INFO"}, props.status)
        return {"FINISHED"}


class CROWD_OT_add_m2_semantic(Operator):
    bl_idname = "crowd.add_m2_semantic"
    bl_label = "Add M2 Semantic"
    bl_description = "Add an editable queue, lane, or cost-region contract"
    bl_options = {"REGISTER", "UNDO"}

    entity_type: EnumProperty(
        items=(
            ("queue", "Queue", ""),
            ("lane", "Lane", ""),
            ("cost_region", "Cost Region", ""),
        )
    )

    def execute(self, context):
        props = context.scene.crowd_project
        if self.entity_type == "queue":
            item = props.queues.add()
            item.logical_id = "new_queue"
            item.portal_id = "east_gate"
            item.slots_json = "[[0.0,0.0]]"
        elif self.entity_type == "lane":
            item = props.lanes.add()
            item.logical_id = "new_lane"
            item.points_json = "[[0.0,0.0],[1.0,0.0]]"
        else:
            item = props.cost_regions.add()
            item.logical_id = "new_region"
            item.walkable_id = "central_concourse"
            item.bounds_json = '{"min":[0.0,0.0],"max":[1.0,1.0]}'
        return {"FINISHED"}


class CROWD_OT_remove_m2_semantic(Operator):
    bl_idname = "crowd.remove_m2_semantic"
    bl_label = "Remove M2 Semantic"
    bl_description = "Remove the last semantic contract of the selected kind"
    bl_options = {"REGISTER", "UNDO"}

    entity_type: EnumProperty(
        items=(
            ("queue", "Queue", ""),
            ("lane", "Lane", ""),
            ("cost_region", "Cost Region", ""),
        )
    )

    def execute(self, context):
        props = context.scene.crowd_project
        collection = getattr(props, "{}s".format(self.entity_type))
        if not collection:
            self.report({"WARNING"}, "no {} to remove".format(self.entity_type))
            return {"CANCELLED"}
        collection.remove(len(collection) - 1)
        return {"FINISHED"}


class CROWD_OT_add_group(Operator):
    bl_idname = "crowd.add_group"
    bl_label = "Add M2 Social Group"
    bl_description = "Add an editable social-group contract"
    bl_options = {"REGISTER", "UNDO"}

    def execute(self, context):
        group = context.scene.crowd_project.groups.add()
        group.logical_id = "new_group_{}".format(len(context.scene.crowd_project.groups))
        group.kind = "couple"
        group.member_agent_ids_json = "[]"
        group.shared_destination_id = ""
        group.max_separation_millimeters = 2000
        group.bottleneck_policy = "individual"
        return {"FINISHED"}


class CROWD_OT_remove_group(Operator):
    bl_idname = "crowd.remove_group"
    bl_label = "Remove Last M2 Social Group"
    bl_description = "Remove the last authored social-group contract"
    bl_options = {"REGISTER", "UNDO"}

    def execute(self, context):
        groups = context.scene.crowd_project.groups
        if not groups:
            self.report({"WARNING"}, "no social group to remove")
            return {"CANCELLED"}
        groups.remove(len(groups) - 1)
        return {"FINISHED"}


class CROWD_OT_add_population(Operator):
    bl_idname = "crowd.add_population"
    bl_label = "Add Population"
    bl_description = "Add a population with an explicit, editable M2 contract"
    bl_options = {"REGISTER", "UNDO"}

    def execute(self, context):
        populations = context.scene.crowd_project.populations
        population = populations.add()
        population.logical_id = "new_population_{}".format(len(populations))
        population.count = 1
        population.emission_interval_ticks = 1
        population.spawn_source_ids_json = "[]"
        population.destinations_json = "[]"
        population.archetypes_json = "[]"
        population.appearances_json = "[]"
        return {"FINISHED"}


class CROWD_OT_remove_population(Operator):
    bl_idname = "crowd.remove_population"
    bl_label = "Remove Population"
    bl_description = "Remove the last authored population"
    bl_options = {"REGISTER", "UNDO"}

    def execute(self, context):
        populations = context.scene.crowd_project.populations
        if not populations:
            self.report({"WARNING"}, "no population to remove")
            return {"CANCELLED"}
        populations.remove(len(populations) - 1)
        return {"FINISHED"}


class CROWD_OT_add_m2_asset(Operator):
    bl_idname = "crowd.add_m2_asset"
    bl_label = "Add M2 Asset Contract"
    bl_description = "Add an editable retarget, clip, or variation contract"
    bl_options = {"REGISTER", "UNDO"}

    entity_type: EnumProperty(
        items=(
            ("retarget_profile", "Retarget Profile", ""),
            ("clip", "Clip", ""),
            ("variation", "Variation", ""),
        )
    )

    def execute(self, context):
        props = context.scene.crowd_project
        if self.entity_type == "retarget_profile":
            item = props.retarget_profiles.add()
            item.logical_id = "new_retarget_{}".format(len(props.retarget_profiles))
            item.source_rig_id = "source_rig"
            item.root_bone = "root"
            item.forward_axis = "-Y"
            item.scale_millimeters = 1000
            item.bone_map_json = '{"hips":"hips","left_foot":"foot.L","right_foot":"foot.R"}'
        elif self.entity_type == "clip":
            item = props.clips.add()
            item.logical_id = "new_clip_{}".format(len(props.clips))
            item.retarget_profile_id = ""
            item.duration_ticks = 30
            item.loop_start_tick = 0
            item.loop_end_tick = 29
            item.average_root_speed_mmps = 1000
            item.left_foot_contacts_json = "[]"
            item.right_foot_contacts_json = "[]"
        else:
            item = props.variations.add()
            item.logical_id = "new_variation_{}".format(len(props.variations))
            item.bodies_json = "[]"
            item.clothing_json = "[]"
            item.materials_json = "[]"
            item.props_json = "[]"
            item.clips_json = "[]"
        return {"FINISHED"}


class CROWD_OT_remove_m2_asset(Operator):
    bl_idname = "crowd.remove_m2_asset"
    bl_label = "Remove M2 Asset Contract"
    bl_description = "Remove the last asset contract of the selected kind"
    bl_options = {"REGISTER", "UNDO"}

    entity_type: EnumProperty(
        items=(
            ("retarget_profile", "Retarget Profile", ""),
            ("clip", "Clip", ""),
            ("variation", "Variation", ""),
        )
    )

    def execute(self, context):
        collection_name = {
            "retarget_profile": "retarget_profiles",
            "clip": "clips",
            "variation": "variations",
        }[self.entity_type]
        collection = getattr(context.scene.crowd_project, collection_name)
        if not collection:
            self.report({"WARNING"}, "no {} to remove".format(self.entity_type))
            return {"CANCELLED"}
        collection.remove(len(collection) - 1)
        return {"FINISHED"}


class CROWD_OT_add_layout(Operator):
    bl_idname = "crowd.add_layout"
    bl_label = "Add Layout"
    bl_description = "Add a persisted region, curve, formation, or seating layout"
    bl_options = {"REGISTER", "UNDO"}

    entity_type: EnumProperty(
        items=(
            ("region", "Region", ""),
            ("curve", "Curve/Lane", ""),
            ("formation", "Formation", ""),
            ("seating", "Seating", ""),
        )
    )

    def execute(self, context):
        props = context.scene.crowd_project
        item = props.layouts.add()
        item.logical_id = "new_{}_layout_{}".format(self.entity_type, len(props.layouts))
        item.kind = self.entity_type
        item.population_id = props.populations[0].logical_id if props.populations else ""
        item.source_id = "eastbound_lane" if self.entity_type == "curve" else ""
        item.rows = 5 if self.entity_type == "seating" else 1
        item.columns = 10 if self.entity_type == "seating" else 1
        item.points_json = "[[0.0,0.0],[1.0,0.0]]" if self.entity_type == "formation" else "[]"
        return {"FINISHED"}


class CROWD_OT_remove_layout(Operator):
    bl_idname = "crowd.remove_layout"
    bl_label = "Remove Layout"
    bl_description = "Remove the last authored layout contract"
    bl_options = {"REGISTER", "UNDO"}

    def execute(self, context):
        layouts = context.scene.crowd_project.layouts
        if not layouts:
            self.report({"WARNING"}, "no layout to remove")
            return {"CANCELLED"}
        layouts.remove(len(layouts) - 1)
        return {"FINISHED"}


class CROWD_OT_materialize_layout_guides(Operator):
    bl_idname = "crowd.materialize_layout_guides"
    bl_label = "Refresh Layout Guides"
    bl_description = "Materialize deterministic viewport guides for the authored layouts"
    bl_options = {"REGISTER", "UNDO"}

    def execute(self, context):
        try:
            count = layout_editor.materialize_guides(context.scene)
        except ValueError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        context.scene.crowd_project.status = "Layout guides: {}".format(count)
        return {"FINISHED"}


def _encode_ir(ir):
    return json.dumps(ir, sort_keys=True, separators=(",", ":"))


def _authorable_project(scene, base_json):
    """Build the typed M2 payload shared by validation and native baking."""
    authorable = json.loads(blender_crowd_native.migrate_project_v1(base_json))
    graph = json.loads(project.behavior_graph_json())
    authorable["behavior_graphs"] = [graph]
    authorable["semantics"] = project.extract_authoring_semantics(scene)
    authorable["groups"] = project.extract_authorable_groups(scene)
    authorable["assets"] = project.extract_authorable_assets(scene)
    # Validate layout references here so an invalid editor layout cannot be
    # silently ignored while the native authorable project is compiled.
    layout_editor.extract_layouts(scene)
    for assignment in authorable["population_behaviors"]:
        assignment["graph_id"] = graph["id"]
    return authorable


def _bake_worker(project_json, cache_path, ticks, token, job):
    """Run native-only work. This function must never access `bpy`."""
    try:
        compiled = blender_crowd_native.compile_authorable_runtime(project_json)
        session = compiled.create_session()
        outcome = dict(session.bake(cache_path, ticks, token))
        del session
        with _BAKE_LOCK:
            job["result"] = outcome
            job["progress"] = 1.0
    except Exception as error:  # Blender must surface native diagnostics verbatim.
        with _BAKE_LOCK:
            job["error"] = "{}: {}".format(type(error).__name__, error)
    finally:
        with _BAKE_LOCK:
            job["done"] = True


def _start_bake(project_json, cache_path, ticks):
    global _BAKE_JOB
    token = blender_crowd_native.CancelToken()
    job = {
        "token": token,
        "thread": None,
        "done": False,
        "progress": 0.0,
        "result": None,
        "error": None,
    }
    thread = threading.Thread(
        target=_bake_worker,
        args=(project_json, cache_path, ticks, token, job),
        name="crowd-cache-bake",
        daemon=True,
    )
    job["thread"] = thread
    with _BAKE_LOCK:
        _BAKE_JOB = job
    thread.start()
    return job


def active_bake():
    with _BAKE_LOCK:
        return _BAKE_JOB


def wait_for_bake(timeout=None):
    """Wait for the current worker without entering Blender from that worker."""
    job = active_bake()
    if job is None:
        return None
    job["thread"].join(timeout)
    if job["thread"].is_alive():
        return None
    with _BAKE_LOCK:
        if job["error"] is not None:
            return {"status": "error", "error": job["error"]}
        return dict(job["result"]) if job["result"] is not None else None


def _update_project_bake_status(scene):
    job = active_bake()
    if job is None:
        return False
    with _BAKE_LOCK:
        done = job["done"]
        error = job["error"]
        result = dict(job["result"]) if job["result"] is not None else None
    if not done:
        scene.crowd_project.status = "Baking cache"
        scene.crowd_project.operation_progress = 0.5
        return False
    if error is not None:
        scene.crowd_project.status = "Bake failed: {}".format(error)
        health.set_workflow(scene, "Bake failed", "Review diagnostics and fix the project", progress=0.0)
        health.record(scene, "ERROR", "Cache bake failed", error, scene.crowd_project.cache_path)
    elif result is not None:
        scene.crowd_project.status = "Cache {}".format(result["status"])
        scene.crowd_project.operation_progress = 1.0
        if result["status"] == "complete":
            health.set_workflow(scene, "Cache baked", "Attach Crowd Cache", progress=1.0)
            health.record(scene, "INFO", "Cache bake complete", "Ready to attach the resulting cache.", result["path"])
        else:
            health.set_workflow(scene, "Recover cache", "Rebake the project", progress=0.0)
            health.record(scene, "WARNING", "Cache bake did not complete", "Status: {}".format(result["status"]), result["path"])
    return True


class CROWD_OT_bake_cache(Operator):
    bl_idname = "crowd.bake_cache"
    bl_label = "Bake Crowd Cache"
    bl_description = "Bake the current Crowd Project on a native worker thread"

    _timer = None

    def execute(self, context):
        existing = active_bake()
        if existing is not None and existing["thread"].is_alive():
            self.report({"ERROR"}, "a crowd bake is already running")
            return {"CANCELLED"}
        try:
            ir = project.extract_ir(context.scene)
            project_json = _encode_ir(_authorable_project(context.scene, _encode_ir(ir)))
            blender_crowd_native.compile_authorable_runtime(project_json)
            cache_path = bpy.path.abspath(context.scene.crowd_project.cache_path)
            if not cache_path:
                raise ValueError("choose a cache path")
            ticks = ir["clock"]["frame_end"] - ir["clock"]["frame_start"] + 1
            _start_bake(project_json, cache_path, ticks)
        except (OSError, RuntimeError, ValueError) as error:
            context.scene.crowd_project.status = "Bake failed: {}".format(error)
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        context.scene.crowd_project.status = "Baking cache"
        health.set_workflow(
            context.scene,
            "Baking cache",
            "Wait for completion or cancel safely",
            progress=0.0,
        )
        health.set_bake_estimate(
            context.scene, sum(item["count"] for item in ir["populations"]), ticks
        )
        health.record(context.scene, "INFO", "Cache bake started", context.scene.crowd_project.operation_estimate, cache_path)
        if context.window is None:
            return {"FINISHED"}
        self._timer = context.window_manager.event_timer_add(0.1, window=context.window)
        context.window_manager.modal_handler_add(self)
        return {"RUNNING_MODAL"}

    def modal(self, context, event):
        if event.type != "TIMER":
            return {"PASS_THROUGH"}
        if not _update_project_bake_status(context.scene):
            return {"RUNNING_MODAL"}
        context.window_manager.event_timer_remove(self._timer)
        self._timer = None
        outcome = wait_for_bake(timeout=0.0)
        if outcome is not None and outcome.get("status") == "error":
            self.report({"ERROR"}, outcome["error"])
            return {"CANCELLED"}
        self.report({"INFO"}, context.scene.crowd_project.status)
        return {"FINISHED"}


class CROWD_OT_cancel_bake(Operator):
    bl_idname = "crowd.cancel_bake"
    bl_label = "Cancel Bake"
    bl_description = "Request safe cancellation of the active crowd bake"

    def execute(self, context):
        job = active_bake()
        if job is None or job["done"]:
            self.report({"WARNING"}, "no crowd bake is running")
            return {"CANCELLED"}
        job["token"].cancel()
        context.scene.crowd_project.status = "Canceling cache"
        health.set_workflow(context.scene, "Canceling bake", "Wait for recovery inspection", progress=context.scene.crowd_project.operation_progress)
        health.record(context.scene, "WARNING", "Cache cancellation requested", "The partial cache will not be attachable.", context.scene.crowd_project.cache_path)
        self.report({"INFO"}, "crowd bake cancellation requested")
        return {"FINISHED"}



class CROWD_OT_estimate_m5_preflight(Operator):
    bl_idname = "crowd.estimate_m5_preflight"
    bl_label = "Estimate M5 Scale Cost"
    bl_description = "Preflight memory, cache, and extraction estimates for the declared population"

    def execute(self, context):
        props = context.scene.crowd_project
        scene = context.scene
        frames = max(1, scene.frame_end - scene.frame_start + 1)
        agents = props.m5_s0_count + props.m5_s1_count + props.m5_s2_count + props.m5_s3_count
        if agents <= 0:
            self.report({"ERROR"}, "declare an S0-S3 tier mix before estimating")
            return {"CANCELLED"}
        try:
            estimate = m5_scale.estimate(agents, frames)
        except ValueError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        props.m5_estimated_memory = estimate["memory"]
        props.m5_estimated_cache = estimate["cache"]
        props.m5_estimated_extract = estimate["extraction"]
        # Deliberately not written into the measured fields: an estimate that
        # can be read as a result is worse than no estimate.
        health.record(
            scene,
            "INFO",
            "M5 preflight estimated",
            "Estimates only, for {} agents over {} frames. Attach a scale report for measurements.".format(agents, frames),
        )
        self.report({"INFO"}, "M5 preflight estimated (not measured)")
        return {"FINISHED"}


class CROWD_OT_load_m5_report(Operator):
    bl_idname = "crowd.load_m5_report"
    bl_label = "Attach M5 Scale Report"
    bl_description = "Populate the scale panel from a measured crowd-bench report and its gate adjudication"

    def execute(self, context):
        props = context.scene.crowd_project
        if not props.m5_report_path:
            self.report({"ERROR"}, "set a scale report path first")
            return {"CANCELLED"}
        try:
            report = m5_scale.load_report(props.m5_report_path)
            counts = m5_scale.declared_tier_counts(report)
        except (OSError, KeyError, ValueError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}

        for name in m5_scale.SIMULATION_TIERS:
            setattr(props, "m5_{}_count".format(name.lower()), counts["simulation"][name])
        for name in m5_scale.RENDER_TIERS:
            setattr(props, "m5_{}_count".format(name.lower()), counts["render"][name])
        props.m5_measured_summary = m5_scale.measured_summary(report)
        props.m5_bottleneck = m5_scale.bottleneck(report)
        props.m5_animation_scheduling = m5_scale.animation_scheduling_summary(report)

        if props.m5_adjudication_path:
            try:
                adjudication = m5_scale.load_adjudication(props.m5_adjudication_path)
            except (OSError, KeyError, ValueError) as error:
                self.report({"ERROR"}, str(error))
                return {"CANCELLED"}
            props.m5_gate_result = m5_scale.gate_result(adjudication)
        else:
            # An unadjudicated report is measured evidence, not a passing gate,
            # and the panel must not let the two read the same.
            props.m5_gate_result = "Not adjudicated: attach an m5-gate adjudication"

        props.m5_profile_status = "Measured report attached: {}".format(report["scene"])
        health.record(
            context.scene,
            "INFO",
            "M5 scale report attached",
            "{}\n{}".format(props.m5_measured_summary, props.m5_gate_result),
        )
        self.report({"INFO"}, props.m5_measured_summary)
        return {"FINISHED"}


class CROWD_OT_summarize_m5_playback(Operator):
    bl_idname = "crowd.summarize_m5_playback"
    bl_label = "Summarize Playback Tiers"
    bl_description = "Aggregate the attached cache's render tiers without listing individual agents"

    def execute(self, context):
        playback = active_cache_playback()
        if playback is None:
            self.report({"ERROR"}, "attach a complete crowd cache first")
            return {"CANCELLED"}
        try:
            histogram = m5_scale.playback_tier_histogram(playback)
        except (KeyError, ValueError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        props = context.scene.crowd_project
        parts = [
            "{} {}".format(name, count)
            for name, count in histogram["tiers"].items()
            if count
        ]
        if histogram["not_present"]:
            parts.append("{} not present at this tick".format(histogram["not_present"]))
        props.m5_playback_tiers = ", ".join(parts) or "no agents in the attached cache"
        self.report({"INFO"}, props.m5_playback_tiers)
        return {"FINISHED"}


_CLASSES = (
    CROWD_OT_load_trace,
    CROWD_OT_attach_cache,
    CROWD_OT_inspect_cache_health,
    CROWD_OT_write_support_bundle,
    CROWD_OT_apply_terrain_presentation,
    CROWD_OT_inspect_agent,
    CROWD_OT_pin_selected_agent,
    CROWD_OT_apply_m4_layers,
    CROWD_OT_export_m4_usd,
    CROWD_OT_add_m4_transform_layer,
    CROWD_OT_inspect_m4_layout,
    CROWD_OT_select_m4_nearest_agent,
    CROWD_OT_toggle_m4_layer_mute,
    CROWD_OT_toggle_m4_layer_solo,
    CROWD_OT_add_m4_region_density,
    CROWD_OT_add_m4_curve_retiming,
    CROWD_OT_flatten_m4_layout,
    CROWD_OT_add_m4_physics_handoff,
    CROWD_OT_add_m4_local_resimulation,
    CROWD_OT_render_reference_frame,
    CROWD_OT_create_reference_project,
    CROWD_OT_validate_project,
    CROWD_OT_validate_behavior_graph,
    CROWD_OT_validate_authorable_project,
    CROWD_OT_inspect_m6_trace,
    CROWD_OT_search_m6_graph,
    CROWD_OT_navigate_m6_context,
    CROWD_OT_apply_m6_brain_preset,
    CROWD_OT_load_m6_layers,
    CROWD_OT_toggle_m6_layers_mute,
    CROWD_OT_remove_m6_layers,
    CROWD_OT_add_m2_semantic,
    CROWD_OT_remove_m2_semantic,
    CROWD_OT_add_group,
    CROWD_OT_remove_group,
    CROWD_OT_add_population,
    CROWD_OT_remove_population,
    CROWD_OT_add_m2_asset,
    CROWD_OT_remove_m2_asset,
    CROWD_OT_add_layout,
    CROWD_OT_remove_layout,
    CROWD_OT_materialize_layout_guides,
    CROWD_OT_estimate_m5_preflight,
    CROWD_OT_load_m5_report,
    CROWD_OT_summarize_m5_playback,
    CROWD_OT_bake_cache,
    CROWD_OT_cancel_bake,
)


def register():
    for cls in _CLASSES:
        bpy.utils.register_class(cls)


def unregister():
    job = active_bake()
    if job is not None and not job["done"]:
        job["token"].cancel()
        job["thread"].join(1.0)
    for cls in reversed(_CLASSES):
        bpy.utils.unregister_class(cls)
    _ACTIVE.clear()
