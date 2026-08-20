"""Pure JSON helpers for declared M6 physics/recovery boundaries."""

import json
from pathlib import Path


SCHEMA_VERSION = 1
FAILURE_POLICIES = {"fallback", "reject", "hold"}
_HERO_KEYS = {
    "integration_id",
    "solver",
    "cache_policy",
    "supported_render_tiers",
    "failure_policy",
}


def new_transition_layer(
    transition_id,
    cache_hash,
    agent_ids,
    tick_start,
    tick_end,
    solver,
    recovery,
    failure_policy,
):
    layer = {
        "schema_version": SCHEMA_VERSION,
        "transition_id": str(transition_id),
        "agent_ids": sorted({int(agent_id) for agent_id in agent_ids}),
        "tick_start": int(tick_start),
        "tick_end": int(tick_end),
        "solver": str(solver),
        "cache_hash": str(cache_hash),
        "recovery": str(recovery),
        "failure_policy": str(failure_policy),
    }
    validate_transition(layer, cache_hash)
    return layer


def validate_transition(layer, expected_cache_hash=None):
    if not isinstance(layer, dict):
        raise ValueError("M6 physics transition must be an object")
    if layer.get("schema_version") != SCHEMA_VERSION:
        raise ValueError("unsupported M6 physics transition version")
    for field in ("transition_id", "solver", "recovery"):
        if not isinstance(layer.get(field), str) or not layer[field]:
            raise ValueError("M6 physics transition {} must be non-empty".format(field))
    cache_hash = layer.get("cache_hash")
    if not isinstance(cache_hash, str) or len(cache_hash) != 64 or any(
        character not in "0123456789abcdef" for character in cache_hash
    ):
        raise ValueError("M6 physics transition cache hash must be 64 lowercase hex characters")
    if expected_cache_hash is not None and cache_hash != expected_cache_hash:
        raise ValueError("physics transition belongs to another base cache")
    if not isinstance(layer.get("agent_ids"), list) or not layer["agent_ids"]:
        raise ValueError("M6 physics transition must target stable agents")
    if len(layer["agent_ids"]) != len(set(layer["agent_ids"])):
        raise ValueError("M6 physics transition agent IDs must be unique")
    if layer.get("tick_start", 0) < 0 or layer.get("tick_start") > layer.get("tick_end"):
        raise ValueError("M6 physics transition tick range is invalid")
    if layer.get("failure_policy") not in FAILURE_POLICIES:
        raise ValueError("M6 physics transition failure policy is unsupported")
    return layer


def write_transition(path, layer):
    validate_transition(layer)
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    temporary = target.with_suffix(target.suffix + ".tmp")
    temporary.write_text(json.dumps(layer, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(target)


def load_transition(path, expected_cache_hash):
    with Path(path).open(encoding="utf-8") as handle:
        layer = json.load(handle)
    return validate_transition(layer, expected_cache_hash)


def validate_hero_boundary(boundary):
    """Validate the declared Blender-side support boundary without claiming a solver run."""
    if not isinstance(boundary, dict):
        raise ValueError("M6 hero integration boundary must be an object")
    unknown = set(boundary) - _HERO_KEYS
    missing = _HERO_KEYS - set(boundary)
    if unknown:
        raise ValueError("M6 hero integration has unknown fields: {}".format(", ".join(sorted(unknown))))
    if missing:
        raise ValueError("M6 hero integration is missing fields: {}".format(", ".join(sorted(missing))))
    for field in ("integration_id", "solver", "cache_policy", "failure_policy"):
        if not isinstance(boundary[field], str) or not boundary[field]:
            raise ValueError("M6 hero integration {} must be non-empty".format(field))
    tiers = boundary["supported_render_tiers"]
    if not isinstance(tiers, list) or not tiers or any(
        not isinstance(tier, str) or not tier for tier in tiers
    ):
        raise ValueError("M6 hero integration must declare supported render tiers")
    return boundary


def load_hero_boundary(path):
    with Path(path).open(encoding="utf-8") as handle:
        boundary = json.load(handle)
    return validate_hero_boundary(boundary)
