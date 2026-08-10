"""Blender operators for the trace bridge and narrow M1 project workflow."""

import json
import threading

import bpy
from bpy.props import StringProperty
from bpy.types import Operator

import blender_crowd_native

# Relative import: extensions are imported as `bl_ext.user_default.blender_crowd`,
# so an absolute `from blender_crowd.x import y` fails with "package not found".
from .trace_playback import TracePlayback
from . import cache_playback, geometry_nodes, project, reference_assets

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


class CROWD_OT_attach_cache(Operator):
    bl_idname = "crowd.attach_cache"
    bl_label = "Attach Crowd Cache"
    bl_description = "Attach a complete Cache v1 without creating a simulation session"

    filepath: StringProperty(subtype="FILE_PATH")

    def execute(self, context):
        path = self.filepath or context.scene.crowd_project.cache_path
        path = bpy.path.abspath(path)
        try:
            assets = reference_assets.ensure_reference_assets(context.scene)
            playback = cache_playback.CachePlayback(path)
            geometry_nodes.attach_cache(playback.object, assets["prototypes"])
            cache_playback.set_active(playback)
            _ACTIVE["cache_playback"] = playback
            playback.sync_to_frame(context.scene, context.scene.frame_current)
        except (OSError, RuntimeError, ValueError) as error:
            context.scene.crowd_project.status = "Attach failed: {}".format(error)
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        context.scene.crowd_project.cache_path = path
        context.scene.crowd_project.status = "Cache attached: {} agents".format(
            playback.agent_count
        )
        self.report({"INFO"}, context.scene.crowd_project.status)
        return {"FINISHED"}


class CROWD_OT_create_reference_project(Operator):
    bl_idname = "crowd.create_reference_project"
    bl_label = "Create Reference Concourse"
    bl_description = "Create the self-contained M1 concourse and proxy assets"

    def execute(self, context):
        try:
            project.create_reference_project(context.scene)
            reference_assets.ensure_reference_assets(context.scene)
            ir = project.extract_ir(context.scene)
            compiled = blender_crowd_native.compile_project(_encode_ir(ir))
        except (OSError, RuntimeError, ValueError) as error:
            context.scene.crowd_project.status = "Create failed: {}".format(error)
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
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
        self.report({"INFO"}, context.scene.crowd_project.status)
        return {"FINISHED"}


def _encode_ir(ir):
    return json.dumps(ir, sort_keys=True, separators=(",", ":"))


def _bake_worker(project_json, cache_path, ticks, token, job):
    """Run native-only work. This function must never access `bpy`."""
    try:
        compiled = blender_crowd_native.compile_project(project_json)
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
        return False
    if error is not None:
        scene.crowd_project.status = "Bake failed: {}".format(error)
    elif result is not None:
        scene.crowd_project.status = "Cache {}".format(result["status"])
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
            project_json = _encode_ir(ir)
            blender_crowd_native.compile_project(project_json)
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
        self.report({"INFO"}, "crowd bake cancellation requested")
        return {"FINISHED"}


_CLASSES = (
    CROWD_OT_load_trace,
    CROWD_OT_attach_cache,
    CROWD_OT_create_reference_project,
    CROWD_OT_validate_project,
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
