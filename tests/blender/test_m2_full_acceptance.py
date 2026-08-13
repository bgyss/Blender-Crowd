"""Full M2 authorable cache, replay, debug, and render acceptance workflow."""

import json
import os
import sys
import time
from hashlib import sha256
from pathlib import Path

import addon_utils
import bpy


EXTENSION = "bl_ext.user_default.blender_crowd"
EXPECTED_AGENTS = 1000


def fail(message):
    print("FAIL: {}".format(message))
    sys.exit(1)


def require(condition, message):
    if not condition:
        fail(message)


def require_rejected(operation, message, expected_text=None):
    """Accept Blender's two Python representations of an operator rejection."""
    try:
        outcome = operation()
    except RuntimeError as error:
        if expected_text is not None:
            require(expected_text.lower() in str(error).lower(), message)
        return
    require(outcome == {"CANCELLED"}, message)


def agent_id_parts(agent_id):
    return agent_id & 0xFFFFFFFF, (agent_id >> 32) & 0xFFFFFFFF


def base_cache_hashes(cache_path):
    """Hash immutable cache artifacts while excluding sparse override layers."""
    result = {}
    for path in sorted(Path(cache_path).rglob("*")):
        if path.is_file() and "overrides" not in path.parts:
            result[str(path.relative_to(cache_path))] = sha256(path.read_bytes()).hexdigest()
    return result


def wait_for_bake(operators, timeout_seconds):
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        outcome = operators.wait_for_bake(timeout=0.25)
        if outcome is not None:
            return outcome
    fail("authorable bake exceeded {} seconds".format(timeout_seconds))


