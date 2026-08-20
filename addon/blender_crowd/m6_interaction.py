"""M6 interaction-layer persistence without a Blender or model dependency.

The helpers deliberately operate on plain JSON-compatible values. Blender
operators can call them from a coarse-grained action, while tests and offline
workers can use the same validation and cache-isolation rules.
"""

import json
import importlib.util
from pathlib import Path


INTERACTION_LAYER_SCHEMA_VERSION = 1
_LAYER_KEYS = {
    "schema_version",
    "layer_id",
    "interaction_id",
    "base_cache_hash",
    "target_agent_ids",
    "tick_start",
    "tick_end",
    "priority",
    "enabled",
    "provenance",
    "edits",
    "fallback",
}
_EDIT_KEYS = {"agent_id", "tick", "clip_id", "phase_millionths"}
_FALLBACK_KEYS = {"clip_set_id", "clip_id", "reason"}
_MOTION_KEYS = {
    "schema_version",
    "request_id",
    "participants",
    "contacts",
    "provenance",
    "diagnostics",
    "fallback",
}


def new_animation_layer(
    layer_id,
    interaction_id,
    base_cache_hash,
    target_agent_ids,
    tick_start,
    tick_end,
    edits=None,
    priority=40,
    enabled=True,
    provenance="authored-paired-clip-v1",
    fallback=None,
):
    """Build and validate one sparse, removable interaction layer."""
    targets = sorted({int(agent_id) for agent_id in target_agent_ids})
    layer = {
        "schema_version": INTERACTION_LAYER_SCHEMA_VERSION,
        "layer_id": str(layer_id),
        "interaction_id": str(interaction_id),
        "base_cache_hash": str(base_cache_hash),
        "target_agent_ids": targets,
        "tick_start": int(tick_start),
        "tick_end": int(tick_end),
        "priority": int(priority),
        "enabled": bool(enabled),
        "provenance": str(provenance),
        "edits": [
            {
                "agent_id": int(edit["agent_id"]),
                "tick": int(edit["tick"]),
                "clip_id": int(edit["clip_id"]),
                "phase_millionths": int(edit["phase_millionths"]),
            }
            for edit in (edits or [])
        ],
        "fallback": fallback or {
            "clip_set_id": "pedestrian_basic",
            "clip_id": "walk",
            "reason": "deterministic paired-clip fallback",
        },
    }
    validate_layer(layer)
    return layer


def fallback_layer(layer_id, interaction_id, base_cache_hash, target_agent_ids, tick_start, tick_end):
    """Create a deterministic clip-state fallback for every participant."""
    targets = sorted({int(agent_id) for agent_id in target_agent_ids})
    return new_animation_layer(
        layer_id,
        interaction_id,
        base_cache_hash,
        targets,
        tick_start,
        tick_end,
        edits=[
            {"agent_id": agent_id, "tick": int(tick_start), "clip_id": 0, "phase_millionths": 0}
            for agent_id in targets
        ],
        provenance="deterministic-fallback-v1",
        fallback={
            "clip_set_id": "pedestrian_basic",
            "clip_id": "walk",
            "reason": "interaction validation or worker failure",
        },
    )


