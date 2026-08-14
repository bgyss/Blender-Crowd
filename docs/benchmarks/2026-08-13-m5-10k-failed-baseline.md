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

## 10K schema-v4 declared-profile rerun

Date: 2026-08-13
Status: **valid declared-profile simulation evidence; failed quality gate; do not begin 100K**

The schema-v4 report recorded 1,013 S1/R1 and 8,987 S2/R2 agents, within a
small hash-partition variance of the declared 10%/90% target. All 10,000
agents arrived. The run achieved 58.25 ticks/s (772.56 s wall time for 45,000
ticks), clearing the contract's 10 ticks/s 10K engineering budget.

| Measure | Schema-v4 result | Assessment |
| --- | ---: | --- |
| Spawned / arrived | 10,000 / 10,000 | Completion pass |
| Simulation rate | 58.25 ticks/s | Performance budget pass |
| Penetration pair-ticks | 5,952 | Quality failure pending reduction/threshold |
| Maximum penetration | 0.333 m | Quality failure |
| Agents ever stalled | 6,818 | Quality failure |
| Stall agent-ticks | 3,914,299 | Quality failure |
| Heading reversals | 9,568,588 | Must remain reported and receive tolerance |
| Abrupt turns | 6,861 | Quality failure |
| Gate crossings | 4,170 | Diagnostic only; current gate counts one flow direction |

Steering remains the dominant cost at 79.8% of measured phase time and
perception is 15.3%. The next optimization round must improve the crowded S2
lanes' quality without losing the validated profile mix or 10K throughput.
This result also lacks the required cache rerun, Blender viewport/render,
tier-transition, CPU-fallback, and declared-threshold evidence; none may be
inferred from the Rust simulation report.

## Dense-S2 cadence quality pass

Date: 2026-08-13
Status: **candidate validated at 1K; repeat 10K before accepting**

The original S2 schedule refreshed every S2 agent together, producing a dense
lane's collective correction on one tick and holding it for the next three.
The scheduler now assigns each S2 agent a stable-ID phase and refreshes it every
two ticks (66.7 ms maximum stale interval at 30 Hz). This doubles S2 avoidance
work relative to the prior four-tick policy but keeps it below S1's every-tick
work and avoids synchronized correction waves.

At 1K, the declared profile remained 101 S1/R1 and 899 S2/R2. All 1,000
agents arrived. Penetration fell from 40 to 6 pair-ticks and maximum depth fell
from 0.068 m to 0.010 m. The tradeoff is explicit: throughput fell from 985.9
to 592.6 ticks/s, agents ever stalled rose from 50 to 120, and abrupt turns
rose from 43 to 45. The candidate is therefore a contact-quality improvement,
not a completed quality gate; the next 10K report must evaluate the stall and
throughput tradeoff under the same schema-v4 declared profile.

## Two-tick S2 10K rerun

Date: 2026-08-14
Status: **contact-quality improvement; stalled-population gate remains failed; do not begin 100K**

The schema-v4 two-tick run completed 10,000 / 10,000 at 36.62 ticks/s. Compared
with the preceding four-tick declared-profile run, penetration pair-ticks fell
from 5,952 to 3,311, peak penetration from 0.333 m to 0.216 m, abrupt turns
from 6,861 to 3,767, and stall agent-ticks from 3,914,299 to 3,040,879.
However, 7,805 agents were marked as ever stalled, so this does not pass the
quality gate.

Review found that sparse S2 ticks cleared `solver_status` while preserving the
previous braking target. That counted periodic brake samples as separate stall
episodes rather than continuous braking. The metric now preserves and accounts
for the retained braking status; prior stalled-agent values across sparse
cadences are not comparable to the corrected metric.

## Three-tick S2 candidate after corrected stall accounting

Date: 2026-08-14
Status: **rejected at 10K; do not use**

The three-tick candidate used stable-ID-staggered S2 perception and steering every
three ticks (100 ms maximum stale interval at 30 Hz). At 1K it completed all
agents at 815.8 ticks/s, with 17 penetration pair-ticks, 0.063 m maximum
penetration, 216 agents ever stalled, 4,618 stall agent-ticks, and 26 abrupt
turns. It is the selected balance between the old four-tick contact failures
and the two-tick candidate's heavier continuous braking. Its 10K rerun completed
all agents at 48.96 ticks/s but recorded 3,583 pair-ticks, 0.343 m maximum
penetration, 9,097 agents ever stalled, 4,469,568 stall agent-ticks, and 4,963
abrupt turns. It is rejected: peak penetration and corrected stalls are worse
than the two-tick contact candidate.

