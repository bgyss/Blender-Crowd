"""End-to-end check of the built `blender_crowd_native` wheel.

Deliberately reads the trace a second time with hand-rolled `struct` parsing
instead of trusting the extension module's own numbers. A bridge that agrees
with itself proves nothing; the point is to confirm that what crosses the FFI
boundary still matches the bytes on disk, in the layout `numpy.frombuffer`
plus `foreach_set` will assume.

Stdlib only: this must run in a bare interpreter, and in Blender's Python
(task 7) where third-party packages are not a given.

Usage: python scripts/verify_wheel.py <trace-file>
"""

import struct
import sys

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

    # Pick an agent that actually exists: unspawned slots are all-zero padding,
    # so a zero id there would make the round trip pass without proving it.
    index = None
    for candidate in range(agents):
        if read_record(path, 0, candidate, agents)[0] != 0:
            index = candidate
            break
    check(index is not None, "tick 0 has at least one spawned agent")

    on_disk = read_record(path, 0, index, agents)
    agent_id, x, y, orientation, flags, clip, phase, rate, tier = on_disk

    def field(name, offset=0, count=1, fmt="i"):
        return struct.unpack_from(f"<{fmt}", buffers[name], (index * count + offset) * 4)[0]

    print(f"agent slot {index} (id {agent_id}):")
    lo = field("agent_id_lo") & 0xFFFFFFFF
    hi = field("agent_id_hi") & 0xFFFFFFFF
    check(lo | (hi << 32) == agent_id, f"agent_id_lo | (agent_id_hi << 32) == {agent_id}")
    check(field("position", 0, 3, "f") == x, f"position.x == {x}")
    check(field("position", 1, 3, "f") == y, f"position.y == {y}")
    check(field("position", 2, 3, "f") == 0.0, "position.z == 0.0")
    check(field("orientation", fmt="f") == orientation, f"orientation == {orientation}")
    check(field("flags") == flags, f"flags == {flags}")
    check(field("clip_index") == clip, f"clip_index == {clip}")
    check(field("phase", fmt="f") == phase, f"phase == {phase}")
    check(field("playback_rate", fmt="f") == rate, f"playback_rate == {rate}")
    check(field("render_tier") == tier, f"render_tier == {tier}")

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

    print("PASS")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    try:
        main(sys.argv[1])
    except Failure as failure:
        sys.exit(f"FAIL: {failure}")
