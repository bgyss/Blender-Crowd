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
        layout.operator("crowd.validate_behavior_graph")
        layout.operator("crowd.validate_authorable_project")
        row = layout.row(align=True)
        row.operator("crowd.bake_cache")
        row.operator("crowd.cancel_bake")
        layout.operator("crowd.attach_cache")
        layout.separator()
        row = layout.row(align=True)
        row.prop(props, "selected_agent_id_lo")
        row.prop(props, "selected_agent_id_hi")
        row = layout.row(align=True)
        row.prop(props, "override_tick_start")
        row.prop(props, "override_tick_end")
        layout.prop(props, "override_enabled")
        layout.operator("crowd.inspect_agent")
        layout.operator("crowd.pin_selected_agent")
        layout.operator("crowd.render_reference_frame")


_CLASSES = (CROWD_PT_project,)


def register():
    for cls in _CLASSES:
        bpy.utils.register_class(cls)


def unregister():
    for cls in reversed(_CLASSES):
        bpy.utils.unregister_class(cls)
