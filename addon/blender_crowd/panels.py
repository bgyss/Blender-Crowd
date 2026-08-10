"""The deliberately narrow M1 project panel."""

import bpy
from bpy.types import Panel


class CROWD_PT_project(Panel):
    bl_label = "Crowd Project"
    bl_idname = "CROWD_PT_project"
    bl_space_type = "PROPERTIES"
    bl_region_type = "WINDOW"
    bl_context = "scene"

    def draw(self, context):
        layout = self.layout
        props = context.scene.crowd_project
        layout.label(text=props.status)
        layout.prop(props, "project_uuid")
        row = layout.row(align=True)
        row.prop(props, "seed")
        row.prop(props, "ticks_per_second")
        layout.prop(props, "cache_path")
        layout.operator("crowd.create_reference_project")
        layout.operator("crowd.validate_project")
        row = layout.row(align=True)
        row.operator("crowd.bake_cache")
        row.operator("crowd.cancel_bake")


_CLASSES = (CROWD_PT_project,)


def register():
    for cls in _CLASSES:
        bpy.utils.register_class(cls)


def unregister():
    for cls in reversed(_CLASSES):
        bpy.utils.unregister_class(cls)
