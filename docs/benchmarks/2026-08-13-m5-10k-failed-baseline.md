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

## Initial 1K post-optimization confirmation

Date: 2026-08-13
Status: **confirmation completed; neither a 10K acceptance nor authorization for 100K**

The declared profile was present: 100 S1/R1 agents and 900 S2/R2 agents, with
S2 perception and steering scheduled every four ticks. The lane-separated
fixture completed 1,000 / 1,000 agents in 14,230 ticks at 527.6 ticks/s
(26.97 s wall time).

Quality remains an explicit optimization concern: the run recorded 3,036
penetration pair-ticks, a 0.239 m maximum penetration, 759 agents ever stalled,
and 269,057 heading reversals. These measurements must be judged against
declared per-tier thresholds before a 10K result can be accepted; they cannot
be treated as a pass merely because destination completion and throughput
improved.

## 1K quality-optimization pass

Date: 2026-08-13
Status: **simulation rerun is justified; this remains neither a 10K acceptance nor authorization for 100K**

Root-cause review found that each direction's tall spawn band was routed onto a
single waypoint centreline. That created an unintentional merge before agents
could make forward progress. The fixture now uses twelve parallel lane strips
(six per direction), each with its own direct route and destination. Sparse S2
steering also now retains its last solved target velocity between evaluations;
it previously reused the current velocity, producing a stop-start acceleration
sawtooth.

| Measure | Initial 1K confirmation | Quality-optimization pass |
| --- | ---: | ---: |
| Spawned / arrived | 1,000 / 1,000 | 1,000 / 1,000 |
| Simulation rate | 527.6 ticks/s | 1,249.8 ticks/s |
| Wall time | 26.97 s | 11.39 s |
| Penetration pair-ticks | 3,036 | 40 |
| Maximum penetration | 0.239 m | 0.026 m |
| Agents ever stalled | 759 | 22 |
| Abrupt turns | 522 | 27 |

The raw heading-reversal counter remains high (259,797) because it counts
alternating signed corrections above 0.001 radians; it must stay visible in the
10K report and receive a declared per-tier tolerance rather than being treated
as resolved by this pass. The 40 pair-ticks, 2.6 cm peak depth, 22 stalled
agents, full completion, and retained 10% S1 / 90% S2 profile justify repeating
the 10K simulation gate. The required Blender, CPU-fallback, and formal
threshold evidence still blocks 10K acceptance and all 100K work.

## First 10K post-optimization rerun

Date: 2026-08-13
Status: **invalid profile evidence; do not accept and do not begin 100K**

The rerun completed 10,000 / 10,000 agents at 72.9 ticks/s in 617.47 s, with
6,479 penetration pair-ticks, a 0.316 m peak penetration, 6,570 agents ever
stalled, and 7,275 abrupt turns. This is a substantial improvement over the
original pre-profile baseline, but it cannot be used for M5 acceptance.

The report was schema v3 and incorrectly declared 1,000 S1 / 9,000 S2 agents.
Its scheduler applied the 90% cutoff to each lane-local ordinal, so every lane
(about 833 agents) was actually S2. The profile is repaired in schema v4:
classification now uses a stable-agent-ID hash and the report counts committed
S/R assignments rather than restating the requested target. Repeat both the 1K
confirmation and 10K simulation with schema v4 before making any quality or
performance claim for the declared mix.

## 1K confirmation after profile repair

Date: 2026-08-13
Status: **profile assignment confirmed; repeat 10K next**

Schema v4 recorded 101 S1/R1 and 899 S2/R2 agents, close to the declared
10%/90% target and now derived from stable IDs rather than lane-local ordinals.
The run completed 1,000 / 1,000 at 985.9 ticks/s in 14.43 s, with 40
penetration pair-ticks, 0.068 m maximum penetration, 50 agents ever stalled,
and 43 abrupt turns. This confirms the repaired profile is active, but does
not replace the pending 10K scale, Blender, fallback, or threshold gates.
