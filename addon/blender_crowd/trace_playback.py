"""Push one tick of a baked trace into a Blender point cloud.

This module is presentation only. It never simulates and never decides
anything about agent behaviour: it reads what the Rust side baked and moves
it into attributes a Geometry Nodes tree can instance.
"""

import numpy as np

import bpy

import blender_crowd_native

# Point attributes written every tick. Blender point attributes are 32-bit,
# so the 64-bit stable agent ID is carried as two halves rather than being
# narrowed -- a truncated stable ID is not a stable ID.
_INT_CHANNELS = (
    "agent_id_lo",
    "agent_id_hi",
    "flags",
    "clip_index",
    "render_tier",
)
_FLOAT_CHANNELS = ("orientation", "phase", "playback_rate")


def ensure_point_cloud(name, agent_count):
    """Return an object holding a point cloud of exactly `agent_count` points."""
    data = bpy.data.pointclouds.get(name)
    if data is None:
        data = bpy.data.pointclouds.new(name)
    # The API is `resize`, not `add`.
    data.resize(agent_count)

    for channel in _INT_CHANNELS:
        if channel not in data.attributes:
            data.attributes.new(channel, "INT", "POINT")
    for channel in _FLOAT_CHANNELS:
        if channel not in data.attributes:
            data.attributes.new(channel, "FLOAT", "POINT")

    obj = bpy.data.objects.get(name)
    if obj is None or obj.data is not data:
        obj = bpy.data.objects.new(name, data)
        bpy.context.scene.collection.objects.link(obj)
    return obj


class TracePlayback:
    """A trace file bound to a point-cloud object."""

    def __init__(self, path, object_name="crowd"):
        self._trace = blender_crowd_native.Trace(path)
        self._object = ensure_point_cloud(object_name, self._trace.agent_count)
        self._data = self._object.data

    @property
    def agent_count(self):
        return self._trace.agent_count

    @property
    def tick_count(self):
        return self._trace.tick_count

    @property
    def object(self):
        return self._object

    def sync_to_tick(self, tick):
        """Write one tick's channels into the point cloud's attributes."""
        buffers = self._trace.read_tick(tick)

        self._data.attributes["position"].data.foreach_set(
            "vector", np.frombuffer(buffers["position"], dtype=np.float32)
        )
        for channel in _FLOAT_CHANNELS:
            self._data.attributes[channel].data.foreach_set(
                "value", np.frombuffer(buffers[channel], dtype=np.float32)
            )
        for channel in _INT_CHANNELS:
            self._data.attributes[channel].data.foreach_set(
                "value", np.frombuffer(buffers[channel], dtype=np.int32)
            )
        self._data.update_tag()
