"""Blender Crowd -- deterministic crowd playback from a baked trace.

Targets Blender 5.2 LTS only. All imports inside this package are relative:
the extension is imported as `bl_ext.user_default.blender_crowd`, so absolute
imports of the package name fail.
"""

from . import operators, panels, properties


def register():
    properties.register()
    operators.register()
    panels.register()


def unregister():
    panels.unregister()
    operators.unregister()
    properties.unregister()
