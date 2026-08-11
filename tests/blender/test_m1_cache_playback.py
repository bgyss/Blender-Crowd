"""Verify cache-only M1 point playback and the Geometry Nodes contract."""

import os
import sys

import addon_utils
import bpy
import numpy as np


EXTENSION = "bl_ext.user_default.blender_crowd"
EXPECTED_AGENTS = 1000
EXPECTED_ATTRIBUTES = {
    "crowd_agent_id_lo": "INT",
    "crowd_agent_id_hi": "INT",
    "crowd_population_id": "INT",
    "crowd_position": "FLOAT_VECTOR",
    "crowd_orientation": "FLOAT",
    "crowd_scale": "FLOAT",
    "crowd_variant_id": "INT",
    "crowd_clip_id": "INT",
    "crowd_clip_phase": "FLOAT",
    "crowd_playback_rate": "FLOAT",
    "crowd_behavior_state": "INT",
    "crowd_decision_reason": "INT",
    "crowd_render_tier": "INT",
    "crowd_visible": "INT",
}


def fail(message):
    print("FAIL: {}".format(message))
    sys.exit(1)


def require(condition, message):
    if not condition:
        fail(message)


def read_attribute(data, name, dtype, width=1):
    values = np.empty(len(data.points) * width, dtype=dtype)
    property_name = "vector" if width == 3 else "value"
    data.attributes[name].data.foreach_get(property_name, values)
    return values


def assert_frame(playback, native_cache, tick):
    playback.sync_to_tick(tick)
    expected = native_cache.read_tick(tick)
    data = playback.object.data
    for attribute, channel, dtype, width in (
        ("crowd_position", "position", np.float32, 3),
        ("crowd_orientation", "orientation", np.float32, 1),
        ("crowd_scale", "scale", np.float32, 1),
        ("crowd_variant_id", "variant_id", np.int32, 1),
        ("crowd_clip_id", "clip_id", np.int32, 1),
        ("crowd_clip_phase", "phase", np.float32, 1),
        ("crowd_playback_rate", "playback_rate", np.float32, 1),
        ("crowd_behavior_state", "behavior_state", np.int32, 1),
        ("crowd_decision_reason", "decision_reason", np.int32, 1),
        ("crowd_render_tier", "render_tier", np.int32, 1),
        ("crowd_visible", "visible", np.int32, 1),
    ):
        actual = read_attribute(data, attribute, dtype, width)
        reference = np.frombuffer(expected[channel], dtype=dtype)
        require(
            np.array_equal(actual, reference),
            "{} differs from cache at tick {}".format(attribute, tick),
        )


def main():
    addon_utils.enable(EXTENSION, default_set=True)
    try:
        from bl_ext.user_default.blender_crowd import cache_playback, operators
    except ImportError as error:
        fail("cache-only playback module did not import: {}".format(error))

    cache_path = os.environ.get("CROWD_M1_CACHE_PATH")
    require(cache_path and os.path.isdir(cache_path), "CROWD_M1_CACHE_PATH is not a cache")
    result = bpy.ops.crowd.attach_cache(filepath=cache_path)
    require(result == {"FINISHED"}, "crowd.attach_cache did not finish")
    playback = operators.active_cache_playback()
    require(
        isinstance(playback, cache_playback.CachePlayback),
        "operator did not retain CachePlayback",
    )
    require(not hasattr(playback, "session"), "playback retained a simulation session")
    require(not hasattr(playback, "_session"), "playback retained a private session")
    require(playback.agent_count == EXPECTED_AGENTS, "expected 1,000 cache agents")

    data = playback.object.data
    require(len(data.points) == EXPECTED_AGENTS, "point cloud count is not 1,000")
    for name, data_type in EXPECTED_ATTRIBUTES.items():
        attribute = data.attributes.get(name)
        require(attribute is not None, "missing {}".format(name))
        require(
            attribute.data_type == data_type,
            "{} has type {}, expected {}".format(name, attribute.data_type, data_type),
        )
        require(len(attribute.data) == EXPECTED_AGENTS, "{} count is wrong".format(name))

    import blender_crowd_native

    native_cache = blender_crowd_native.Cache(cache_path)
    ticks = [
        native_cache.tick_start,
        native_cache.tick_start + 913,
        (native_cache.tick_start + native_cache.tick_end) // 2,
        native_cache.tick_end,
        native_cache.tick_start + 137,
    ]
    for tick in ticks:
        assert_frame(playback, native_cache, tick)

    static_agents = native_cache.read_agents()
    expected_lo = np.frombuffer(static_agents["agent_id_lo"], dtype=np.uint32)
    expected_hi = np.frombuffer(static_agents["agent_id_hi"], dtype=np.uint32)
    actual_lo = read_attribute(data, "crowd_agent_id_lo", np.int32).view(np.uint32)
    actual_hi = read_attribute(data, "crowd_agent_id_hi", np.int32).view(np.uint32)
    expected_ids = expected_lo[:5].astype(np.uint64) | (
        expected_hi[:5].astype(np.uint64) << np.uint64(32)
    )
    actual_ids = actual_lo[:5].astype(np.uint64) | (
        actual_hi[:5].astype(np.uint64) << np.uint64(32)
    )
    require(np.array_equal(actual_ids, expected_ids), "stable ID halves do not round trip")

    require(
        playback.sync_to_frame(bpy.context.scene, -100) == native_cache.tick_start,
        "pre-start frame did not clamp",
    )
    require(playback.last_warning, "pre-start clamp did not expose a warning")
    require(
        playback.sync_to_frame(bpy.context.scene, native_cache.tick_end + 100)
        == native_cache.tick_end,
        "post-end frame did not clamp",
    )
    require(playback.last_warning, "post-end clamp did not expose a warning")

    modifier = playback.object.modifiers.get("CrowdCacheInstancesV1")
    require(modifier is not None, "cache Geometry Nodes modifier is missing")
    require(
        modifier.node_group.name == "CrowdCacheInstancesV1",
        "wrong cache Geometry Nodes group",
    )
    node_names = {node.name for node in modifier.node_group.nodes}
    require("M1 Variant Instances" in node_names, "GN variant selection is missing")
    require("M1 Clip Phase Sine" in node_names, "GN phase motion is missing")

    print("sample stable IDs: {}".format(list(map(int, actual_ids))))
    print("sampled ticks: {}".format(ticks))
    print("PASS: cache-only M1 Geometry Nodes playback")


try:
    main()
except SystemExit:
    raise
except Exception as error:
    fail("unexpected {}: {}".format(type(error).__name__, error))
