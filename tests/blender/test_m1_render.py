"""Render the same cache-only M1 reference frame with Eevee and Cycles CPU."""

import json
import os
import sys

import addon_utils
import bpy
import numpy as np


EXTENSION = "bl_ext.user_default.blender_crowd"
EXPECTED_SIZE = (320, 180)


def fail(message):
    print("FAIL: {}".format(message))
    sys.exit(1)


def require(condition, message):
    if not condition:
        fail(message)


def assert_render(path):
    require(os.path.isfile(path), "render missing: {}".format(path))
    require(os.path.getsize(path) > 1024, "render is unexpectedly small: {}".format(path))
    image = bpy.data.images.load(path, check_existing=False)
    try:
        require(tuple(image.size) == EXPECTED_SIZE, "wrong render dimensions")
        pixels = np.array(image.pixels[:], dtype=np.float32).reshape((-1, 4))[:, :3]
        background = pixels[0]
        changed = np.count_nonzero(np.any(np.abs(pixels - background) > 0.02, axis=1))
        require(changed > 100, "render contains no meaningful non-background pixels")
    finally:
        bpy.data.images.remove(image)


def main():
    addon_utils.enable(EXTENSION, default_set=True)
    try:
        from bl_ext.user_default.blender_crowd import operators, render_workflow
    except ImportError as error:
        fail("reference render workflow did not import: {}".format(error))

    cache_path = os.environ.get("CROWD_M1_CACHE_PATH")
    output_dir = os.environ.get("CROWD_M1_RENDER_DIR")
    require(cache_path and os.path.isdir(cache_path), "CROWD_M1_CACHE_PATH is not a cache")
    require(output_dir, "CROWD_M1_RENDER_DIR is not set")
    require(
        bpy.ops.crowd.attach_cache(filepath=cache_path) == {"FINISHED"},
        "cache attachment failed",
    )
    require(not hasattr(operators.active_cache_playback(), "session"), "render has a Session")
    result = bpy.ops.crowd.render_reference_frame(output_dir=output_dir)
    require(result == {"FINISHED"}, "render operator did not finish")

    metrics_path = os.path.join(output_dir, "m1-render-metrics.json")
    require(os.path.isfile(metrics_path), "render metrics JSON is missing")
    with open(metrics_path, encoding="utf-8") as handle:
        metrics = json.load(handle)
    require(metrics["schema_version"] == 1, "wrong metrics schema")
    require(metrics["cache_only"] is True, "metrics do not declare cache-only playback")
    require(metrics["agent_count"] == 1000, "render did not use 1,000 agents")
    require(
        metrics["proxy_instance_count"] >= 600,
        "render did not evaluate a substantial commuter crowd",
    )
    require(metrics["image_size"] == [320, 180], "metrics image size is wrong")
    # Rendering fires the playback frame handler, so the tick that was measured
    # is not necessarily the tick that was drawn. Require them to be the same.
    require(
        metrics["rendered_tick"] == metrics["reference_tick"],
        "renders were drawn at tick {}, not the reference tick {}".format(
            metrics["rendered_tick"], metrics["reference_tick"]
        ),
    )
    require(
        metrics["post_render_proxy_instance_count"] >= 600,
        "the rendered state held only {} commuter instances".format(
            metrics["post_render_proxy_instance_count"]
        ),
    )
    require(metrics["point_upload_seconds"] >= 0.0, "point upload timing missing")
    require(metrics["armature_evaluation_seconds"] >= 0.0, "armature timing missing")
    require(set(metrics["renders"]) == {"eevee", "cycles"}, "renderer metrics missing")
    require(
        metrics["renders"]["eevee"]["engine"]
        in {"BLENDER_EEVEE_NEXT", "BLENDER_EEVEE"},
        "wrong Eevee engine",
    )
    require(metrics["renders"]["cycles"]["engine"] == "CYCLES", "wrong Cycles engine")
    require(metrics["renders"]["cycles"]["device"] == "CPU", "Cycles was not CPU")
    require("simulation_seconds" not in metrics, "render metrics conflated simulation cost")

    for engine in ("eevee", "cycles"):
        assert_render(metrics["renders"][engine]["output_path"])

    require(
        render_workflow.last_metrics() == metrics,
        "operator did not retain its measured result",
    )
    print("render metrics: {}".format(metrics_path))
    print("Eevee seconds: {:.4f}".format(metrics["renders"]["eevee"]["seconds"]))
    print("Cycles CPU seconds: {:.4f}".format(metrics["renders"]["cycles"]["seconds"]))
    print("PASS: self-contained M1 cache-only reference render")


try:
    main()
except SystemExit:
    raise
except Exception as error:
    fail("unexpected {}: {}".format(type(error).__name__, error))