## Two-tick S2 candidate after corrected stall accounting

Date: 2026-08-14
Status: **rejected on corrected continuous-stall evidence; do not begin 100K**

The current-code rerun completed all 10,000 agents at 37.12 ticks/s (1,212.30 s
wall time). It retained the best contact result of the cadence candidates so
far: 3,311 penetration pair-ticks, 0.216 m maximum penetration, and 3,767
abrupt turns. Corrected continuous-braking accounting recorded 8,764 agents
ever stalled and 3,682,282 stall-agent-ticks. The lower contact count is real,
but it does not make the run acceptable: 87.6% of the population entered a
continuous braking state.

This result establishes that cadence alone cannot fix the dense-lane quality
failure. It also exposed a fixture-scale defect: at 10K the scene still used
twelve physical route centrelines. Route length had grown by 10x from the
100-agent reference while population had grown by 100x, placing about 10x more
agents on each one-dimensional lane. The next fixture revision scales lanes
per direction with the linear scene scale (six at 100 agents, sixty at 10K),
so total route capacity grows with population while lane pitch stays near
2.7 m. All earlier 10K cadence comparisons are retained as evidence for the
old twelve-lane fixture, but are not evidence for the scaled-lane revision.

The next required evidence is a fresh 1K confirmation and then a 10K rerun of
the scaled-lane fixture. Do not begin 100K unless that new 10K report is
accepted and all non-simulation M5 gates are separately evidenced.

## Scaled-lane 1K confirmation

Date: 2026-08-14
Status: **fixture revision confirmed at 1K; repeat 10K next**

The revised fixture used 19 lanes per direction (38 total), compared with the
old fixed twelve lanes. It completed 1,000 / 1,000 agents at 648.43 ticks/s in
21.95 s. It recorded one penetration pair-tick, 0.00029 m maximum penetration,
53 agents ever stalled, 2,208 stall-agent-ticks, and 28 abrupt turns. The
declared stable-ID profile remained active with 102 S1/R1 and 898 S2/R2 agents
at the two-tick cadence.

This is strong 1K evidence that lane-capacity scaling removes the artificial
single-file queue. It is only a confirmation run: repeat the full 10K
simulation with a new output directory before assessing the revised fixture.

## Scaled-lane 10K simulation result

Date: 2026-08-14
Status: **simulation sub-gate measured successfully; full 10K acceptance remains pending**

The revised fixture used 60 lanes per direction (120 total) under the declared
stable-ID target profile: 959 S1/R1 and 9,041 S2/R2 agents. All 10,000 agents
arrived. The run completed on the Apple M1 Max reference workstation at 60.15
ticks/s (748.16 s wall time for 45,000 ticks).

| Measure | Scaled-lane 10K result | Change from fixed-lane two-tick rerun |
| --- | ---: | ---: |
| Spawned / arrived | 10,000 / 10,000 | Full completion retained |
| Simulation rate | 60.15 ticks/s | 37.12 to 60.15 ticks/s |
| Penetration pair-ticks | 49 | 3,311 to 49 |
| Penetration agent-ticks | 99 | Newly recorded comparison field |
| Maximum penetration | 0.0183 m | 0.216 m to 0.0183 m |
| Agents ever stalled | 998 | 8,764 to 998 |
| Stall episodes / agent-ticks | 1,455 / 92,322 | 62,708 / 3,682,282 to corrected values |
| Abrupt turns | 346 | 3,767 to 346 |
| Gate crossings | 4,422 | Diagnostic only: one direction is counted |

This is the first valid 10K result for the scaled-lane fixture, and it meets
the existing 10 ticks/s engineering budget with low measured contact error.
It is not yet a passing M5 10K report: the governing contract requires fixed
per-tier thresholds for destination, penetration, stall, oscillation, and group
metrics. These measurements are the required checked-in baseline from which
those thresholds can be set; a subjective reading of the improvement cannot
replace them. Cache, Blender playback/render and UI evidence, tier-transition
evidence, and CPU-fallback compatibility evidence also remain required before
100K work begins.
