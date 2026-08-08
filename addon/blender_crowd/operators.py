"""Blender operators for the crowd bridge."""

import bpy
from bpy.props import StringProperty
from bpy.types import Operator

# Relative import: extensions are imported as `bl_ext.user_default.blender_crowd`,
# so an absolute `from blender_crowd.x import y` fails with "package not found".
from .trace_playback import TracePlayback

_ACTIVE = {}


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


_CLASSES = (CROWD_OT_load_trace,)


def register():
    for cls in _CLASSES:
        bpy.utils.register_class(cls)


def unregister():
    for cls in reversed(_CLASSES):
        bpy.utils.unregister_class(cls)
    _ACTIVE.clear()
