"""Blender-process proof for M6 interaction, physics, and hero layers."""

import hashlib
import copy
import json
import os
import sys
import tempfile

import addon_utils
import bpy
import blender_crowd_native


EXTENSION = "bl_ext.user_default.blender_crowd"


def fail(message):
    print("FAIL: {}".format(message))
    sys.exit(1)


def require(condition, message):
    if not condition:
        fail(message)


def require_rejected(operation, message, expected_text):
    """Accept Blender's two Python representations of an operator rejection."""
    try:
        outcome = operation()
    except RuntimeError as error:
        require(expected_text.lower() in str(error).lower(), message)
        return
    require(outcome == {"CANCELLED"}, message)


def write_json(directory, name, value):
    path = os.path.join(directory, name)
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
    return path


def main():
    if os.environ.get("CROWD_SOURCE_ADDON"):
        from addon import blender_crowd

        blender_crowd.register()
        from addon.blender_crowd import m4_layout, m6_interaction, m6_physics, operators, project
    else:
        addon_utils.enable(EXTENSION, default_set=True)
        from bl_ext.user_default.blender_crowd import m4_layout, m6_interaction, m6_physics, operators, project

    scene = bpy.context.scene
    require(bpy.ops.crowd.create_reference_project() == {"FINISHED"}, "reference project failed")
    compiled = blender_crowd_native.compile_project(json.dumps(project.extract_ir(scene)))
    cache_dir = os.path.join(tempfile.mkdtemp(prefix="blender-crowd-m6-layer-cache-"), "cache")
    outcome = compiled.create_session(agent_count=10).bake(
        cache_dir, 31, blender_crowd_native.CancelToken()
    )
    require(outcome["status"] == "complete", "M6 layer base cache did not bake")
    manifest_path = os.path.join(cache_dir, "manifest.json")
    manifest_hash = hashlib.sha256(open(manifest_path, "rb").read()).hexdigest()
    playback = operators.attach_cache_path(scene, cache_dir)
    base_hash = playback.base_cache_hash
    agent_ids = compiled.agent_ids()[:3]
    target_ids = agent_ids[:2]
    unrelated_id = agent_ids[2]
    playback.sync_to_tick(15)
    target_baseline = [playback.inspect_agent(agent_id, 15) for agent_id in target_ids]
    unrelated_baseline = playback.inspect_agent(unrelated_id, 15)
    physics_baseline = playback.inspect_agent(target_ids[0], 25)
    m4_override = m4_layout.new_transform_layer(
        "m4-unrelated-agent-override",
        "layout",
        base_hash,
        [unrelated_id],
        10,
        30,
        (3.0, 0.0, 0.0),
    )
    playback.set_layout_layers([m4_override])
    unrelated_m4 = playback.inspect_agent(unrelated_id, 15)
    require(
        unrelated_m4["position"] != unrelated_baseline["position"],
        "M4 sparse override did not establish its unrelated-agent regression state",
    )

    directory = tempfile.mkdtemp(prefix="blender-crowd-m6-layer-artifacts-")
    layer = m6_interaction.new_animation_layer(
        "interaction-pair",
        "request-pair",
        base_hash,
        target_ids,
        10,
        20,
        edits=[
            {"agent_id": target_ids[0], "tick": 15, "clip_id": 42, "phase_millionths": 500_000},
            {"agent_id": target_ids[1], "tick": 15, "clip_id": 43, "phase_millionths": 500_000},
        ],
    )
    motion = {
        "schema_version": 1,
        "request_id": "request-pair",
        "participants": [
            {
                "agent_id": target_ids[0],
                "root_samples": [
                    {"tick": 10, "translation": [0.0, 0.0, 0.0], "yaw": 0.0},
                    {"tick": 15, "translation": [0.25, 0.0, 0.0], "yaw": 0.0},
                    {"tick": 20, "translation": [0.5, 0.0, 0.0], "yaw": 0.0},
                ],
                "skeletal_channels": [],
            },
            {
                "agent_id": target_ids[1],
                "root_samples": [
                    {"tick": 10, "translation": [1.0, 0.0, 0.0], "yaw": 3.141592653589793},
                    {"tick": 15, "translation": [0.75, 0.0, 0.0], "yaw": 3.141592653589793},
                    {"tick": 20, "translation": [0.5, 0.0, 0.0], "yaw": 3.141592653589793},
                ],
                "skeletal_channels": [],
            },
        ],
        "contacts": [{
            "contact_id": "touch-pair",
            "label": "touch",
            "owner_agent_id": target_ids[0],
            "other_agent_id": target_ids[1],
            "tick": 15,
            "distance_m": 0.0,
        }],
        "provenance": {
            "backend": "authored-paired-clip",
            "model_hash": None,
            "seed": 2026,
            "config_hash": "blender-layer-smoke-v1",
        },
        "diagnostics": [],
        "fallback": {
            "clip_set_id": "pedestrian_basic",
            "clip_id": "walk",
            "reason": "deterministic baseline",
        },
    }
    transition = m6_physics.new_transition_layer(
        "hero-recovery",
        base_hash,
        [target_ids[0]],
        20,
        30,
        "deterministic-kinematic-reference",
        "resume-walk",
        "fallback",
    )
    hero = {
        "integration_id": "hero-cloth-boundary",
        "solver": "blender-cloth",
        "cache_policy": "adjacent-layer",
        "supported_render_tiers": ["hero"],
        "failure_policy": "fallback-to-cached-body",
    }
    props = scene.crowd_project
    valid_layer_path = write_json(directory, "interaction.json", layer)
    valid_motion_path = write_json(directory, "motion.json", motion)
    props.m6_interaction_layer_path = valid_layer_path
    props.m6_interaction_motion_path = valid_motion_path
    props.m6_physics_transition_path = write_json(directory, "physics.json", transition)
    props.m6_hero_boundary_path = write_json(directory, "hero.json", hero)

    require(bpy.ops.crowd.load_m6_layers() == {"FINISHED"}, "M6 layers did not load")
    require(props.m6_layers_attached, "M6 layer state did not report attached")
    require("touch-pair" in props.m6_layer_contacts, "contact evidence was not exposed")
    require(str(target_ids[0]) in props.m6_layer_owner, "physics owner was not exposed")
    require("10..20" in props.m6_layer_interval and "20..30" in props.m6_layer_interval, "layer intervals were not exposed")
    require("deterministic-kinematic-reference" in props.m6_layer_provenance, "physics solver was not exposed")
    require("resume-walk" in props.m6_layer_recovery, "recovery policy was not exposed")
    require("fallback" in props.m6_layer_failure_policy, "failure policy was not exposed")
    require(
        "declaration-only unsupported" in props.m6_hero_support
        and "not attached" in props.m6_hero_support,
        "hero declaration was presented as an attached solver boundary",
    )
    playback.sync_to_tick(15)
    require(playback.inspect_agent(target_ids[0], 15)["clip_id"] == 42, "first interaction edit was not composed")
    require(playback.inspect_agent(target_ids[1], 15)["clip_id"] == 43, "second interaction edit was not composed")
    require(playback.inspect_agent(unrelated_id, 15) == unrelated_m4, "M6 composition replaced an unrelated M4 override")

    playback.sync_to_tick(25)
    physics_active = playback.inspect_agent(target_ids[0], 25)
    require(physics_active["physics_active"], "physics handoff was not active inside 20..30")
    require(physics_active["position"] != physics_baseline["position"], "physics interval did not change the target state")

    summaries_before_failed_attach = tuple(
        getattr(props, name)
        for name in (
            "m6_layer_owner",
            "m6_layer_interval",
            "m6_layer_contacts",
            "m6_layer_provenance",
            "m6_layer_recovery",
            "m6_layer_failure_policy",
            "m6_hero_support",
        )
    )
    invalid_motion = copy.deepcopy(motion)
    invalid_motion["participants"][0]["root_samples"] = []
    props.m6_interaction_motion_path = write_json(directory, "invalid-motion.json", invalid_motion)
    require_rejected(
        bpy.ops.crowd.load_m6_layers,
        "Rust-invalid interaction motion was accepted by Blender",
        "root samples must cover",
    )
    require(playback.inspect_agent(target_ids[0], 15)["clip_id"] == 42, "invalid motion replaced the attached native stack")
    require(
        summaries_before_failed_attach
        == tuple(
            getattr(props, name)
            for name in (
                "m6_layer_owner",
                "m6_layer_interval",
                "m6_layer_contacts",
                "m6_layer_provenance",
                "m6_layer_recovery",
                "m6_layer_failure_policy",
                "m6_hero_support",
            )
        ),
        "invalid motion replaced attached evidence properties",
    )
    props.m6_interaction_motion_path = valid_motion_path

    invalid_id = max(agent_ids) + 1_000_000
    invalid_layer = copy.deepcopy(layer)
    invalid_layer["target_agent_ids"][0] = invalid_id
    invalid_layer["edits"][0]["agent_id"] = invalid_id
    invalid_target_motion = copy.deepcopy(motion)
    invalid_target_motion["participants"][0]["agent_id"] = invalid_id
    invalid_target_motion["contacts"][0]["owner_agent_id"] = invalid_id
    props.m6_interaction_layer_path = write_json(directory, "invalid-target-layer.json", invalid_layer)
    props.m6_interaction_motion_path = write_json(directory, "invalid-target-motion.json", invalid_target_motion)
    require_rejected(
        bpy.ops.crowd.load_m6_layers,
        "M6 layer targeting an agent absent from the cache was accepted",
        "absent from the base",
    )
    playback.sync_to_tick(15)
    require(playback.inspect_agent(target_ids[0], 15)["clip_id"] == 42, "invalid target replaced the old M6 stack")
    require(playback.inspect_agent(unrelated_id, 15) == unrelated_m4, "invalid target replaced the M4 stack")
    props.m6_interaction_layer_path = valid_layer_path
    props.m6_interaction_motion_path = valid_motion_path

    require(bpy.ops.crowd.toggle_m6_layers_mute() == {"FINISHED"}, "M6 mute failed")
    playback.sync_to_tick(15)
    require(playback.inspect_agent(target_ids[0], 15) == target_baseline[0], "mute did not restore first target")
    require(playback.inspect_agent(target_ids[1], 15) == target_baseline[1], "mute did not restore second target")
    require(playback.inspect_agent(unrelated_id, 15) == unrelated_m4, "mute replaced an unrelated M4 override")
    playback.sync_to_tick(25)
    require(playback.inspect_agent(target_ids[0], 25) == physics_baseline, "mute did not restore the physics target")

    require(bpy.ops.crowd.toggle_m6_layers_mute() == {"FINISHED"}, "M6 unmute failed")
    playback.sync_to_tick(25)
    require(playback.inspect_agent(target_ids[0], 25)["physics_active"], "unmute did not restore physics interval")
    require(bpy.ops.crowd.remove_m6_layers() == {"FINISHED"}, "M6 remove failed")
    require(not props.m6_layers_attached, "M6 remove left the bundle attached")
    playback.sync_to_tick(15)
    require(playback.inspect_agent(target_ids[0], 15) == target_baseline[0], "remove did not restore first target")
    require(playback.inspect_agent(unrelated_id, 15) == unrelated_m4, "remove replaced an unrelated M4 override")
    for name, expected in (
        ("m6_layer_owner", "No M6 layer loaded"),
        ("m6_layer_interval", "No M6 layer loaded"),
        ("m6_layer_contacts", "No M6 contact evidence loaded"),
        ("m6_layer_provenance", "No M6 provenance loaded"),
        ("m6_layer_recovery", "No M6 recovery loaded"),
        ("m6_layer_failure_policy", "No M6 failure policy loaded"),
        ("m6_hero_support", "No M6 hero boundary loaded"),
    ):
        require(getattr(props, name) == expected, "M6 remove did not clear {}".format(name))

    require(bpy.ops.crowd.load_m6_layers() == {"FINISHED"}, "M6 reload failed")
    playback.sync_to_tick(15)
    require(playback.inspect_agent(target_ids[0], 15)["clip_id"] == 42, "reload lost interaction edit")
    require(playback.base_cache_hash == base_hash, "M6 lifecycle changed immutable cache identity")
    require(hashlib.sha256(open(manifest_path, "rb").read()).hexdigest() == manifest_hash, "M6 lifecycle rewrote the base cache")
    require(playback.inspect_agent(unrelated_id, 15) == unrelated_m4, "reload replaced an unrelated M4 override")
    playback.sync_to_tick(25)
    require(playback.inspect_agent(target_ids[0], 25)["physics_active"], "reload lost physics interval")
    print("M6 Blender physics/hero layers: PASS")
    bpy.ops.wm.quit_blender()


try:
    main()
except SystemExit:
    raise
except Exception as error:
    fail("unexpected {}: {}".format(type(error).__name__, error))
