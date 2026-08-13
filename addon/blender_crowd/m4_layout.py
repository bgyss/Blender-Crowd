"""M4 layer-stack persistence and cache-only interchange helpers.

The layer list is an adjacent artifact, never data written into the base cache.
It can therefore be muted, replaced, or exported without rebaking a shot.
"""

import json
from pathlib import Path
from datetime import datetime, timezone


LAYER_STACK_FILENAME = "layout-layers-v1.json"


def default_layer_stack_path(cache_path):
    return str(Path(cache_path) / "layers" / LAYER_STACK_FILENAME)


def load_layer_stack(path):
    with Path(path).open(encoding="utf-8") as handle:
        layers = json.load(handle)
    if not isinstance(layers, list):
        raise ValueError("M4 layer stack must be a JSON array")
    return layers


def write_layer_stack(path, layers):
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    temporary = target.with_suffix(target.suffix + ".tmp")
    with temporary.open("w", encoding="utf-8") as handle:
        json.dump(layers, handle, indent=2, sort_keys=True)
        handle.write("\n")
    temporary.replace(target)


def attach_layer_stack(playback, path):
    layers = load_layer_stack(path)
    for layer in layers:
        if layer.get("base_cache_hash") != playback.base_cache_hash:
            raise ValueError(
                "layer {} belongs to another base cache; choose its matching cache or migrate it".format(
                    layer.get("layer_id", "<unknown>")
                )
            )
    playback.set_layout_layers(layers)
    return layers


def write_usda(playback, tick, path):
    playback.export_usda(tick, path)


def write_flattened(playback, tick, path):
    """Write a disposable flattened view; the original layer stack is retained."""
    playback.flatten_layout(tick, path)


def status(playback, tick=None):
    return playback.inspect_layout(tick)


def new_transform_layer(
    layer_id, kind, source_hash, agent_ids, tick_start, tick_end, translation,
    operation="additive", order=10, priority=0, dependencies=None,
):
    """Create an inspectable per-agent or bulk correction without raw JSON UI."""
    if not agent_ids:
        raise ValueError("choose at least one stable agent ID")
    if tick_start > tick_end:
        raise ValueError("correction start must not be after its end")
    return {
        "schema_version": 1,
        "layer_id": layer_id,
        "kind": kind,
        "order": int(order),
        "priority": int(priority),
        "muted": False,
        "solo": False,
        "author": "Blender Crowd M4 layout operator",
        "created_at": datetime.now(timezone.utc).isoformat(),
        "base_cache_hash": source_hash,
        "provenance": "viewport numeric correction",
        "dependencies": list(dependencies or []),
        "stale": False,
        "local_resimulation": None,
        "target": {
            "agent_ids": [int(agent_id) for agent_id in agent_ids],
            "tick_start": int(tick_start),
            "tick_end": int(tick_end),
        },
        "edits": [{
            "type": "transform",
            "operation": operation,
            "samples": [{"tick": int(tick_start), "translation": [float(value) for value in translation]}],
        }],
    }


def append_layer(path, layer):
    layers = load_layer_stack(path) if Path(path).is_file() else []
    if any(existing.get("layer_id") == layer["layer_id"] for existing in layers):
        raise ValueError("M4 layer IDs must be unique")
    layers.append(layer)
    write_layer_stack(path, layers)
    return layers


def set_layer_enabled_state(path, layer_index, field, enabled):
    """Persist an explicit mute/solo choice without rewriting any layer edit.

    UI rows are summaries, not the source of truth.  This helper changes the
    selected adjacent layer artifact and returns the reloaded stack for the
    cache composer.  Keeping the operation here makes it usable by Blender
    operators and independently testable without a Blender runtime.
    """
    if field not in {"muted", "solo"}:
        raise ValueError("M4 layer state must be muted or solo")
    layers = load_layer_stack(path)
    if not 0 <= int(layer_index) < len(layers):
        raise ValueError("choose an M4 layer row before changing its state")
    layer = layers[int(layer_index)]
    layer[field] = bool(enabled)
    write_layer_stack(path, layers)
    return layers