def validate_layer(layer):
    """Raise ``ValueError`` with an actionable message for invalid layer data."""
    if not isinstance(layer, dict):
        raise ValueError("M6 interaction layer must be an object")
    unknown = set(layer) - _LAYER_KEYS
    if unknown:
        raise ValueError("M6 interaction layer has unknown fields: {}".format(", ".join(sorted(unknown))))
    missing = _LAYER_KEYS - set(layer)
    if missing:
        raise ValueError("M6 interaction layer is missing fields: {}".format(", ".join(sorted(missing))))
    if layer["schema_version"] != INTERACTION_LAYER_SCHEMA_VERSION:
        raise ValueError("unsupported M6 interaction layer version {}".format(layer["schema_version"]))
    for field in ("layer_id", "interaction_id", "provenance"):
        if not isinstance(layer[field], str) or not layer[field]:
            raise ValueError("M6 interaction layer {} must be non-empty".format(field))
    if not _is_hash(layer["base_cache_hash"]):
        raise ValueError("M6 interaction layer base cache hash must be 64 lowercase hex characters")
    targets = layer["target_agent_ids"]
    if not isinstance(targets, list) or not targets:
        raise ValueError("M6 interaction layer must target at least one stable agent")
    if len(targets) != len(set(targets)):
        raise ValueError("M6 interaction layer target IDs must be unique")
    if any(not isinstance(agent_id, int) or agent_id < 0 for agent_id in targets):
        raise ValueError("M6 interaction layer target IDs must be non-negative integers")
    start, end = layer["tick_start"], layer["tick_end"]
    if not isinstance(start, int) or not isinstance(end, int) or start < 0 or start > end:
        raise ValueError("M6 interaction layer tick range is invalid")
    edits = layer["edits"]
    if not isinstance(edits, list) or not edits:
        raise ValueError("M6 interaction layer must contain at least one edit")
    seen = set()
    for edit in edits:
        if not isinstance(edit, dict):
            raise ValueError("M6 interaction edit must be an object")
        unknown_edit = set(edit) - _EDIT_KEYS
        if unknown_edit:
            raise ValueError("M6 interaction edit has unknown fields: {}".format(", ".join(sorted(unknown_edit))))
        if _EDIT_KEYS - set(edit):
            raise ValueError("M6 interaction edit is missing fields")
        key = (edit["agent_id"], edit["tick"])
        if edit["agent_id"] not in targets:
            raise ValueError("M6 interaction edit targets undeclared agent {}".format(edit["agent_id"]))
        if edit["tick"] < start or edit["tick"] > end:
            raise ValueError("M6 interaction edit tick {} is outside the layer interval".format(edit["tick"]))
        if key in seen:
            raise ValueError("M6 interaction edit is duplicated for agent {} at tick {}".format(*key))
        seen.add(key)
        if edit["phase_millionths"] < 0 or edit["phase_millionths"] > 1_000_000:
            raise ValueError("M6 interaction edit phase must be between 0 and 1000000")
    fallback = layer["fallback"]
    if not isinstance(fallback, dict) or _FALLBACK_KEYS - set(fallback) or set(fallback) - _FALLBACK_KEYS:
        raise ValueError("M6 interaction layer must declare clip_set_id, clip_id, and reason fallback fields")
    if any(not isinstance(fallback[field], str) or not fallback[field] for field in _FALLBACK_KEYS):
        raise ValueError("M6 interaction fallback fields must be non-empty")
    return layer


def write_layer(path, layer):
    validate_layer(layer)
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    temporary = target.with_suffix(target.suffix + ".tmp")
    with temporary.open("w", encoding="utf-8") as handle:
        json.dump(layer, handle, indent=2, sort_keys=True)
        handle.write("\n")
    temporary.replace(target)


def load_layer(path, expected_base_hash):
    target = Path(path)
    with target.open(encoding="utf-8") as handle:
        layer = json.load(handle)
    validate_layer(layer)
    if layer["base_cache_hash"] != expected_base_hash:
        raise ValueError(
            "layer {} belongs to another base cache; choose its matching cache or migrate it".format(
                layer["layer_id"]
            )
        )
    return layer


