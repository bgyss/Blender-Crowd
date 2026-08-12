"""Privacy-preserving support diagnostics for release triage."""

import json
from pathlib import Path

import bpy


def write_bundle(scene, filepath):
    """Write operational state without serializing scene content or user paths."""
    props = scene.crowd_project
    target = Path(bpy.path.abspath(filepath))
    target.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "schema_version": 1,
        "product": "Blender Crowd",
        "product_version": "1.0.0",
        "blender_version": bpy.app.version_string,
        "cache": {
            "status": props.cache_status,
            "artifact_name": Path(props.cache_resolved_path or props.cache_path).name,
            "source_hash": props.cache_source_hash,
            "readable_tick_range": props.cache_readable_range,
            "last_complete_tick": props.cache_last_complete_tick,
            "valid_chunk_count": props.cache_valid_chunk_count,
            "measured_disk_size": props.cache_disk_size,
            "attached": props.cache_attached,
        },
        "workflow": {
            "stage": props.current_stage,
            "next_action": props.next_action,
            "minimum_playback_buffer": props.playback_buffer_estimate,
        },
        # Full details and paths can contain private scene or filesystem data.
        "diagnostics": [
            {"sequence": item.sequence, "severity": item.severity, "summary": item.summary}
            for item in props.diagnostics
        ],
        "privacy": {
            "scene_contents_included": False,
            "absolute_paths_included": False,
            "user_authored_diagnostic_details_included": False,
        },
    }
    target.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return target
