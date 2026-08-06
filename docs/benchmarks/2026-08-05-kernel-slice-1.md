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

| Scene | Completion | Median travel | Mean TTC | Near-miss agent-ticks | Penetration pair-ticks | Max depth | Agents stalled | Heading reversals | Ticks/s | Peak alloc |
|---|---|---|---|---|---|---|---|---|---|---|
| bidirectional_corridor | 24.5% | 124 s | 6.34 s | 20,481 | 11,104 | 0.68 m | 950 | 348,943 | 104 | 0.34 MB |
| crossing | 21.9% | 128 s | 6.21 s | 32,894 | 13,705 | 0.73 m | 967 | 277,274 | 102 | 0.48 MB |
| bottleneck | 37.0% | 216 s | 6.21 s | 1,258,142 | 765,639 | 0.75 m | 986 | 331,504 | 107 | 0.40 MB |
| dense_flow | 33.6% | 197 s | 6.25 s | 670,357 | 362,538 | 0.72 m | 974 | 349,094 | 101 | 0.44 MB |
| circle | 44.4% | 174 s | 6.40 s | 29,252 | 10,470 | 0.71 m | 1,000 | 320,794 | 103 | 0.51 MB |
| l_corridor | 14.7% | 196 s | 6.16 s | 33,168 | 12,698 | 0.70 m | 953 | 427,371 | 101 | 0.48 MB |

Reading the columns, because several of these were renamed after review found
they did not measure what their names claimed:

- **Mean TTC** is the mean per-agent predicted time to collision, capped at a
  10 s horizon, measured against the velocity each agent actually uses. The
  *minimum* is also recorded but is 0.00 s everywhere — one overlapping pair
  pins it for a whole run, so it cannot compare solvers.
- **Near-miss agent-ticks** counts agent-ticks spent under 0.5 s predicted time
  to collision. It scales with the population instead of saturating.
- **Penetration pair-ticks** is duration, not distinct episodes: a pair
  overlapping for 100 ticks contributes 100.
- **Agents stalled** is distinct agents that stalled at least once — nearly the
  whole population in every scene.

Peak allocation is *allocated* bytes from a counting global allocator, not
resident set size. It excludes allocator overhead and static data.

Throughput through the constrictions: 412 forward crossings in `bottleneck`
and 608 in `dense_flow`, against 370 and 336 arrivals respectively.

No agent in any scene failed to get a route (`agents_unrouted` is zero
throughout), so completion reflects crowd behaviour rather than navigation
failure.

One caveat worth stating before these numbers are compared across scales:
`dense_flow`'s mouth is pinned at 6 m, but the corridor *behind* it still
scales, running 12 m at 100 agents and 38 m at 1,000. The constriction is
fixed; the tunnel behind it is not. Some of that scene's completion drop is
the longer run-out, not solver quality.

## 3. Against the contract's 1K gate

Contract section 8.3 asks for "at least real-time simulation at 30 ticks/s
without armature evaluation."

**Met, comfortably.** 98–105 ticks/s achieved against 30 required — a 3.3x
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

**Completion is 15–44%.** Agents that do finish take 124–216 s median. In a
scene the population can cross in about 90 s unobstructed, that is a crowd
spending most of its time not making progress.

The worst is `l_corridor` at 14.7% — the only scene whose route turns a
corner. That scene was added *because* review noticed every other route was a
straight line, so the corridor-following navigation had never been exercised
at scene scale. It immediately became the hardest scene, which is exactly the
kind of thing a benchmark exists to reveal.

**Penetration is severe, and concentrated.** Maximum depth of 0.71–0.75 m
against agent radii of 0.24–0.38 m means pairs overlap by more than a full body
diameter — agents pass through each other rather than brushing past. The
constricted scenes are where it happens: `bottleneck` logs 766k penetration
pair-ticks against 10–14k in the open scenes, a 60x spread that says the
solver copes in open flow and fails at a doorway.

**Oscillation is high.** Hundreds of thousands of heading reversals per run. The
smoothness term in the cost function is not enough to suppress the flip-flopping
that contract section 6.2 names as a production blocker.

**Stalls are near-universal.** 950–1,000 of the 1,000 agents stall at least
once in every scene. Every agent in `circle` does.

The trajectory SVGs corroborate all of this: the enclosed scenes show real
funnel structure at the doorway, but the flows are tangled rather than laned,
with visible jitter rather than smooth paths.

### 5.1 What this rules in and out

The failure is *not* in the parts the slice set out to prove:

- Determinism holds bitwise, including under spawn-order permutation.
- Throughput exceeds the gate 3x, with the cost concentrated where expected.
- No agent reaches non-finite state, escapes the scene, or exceeds its maximum
  speed, under fuzzing at 800 agents across six scenes and multiple seeds.
- The crowd never deadlocks wholesale — at least 10% of the unfinished
  population is always still moving.

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

Twenty defects were found and fixed, most of them in the plan's own specified
code rather than in its transcription, and eight of them only by the
whole-branch review at the end. They are listed because the pattern is the
useful part: the simulation was easier to get right than the *measurement* of
it, and a demo would have surfaced none of the second kind.

### 6.1 Found by review of the measurement

Every one of these would have poisoned the baselines that the next slice's
avoidance bake-off is judged against:

1. **Two metrics were fully saturated.** `near_miss_ticks` read 11,383 of
   11,384 ticks, and `min_time_to_collision` read 0.00, because both were
   global minima over 1,000 agents — one overlapping pair pins them for an
   entire run. Replaced with per-agent-tick counts and an unsaturated mean.
2. **The reported time to collision described a velocity no agent had.** It
   came from the solver's reciprocal construction, which is correct as a cost
   heuristic but is not a kinematic state. Now measured against the velocity
   the agent actually uses.
3. **Three metrics misnamed what they counted:** stall *episodes* reported as
   stalled agents, pair-*ticks* reported as penetration events, and a
   throughput gate that tested an *infinite line in both directions* — so an
   agent crossing anywhere in a 126 m scene counted as passing through a 5 m
   doorway. After clamping and directing it, the gate then read zero while 37%
   of the population walked through, because the direction test was inverted.
4. **A routing failure counted as a destination completion.** An agent with no
   route was flagged `arrived`, inflating the headline metric with navigation
   failures.
5. **Population scaling dissolved the constrictions.** Scaling geometry by
   `sqrt(population)` while agent radii stay fixed turned the bottleneck's
   1.6 m doorway into 5.06 m at 1,000 agents. The scene named after its
   constriction no longer had one. Constrictions are now held fixed.
6. **Every route was a straight line**, so the corridor-following navigation
   was never exercised at scene scale. Adding `l_corridor` immediately produced
   the worst-performing scene in the set.
7. **A schema-coverage test could not detect what it guarded**, because it
   checked a hardcoded list; and the deadlock test passed with 399 of 400
   agents frozen.

### 6.2 Found earlier, in the simulation itself

The ones that mattered:

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
