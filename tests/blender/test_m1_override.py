"""Exercise selected-agent evidence and a reversible one-agent pin layer."""

import hashlib
import json
import os
import sys

import addon_utils
import bpy
import numpy as np


EXTENSION = "bl_ext.user_default.blender_crowd"
OFFSET = np.array([1.0, -2.0, 0.5], dtype=np.float32)


def fail(message):
    print("FAIL: {}".format(message))
    sys.exit(1)


def require(condition, message):
    if not condition:
        fail(message)


def base_cache_hash(cache_path):
    with open(os.path.join(cache_path, "manifest.json"), encoding="utf-8") as handle:
        manifest = json.load(handle)
    digest = hashlib.blake2b(digest_size=32)
    for relative in ["manifest.json", manifest["agents"]["path"]] + [
        item["path"] for item in manifest["chunks"]
    ]:
        with open(os.path.join(cache_path, relative), "rb") as handle:
            digest.update(handle.read())
    return digest.hexdigest()


def positions(playback, tick):
    playback.sync_to_tick(tick)
    values = np.empty(playback.agent_count * 3, dtype=np.float32)
    playback.object.data.attributes["crowd_position"].data.foreach_get(
        "vector", values
    )
    return values.reshape((-1, 3)).copy()


def signed_word(value):
    return value if value < (1 << 31) else value - (1 << 32)


def main():
    addon_utils.enable(EXTENSION, default_set=True)
    try:
        from bl_ext.user_default.blender_crowd import (
            debug_overlay,
            operators,
            overrides,
        )
    except ImportError as error:
        fail("override modules did not import: {}".format(error))

    cache_path = os.environ.get("CROWD_M1_CACHE_PATH")
    require(cache_path and os.path.isdir(cache_path), "CROWD_M1_CACHE_PATH is not a cache")
    require(
        bpy.ops.crowd.attach_cache(filepath=cache_path) == {"FINISHED"},
        "cache attachment failed",
    )
    playback = operators.active_cache_playback()
    before_hash = base_cache_hash(cache_path)

    import blender_crowd_native

    native_cache = blender_crowd_native.Cache(cache_path)
    static = native_cache.read_agents()
    ids_lo = np.frombuffer(static["agent_id_lo"], dtype=np.uint32)
    ids_hi = np.frombuffer(static["agent_id_hi"], dtype=np.uint32)
    stable_ids = ids_lo.astype(np.uint64) | (ids_hi.astype(np.uint64) << np.uint64(32))
    target_id = int(stable_ids[0])
    target_slot = 0
    props = bpy.context.scene.crowd_project
    props.selected_agent_id_lo = signed_word(target_id & 0xFFFFFFFF)
    props.selected_agent_id_hi = signed_word((target_id >> 32) & 0xFFFFFFFF)
    props.override_tick_start = 30
    props.override_tick_end = 60
    props.override_enabled = True

    pin = bpy.data.objects.new("M1 Test Pin", None)
    bpy.context.scene.collection.objects.link(pin)
    pin.location = OFFSET
    bpy.context.view_layer.objects.active = pin
    pin.select_set(True)

    base_29 = positions(playback, 29)
    base_45 = positions(playback, 45)
    require(
        bpy.ops.crowd.pin_selected_agent() == {"FINISHED"},
        "pin operator did not finish",
    )
    layer_path = overrides.default_layer_path(cache_path)
    require(os.path.isfile(layer_path), "pin operator did not write a layer")
    overridden_45 = positions(playback, 45)
    changed_slots = np.flatnonzero(np.any(overridden_45 != base_45, axis=1))
    require(
        np.array_equal(changed_slots, np.array([target_slot])),
        "override changed slots {}".format(changed_slots.tolist()),
    )
    require(
        np.array_equal(overridden_45[target_slot], base_45[target_slot] + OFFSET),
        "target transform did not receive the literal offset",
    )
    require(np.array_equal(positions(playback, 29), base_29), "out-of-range tick changed")

    layer = overrides.load_layer(layer_path)
    layer["enabled"] = False
    playback.set_override_layers([layer])
    require(
        np.array_equal(positions(playback, 45), base_45),
        "disabling the layer did not restore base positions",
    )
    require(base_cache_hash(cache_path) == before_hash, "base cache files were mutated")

    with open(
        os.path.join(cache_path, "debug", "selected-agent.json"), encoding="utf-8"
    ) as handle:
        selected = json.load(handle)
    selected_id = int(selected["agent_id"])
    props.selected_agent_id_lo = signed_word(selected_id & 0xFFFFFFFF)
    props.selected_agent_id_hi = signed_word((selected_id >> 32) & 0xFFFFFFFF)
    bpy.context.scene.frame_set(int(selected["tick"]))
    require(
        bpy.ops.crowd.inspect_agent() == {"FINISHED"},
        "inspect operator did not finish",
    )
    evidence = debug_overlay.active_evidence()
    require(evidence["agent_id"] == selected_id, "inspector selected the wrong agent")
    require(evidence["corridor_points"], "inspector has no navigation corridor")
    require(evidence["decision_reason"], "inspector has no decision reason")
    for logical_id in ("selected_path", "desired_velocity", "solved_velocity"):
        require(
            any(obj.get("crowd_debug_id") == logical_id for obj in bpy.data.objects),
            "missing {} overlay".format(logical_id),
        )

    print("target stable ID: {}".format(target_id))
    print("base cache hash: {}".format(before_hash))
    print("override layer: {}".format(layer_path))
    print("PASS: selected-agent evidence and sparse pin override")


try:
    main()
except SystemExit:
    raise
except Exception as error:
    fail("unexpected {}: {}".format(type(error).__name__, error))
