# M5 10K city-flow failed baseline

Date: 2026-08-13  
Milestone: [M5 — Scale, GPU tiers, and procedural rendering](../milestones/M5-scale-rendering.md)  
Status: **failed; do not begin the 100K gate**

## Reproduction

```sh
cargo run --release -p crowd-bench -- run \
  --scene m5_city_flow --agents 10000 \
  --out "$HOME/blender-crowd-m5/10k/simulation"
cargo run --release -p crowd-bench -- cache-experiment \
  --agents 10000 --cache-frames 120 \
  --out "$HOME/blender-crowd-m5/10k/cache"
```

The raw artifacts are user-local evidence at the command's output paths. The
simulation report captured Apple M1 Max, 64 GiB RAM, macOS arm64, Rust 1.94.1,
release profile, and timestamp `2026-08-13T17:52:13Z`.

## Simulation result

| Measure | Result | Gate assessment |
| --- | ---: | --- |
| Spawned / arrived | 10,000 / 2,550 | Fail: 25.5% completion |
| Simulation rate | 7.01 ticks/s | Fail: below 10K target of 10 ticks/s |
| Wall time / ticks | 5,139.16 s / 36,000 | Recorded baseline |
| Penetration pair-ticks | 273,543,684 | Fail: severe overlap |
| Maximum penetration | 0.757 m | Fail: severe overlap |
| Agents ever stalled | 9,919 | Fail |
| Stall agent-ticks | 31,048,555 | Fail |
| Heading reversals | 91,773,818 | Fail |
| Abrupt turns | 64,329,894 | Fail |
| Peak measured allocator bytes | 6,768,948 | Not a resident-memory claim |

Steering consumed 67.9% of phase time and perception 30.6%, together 98.6%.
This identifies individual neighbor/avoidance work as the immediate CPU
bottleneck.

## Cache result

The independent 120-frame cache matrix completed all nine candidates and chose
F32 with 120-tick chunks: 72,280,893 bytes, 660.5 write frames/s, 18.3 read
frames/s, zero encoding error, and 65.1 ms cancellation/recovery probe. It is
valid cache evidence only; it cannot offset the simulation failure or establish
viewport/render acceptance.

## Why this is not a valid M5 profile result

The run predated the scale runner's declared 10% S1 / 90% S2 profile and
scheduled S2 steering. It also used the first `m5_city_flow` implementation,
which inherited `dense_flow`'s intentional fixed funnel. It is retained as the
unoptimized baseline—not as a test of the M5 scheduler or final city fixture.

## Next optimization round

1. Enable a stable-ID 10% S1 / 90% S2 profile for `m5_city_flow`.
2. Run S2 perception and avoidance every fourth tick; retain continuous root
   integration and direct coarse desired motion between solves.
3. Replace the inherited fixed funnel with lane-separated city-flow corridors.
   The first 100-agent profile smoke completed 100/100 with zero penetration;
   the 500-agent check reached 75.6% with only 243 penetration pair-ticks but
   showed the original duration was too short for its 218.2s P95 travel time.
4. Increase fixture duration with measured P95/emission slack, then measure a
   1K confirmation run before repeating the full 10K gate.
5. Stop before 100K unless the rerun meets fixed quality and performance gates
   and is supplemented by required Blender/fallback evidence.
