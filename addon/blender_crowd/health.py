"""Persistent, actionable project-health state for the production workflow.

This module deliberately stores its summaries in scene properties.  Blender's
status bar is transient, while cache identity and recovery decisions must still
be visible after save/reload or when a project has moved on disk.
"""

import json
from pathlib import Path

import bpy


MAX_DIAGNOSTICS = 64
TROUBLESHOOTING_DOC = "docs/user/m3-production-recovery.md"
# Cache playback uploads position twice (PointCloud position plus named Crowd
# position), nine i32 channels, and four f32 channels. This is the exact
# point-attribute minimum, excluding host/Geometry Nodes allocator overhead.
POINT_ATTRIBUTE_BYTES_PER_AGENT = 76


def record(scene, severity, summary, detail="", filepath="", object_name="", documentation=TROUBLESHOOTING_DOC):
    """Persist an actionable diagnostic and update the compact project summary."""
    props = scene.crowd_project
    while len(props.diagnostics) >= MAX_DIAGNOSTICS:
        props.diagnostics.remove(0)
    item = props.diagnostics.add()
    item.sequence = props.diagnostic_sequence + 1
    props.diagnostic_sequence = item.sequence
    item.severity = severity
    item.summary = summary
    item.detail = detail
    item.filepath = filepath
    item.object_name = object_name
    item.documentation = documentation
    props.status = "{}: {}".format(severity.title(), summary)
    return item


def set_workflow(scene, stage, next_action, estimate="", progress=0.0):
    props = scene.crowd_project
    props.current_stage = stage
    props.next_action = next_action
    props.operation_estimate = estimate
    props.operation_progress = max(0.0, min(1.0, float(progress)))


def set_selection(scene, label):
    """Persist the meaningful Crowd selection context across panel changes."""
    scene.crowd_project.selection_context = label


def _byte_label(byte_count):
    if byte_count < 1024:
        return "{} B".format(byte_count)
    if byte_count < 1024 * 1024:
        return "{:.1f} KiB".format(byte_count / 1024)
    return "{:.1f} MiB".format(byte_count / (1024 * 1024))


def set_bake_estimate(scene, agent_count, ticks):
    """Expose an exact minimum buffer cost without guessing compression ratio."""
    props = scene.crowd_project
    point_bytes = int(agent_count) * POINT_ATTRIBUTE_BYTES_PER_AGENT
    props.playback_buffer_estimate = "{} minimum point attributes".format(
        _byte_label(point_bytes)
    )
    props.operation_estimate = "{} agents × {} ticks; {}. Cache disk size is measured after bake.".format(
        agent_count, ticks, props.playback_buffer_estimate
    )


def _cache_size(path):
    root = Path(path)
    if not root.is_dir():
        return 0
    total = 0
    try:
        for item in root.rglob("*"):
            try:
                if item.is_file():
                    total += item.stat().st_size
            except OSError:
                continue
    except OSError:
        return total
    return total


def inspect_cache(scene, cache_path):
    """Describe a cache without opening it as playback.

    Inspection is intentionally separate from attachment: incomplete and
    canceled caches retain recovery information but can never be mistaken for
    authoritative playable geometry.
    """
    props = scene.crowd_project
    resolved = bpy.path.abspath(cache_path)
    props.cache_resolved_path = resolved
    props.cache_attached = False
    props.cache_disk_size = _byte_label(_cache_size(resolved))
    try:
        import blender_crowd_native

        report = dict(blender_crowd_native.inspect_cache(resolved))
    except (OSError, RuntimeError, ValueError) as error:
        error_text = str(error)
        props.cache_status = "unsupported" if "unsupported" in error_text.lower() else "missing_or_invalid"
        props.cache_recovery_hint = (
            "This cache schema is unsupported; use a documented migration or rebake."
            if props.cache_status == "unsupported"
            else "Choose a valid cache or rebake the project."
        )
        record(scene, "ERROR", "Cache cannot be inspected", str(error), resolved)
        return None

    status = str(report.get("status", "unknown"))
    props.cache_status = status
    try:
        with open(Path(resolved) / "manifest.json", encoding="utf-8") as handle:
            props.cache_source_hash = str(json.load(handle).get("source_hash") or "")
    except (OSError, ValueError, json.JSONDecodeError):
        props.cache_source_hash = ""
    props.cache_last_complete_tick = int(report.get("last_complete_tick") or 0)
    props.cache_valid_chunk_count = int(report.get("valid_chunk_count") or 0)
    start = report.get("readable_tick_start")
    end = report.get("readable_tick_end")
    props.cache_readable_range = "{}..{}".format(start, end) if start is not None else "none"
    if status == "complete":
        props.cache_recovery_hint = "Complete cache is safe to attach for playback."
    else:
        reason = (
            report.get("integrity_error")
            or report.get("cancellation_reason")
            or "cache is not complete"
        )
        props.cache_recovery_hint = (
            "Do not attach this cache. {} Valid chunks: {}; readable ticks: {}. "
            "Rebake to create an authoritative cache."
        ).format(reason, props.cache_valid_chunk_count, props.cache_readable_range)
    return report


def cache_path_label(path):
    """A short, stable label for diagnostics without exposing unrelated paths."""
    try:
        return Path(path).name or str(path)
    except TypeError:
        return "cache"
