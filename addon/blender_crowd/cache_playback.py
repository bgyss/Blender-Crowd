"""Cache v1 playback into Blender point attributes without a live session."""

import contextlib
import json

import numpy as np

import bpy
from bpy.app.handlers import persistent

import blender_crowd_native


POINT_CLOUD_NAME = "Crowd Cache Points V1"

_INT_ATTRIBUTES = {
    "crowd_agent_id_lo": "agent_id_lo",
    "crowd_agent_id_hi": "agent_id_hi",
    "crowd_population_id": "population_id",
    "crowd_variant_id": "variant_id",
    "crowd_clip_id": "clip_id",
    "crowd_behavior_state": "behavior_state",
    "crowd_decision_reason": "decision_reason",
    "crowd_render_tier": "render_tier",
    "crowd_visible": "visible",
}
_FLOAT_ATTRIBUTES = {
    "crowd_orientation": "orientation",
    "crowd_scale": "scale",
    "crowd_clip_phase": "phase",
    "crowd_playback_rate": "playback_rate",
}
_STATIC_ATTRIBUTES = {
    "crowd_agent_id_lo",
    "crowd_agent_id_hi",
    "crowd_population_id",
    "crowd_variant_id",
    "crowd_scale",
}

_ACTIVE = None


def _ensure_attribute(data, name, data_type):
    attribute = data.attributes.get(name)
    if attribute is not None and attribute.data_type != data_type:
        data.attributes.remove(attribute)
        attribute = None
    if attribute is None:
        attribute = data.attributes.new(name, data_type, "POINT")
    return attribute


def ensure_cache_point_cloud(agent_count):
    data = bpy.data.pointclouds.get(POINT_CLOUD_NAME)
    if data is None:
        data = bpy.data.pointclouds.new(POINT_CLOUD_NAME)
    data.resize(agent_count)
    _ensure_attribute(data, "crowd_position", "FLOAT_VECTOR")
    for name in _INT_ATTRIBUTES:
        _ensure_attribute(data, name, "INT")
    for name in _FLOAT_ATTRIBUTES:
        _ensure_attribute(data, name, "FLOAT")

    obj = bpy.data.objects.get(POINT_CLOUD_NAME)
    if obj is None:
        obj = bpy.data.objects.new(POINT_CLOUD_NAME, data)
        bpy.context.scene.collection.objects.link(obj)
    elif obj.data is not data:
        obj.data = data
    obj.hide_viewport = False
    obj.hide_render = False
    return obj


class CachePlayback:
    """Own a complete cache reader and Blender buffers, never a Session."""

    def __init__(self, path, object_name=POINT_CLOUD_NAME):
        del object_name  # M1 has one stable cache object contract.
        self._cache = blender_crowd_native.Cache(path, require_complete=True)
        self._object = ensure_cache_point_cloud(self._cache.agent_count)
        self._data = self._object.data
        self._static_uploaded = False
        self._current_tick = None
        self._last_warning = ""
        self._override_layers = []
        self._upload_static(self._cache.read_agents())
        self._static_uploaded = True
        self.sync_to_tick(self._cache.tick_start)

    @property
    def agent_count(self):
        return self._cache.agent_count

    @property
    def tick_start(self):
        return self._cache.tick_start

    @property
    def tick_end(self):
        return self._cache.tick_end

    @property
    def source_hash(self):
        return self._cache.source_hash

    @property
    def object(self):
        return self._object

    @property
    def current_tick(self):
        return self._current_tick

    @property
    def last_warning(self):
        return self._last_warning

    def _write_vector(self, attribute_name, values):
        self._data.attributes[attribute_name].data.foreach_set(
            "vector", np.frombuffer(values, dtype=np.float32)
        )

    def _write_float(self, attribute_name, values):
        self._data.attributes[attribute_name].data.foreach_set(
            "value", np.frombuffer(values, dtype=np.float32)
        )

    def _write_int(self, attribute_name, values):
        self._data.attributes[attribute_name].data.foreach_set(
            "value", np.frombuffer(values, dtype=np.int32)
        )

    def _upload_static(self, buffers):
        for attribute_name, channel_name in (
            ("crowd_agent_id_lo", "agent_id_lo"),
            ("crowd_agent_id_hi", "agent_id_hi"),
            ("crowd_population_id", "population_id"),
            ("crowd_variant_id", "variant_id"),
        ):
            self._write_int(attribute_name, buffers[channel_name])
        self._write_float("crowd_scale", buffers["base_scale"])

    def sync_to_tick(self, tick):
        if tick < self.tick_start or tick > self.tick_end:
            raise ValueError(
                "tick {} outside cache range {}..{}".format(
                    tick, self.tick_start, self.tick_end
                )
            )
        buffers = self._cache.read_tick(tick)
        position = np.frombuffer(buffers["position"], dtype=np.float32)
        self._data.attributes["position"].data.foreach_set("vector", position)
        self._write_vector("crowd_position", buffers["position"])
        for attribute_name, channel_name in _FLOAT_ATTRIBUTES.items():
            if self._static_uploaded and attribute_name in _STATIC_ATTRIBUTES:
                continue
            self._write_float(attribute_name, buffers[channel_name])
        for attribute_name, channel_name in _INT_ATTRIBUTES.items():
            if self._static_uploaded and attribute_name in _STATIC_ATTRIBUTES:
                continue
            self._write_int(attribute_name, buffers[channel_name])
        self._current_tick = tick
        self._data.update_tag()
        return tick

    def sync_to_frame(self, scene, frame):
        unclamped = self.tick_start + (int(frame) - int(scene.frame_start))
        tick = max(self.tick_start, min(self.tick_end, unclamped))
        if tick != unclamped:
            self._last_warning = "frame {} clamped to cache tick {}".format(frame, tick)
            self._object["crowd_frame_warning"] = self._last_warning
        else:
            self._last_warning = ""
            self._object["crowd_frame_warning"] = ""
        self.sync_to_tick(tick)
        return tick

    def set_override_layers(self, layers):
        self._override_layers = list(layers)
        self._cache.set_override_layers(
            json.dumps(self._override_layers, sort_keys=True, separators=(",", ":"))
        )
        if self._current_tick is not None:
            self.sync_to_tick(self._current_tick)

    def clear_override_layers(self):
        self._override_layers = []
        self._cache.clear_override_layers()
        if self._current_tick is not None:
            self.sync_to_tick(self._current_tick)

    def inspect_agent(self, agent_id, tick):
        return dict(self._cache.inspect_agent(agent_id, tick))