def validate_motion_evidence(motion, layer):
    """Validate only the attachment relationships Blender consumes.

    Rust remains the interaction-motion validation authority. This guard keeps
    Blender from displaying contacts or provenance from a different request.
    """
    if not isinstance(motion, dict) or set(motion) != _MOTION_KEYS:
        raise ValueError("M6 interaction motion has missing or unknown fields")
    if motion.get("schema_version") != 1:
        raise ValueError("unsupported M6 interaction motion version")
    if motion.get("request_id") != layer["interaction_id"]:
        raise ValueError("M6 interaction motion does not match interaction layer")
    participants = motion.get("participants")
    if not isinstance(participants, list):
        raise ValueError("M6 interaction motion participants must be an array")
    if any(
        not isinstance(participant, dict)
        or not isinstance(participant.get("agent_id"), int)
        or participant["agent_id"] < 0
        for participant in participants
    ):
        raise ValueError("M6 interaction motion participant IDs must be non-negative integers")
    participant_ids = sorted(participant.get("agent_id") for participant in participants)
    if participant_ids != sorted(layer["target_agent_ids"]):
        raise ValueError("M6 interaction motion participants do not match interaction targets")
    contacts = motion.get("contacts")
    if not isinstance(contacts, list):
        raise ValueError("M6 interaction contacts must be an array")
    seen = set()
    for contact in contacts:
        required = {
            "contact_id",
            "label",
            "owner_agent_id",
            "other_agent_id",
            "tick",
            "distance_m",
        }
        if not isinstance(contact, dict) or set(contact) != required:
            raise ValueError("M6 interaction contact has missing or unknown fields")
        if not contact["contact_id"] or contact["contact_id"] in seen:
            raise ValueError("M6 interaction contact IDs must be non-empty and unique")
        seen.add(contact["contact_id"])
        if contact["owner_agent_id"] not in layer["target_agent_ids"] or contact[
            "other_agent_id"
        ] not in layer["target_agent_ids"]:
            raise ValueError("M6 interaction contact names an undeclared participant")
        if not layer["tick_start"] <= contact["tick"] <= layer["tick_end"]:
            raise ValueError("M6 interaction contact is outside the layer interval")
    provenance = motion.get("provenance")
    if not isinstance(provenance, dict) or not isinstance(provenance.get("backend"), str) or not provenance["backend"]:
        raise ValueError("M6 interaction motion provenance must declare a backend")
    fallback = motion.get("fallback")
    if not isinstance(fallback, dict) or set(fallback) != _FALLBACK_KEYS:
        raise ValueError("M6 interaction motion must declare explicit fallback accounting")
    return motion


def load_layer_bundle(
    interaction_layer_path,
    interaction_motion_path,
    physics_transition_path,
    hero_boundary_path,
    expected_base_hash,
):
    """Load one cache-bound M6 interaction/physics/hero attachment bundle."""
    physics = _physics_module()
    layer = load_layer(interaction_layer_path, expected_base_hash)
    with Path(interaction_motion_path).open(encoding="utf-8") as handle:
        motion = validate_motion_evidence(json.load(handle), layer)
    transition = physics.load_transition(physics_transition_path, expected_base_hash)
    hero = physics.load_hero_boundary(hero_boundary_path)
    owners = sorted(set(layer["target_agent_ids"]) | set(transition["agent_ids"]))
    return {
        "base_cache_hash": expected_base_hash,
        "owner_agent_ids": owners,
        "interaction_interval": [layer["tick_start"], layer["tick_end"]],
        "physics_interval": [transition["tick_start"], transition["tick_end"]],
        "contacts": motion["contacts"],
        "interaction_provenance": layer["provenance"],
        "motion_provenance": motion["provenance"],
        "interaction_fallback": layer["fallback"],
        "motion_fallback": motion["fallback"],
        "physics_solver": transition["solver"],
        "recovery": transition["recovery"],
        "physics_failure_policy": transition["failure_policy"],
        "hero_boundary": hero,
        "interaction_layer": layer,
        "physics_transition": transition,
    }


