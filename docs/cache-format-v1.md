# Crowd Cache v1

Crowd Cache v1 is the native, versioned playback cache used between the Rust
simulation and Blender presentation layers. It is a directory format: static
agent identity is stored once, dynamic channels are split into independently
validated tick chunks, and a JSON manifest is the publication record.

The measured default is 120 ticks per chunk with affine 16-bit positions. The
selection comes from the checked 1,000-agent experiment in
[`benchmarks/2026-08-10-cache-v0-experiment.md`](benchmarks/2026-08-10-cache-v0-experiment.md),
not from an assumed compression target.

## Directory layout

```text
cache-directory/
├── manifest.json
├── agents.bin
└── frames/
    ├── 000000-000119.chunk
    ├── 000120-000239.chunk
    └── ...
```

Files are published as a sibling `*.tmp`, flushed with `sync_all`, and renamed
to their final path. The manifest is republished after each completed file.
Recovery uses only files declared complete in the manifest and ignores orphan
temporary files.

## Manifest contract

`manifest.json` conforms to
[`schemas/cache-manifest-v1.schema.json`](../schemas/cache-manifest-v1.schema.json).
Its `schema_version` is `1`, and unknown fields are rejected at versioned object
boundaries. It records:

- engine/project/source identity;
- inclusive tick range and ticks per second;
- agent count and channel declarations;
- the static agent-table path, byte length, CRC-32C, and completion bit;
- every chunk's inclusive range, path, byte length, CRC-32C, and completion bit;
- one of `incomplete`, `canceled`, or `complete`;
- a cancellation reason and last complete tick when applicable.

A `complete` manifest is valid only when the agent table and every declared
chunk are complete. A canceled cache is never accepted by the complete reader.
The recovery inspector instead reports the longest contiguous, checksummed
prefix and its readable tick range. Cancellation before the first tick records
no fabricated last-complete tick.

## Integer and checksum conventions

All binary integers and floats are little-endian. Payload and whole-file
integrity use CRC-32C. BLAKE3 is used for content identity, not as a substitute
for each file's corruption check. Length and count arithmetic is checked before
allocation or slicing.

## Static agent table

`agents.bin` begins with a 32-byte header.

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 8 | Magic `BCAGT\0\x01\0` |
| 8 | 1 | Endianness (`1` = little-endian) |
| 9 | 1 | Reserved, zero |
| 10 | 2 | Agent-table version (`1`) |
| 12 | 4 | Agent count |
| 16 | 8 | Payload byte length |
| 24 | 4 | Payload CRC-32C |
| 28 | 4 | Reserved, zero |

Each 28-byte payload record is ordered by stable cache slot and contains:
`agent_id: u64`, `population_id: u32`, `archetype_id: u32`, `variant_id: u32`,
`base_scale: f32`, and `spawn_ordinal: u32`. IDs must be unique. Every dynamic
frame must use exactly the same ID in each corresponding slot.

## Frame chunk header

Each frame chunk begins with a 64-byte header.

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 8 | Magic `BCFRM\0\x01\0` |
| 8 | 1 | Endianness (`1` = little-endian) |
| 9 | 1 | Position encoding (`0` f32, `1` millimetre i32, `2` affine i16) |
| 10 | 2 | Chunk version (`1`) |
| 12 | 8 | First tick |
| 20 | 4 | Tick count |
| 24 | 4 | Agent count |
| 28 | 8 | Record count (`ticks * agents`) |
| 36 | 8 | Payload byte length |
| 44 | 4 | Payload CRC-32C |
| 48 | 8 | Position origin, two f32 values |
| 56 | 8 | Position scale/declared-bound metadata, two f32 values |

The checksum covers the payload after byte 63. Header magic, version,
endianness, counts, lengths, and discrete boolean values are validated before a
chunk is returned.

## Channel-major payload

Records are flattened tick-major and slot-major, but each channel is stored as
one contiguous array in this fixed order:

1. stable agent ID (`u64`);
2. position (encoding-dependent, two components);
3. orientation (`f32`);
4. scale (`f32`);
5. population ID (`u32`);
6. variant ID (`u32`);
7. clip ID (`u16`);
8. phase (`f32`);
9. playback rate (`f32`);
10. behavior state (`u16`);
11. decision reason (`u16`);
12. destination ID (`u32`);
13. velocity (two `f32` values);
14. visibility (`u8`, only `0` or `1`);
15. render tier (`u8`).

The fixed portion is 52 bytes per record. Position adds eight bytes for `f32`
or millimetre `i32`, and four bytes for affine `i16`.

## Position encodings

`f32` stores both components literally and declares zero codec error.

Millimetre `i32` stores `round(metres * 1000)`. Decode divides by 1000. The
mathematical quantization bound is 0.5 mm, but f32 arithmetic can add a few
micrometres. The encoder therefore measures the actual worst error for the
chunk, never less than 0.5 mm, stores it in the first scale field, and promotes
the manifest channel bound to the maximum across chunks.

Affine `i16` stores each chunk's minimum as origin and `span / 65534` as scale.
Codes cover the inclusive unsigned range after a signed offset. A zero-span
axis uses scale zero and reconstructs the origin exactly. The declared bound is
half the larger axis scale plus a conservative f32 arithmetic allowance derived
from the largest reconstructable coordinate. It must remain at or below 1 mm
for an accepted M0 cache.

## Measured default and scope

On the checked Apple M1 Max run, the selected 120-tick affine candidate used
6,748,865 bytes for 1,000 agents over 120 frames and observed a maximum
position error of 0.0002404 m. Its sequential reader stayed within 10% of the
matching raw-f32 candidate, satisfying the deterministic selection rule.

This experiment validates the cache matrix and 1,000-agent M0 decision only.
It does not establish 10,000- or 100,000-agent performance, Blender render
throughput, or simulation throughput.
