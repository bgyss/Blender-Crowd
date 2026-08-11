"""Blender authoring for versioned sparse transform override layers."""

from datetime import datetime, timezone
import json
from pathlib import Path


LAYER_FILENAME = "hero-pin-v1.json"


def selected_agent_id(project_properties):
    low = int(project_properties.selected_agent_id_lo) & 0xFFFFFFFF
    high = int(project_properties.selected_agent_id_hi) & 0xFFFFFFFF
    return low | (high << 32)


def default_layer_path(cache_path):
    return str(Path(cache_path) / "overrides" / LAYER_FILENAME)


def load_layer(path):
    with Path(path).open(encoding="utf-8") as handle:
        return json.load(handle)


def _write_layer(path, layer):
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    temporary = target.with_suffix(target.suffix + ".tmp")
    with temporary.open("w", encoding="utf-8") as handle:
        json.dump(layer, handle, indent=2, sort_keys=True)
        handle.write("\n")
    temporary.replace(target)


def write_pin_layer(scene, authored_object, playback):
    """Sample one object's world translation and attach the resulting layer."""
    props = scene.crowd_project
    tick_start = int(props.override_tick_start)
    tick_end = int(props.override_tick_end)
    if tick_start > tick_end:
        raise ValueError("override start must not be after override end")
    previous_frame = scene.frame_current
    samples = []
    try:
        for tick in range(tick_start, tick_end + 1):
            scene.frame_set(tick)
            translation = authored_object.matrix_world.translation
            samples.append(
                {
                    "tick": tick,
                    "translation": [
                        float(translation.x),
                        float(translation.y),
                        float(translation.z),
                    ],
                }
            )
    finally:
        scene.frame_set(previous_frame)

    layer = {
        "schema_version": 1,
        "layer_id": "hero-pin",
        "author": "Blender Crowd M1 pin operator",
        "created_at": datetime.now(timezone.utc).isoformat(),
        "priority": 100,
        "enabled": bool(props.override_enabled),
        "target_agent_id": selected_agent_id(props),
        "tick_start": tick_start,
        "tick_end": tick_end,
        "operation": "additive",
        "samples": samples,
    }
    path = default_layer_path(props.cache_path)
    _write_layer(path, layer)
    playback.set_override_layers([layer])
    return path, layer
