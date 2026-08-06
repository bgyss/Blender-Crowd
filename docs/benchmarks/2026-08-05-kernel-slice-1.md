# Crowd simulation kernel, slice 1 — measured results

Date: 2026-08-05
Scope: [Deterministic crowd simulation kernel (slice 1) design](../superpowers/specs/2026-08-04-crowd-sim-kernel-design.md)
Parent contract: [Blender Crowd 1.0 architecture and MVP](../blender-crowd-1.0.md), Phase 0

This is the Phase 0 exit artifact. The contract is explicit that the gate is a
reproducible benchmark report, not working code and not a video.

**Headline: the performance gate is met with a 3x margin. The quality gate is
not met.** Both statements are backed by numbers below, and the quality gap is
the most useful output of this slice.

## 1. Environment

| | |
|---|---|
| CPU | Apple M1 Max |
| RAM | 64 GiB |
| OS / arch | macOS, aarch64 |
| rustc | 1.94.1 (e408947bf 2026-03-25) |
| Build profile | release |
| Tick rate | 30 Hz |
| Solver | `sampled_velocity` |
| Seed | 2026 |

Reproduce with:

```sh
cargo run --release -p crowd-bench -- run --agents 1000 --svg
cargo run --release -p crowd-bench -- check --agents 1000
```

## 2. Results at 1,000 agents

| Scene | Completion | Median travel | p95 travel | Penetration events | Max depth | Stalled | Heading reversals | Ticks/s | Peak alloc | Wall time |
|---|---|---|---|---|---|---|---|---|---|---|
| bidirectional_corridor | 24.5% | 123.8 s | 184.4 s | 188,871 | 0.715 m | 13,827 | 350,846 | 100 | 0.33 MB | 56.9 s |
| crossing | 21.9% | 128.2 s | 184.1 s | 109,500 | 0.728 m | 12,489 | 279,588 | 100 | 0.48 MB | 57.1 s |
| bottleneck | 43.9% | 236.8 s | 368.1 s | 1,184,063 | 0.740 m | 19,332 | 406,662 | 104 | 0.40 MB | 109.4 s |
| dense_flow | 43.9% | 240.3 s | 366.1 s | 1,324,631 | 0.740 m | 19,112 | 402,880 | 103 | 0.44 MB | 110.7 s |
| circle | 44.4% | 173.7 s | 188.3 s | 23,681 | 0.708 m | 11,347 | 324,402 | 103 | 0.51 MB | 55.4 s |

Minimum predicted time to collision is 0.00 s in every scene: agents do make
contact.

Peak allocation is *allocated* bytes from a counting global allocator, not
resident set size. It excludes allocator overhead and static data.

## 3. Against the contract's 1K gate

Contract section 8.3 asks for "at least real-time simulation at 30 ticks/s
without armature evaluation."

**Met, comfortably.** 100–104 ticks/s achieved against 30 required — a 3.3x
margin — with peak allocation under 0.6 MB for 1,000 agents. Memory is not
close to a constraint at this scale.

Phase time shares (bidirectional_corridor):

| Phase | Share |
|---|---|
| steer | 87.3% |
| perceive | 11.2% |
| metrics | 0.9% |
| integrate | 0.5% |

The avoidance solver is the hot path by a wide margin, which is where the
optimisation budget belongs and which is exactly what the tier scheduler in
contract section 8.1 exists to relieve.

## 4. Reproducibility

Verified, and stronger than the contract requires. Contract section 9.4 asks
`Strict` mode to compare exact discrete decisions with bounded continuous
tolerances; this kernel is **bitwise** identical on the same binary and
machine.

Six determinism tests pass (`crates/crowd-core/tests/determinism.rs`):

- repeated runs are bitwise identical in every scene;
- state hashes agree at *every tick*, not just the end — an end-state
  comparison can hide a divergence that later reconverges;
- permuting spawn region order does not change results once compared by
  stable ID;
- adding one agent does not change any existing agent's attributes
  (contract section 4.2);
- changing the seed does change the outcome — a guard against a
  "determinism" implementation that simply ignores its inputs;
- no spawn errors occur in any scene.

Cross-machine identity is **not** claimed and has not been tested.

## 5. Quality: what the numbers say

This is the honest weak spot, and the reason a metrics report is worth more
than a demo.

**Completion is 22–44%.** Agents that do finish take 124–240 s median. In a
scene the population can cross in about 90 s unobstructed, that is a crowd
spending most of its time not making progress.

**Penetration is severe.** Maximum depth of 0.71–0.74 m against agent radii of
0.24–0.38 m means pairs are overlapping by more than a full body diameter —
agents are passing through each other, not brushing past. The bottleneck and
dense_flow scenes log over a million penetration events.

**Oscillation is high.** 280,000–407,000 heading reversals across a run. The
smoothness term in the cost function is not enough to suppress the flip-flopping
that contract section 6.2 names as a production blocker.

**Stalls are common.** 11,000–19,000 stalled agent-ticks per scene.