def set_active(playback):
    global _ACTIVE
    _ACTIVE = playback


def active_playback():
    return _ACTIVE


def detach_active_playback():
    """Hide any old point cloud before a cache ceases to be authoritative."""
    global _ACTIVE
    if _ACTIVE is not None:
        _ACTIVE.object.hide_viewport = True
        _ACTIVE.object.hide_render = True
    _ACTIVE = None


@contextlib.contextmanager
def suspended_frame_sync():
    """Detach the frame handler so scene frame changes do not touch the cache.

    Scoped measurements that step the timeline for their own reasons need this:
    without it every frame_set decodes a cache tick, which both moves playback
    off the tick under measurement and folds cache reads into the measurement.
    """
    attached = _frame_change_handler in bpy.app.handlers.frame_change_post
    if attached:
        bpy.app.handlers.frame_change_post.remove(_frame_change_handler)
    try:
        yield
    finally:
        if attached and _frame_change_handler not in bpy.app.handlers.frame_change_post:
            bpy.app.handlers.frame_change_post.append(_frame_change_handler)


def _frame_change_handler(scene, _depsgraph=None):
    playback = active_playback()
    if playback is not None:
        playback.sync_to_frame(scene, scene.frame_current)


@persistent
def _restore_cache_on_load(_dummy):
    """Restore only an inspected complete cache after opening a saved project."""
    scene = bpy.context.scene
    if scene is None or not hasattr(scene, "crowd_project"):
        return
    props = scene.crowd_project
    if not props.cache_path:
        return
    global _ACTIVE
    _ACTIVE = None
    try:
        # Deferred import avoids the operators -> cache_playback import cycle.
        from . import operators

        operators.attach_cache_path(scene, props.cache_path)
    except (OSError, RuntimeError, TypeError, ValueError):
        # inspect_cache_path records a persistent recovery diagnostic.  A failed
        # restore must never leave stale saved geometry appearing authoritative.
        stale = bpy.data.objects.get(POINT_CLOUD_NAME)
        if stale is not None:
            stale.hide_viewport = True
            stale.hide_render = True
        return


def register():
    if _frame_change_handler not in bpy.app.handlers.frame_change_post:
        bpy.app.handlers.frame_change_post.append(_frame_change_handler)
    if _restore_cache_on_load not in bpy.app.handlers.load_post:
        bpy.app.handlers.load_post.append(_restore_cache_on_load)


def unregister():
    global _ACTIVE
    while _frame_change_handler in bpy.app.handlers.frame_change_post:
        bpy.app.handlers.frame_change_post.remove(_frame_change_handler)
    while _restore_cache_on_load in bpy.app.handlers.load_post:
        bpy.app.handlers.load_post.remove(_restore_cache_on_load)
    _ACTIVE = None
