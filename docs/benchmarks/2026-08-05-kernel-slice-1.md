# Crowd simulation kernel, slice 1 — measured results

Date: 2026-08-05
Scope: [Deterministic crowd simulation kernel (slice 1) design](../superpowers/specs/2026-08-04-crowd-sim-kernel-design.md)
Parent contract: [Blender Crowd 1.0 architecture and MVP](../blender-crowd-1.0.md), Phase 0

This is the Phase 0 exit artifact. The contract is explicit that the gate is a
reproducible benchmark report, not working code and not a video.

**Headline: the performance gate is met with a 3.3x margin. The quality gate is
not met — but not in the way the raw completion numbers suggest.** Nothing
deadlocks; every scene drains given time. The defect is that agents move at
28–43% of their preferred speed, and that contact concentrates sharply at
constrictions. Both statements are backed by measurements below, and the
quality gap is the most useful output of this slice.

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
| bidirectional_corridor | 26.7% | 130 s | 6.38 s | 6,686 | 2,147 | 0.63 m | 946 | 343,476 | 106 | 0.34 MB |
| crossing | 24.0% | 130 s | 6.27 s | 22,698 | 8,015 | 0.68 m | 969 | 278,838 | 105 | 0.48 MB |
| bottleneck | 36.0% | 214 s | 6.52 s | 1,092,251 | 632,719 | 0.75 m | 993 | 302,683 | 110 | 0.40 MB |
| dense_flow | 34.0% | 227 s | 5.91 s | 826,398 | 449,812 | 0.75 m | 979 | 366,178 | 100 | 0.44 MB |
| circle | 41.8% | 174 s | 6.47 s | 21,518 | 6,596 | 0.70 m | 999 | 313,752 | 104 | 0.51 MB |
| l_corridor | 14.0% | 198 s | 6.15 s | 31,048 | 10,996 | 0.75 m | 953 | 421,531 | 101 | 0.48 MB |

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
than a demo. It is also where a first reading of these numbers was wrong, so
the diagnosis matters as much as the totals.

### 5.1 Nothing deadlocks. The crowd is slow.

Completion of 14–42% reads like failure. It is not. Run each scene to three
times its duration and the population essentially all arrives:

| Scene | At scene duration | At 3x duration |
|---|---|---|
| bidirectional_corridor | 245 / 1000 | **1000 / 1000** |
| crossing | 219 / 1000 | **1000 / 1000** |
| bottleneck | 370 / 1000 | 968 / 1000 |
| l_corridor | 147 / 1000 | 944 / 1000 |

So completion here measures a **duration budget**, not a stuck crowd. Scene
durations scale with the square root of population, but congestion slowdown is
superlinear, so the budget falls behind as the crowd grows.

The real number underneath is speed. Live agents move at **28–43% of their
preferred speed** (8% in the bottleneck). A pedestrian crowd at these densities
should lose perhaps 20–30%, not 60–70%. That, not completion, is the quality
defect.

### 5.2 Where the remaining penetration is

Maximum overlap depth of 0.63–0.75 m against agent radii of 0.24–0.38 m means
pairs reaching near-coincident centres. But the *distribution* is the
informative part:

- **Open scenes** (corridor, crossing, circle, l_corridor): 2,100–11,000
  penetration pair-ticks.
- **Constricted scenes** (bottleneck, dense_flow): 450,000–633,000 — roughly
  60x more.

The solver copes in open flow and fails at a doorway. That is a specific,
actionable finding, and it only became visible after a measurement artifact was
removed — see below.

### 5.3 A measurement artifact that looked like a solver failure

An earlier draft of this report called penetration severe across the board. It
was not. In `bidirectional_corridor`, all 11,104 penetration pair-ticks occurred
in the **first tenth of the run and exactly zero afterwards**, with the deepest
overlap at tick 27 of 5,692.