The trajectory SVGs corroborate all of this: the enclosed scenes show real
funnel structure at the doorway, but the flows are tangled rather than laned,
with visible jitter rather than smooth paths.

### 5.1 What this rules in and out

The failure is *not* in the parts the slice set out to prove:

- Determinism holds bitwise, including under spawn-order permutation.
- Throughput exceeds the gate 3x, with the cost concentrated where expected.
- No agent reaches non-finite state, escapes the scene, or exceeds its maximum
  speed, under fuzzing at 800 agents across five scenes and multiple seeds.
- The crowd never deadlocks wholesale — it always retains some moving agents.

The failure is specifically **avoidance quality under sustained density**. That
is precisely the risk contract section 16 lists first ("avoidance looks robotic
or deadlocks") and precisely what Phase 0 exists to surface before it is built
upon.

### 5.2 Measured alternative: collision cost magnitude

One tuning decision was A/B measured rather than argued, and both results are
recorded here because the difference is within scene-to-scene variation and
belongs in a baseline rather than a judgement call.

| Constants | corridor | crossing | bottleneck | dense_flow | circle | mean |
|---|---|---|---|---|---|---|
| Bounded (`OVERLAP_URGENCY` 8, `MIN_TIME_FOR_COST` 0.25) | 86% | 73% | 69% | 100% | 100% | 85.6% |
| Unbounded (100, 0.01) | 75% | 72% | 80% | 86% | 100% | 82.6% |

(Measured at 100 agents.) Bounded was kept: better mean, and the rationale is
sound — with an unbounded `1/t`, a single touching neighbour costs about 200
against a goal term of about 1.4, so goal-seeking is silenced entirely on
contact. But the bottleneck scene prefers the unbounded form, and that
disagreement is a real signal about doorway behaviour worth revisiting.

## 6. Defects this slice found

Twelve genuine defects were found and fixed, most of them in the plan's own
specified code rather than in its transcription. The ones that mattered:

1. **Wall time-to-collision could not detect brief contacts.** A sampled search
   misses grazing collisions entirely, because the overlap window is
   `2·sqrt(r² − d²)`, which goes to zero as the miss distance approaches the
   radius — so *no* step size makes sampling complete. Replaced with an exact
   closed-form swept capsule, which is also cheaper.
2. **Routing chased nodes instead of following corridors.** Every waypoint was
   a mandatory pass-through point, so a population converged on a single spot
   and jammed permanently: 20/20 agents braking, route index frozen at 0 for
   180 ticks. All five scenes would have deadlocked. Replaced with
   corridor-following, which is what the navmesh this module stands in for
   computes.
3. **Arrived agents plugged their own destination.** Agents that reach a goal
   park on it and never move, and still registered as obstacles — so the first
   arrivals blocked everyone behind them. Completion went from 2–8% to 69–100%
   at 100 agents when arrived agents stopped obstructing.
4. **Scene geometry did not scale with population.** Running 1,000 agents
   through geometry sized for 100 put the corridor at 3.1 agents/m², past the
   jamming threshold. The benchmark was measuring over-subscription rather than
   solver quality.
5. **`f32::clamp` panicked on inverted bounds** for any population with a mean
   speed below 0.2 m/s, breaking the infallible-tick-loop guarantee. Fixed by
   validating populations at scene compile time.
6. **The spatial index allocated every tick**, undercutting the
   no-allocation-in-the-hot-path property it exists to provide.
7. **Signed zero broke the determinism hash.** `-0.0` and `0.0` are numerically
   equal but hash differently, so a cancelled velocity component could report a
   false determinism failure.
8. **The scene hash ignored waypoint edges**, so a rewired graph routed
   differently while hashing identically — defeating the hash's only purpose.

## 7. What this does not prove

- **No navmesh.** Routing is an authored waypoint graph, deliberately a
  stand-in. Contract section 6.1 navigation is untouched.
- **No behavior graph, blackboards, groups, queues, portals, or lanes.**
- **No animation, no fidelity tiers, no cache, no PyO3 bridge, no Blender, no
  Geometry Nodes.** Nothing here has been near Blender.
- **Single-threaded, single machine.** The contract's `Fast` mode does not
  exist, and cross-machine reproducibility is untested.
- **One solver, no comparison.** The contract requires the sampled-velocity
  solver to be measured against ORCA-style and scoped time-to-collision
  candidates before an avoidance approach is selected. That bake-off is the
  next slice, and these baselines exist to make it a measurement rather than an
  argument.
- **Quality thresholds are deliberately absent.** Per contract section 12.3,
  thresholds are fixed only after a baseline is measured and reviewed. This
  report is that first measurement.

## 8. Recommendation

Do not proceed to the navmesh or the Blender bridge yet.

The throughput and determinism foundations are sound and exceed their gates.
The avoidance solver does not yet produce crowds worth caching: at 22–44%
completion with body-diameter interpenetration, a cache of this output would
record a jam.

The next slice should be the contract's own avoidance bake-off (section 6.2),
measured against the baselines checked in here. That is the decision Phase 0
was designed to inform, and this report is the evidence it needs — including
the finding that the current baseline is not good enough.