def main():
    addon_utils.enable(EXTENSION, default_set=True)
    from bl_ext.user_default.blender_crowd import debug_overlay, operators, overrides

    cache_path = os.environ["CROWD_M2_CACHE_PATH"]
    output_dir = os.environ["CROWD_M2_RENDER_DIR"]
    report_path = os.environ["CROWD_M2_ACCEPTANCE_REPORT"]
    ticks = int(os.environ.get("CROWD_M2_TICKS", "10000"))
    timeout_seconds = float(os.environ.get("CROWD_M2_BAKE_TIMEOUT_SECONDS", "900"))
    require(ticks == 10000, "M2 acceptance must cover the complete 10,000-tick reference")
    require(not os.path.exists(cache_path), "M2 cache path must not already exist")

    scene = bpy.context.scene
    require(
        bpy.ops.crowd.create_reference_project() == {"FINISHED"},
        "reference authoring project was not created",
    )
    props = scene.crowd_project
    props.cache_path = cache_path
    scene.frame_start = 0
    scene.frame_end = ticks - 1

    started = time.perf_counter()
    bake_start = bpy.ops.crowd.bake_cache()
    require(
        bake_start in ({"FINISHED"}, {"RUNNING_MODAL"}),
        "authorable bake did not start: {} ({})".format(bake_start, props.status),
    )
    outcome = wait_for_bake(operators, timeout_seconds)
    bake_seconds = time.perf_counter() - started
    require(outcome["status"] == "complete", "authorable bake was not complete: {}".format(outcome))

    manifest_path = os.path.join(cache_path, "manifest.json")
    with open(manifest_path, encoding="utf-8") as handle:
        manifest = json.load(handle)
    require(manifest["status"] == "complete", "cache manifest is not complete")
    require(manifest["agent_count"] == EXPECTED_AGENTS, "cache agent count is not 1,000")
    require(manifest["tick_start"] == 0 and manifest["tick_end"] == ticks - 1, "cache tick range is wrong")
    events_def = manifest.get("behavior_events")
    require(events_def and events_def["complete"], "authorable behavior sidecar is missing")
    events_path = os.path.join(cache_path, events_def["path"])
    with open(events_path, encoding="utf-8") as handle:
        event_log = json.load(handle)
    events = event_log["events"]
    require(events, "authorable behavior sidecar has no events")
    kinds = {event["kind"] for event in events}
    require("decision" in kinds, "sidecar lacks decision evidence")
    require("queue_requested" in kinds, "sidecar lacks queue evidence")
    require("group_split" in kinds, "sidecar lacks group evidence")
    require(len(events) < EXPECTED_AGENTS * ticks, "decision sidecar was not transition-compacted")

    require(
        bpy.ops.crowd.attach_cache(filepath=cache_path) == {"FINISHED"},
        "cache-only playback attachment failed",
    )
    # A complete file is still stale when its authoritative scene input has
    # changed. M3 must reject it rather than displaying old crowd geometry.
    original_seed = props.seed
    props.seed = original_seed + 1
    require_rejected(
        lambda: bpy.ops.crowd.attach_cache(filepath=cache_path),
        "stale cache attached after project input changed",
        "stale",
    )
    require(not props.cache_attached, "stale cache remained marked authoritative")
    props.seed = original_seed
    require(
        bpy.ops.crowd.attach_cache(filepath=cache_path) == {"FINISHED"},
        "cache did not attach after the original project input was restored",
    )
    playback = operators.active_cache_playback()
    require(playback.agent_count == EXPECTED_AGENTS, "cache playback agent count is wrong")
    require(not hasattr(playback, "session"), "cache playback retained a simulation session")
    native_cache = __import__("blender_crowd_native").Cache(cache_path)
    midpoint = (native_cache.tick_start + native_cache.tick_end) // 2
    playback.sync_to_tick(midpoint)
    require(playback.current_tick == midpoint, "cache did not replay midpoint tick")
    require(native_cache.read_tick(midpoint)["position"], "native cache replay has no positions")
    playback_started = time.perf_counter()
    scanned_ticks = native_cache.scan_ticks()
    playback_seconds = time.perf_counter() - playback_started
    require(scanned_ticks == ticks, "sequential cache scan did not cover every tick")
    sequential_cache_ticks_per_second = scanned_ticks / playback_seconds

    decision = next(event for event in events if event["kind"] == "decision")
    agent_id = decision["agent_id"]
    props.selected_agent_id_lo, props.selected_agent_id_hi = agent_id_parts(agent_id)
    scene.frame_set(scene.frame_start + (decision["tick"] - native_cache.tick_start))
    require(bpy.ops.crowd.inspect_agent() == {"FINISHED"}, "selected-agent inspection failed")
    evidence = debug_overlay.active_evidence()
    require(evidence and evidence["agent_id"] == agent_id, "debug overlay did not expose cached agent evidence")
    trace = evidence.get("behavior_events")
    require(trace and trace[0]["graph_id"], "debug overlay did not expose durable graph evidence")
    debug_query_count = 25
    debug_started = time.perf_counter()
    for _index in range(debug_query_count):
        require(
            debug_overlay.inspect(playback, agent_id, decision["tick"]),
            "cached debug inspection returned no evidence",
        )
    debug_inspection_seconds_per_query = (
        time.perf_counter() - debug_started
    ) / debug_query_count

    before_override = base_cache_hashes(cache_path)
    pin = bpy.data.objects.new("M2 Acceptance Hero Pin", None)
    scene.collection.objects.link(pin)
    pin.location = (0.25, 0.0, 0.0)
    bpy.context.view_layer.objects.active = pin
    pin.select_set(True)
    require(
        bpy.ops.crowd.pin_selected_agent() == {"FINISHED"},
        "cache-only sparse correction failed",
    )
    override_path = overrides.default_layer_path(cache_path)
    require(os.path.isfile(override_path), "sparse correction layer was not written")
    require(
        base_cache_hashes(cache_path) == before_override,
        "sparse correction mutated a base-cache artifact",
    )

    require(
        bpy.ops.crowd.render_reference_frame(output_dir=output_dir) == {"FINISHED"},
        "cache-only reference render failed",
    )
    metrics_path = os.path.join(output_dir, "m1-render-metrics.json")
    with open(metrics_path, encoding="utf-8") as handle:
        render_metrics = json.load(handle)
    require(render_metrics["cache_only"] is True, "render metrics do not prove cache-only playback")
    require(render_metrics["agent_count"] == EXPECTED_AGENTS, "render did not use 1,000 agents")
    require(set(render_metrics["renders"]) == {"eevee", "cycles"}, "both renderers were not measured")
    for renderer in render_metrics["renders"].values():
        require(os.path.getsize(renderer["output_path"]) > 1024, "render output is too small")

    depsgraph_base_hashes = base_cache_hashes(cache_path)
    depsgraph_tick = playback.current_tick
    for _index in range(50):
        bpy.context.view_layer.update()
        tuple(bpy.context.evaluated_depsgraph_get().updates)
    require(
        base_cache_hashes(cache_path) == depsgraph_base_hashes,
        "dependency-graph evaluation mutated authoritative cache data",
    )
    require(
        playback.current_tick == depsgraph_tick,
        "dependency-graph evaluation advanced authoritative playback state",
    )

    missing_prototypes = [
        item for item in bpy.data.objects if item.get("crowd_asset_kind") == "prototype"
    ]
    require(missing_prototypes, "reference render has no prototype assets to remove")
    for missing_prototype in missing_prototypes:
        bpy.data.objects.remove(missing_prototype, do_unlink=True)
    require_rejected(
        lambda: bpy.ops.crowd.render_reference_frame(output_dir=output_dir),
        "missing presentation asset did not stop the render",
        "instances",
    )

    corrupt_chunk = next(Path(cache_path, "frames").glob("*.chunk"))
    with corrupt_chunk.open("r+b") as handle:
        first = handle.read(1)
        handle.seek(0)
        handle.write(bytes([first[0] ^ 0x01]))
    require(
        bpy.ops.crowd.inspect_cache_health() == {"FINISHED"},
        "corrupt cache could not be inspected",
    )
    require(props.cache_status == "corrupt", "corrupt cache was not labeled corrupt")
    require_rejected(
        lambda: bpy.ops.crowd.attach_cache(filepath=cache_path),
        "corrupt cache attached as authoritative playback",
        "do not attach",
    )

    report = {
        "schema_version": 1,
        "agent_count": EXPECTED_AGENTS,
        "ticks": ticks,
        "authorable_bake_seconds": bake_seconds,
        "sequential_cache_ticks_per_second": sequential_cache_ticks_per_second,
        "debug_inspection_seconds_per_query": debug_inspection_seconds_per_query,
        "cache_status": manifest["status"],
        "behavior_event_count": len(events),
        "behavior_event_kinds": sorted(kinds),
        "selected_agent_id": agent_id,
        "selected_agent_tick": decision["tick"],
        "cache_only_render": render_metrics["cache_only"],
        "sparse_override_base_cache_unchanged": True,
        "stale_cache_rejected": True,
        "corrupt_cache_rejected": True,
        "dependency_graph_stress_passed": True,
        "missing_asset_rejected": True,
        "render_metrics_path": metrics_path,
        "acceptance_subgate_passed": True,
        "m2_milestone_accepted": False,
    }
    with open(report_path, "w", encoding="utf-8") as handle:
        json.dump(report, handle, indent=2, sort_keys=True)
        handle.write("\n")
    print("M2 full acceptance subgate: PASS {}".format(report_path))


try:
    main()
except SystemExit:
    raise
except Exception as error:
    fail("unexpected {}: {}".format(type(error).__name__, error))
