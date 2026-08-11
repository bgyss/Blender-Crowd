"""End-to-end check of the built `blender_crowd_native` wheel.

Deliberately reads the trace a second time with hand-rolled `struct` parsing
instead of trusting the extension module's own numbers. A bridge that agrees
with itself proves nothing; the point is to confirm that what crosses the FFI
boundary still matches the bytes on disk, in the layout `numpy.frombuffer`
plus `foreach_set` will assume. It then drives the coarse project/session/cache
facade, including cross-thread cancellation while the native bake releases the
GIL.

Stdlib only: this must run in a bare interpreter, and in Blender's Python
(task 7) where third-party packages are not a given.

Usage: python scripts/verify_wheel.py <trace-file>
"""

import glob
import os
from pathlib import Path
import struct
import sys
import tempfile
import threading
import time

import blender_crowd_native

HEADER_BYTES = 32
RECORD_BYTES = 35
MAGIC = b"CRWDTRC0"

# Per-agent element counts, matching what the addon binds to Blender point
# attributes. Everything is 4 bytes wide: floats are f32 and integers are i32,
# because a Blender point attribute has no other integer width.
CHANNELS = {
    "position": 3,
    "orientation": 1,
    "agent_id_lo": 1,
    "agent_id_hi": 1,
    "flags": 1,
    "clip_index": 1,
    "phase": 1,
    "playback_rate": 1,
    "render_tier": 1,
}

CACHE_CHANNELS = {
    "position": 3,
    "orientation": 1,
    "scale": 1,
    "agent_id_lo": 1,
    "agent_id_hi": 1,
    "population_id": 1,
    "archetype_id": 1,
    "variant_id": 1,
    "spawn_ordinal": 1,
    "clip_id": 1,
    "phase": 1,
    "playback_rate": 1,
    "behavior_state": 1,
    "decision_reason": 1,
    "destination_id": 1,
    "velocity": 3,
    "visible": 1,
    "render_tier": 1,
}


class Failure(Exception):
    pass


def check(condition, message):
    if not condition:
        raise Failure(message)
    print(f"  ok: {message}")


def read_header(path):
    with open(path, "rb") as handle:
        raw = handle.read(HEADER_BYTES)
    magic, version, ticks, agents, tps, scale = struct.unpack("<8sIQIIf", raw)
    check(magic == MAGIC, f"file magic is {MAGIC!r}")
    check(version == 0, "trace format version is 0")
    return ticks, agents, tps, scale


def read_record(path, tick, index, agent_count):
    offset = HEADER_BYTES + (tick * agent_count + index) * RECORD_BYTES
    with open(path, "rb") as handle:
        handle.seek(offset)
        raw = handle.read(RECORD_BYTES)
    agent_id, x, y, orientation, flags, clip, phase, rate, tier = struct.unpack(
        "<QfffIHffB", raw
    )
    return agent_id, x, y, orientation, flags, clip, phase, rate, tier


