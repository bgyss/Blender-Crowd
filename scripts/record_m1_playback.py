"""Record the M1 reference concourse as an Eevee frame sequence, cache-only.

Runs inside Blender under scripts/make-m1-recording.sh. Every frame is a render
of the point cloud the shipped CachePlayback synced from a completed Cache v1,
instanced by the shipped node group, in a process that never creates a
simulation Session.

This is a visualisation, not a measurement. Frames are rendered one at a time
with a cache sync between them, so neither the clip's length nor its frame rate
says anything about playback or simulation speed. The measured costs are
reported separately in docs/benchmarks/2026-08-10-m1-vertical-slice.md.
"""

import json
import os
import sys
import time

import addon_utils
import bpy


EXTENSION = "bl_ext.user_default.blender_crowd"
EXPECTED_AGENTS = 1000


def fail(message):
    print("FAIL: {}".format(message))
    sys.exit(1)


def require(condition, message):
    if not condition:
        fail(message)


def env_int(name, default):
    raw = os.environ.get(name)
    if not raw:
        return default
    try:
        return int(raw)
    except ValueError:
        fail("{} is not an integer: {!r}".format(name, raw))


def main():
    addon_utils.enable(EXTENSION, default_set=True)
    try:
        from bl_ext.user_default.blender_crowd import operators, render_workflow
    except ImportError as error:
        fail("extension did not import: {}".format(error))

    cache_path = os.environ.get("CROWD_M1_CACHE_PATH")
    frame_dir = os.environ.get("CROWD_M1_FRAME_DIR")
    require(cache_path and os.path.isdir(cache_path), "CROWD_M1_CACHE_PATH is not a cache")
    require(frame_dir, "CROWD_M1_FRAME_DIR is not set")
    os.makedirs(frame_dir, exist_ok=True)

    tick_step = env_int("CROWD_TICK_STEP", 20)
    require(tick_step >= 1, "CROWD_TICK_STEP must be at least 1")
    res_x = env_int("CROWD_RES_X", 960)
    res_y = env_int("CROWD_RES_Y", 540)
    samples = env_int("CROWD_EEVEE_SAMPLES", 16)

    require(
        bpy.ops.crowd.attach_cache(filepath=cache_path) == {"FINISHED"},
        "cache attachment failed",
    )
    playback = operators.active_cache_playback()
    require(playback is not None, "no cache playback after attach")
    # The clip's entire claim is cache-only playback. Refuse to record one that
    # would not prove it rather than produce a misleading visualisation.
    require(not hasattr(playback, "session"), "recording process has a Session")
    require(
        playback.agent_count == EXPECTED_AGENTS,
        "expected {} agents, cache has {}".format(EXPECTED_AGENTS, playback.agent_count),
    )

    scene = bpy.context.scene
    render_workflow.configure_reference_scene(scene)
    # configure_reference_scene fixes the accepted 320x180 reference framing.
    # Resolution is the only deviation: the clip is meant to be watched.
    scene.render.resolution_x = res_x
    scene.render.resolution_y = res_y
    scene.render.engine = render_workflow._eevee_engine_identifier(scene)
    if hasattr(scene, "eevee") and hasattr(scene.eevee, "taa_render_samples"):
        scene.eevee.taa_render_samples = samples

    ticks = list(range(playback.tick_start, playback.tick_end + 1, tick_step))
    print(
        "recording {} frames, ticks {}..{} step {}".format(
            len(ticks), playback.tick_start, playback.tick_end, tick_step
        )
    )

    started = time.perf_counter()
    for index, tick in enumerate(ticks):
        # Drive playback through the scene frame, not sync_to_tick: rendering
        # fires the extension's frame_change_post handler, which re-syncs to
        # frame_current and would silently discard a direct sync. Going through
        # the frame is also the shipped playback path, which is what the clip
        # claims to show.
        scene.frame_set(scene.frame_start + (tick - playback.tick_start))
        require(
            playback.current_tick == tick,
            "frame drove playback to tick {}, expected {}".format(
                playback.current_tick, tick
            ),
        )
        scene.render.filepath = os.path.join(frame_dir, "frame-{:05d}.png".format(index))
        bpy.ops.render.render(write_still=True)
        if index % 25 == 0:
            print("  frame {}/{} (tick {})".format(index, len(ticks), tick))
    elapsed = time.perf_counter() - started

    written = sorted(
        name for name in os.listdir(frame_dir) if name.startswith("frame-")
    )
    require(
        len(written) == len(ticks),
        "wrote {} frames, expected {}".format(len(written), len(ticks)),
    )

    sidecar = {
        "schema_version": 1,
        "cache_only": True,
        # Frames are rendered one at a time with a sync between them. Nothing
        # here, including the wall clock below, is a throughput measurement.
        "measurement": False,
        "blender_version": bpy.app.version_string,
        "agent_count": playback.agent_count,
        "cache_manifest_hash": render_workflow._manifest_hash(cache_path),
        "tick_start": playback.tick_start,
        "tick_end": playback.tick_end,
        "tick_step": tick_step,
        "frame_count": len(ticks),
        "resolution": [res_x, res_y],
        "eevee_samples": samples,
        "render_wall_seconds": elapsed,
    }
    sidecar_path = os.path.join(frame_dir, "m1-recording.json")
    with open(sidecar_path, "w", encoding="utf-8") as handle:
        json.dump(sidecar, handle, indent=2, sort_keys=True)
        handle.write("\n")

    print("recording sidecar: {}".format(sidecar_path))
    print("PASS: cache-only M1 concourse recording, {} frames".format(len(ticks)))


try:
    main()
except SystemExit:
    raise
except Exception as error:
    fail("unexpected {}: {}".format(type(error).__name__, error))
