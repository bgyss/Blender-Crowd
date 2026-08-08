"""Assert the native module loads from a clean Blender install.

Runs inside Blender via `--python`. Exits non-zero on failure so the calling
shell script fails loudly. This automates M0 acceptance criterion 5: "Blender
loads the native module from a clean supported install with no absolute links
to a contributor environment."
"""

import os
import sys

import addon_utils

EXTENSION = "bl_ext.user_default.blender_crowd"


def fail(message):
    print("FAIL: {}".format(message))
    sys.exit(1)


def main():
    addon_utils.enable(EXTENSION, default_set=True)

    try:
        import blender_crowd_native
    except ImportError as error:
        fail("native module did not import: {}".format(error))

    origin = os.path.realpath(blender_crowd_native.__file__)
    print("module origin: {}".format(origin))
    print("module version: {}".format(blender_crowd_native.__version__))

    # It must have been installed, not picked up from the working checkout.
    if "extensions" not in origin.split(os.sep):
        fail("module did not load from the Blender extensions directory")

    repo_root = os.path.realpath(os.environ["CROWD_REPO_ROOT"])
    if origin.startswith(repo_root + os.sep):
        fail("module resolved into the source checkout at {}".format(repo_root))

    if not hasattr(blender_crowd_native, "Trace"):
        fail("native module has no Trace class")

    print("PASS: native module loaded from a clean install")


main()
