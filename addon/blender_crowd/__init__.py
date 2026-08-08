"""Blender Crowd -- deterministic crowd playback from a baked trace.

Targets Blender 5.2 LTS only. All imports inside this package are relative:
the extension is imported as `bl_ext.user_default.blender_crowd`, so absolute
imports of the package name fail.
"""

from . import operators


def register():
    operators.register()


def unregister():
    operators.unregister()
