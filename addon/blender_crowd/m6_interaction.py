"""M6 interaction-layer persistence without a Blender or model dependency.

The helpers deliberately operate on plain JSON-compatible values. Blender
operators can call them from a coarse-grained action, while tests and offline
workers can use the same validation and cache-isolation rules.
"""

import json
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