def new_scoped_layer(layer_id, kind, source_hash, agent_ids, tick_start, tick_end, edit, order=10, priority=0, dependencies=None):
    if not agent_ids:
        raise ValueError("choose at least one stable agent ID")
    if tick_start > tick_end:
        raise ValueError("operation start must not be after its end")
    return {
        "schema_version": 1, "layer_id": layer_id, "kind": kind, "order": int(order),
        "priority": int(priority), "muted": False, "solo": False,
        "author": "Blender Crowd M4 layout operator", "created_at": datetime.now(timezone.utc).isoformat(),
        "base_cache_hash": source_hash, "provenance": "viewport scoped correction",
        "dependencies": list(dependencies or []), "stale": False, "local_resimulation": None,
        "target": {"agent_ids": [int(agent_id) for agent_id in agent_ids], "tick_start": int(tick_start), "tick_end": int(tick_end)},
        "edits": [edit],
    }


def new_physics_handoff_layer(native, layer_id, source_hash, agent_id, tick_start, tick_end, ticks_per_second, incoming_position, incoming_velocity, masks, restitution_millionths):
    """Create an authored M4 physics-cache interval from selected cache state."""
    if tick_start > tick_end:
        raise ValueError("physics start must not be after its end")
    masks = [mask.strip() for mask in masks if mask.strip()]
    spec = {
        "tick_start": int(tick_start), "tick_end": int(tick_end), "ticks_per_second": int(ticks_per_second),
        "incoming_position": [float(value) for value in incoming_position],
        "incoming_velocity": [float(value) for value in incoming_velocity],
        "gravity_mps2": -9.8, "floor_z": 0.0,
        "restitution_millionths": int(restitution_millionths), "collision_masks": masks,
    }
    samples = json.loads(native.simulate_physics_handoff(json.dumps(spec, sort_keys=True)))
    return new_scoped_layer(
        layer_id, "physics", source_hash, [agent_id], tick_start, tick_end,
        {"type": "physics_handoff", "collision_masks": masks, "incoming_position": spec["incoming_position"],
         "incoming_velocity": spec["incoming_velocity"], "cached_samples": samples, "recovery_tick": int(tick_end) + 1},
        order=40,
    )


def new_local_resimulation_layer(native, layer_id, source_hash, agent_id, tick_start, tick_end, ticks_per_second, incoming_position, incoming_velocity, target_position, max_speed_mps):
    request = {
        "tick_start": int(tick_start), "tick_end": int(tick_end), "ticks_per_second": int(ticks_per_second),
        "incoming_position": [float(value) for value in incoming_position],
        "incoming_velocity": [float(value) for value in incoming_velocity],
        "target_position": [float(value) for value in target_position], "max_speed_mps": float(max_speed_mps),
    }
    samples = json.loads(native.resimulate_local_kinematic(json.dumps(request, sort_keys=True)))
    layer = new_scoped_layer(
        layer_id, "layout", source_hash, [agent_id], tick_start, tick_end,
        {"type": "transform", "operation": "absolute", "samples": samples}, order=10,
    )
    layer["local_resimulation"] = {
        "affected_agent_ids": [agent_id], "tick_start": int(tick_start), "tick_end": int(tick_end),
        "source_base_hash": source_hash, "reason": "bounded local kinematic redirect",
    }
    return layer


def sync_layer_summaries(scene, layers, validity="valid"):
    """Mirror the persisted stack into Blender-visible, saveable UI rows."""
    rows = scene.crowd_project.m4_layers
    rows.clear()
    for layer in sorted(layers, key=lambda item: (item.get("order", 0), item.get("priority", 0), item.get("layer_id", ""))):
        row = rows.add()
        row.layer_id = str(layer.get("layer_id", "<unnamed>"))
        row.kind = str(layer.get("kind", "unknown"))
        row.order = int(layer.get("order", 0))
        row.priority = int(layer.get("priority", 0))
        row.muted = bool(layer.get("muted", False))
        row.solo = bool(layer.get("solo", False))
        row.stale = bool(layer.get("stale", False))
        target = layer.get("target", {})
        row.target_summary = "{} ID(s), ticks {}..{}".format(
            len(target.get("agent_ids", [])), target.get("tick_start", "?"), target.get("tick_end", "?")
        )
        row.provenance = str(layer.get("provenance", ""))
        row.validity = "stale" if layer.get("stale", False) else validity
