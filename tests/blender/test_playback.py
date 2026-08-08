"""Play a 1,000-agent trace back through Geometry Nodes point attributes.

Runs inside Blender via `--python`. Automates M0 acceptance criterion 6:
Blender plays 1,000 cached point transforms with stable IDs, and simulation
and playback costs are reported separately.
"""

import os
import sys
import time

import numpy as np

import addon_utils
import bpy

EXTENSION = "bl_ext.user_default.blender_crowd"
EXPECTED_AGENTS = 1000


def fail(message):
    print("FAIL: {}".format(message))
    sys.exit(1)


def main():
    addon_utils.enable(EXTENSION, default_set=True)
    from bl_ext.user_default.blender_crowd.trace_playback import TracePlayback

    trace_path = os.environ["CROWD_TRACE_PATH"]
    playback = TracePlayback(trace_path)

    if playback.agent_count != EXPECTED_AGENTS:
        fail("expected {} agents, got {}".format(EXPECTED_AGENTS, playback.agent_count))

    data = playback.object.data
    positions = np.empty(playback.agent_count * 3, dtype=np.float32)
    ids_lo = np.empty(playback.agent_count, dtype=np.int32)
    ids_hi = np.empty(playback.agent_count, dtype=np.int32)

    # Scenes spawn agents on a stagger (see `SpawnRegion::per_tick` in
    # crowd-core), so tick 0 has only a handful of agents assigned real IDs;
    # the rest of the point cloud reads as the all-zero padding record
    # documented in crowd-trace until that slot's agent spawns. That is
    # expected spawn-ramp behaviour, not ID instability. Find the first tick
    # where every slot has a real (non-zero) ID and use THAT as the
    # stability baseline, not tick 0. This scan runs before the timed
    # playback loop below, so it does not pollute the playback timing.
    settle_tick = None
    for tick in range(playback.tick_count):
        playback.sync_to_tick(tick)
        data.attributes["agent_id_lo"].data.foreach_get("value", ids_lo)
        data.attributes["agent_id_hi"].data.foreach_get("value", ids_hi)
        if np.all(ids_lo | ids_hi):
            settle_tick = tick
            break
    if settle_tick is None:
        fail("not every agent had spawned by the end of the trace")
    first_ids = (ids_lo.copy(), ids_hi.copy())

    # Time every tick. This measures Blender-side playback only: the
    # simulation cost is reported separately by the calling script, because
    # conflating them is exactly what M0 criterion 6 forbids.
    start = time.perf_counter()
    for tick in range(playback.tick_count):
        playback.sync_to_tick(tick)
    elapsed = time.perf_counter() - start

    # Stable IDs must not drift across playback. Read them at the LAST tick
    # the loop reached and compare against the settle-tick baseline: re-
    # syncing to the settle tick and re-reading it would only prove the
    # reader is repeatable, which is not the invariant at issue.
    data.attributes["agent_id_lo"].data.foreach_get("value", ids_lo)
    data.attributes["agent_id_hi"].data.foreach_get("value", ids_hi)
    if not np.array_equal(ids_lo, first_ids[0]) or not np.array_equal(ids_hi, first_ids[1]):
        fail("agent IDs changed between the settle tick and the last tick of playback")

    # Positions must match the Rust reader exactly, not approximately. The
    # timed loop above left `data` synced to the LAST tick, so re-sync to
    # tick 0 before comparing -- otherwise this would compare the final
    # tick's Blender-side positions against the Rust reader's tick 0, which
    # is not a real invariant.
    import blender_crowd_native

    playback.sync_to_tick(0)
    reference = blender_crowd_native.Trace(trace_path).read_tick(0)
    expected = np.frombuffer(reference["position"], dtype=np.float32)
    data.attributes["position"].data.foreach_get("vector", positions)
    if not np.array_equal(positions, expected):
        fail("point positions do not match the Rust reader")

    per_tick_ms = (elapsed / max(playback.tick_count, 1)) * 1000.0
    print("agents: {}".format(playback.agent_count))
    print("ticks: {}".format(playback.tick_count))
    print("blender_playback_total_s: {:.4f}".format(elapsed))
    print("blender_playback_per_tick_ms: {:.4f}".format(per_tick_ms))
    print("PASS: 1,000-point playback with stable IDs")


main()