def build_layout_layers(bundle, physics_samples, muted=False):
    """Lower validated M6 artifacts into native cache-composer layers."""
    layer = bundle["interaction_layer"]
    derived = []
    for edit in sorted(layer["edits"], key=lambda item: (item["tick"], item["agent_id"])):
        derived.append({
            "schema_version": 1,
            "layer_id": "m6-animation-{}-{}-{}".format(
                layer["layer_id"], edit["agent_id"], edit["tick"]
            ),
            "kind": "animation_fix",
            "order": 50,
            "priority": layer["priority"],
            "muted": bool(muted) or not layer["enabled"],
            "solo": False,
            "author": "Blender Crowd M6 artifact attachment",
            "created_at": "derived-from-versioned-m6-artifact",
            "base_cache_hash": bundle["base_cache_hash"],
            "provenance": "{}; {}".format(
                bundle["interaction_provenance"], bundle["motion_provenance"]["backend"]
            ),
            "dependencies": [],
            "stale": False,
            "local_resimulation": None,
            "target": {
                "agent_ids": [edit["agent_id"]],
                "tick_start": edit["tick"],
                "tick_end": edit["tick"],
            },
            "edits": [{
                "type": "animation",
                "clip_id": edit["clip_id"],
                "phase_millionths": edit["phase_millionths"],
            }],
        })
    transition = bundle["physics_transition"]
    if not physics_samples:
        raise ValueError("M6 physics layer requires cached transition samples")
    derived.append({
        "schema_version": 1,
        "layer_id": "m6-physics-{}".format(transition["transition_id"]),
        "kind": "physics",
        "order": 60,
        "priority": layer["priority"],
        "muted": bool(muted),
        "solo": False,
        "author": "Blender Crowd M6 artifact attachment",
        "created_at": "derived-from-versioned-m6-artifact",
        "base_cache_hash": bundle["base_cache_hash"],
        "provenance": "{}; recovery {}".format(
            transition["solver"], transition["recovery"]
        ),
        "dependencies": [layer["layer_id"]],
        "stale": False,
        "local_resimulation": None,
        "target": {
            "agent_ids": list(transition["agent_ids"]),
            "tick_start": transition["tick_start"],
            "tick_end": transition["tick_end"],
        },
        "edits": [{
            "type": "physics_handoff",
            "collision_masks": ["crowd", "ground"],
            "incoming_position": list(physics_samples[0]["position"]),
            "incoming_velocity": list(physics_samples[0]["velocity"]),
            "cached_samples": list(physics_samples),
            "recovery_tick": transition["tick_end"] + 1,
        }],
    })
    return derived


def _physics_module():
    try:
        from . import m6_physics

        return m6_physics
    except ImportError:
        module_path = Path(__file__).with_name("m6_physics.py")
        spec = importlib.util.spec_from_file_location("m6_physics", module_path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module


def write_layer_stack(path, layers):
    if not isinstance(layers, list):
        raise ValueError("M6 interaction layer stack must be a JSON array")
    seen = set()
    for layer in layers:
        validate_layer(layer)
        if layer["layer_id"] in seen:
            raise ValueError("M6 interaction layer IDs must be unique")
        seen.add(layer["layer_id"])
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    temporary = target.with_suffix(target.suffix + ".tmp")
    with temporary.open("w", encoding="utf-8") as handle:
        json.dump(layers, handle, indent=2, sort_keys=True)
        handle.write("\n")
    temporary.replace(target)


def load_layer_stack(path, expected_base_hash=None):
    with Path(path).open(encoding="utf-8") as handle:
        layers = json.load(handle)
    if not isinstance(layers, list):
        raise ValueError("M6 interaction layer stack must be a JSON array")
    for layer in layers:
        validate_layer(layer)
        if expected_base_hash is not None and layer["base_cache_hash"] != expected_base_hash:
            raise ValueError("layer {} belongs to another base cache".format(layer["layer_id"]))
    return layers


def remove_layer(path, layer_id):
    layers = load_layer_stack(path)
    remaining = [layer for layer in layers if layer["layer_id"] != layer_id]
    if len(remaining) == len(layers):
        raise ValueError("M6 interaction layer {} was not found".format(layer_id))
    write_layer_stack(path, remaining)
    return remaining


def _is_hash(value):
    return isinstance(value, str) and len(value) == 64 and all(
        character in "0123456789abcdef" for character in value
    )