The cause was spawn placement, not steering: agents were positioned uniformly
at random with no overlap rejection, so they started inside each other and
pushed apart. The arithmetic confirms it — a 0.68 m overlap requires two
near-maximum-radius agents about 0.01 m apart, which steering does not produce
but random placement does.

Adding rejection sampling to spawn placement cut open-scene penetration about
fivefold and left the constricted scenes essentially unchanged, which is
exactly the signature of an artifact being removed rather than a behaviour
being improved.

### 5.4 The density speed term is counterproductive

Contract section 6.2 requires density-aware speed reduction, and this kernel
implements it as `speed x 1 / (1 + k * neighbours_within_personal_space)`.
Measured against `k`:

| `density_speed_factor` | corridor arrived | bottleneck arrived | bottleneck penetration |
|---|---|---|---|
| 0.18 (current) | 245 | 370 | 765,639 |
| 0.06 | 262 | 396 | 335,565 |
| **0.00** | **358** | **472** | **355,398** |

Disabling it improves completion by 28–46% *and* halves penetration in the
bottleneck. Slowing agents inside a crowd keeps them in the crowd longer, so
they accumulate more contact rather than less.

The calibration is also too aggressive on its own terms: at roughly 1.4
agents/m² of local density it applies a 0.53 speed factor where the empirical
pedestrian fundamental diagram gives about 0.75.

This is left as-is deliberately. The term is contract-mandated, the fix is
solver tuning, and the next slice exists to compare three solvers against these
baselines — tuning one of them now would partly pre-empt that comparison. It is
recorded here as the single highest-value lead going into it.

### 5.5 Other quality signals

**Oscillation is high.** Hundreds of thousands of heading reversals per run.
The smoothness term in the cost function does not suppress the flip-flopping
contract section 6.2 names as a production blocker.

**Stalls are near-universal.** 950–1,000 of 1,000 agents stall at least once in
every scene.

The trajectory SVGs corroborate this: the constricted scenes show real funnel
structure, but flows are tangled rather than laned, with visible jitter.

### 5.6 What this rules in and out

The failure is *not* in the parts the slice set out to prove:

- Determinism holds bitwise, including under spawn-order permutation.
- Throughput exceeds the gate 3x, with cost concentrated where expected.
- No agent reaches non-finite state, escapes the scene, or exceeds its maximum
  speed, under fuzzing at 800 agents across six scenes and multiple seeds.
- Nothing deadlocks: every scene drains given time.

The failure is specifically **speed loss and contact under sustained density,
concentrated at constrictions**. That is the risk contract section 16 lists
first, and exactly what Phase 0 exists to surface before it is built upon.

### 5.7 Measured alternative: collision cost magnitude

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
The avoidance solver does not yet produce crowds worth caching: agents move at
roughly a third of their preferred speed, and a doorway accumulates 60x the
contact of open flow. Caching that would cache a traffic jam.

The next slice should be the contract's own avoidance bake-off (section 6.2),
measured against the baselines checked in here. Three concrete leads go into
it, in descending order of measured value:

1. **The density speed term is counterproductive** (section 5.4). Disabling it
   gains 28–46% completion and halves bottleneck penetration. It is
   contract-mandated, so the work is recalibration against the pedestrian
   fundamental diagram rather than removal — the current constant is roughly
   30% too aggressive.
2. **Constrictions are where the solver fails** (section 5.2). Open flow is
   fine. A doorway is not. Contract section 6.3's explicit queues exist for
   precisely this, and this measurement is the argument for pulling them
   forward.
3. **Oscillation is unsuppressed** (section 5.5). The smoothness term is not
   doing its job; the bake-off should measure heading reversals as a
   first-class comparison axis, not a footnote.

Scene durations should also be revisited. They scale with the square root of
population while congestion slowdown is superlinear, so completion at 1,000
agents partly measures the budget rather than the crowd. Either scale duration
against measured travel time, or report completion at a fixed multiple of free
travel time.
