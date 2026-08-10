"""Typed Blender properties owned by the narrow M1 project workflow."""

import bpy
from bpy.props import IntProperty, PointerProperty, StringProperty
from bpy.types import PropertyGroup


class CrowdProjectProperties(PropertyGroup):
    project_uuid: StringProperty(name="Project UUID")
    seed: IntProperty(name="Seed", min=0, default=2026)
    ticks_per_second: IntProperty(name="Ticks per Second", min=1, default=30)
    cache_path: StringProperty(name="Cache Path", subtype="DIR_PATH")
    status: StringProperty(name="Status", default="Not created")
    selected_agent_id_lo: IntProperty(name="Selected Agent ID Low", default=0)
    selected_agent_id_hi: IntProperty(name="Selected Agent ID High", default=0)
    reference_fixture_version: StringProperty(name="Reference Fixture Version")


_CLASSES = (CrowdProjectProperties,)


def register():
    for cls in _CLASSES:
        bpy.utils.register_class(cls)
    bpy.types.Scene.crowd_project = PointerProperty(type=CrowdProjectProperties)


def unregister():
    if hasattr(bpy.types.Scene, "crowd_project"):
        del bpy.types.Scene.crowd_project
    for cls in reversed(_CLASSES):
        bpy.utils.unregister_class(cls)
