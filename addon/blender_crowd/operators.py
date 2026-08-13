"""Blender operators for the trace bridge and narrow M1 project workflow."""

import json
import threading

import bpy
from bpy.props import EnumProperty, StringProperty
from bpy.types import Operator

import blender_crowd_native

# Relative import: extensions are imported as `bl_ext.user_default.blender_crowd`,
# so an absolute `from blender_crowd.x import y` fails with "package not found".
from .trace_playback import TracePlayback
from . import (
    cache_playback,
    debug_overlay,
    geometry_nodes,
    health,
    layout_editor,
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


_CLASSES = (
    CROWD_OT_load_trace,
    CROWD_OT_attach_cache,
    CROWD_OT_inspect_cache_health,
    CROWD_OT_write_support_bundle,
    CROWD_OT_apply_terrain_presentation,
    CROWD_OT_inspect_agent,
    CROWD_OT_pin_selected_agent,
    CROWD_OT_render_reference_frame,
    CROWD_OT_create_reference_project,
    CROWD_OT_validate_project,
    CROWD_OT_validate_behavior_graph,
    CROWD_OT_validate_authorable_project,
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
