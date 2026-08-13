"""Archive-installed checks for extension-owned accessibility invariants."""

import sys

import addon_utils
import bpy


EXTENSION = "bl_ext.user_default.blender_crowd"


def fail(message):
    print("FAIL: {}".format(message))
    sys.exit(1)


def require(condition, message):
    if not condition:
        fail(message)


def main():
    addon_utils.enable(EXTENSION, default_set=True)
    from bl_ext.user_default.blender_crowd import operators, panels, properties

    require(
        "DEFAULT_CLOSED" not in getattr(panels.CROWD_PT_workflow, "bl_options", set()),
        "primary workflow is hidden by default",
    )
    require(
        "DEFAULT_CLOSED" in panels.CROWD_PT_project.bl_options,
        "raw advanced authoring is not separated",
    )
    require(issubclass(panels.CROWD_UL_diagnostics, bpy.types.UIList), "diagnostics are not keyboard-list controls")
    for operator in operators._CLASSES:
        require(operator.bl_label.strip(), "{} has no readable label".format(operator.__name__))
        require(operator.bl_description.strip(), "{} has no assistive description".format(operator.__name__))
    scene_rna = properties.CrowdProjectProperties.bl_rna
    for name in (
        "current_stage",
        "selection_context",
        "next_action",
        "operation_progress",
        "cache_status",
        "cache_recovery_hint",
        "diagnostics",
    ):
        require(scene_rna.properties[name].name.strip(), "{} has no readable property label".format(name))
    print("M3 accessibility invariants: PASS")
    bpy.ops.wm.quit_blender()


try:
    main()
except SystemExit:
    raise
except Exception as error:
    fail("unexpected {}: {}".format(type(error).__name__, error))
