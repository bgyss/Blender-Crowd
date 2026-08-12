"""Blender Crowd -- deterministic crowd playback from a baked trace.

Targets Blender 5.2 LTS only. All imports inside this package are relative:
the extension is imported as `bl_ext.user_default.blender_crowd`, so absolute
imports of the package name fail.
"""

from . import behavior_editor, cache_playback, operators, panels, properties


def register():
    behavior_editor.register()
    properties.register()
    cache_playback.register()
    operators.register()
    panels.register()


def unregister():
    panels.unregister()
    operators.unregister()
    cache_playback.unregister()
    properties.unregister()
    behavior_editor.unregister()