def main(path):
    ticks, agents, tps, scale = read_header(path)
    print(f"trace {path}: {ticks} ticks, {agents} agents")

    print("module:")
    check(
        isinstance(blender_crowd_native.__version__, str),
        f"__version__ is a str ({blender_crowd_native.__version__})",
    )

    trace = blender_crowd_native.Trace(path)
    print("header:")
    check(trace.tick_count == ticks, f"tick_count == {ticks}")
    check(trace.agent_count == agents, f"agent_count == {agents}")
    check(trace.ticks_per_second == tps, f"ticks_per_second == {tps}")
    check(abs(trace.world_to_meter - scale) < 1e-9, f"world_to_meter == {scale}")

    buffers = trace.read_tick(0)
    print("read_tick(0):")
    check(
        set(buffers) == set(CHANNELS),
        f"returns exactly {len(CHANNELS)} channels",
    )
    for name, per_agent in CHANNELS.items():
        expected = agents * per_agent * 4
        check(
            isinstance(buffers[name], bytes) and len(buffers[name]) == expected,
            f"{name} is {expected} bytes ({per_agent} x 4 per agent)",
        )

    # Check every spawned slot, not just one: a bug where every agent gets
    # agent 0's values would pass a single-slot check trivially, because
    # agent 0's own offset is 0. Unspawned slots are all-zero padding
    # (flags == 0) and are skipped, per trace v0's padding contract.
    spawned = [
        candidate
        for candidate in range(agents)
        if read_record(path, 0, candidate, agents)[4] != 0
    ]
    check(len(spawned) > 0, "tick 0 has at least one spawned agent")

    def field(name, index, offset=0, count=1, fmt="i"):
        return struct.unpack_from(f"<{fmt}", buffers[name], (index * count + offset) * 4)[0]

    print(f"agent slots ({len(spawned)} spawned of {agents}):")
    for index in spawned:
        on_disk = read_record(path, 0, index, agents)
        agent_id, x, y, orientation, flags, clip, phase, rate, tier = on_disk

        lo = field("agent_id_lo", index) & 0xFFFFFFFF
        hi = field("agent_id_hi", index) & 0xFFFFFFFF
        check(
            lo | (hi << 32) == agent_id,
            f"slot {index}: agent_id_lo | (agent_id_hi << 32) == {agent_id}",
        )
        check(
            field("position", index, 0, 3, "f") == x,
            f"slot {index}: position.x == {x}",
        )
        check(
            field("position", index, 1, 3, "f") == y,
            f"slot {index}: position.y == {y}",
        )
        check(
            field("position", index, 2, 3, "f") == 0.0,
            f"slot {index}: position.z == 0.0",
        )
        check(
            field("orientation", index, fmt="f") == orientation,
            f"slot {index}: orientation == {orientation}",
        )
        check(field("flags", index) == flags, f"slot {index}: flags == {flags}")
        check(field("clip_index", index) == clip, f"slot {index}: clip_index == {clip}")
        check(
            field("phase", index, fmt="f") == phase,
            f"slot {index}: phase == {phase}",
        )
        check(
            field("playback_rate", index, fmt="f") == rate,
            f"slot {index}: playback_rate == {rate}",
        )
        check(field("render_tier", index) == tier, f"slot {index}: render_tier == {tier}")

    print("errors:")
    try:
        blender_crowd_native.Trace(path + ".does-not-exist")
    except OSError as exc:
        print(f"  ok: missing file raises OSError ({exc})")
    else:
        raise Failure("missing file did not raise OSError")
    try:
        trace.read_tick(ticks)
    except OSError as exc:
        print(f"  ok: out-of-range tick raises OSError ({exc})")
    else:
        raise Failure("out-of-range tick did not raise OSError")
    try:
        trace.read_tick(-1)
    except OSError as exc:
        print(f"  ok: negative tick raises OSError ({exc})")
    else:
        raise Failure("negative tick did not raise OSError")

    print("project/session/cache facade:")
    project_path = Path(__file__).resolve().parents[1] / "assets/reference/concourse-project-v1.json"
    project_json = project_path.read_text(encoding="utf-8")
    project = blender_crowd_native.compile_project(project_json)
    check(project.agent_count == 1000, "reference project compiles to 1,000 agents")
    agent_ids = project.agent_ids()
    check(len(agent_ids) == 1000 and len(set(agent_ids)) == 1000, "compiled agent IDs are unique")

    session = project.create_session(agent_count=25)
    check(session.agent_count == 25, "strict session contains 25 requested agents")
    session.step(10)
    snapshot = session.query_agent(agent_ids[0])
    check(snapshot["agent_id"] == agent_ids[0], "query_agent preserves the stable ID")
    check(snapshot["visible"] is True, "queried agent has spawned after ten ticks")

    with tempfile.TemporaryDirectory(prefix="blender-crowd-wheel-") as temp:
        complete_path = os.path.join(temp, "complete.crowd")
        complete_result = session.bake(
            complete_path,
            ticks=60,
            cancel_token=blender_crowd_native.CancelToken(),
        )
        check(complete_result["status"] == "complete", "facade bake completes")
        del session

        cache = blender_crowd_native.Cache(complete_path, require_complete=True)
        check(cache.agent_count == 25, "complete cache reopens after session destruction")
        buffers = cache.read_tick(cache.tick_start)
        check(set(buffers) == set(CACHE_CHANNELS), "cache returns every v1 playback channel")
        for name, per_agent in CACHE_CHANNELS.items():
            expected = cache.agent_count * per_agent * 4
            check(
                isinstance(buffers[name], bytes) and len(buffers[name]) == expected,
                f"cache {name} is {expected} bytes",
            )

        canceled_path = os.path.join(temp, "canceled.crowd")
        token = blender_crowd_native.CancelToken()
        worker_result = {}
        worker_error = []
        ready = threading.Event()

        def bake_worker():
            try:
                canceled_session = project.create_session(agent_count=25)
                ready.set()
                worker_result.update(
                    canceled_session.bake(canceled_path, ticks=6000, cancel_token=token)
                )
            except Exception as error:  # surfaced below with its exact repr
                worker_error.append(error)

        worker = threading.Thread(target=bake_worker, name="crowd-bake-verifier")
        worker.start()
        check(ready.wait(timeout=5), "cancellation worker entered bake setup")
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if glob.glob(os.path.join(canceled_path, "frames", "*.chunk")):
                break
            if not worker.is_alive():
                break
            time.sleep(0.005)
        check(
            bool(glob.glob(os.path.join(canceled_path, "frames", "*.chunk"))),
            "cancellation waits for one atomically published chunk",
        )
        token.cancel()
        worker.join(timeout=15)
        check(not worker.is_alive(), "canceled bake worker joins")
        check(not worker_error, f"canceled bake raised no worker error ({worker_error!r})")
        check(worker_result.get("status") == "canceled", "bake reports canceled")

        try:
            blender_crowd_native.Cache(canceled_path, require_complete=True)
        except OSError as exc:
            print(f"  ok: complete reader rejects canceled cache ({exc})")
        else:
            raise Failure("complete reader accepted canceled cache")
        recovery = blender_crowd_native.inspect_cache(canceled_path)
        check(recovery["status"] == "canceled", "recovery inspector reports canceled")
        check(recovery["valid_chunk_count"] >= 1, "recovery inspector preserves completed chunks")

    print("PASS")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    try:
        main(sys.argv[1])
    except Failure as failure:
        sys.exit(f"FAIL: {failure}")
