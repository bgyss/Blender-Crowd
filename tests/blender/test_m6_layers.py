"""Blender-process proof for M6 interaction, physics, and hero layers."""

import hashlib
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


def write_json(directory, name, value):
    path = os.path.join(directory, name)
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
    return path


def main():
    if os.environ.get("CROWD_SOURCE_ADDON"):
        from addon import blender_crowd

        blender_crowd.register()
        from addon.blender_crowd import m6_interaction, m6_physics, operators, project
    else:
        addon_utils.enable(EXTENSION, default_set=True)
        from bl_ext.user_default.blender_crowd import m6_interaction, m6_physics, operators, project

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
            {"agent_id": agent_id, "root_samples": [], "skeletal_channels": []}
            for agent_id in target_ids
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
    props.m6_interaction_layer_path = write_json(directory, "interaction.json", layer)
    props.m6_interaction_motion_path = write_json(directory, "motion.json", motion)
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
    require("hero" in props.m6_hero_support and "adjacent-layer" in props.m6_hero_support, "hero support boundary was not exposed")
    playback.sync_to_tick(15)
    require(playback.inspect_agent(target_ids[0], 15)["clip_id"] == 42, "first interaction edit was not composed")
    require(playback.inspect_agent(target_ids[1], 15)["clip_id"] == 43, "second interaction edit was not composed")
    require(playback.inspect_agent(unrelated_id, 15) == unrelated_baseline, "interaction layer mutated an unrelated agent")

    require(bpy.ops.crowd.toggle_m6_layers_mute() == {"FINISHED"}, "M6 mute failed")
    playback.sync_to_tick(15)
    require(playback.inspect_agent(target_ids[0], 15) == target_baseline[0], "mute did not restore first target")
    require(playback.inspect_agent(target_ids[1], 15) == target_baseline[1], "mute did not restore second target")
    require(playback.inspect_agent(unrelated_id, 15) == unrelated_baseline, "mute mutated an unrelated agent")

    require(bpy.ops.crowd.toggle_m6_layers_mute() == {"FINISHED"}, "M6 unmute failed")
    require(bpy.ops.crowd.remove_m6_layers() == {"FINISHED"}, "M6 remove failed")
    require(not props.m6_layers_attached, "M6 remove left the bundle attached")
    playback.sync_to_tick(15)
    require(playback.inspect_agent(target_ids[0], 15) == target_baseline[0], "remove did not restore first target")
    require(playback.inspect_agent(unrelated_id, 15) == unrelated_baseline, "remove mutated an unrelated agent")

    require(bpy.ops.crowd.load_m6_layers() == {"FINISHED"}, "M6 reload failed")
    playback.sync_to_tick(15)
    require(playback.inspect_agent(target_ids[0], 15)["clip_id"] == 42, "reload lost interaction edit")
    require(playback.base_cache_hash == base_hash, "M6 lifecycle changed immutable cache identity")
    require(hashlib.sha256(open(manifest_path, "rb").read()).hexdigest() == manifest_hash, "M6 lifecycle rewrote the base cache")
    require(playback.inspect_agent(unrelated_id, 15) == unrelated_baseline, "reload mutated an unrelated agent")
    print("M6 Blender physics/hero layers: PASS")
    bpy.ops.wm.quit_blender()


try:
    main()
except SystemExit:
    raise
except Exception as error:
    fail("unexpected {}: {}".format(type(error).__name__, error))
