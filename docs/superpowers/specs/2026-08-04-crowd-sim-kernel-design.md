# Deterministic crowd simulation kernel (slice 1) — design

Date: 2026-08-04
Status: approved design, ready for implementation planning
Parent contract: [Blender Crowd 1.0 architecture and MVP](../../blender-crowd-1.0.md)

## 1. Scope

`docs/blender-crowd-1.0.md` describes a 9-to-14-month program spanning a Rust
simulation core, tiled navigation, a behavior graph compiler, a versioned cache,
a PyO3 bridge, a Blender add-on, Geometry Nodes presentation, and an asset and
retargeting pipeline. That is too large for one spec. The contract decomposes
itself in section 13, and this document specs only the first sub-project.

This slice implements the contract's section 13.2, weeks 1 to 4: lock the
coordinate, clock, identity, and determinism contracts, then build a headless
Rust simulation kernel with structure-of-arrays agents, a fixed tick, spatial
queries, one baseline avoidance solver, synthetic benchmark scenes, deterministic
seeding, and a measured metrics report.

### 1.1 In scope

- Foundational unit, coordinate, clock, identity, randomness, and determinism
  contracts.
- A `crowd-core` library: SoA agent storage, fixed-step tick pipeline, uniform
  grid spatial index, analytic static geometry, authored waypoint routing, one
  sampled-velocity avoidance solver, and metrics accumulation.
- A `crowd-bench` binary: scene runner, JSON metrics report, SVG trajectory dump,
  and a baseline regression check.
- Unit, property, golden, determinism, and randomized fuzz tests.

### 1.2 Explicitly out of scope

Deferred to later slices, in roughly the contract's own order:

- the tiled navmesh and polygon corridors (contract section 6.1);
- the ORCA-style and scoped time-to-collision solvers and the three-way
  avoidance bake-off (section 6.2);
- the behavior graph, IR, compiler, and blackboards (sections 4.4, 5.3);
- groups, queues, portals, lanes, and the wider semantic vocabulary
  (sections 4.3, 6.3);
- animation state, clips, and phase (section 7);
- fidelity tiers (section 8);
- the cache format, the PyO3 bridge, the Blender add-on, and Geometry Nodes
  presentation (sections 9, 10, 11);
- parallel execution and the contract's `Fast` determinism mode (section 9.4).

The design must not preclude any of these. Where a deferred subsystem has an
obvious insertion point, this document names it.

### 1.3 Success criteria

1. Five benchmark scenes run headlessly at 1,000 agents and emit a metrics
   report recording the environment required by section 8.3.
2. Repeating a run with unchanged inputs produces bitwise-identical per-tick
   state hashes on the same binary and machine.
3. Permuting spawn input order does not change results once compared by stable
   ID.
4. Adding one agent to a population does not change any other agent's derived
   attributes.
5. Baselines are checked in, and a regression check command fails on relative
   drift.
6. Trajectory SVGs make avoidance quality visually assessable without Blender.

Absolute quality thresholds are deliberately **not** part of these criteria.
Contract section 12.3 fixes thresholds only after a checked-in baseline is
measured, and inventing pass/fail numbers beforehand would substitute
judgement for evidence.

## 2. Foundational contracts

### 2.1 Units and coordinates

Meters, seconds, radians. Z-up right-handed, matching Blender exactly so no
conversion occurs at the future bridge. Pedestrian kinematics are planar in XY;
ground height Z is resolved from the environment rather than integrated. Agent
orientation is a single yaw scalar about Z, not a quaternion — cheaper, and it
keeps bitwise state comparison trivial.

Positions are `f32` relative to an `f64` world origin per contract section 3.3.
This slice stores the origin and asserts it is zero; large-world rebasing has no
test until scenes exceed `f32` precision.

Project settings carry `world_to_meter` per section 4.1. This slice asserts it
is `1.0` rather than implementing scaling that cannot yet be tested.

### 2.2 Clock

An integer `ticks_per_second` (default 30), a `dt` derived once from it, and a
`u64` tick counter as the only notion of time. No kernel code reads a wall
clock, thread ID, or address. Frame sampling and frame-to-tick mapping are
deferred with the cache.

### 2.3 Identity

`AgentId(u64)`, derived per contract section 5.1 from the project UUID,
population ID, spawn source ID, and spawn ordinal.

The mixing function is vendored into the crate — a SplitMix64-style finalizer —
rather than taken from an external hasher. An external hash's exact output is
not a stability guarantee; if it changed, every cache and every checked-in
baseline in the project would silently break. Vendoring makes the function part
of the versioned contract.

Spawning checks for duplicate IDs and returns a diagnostic. Contract section
10.3 makes duplicate stable IDs a bake-blocking error.

### 2.4 Randomness

No `rand` crate, for the same stability reason. Per-agent attributes are drawn
from `hash(global_seed, agent_id, purpose_tag)`, where `purpose_tag` names the
attribute.

This is what delivers section 4.2's promise. Because each attribute has its own
independent stream keyed by stable ID, adding an agent does not reshuffle
existing variants, and adding a *new* attribute does not shift existing ones
either.

### 2.5 Determinism

`Strict` mode only. The claim is bitwise-identical output for the same binary on
the same machine. Cross-machine identity is not claimed; contract section 9.4
explicitly declines to promise it until demonstrated.

Enforced by rules the tests police:

- no `HashMap` iteration in the tick — `BTreeMap` or sorted vectors only;
- no value derived from addresses, thread identity, or time;
- all ties broken by `AgentId`;
- no fast-math or reassociating float flags.

## 3. Repository shape

A Cargo workspace, matching contract section 14 while honoring its note that
crates may begin consolidated:

```text
Cargo.toml                 workspace
crates/
  crowd-core/              library: contracts, world, phases, avoidance,
                           scenes, metrics
  crowd-bench/             binary: scene runner, JSON report, SVG dump, check
benchmarks/
  baselines/               checked-in measured baselines
  reports/                 generated, ignored by git
docs/superpowers/specs/    this document
```

Dependencies are held to `serde` and `serde_json` for reports, with `proptest`
as a dev-dependency. SVG is emitted as plain text. Peak memory is reported as
peak *allocated* bytes, measured by a counting global-allocator wrapper; this
avoids platform-specific resident-set APIs and is itself deterministic, at the
cost of excluding allocator overhead and static data.

## 4. Runtime data model

### 4.1 Storage

A single `World` holding parallel `Vec` columns indexed by dense slot. Only
fields this slice uses exist; `group_id`, `fidelity_tier`, `blackboard_handle`,
and animation columns from section 5.2 are omitted because nothing would write
them.

- identity: `agent_id`, `population_id`
- kinematic: `pos_x`, `pos_y`, `yaw`, `vel_x`, `vel_y`, `radius`, `max_speed`,
  `preferred_speed`
- navigation: `route`, `route_index`, `arrived`
- staging: `des_vel_x`, `des_vel_y`, `next_pos_x`, `next_pos_y`, `next_vel_x`,
  `next_vel_y`, `next_yaw`
- debug: `solver_status`, `stall_ticks`

A stable-ID-to-slot table keeps IDs stable while slots stay dense. Every
variable-size structure — neighbor lists, routes, event queues — lives in a
preallocated arena that is cleared rather than freed, so the tick loop does not
allocate after warmup, per section 5.2.

Slot order is derived from stable IDs, so iteration order is deterministic by
construction.

### 4.2 Phase communication

Phases communicate through explicit staged buffers. Perceive writes a neighbor
observation arena; decide and steer write `des_vel_*`; integrate is the sole
writer of position and orientation. One writer per field, checked by debug
assertions in tests.

Mutating in place was rejected: agent *i* would then observe agent *j*'s
already-updated state, making results depend on iteration order — the exact
failure the determinism contract exists to prevent.

Each phase is a free function taking immutable previous-state buffers and
mutable next-state buffers, so read and write sets are visible in the signature
and a later Rayon pass requires no semantic change.

### 4.3 Rejected storage alternatives

- **ECS (`hecs`, `bevy_ecs`).** Archetype layout changes iteration order when
  components are added or removed, which is a determinism hazard the contract
  forbids, and it imports a large dependency into the most control-sensitive
  layer.
- **Array-of-structs `Vec<Agent>`.** Simpler to read and adequate at 1,000
  agents, but contract section 2.4 mandates SoA and converting later would touch
  every phase.

## 5. Navigation stand-in

With analytic walls, straight-line steering to a goal deadlocks in the corner
beside a doorway, so agents need a global route before the navmesh exists.

Each scene authors a small waypoint graph by hand. At spawn, an agent runs one
Dijkstra search to its destination and stores the resulting polyline as a route
handle.

This is deliberately a stand-in, and the point is the interface. A route exposes
exactly one operation: *given my position, what is the next steering target?*
That is precisely what a navmesh polygon corridor will implement. When contract
section 6.1 navigation lands, it replaces one module and touches no agent state.

## 6. Tick pipeline

The subset of contract section 6's nine phases that this slice can honestly
implement, in fixed order:

1. **Apply inputs.** Spawn agents scheduled for this tick.
2. **Update spatial index.** Rebuild the uniform grid.
3. **Perceive.** Collect neighbors within a query radius under a fixed budget.
4. **Decide.** Fixed rule: advance along route, mark arrived.
5. **Plan.** Advance the waypoint index; recompute a route on demand.
6. **Steer.** Preferred velocity toward the next waypoint, then avoidance,
   producing desired velocity.
7. **Integrate.** Apply acceleration and turn-rate limits, advance state,
   resolve residual wall penetration.
8. **Emit.** Accumulate metrics and events.

**Animate** is omitted entirely rather than stubbed, because there is no clip
data to select from.

Perceive sorts neighbors by `(distance_squared, agent_id)` so that a budget
cutoff is never ambiguous between two equidistant neighbors.

### 6.1 Spatial index

A uniform grid rebuilt every tick by counting sort. Counting sort is chosen
because it is O(n), allocates into a reused buffer, and — unlike a hash map —
produces one canonical ordering, so neighbor lists never depend on insertion
history. Cell size is derived from the maximum query radius.

Static wall segments are bucketed into the same grid once at scene build time.

### 6.2 Static geometry

A flat ground plane, walls as line segments, and simple convex blockers,
authored directly in the scene definition. This is enough to build crossing,
bottleneck, and dense-flow scenes with a real doorway, and walls feed the
avoidance solver as static constraints.

## 7. Avoidance

A sampled-velocity solver behind an `AvoidanceSolver` trait, so the ORCA-style
and scoped time-to-collision candidates slot in for the next slice's bake-off.

Each agent evaluates a fixed, fixed-order set of candidate velocities — rings of
speeds and headings, plus the preferred velocity itself — and scores each by:

- distance from preferred velocity;
- predicted time to collision against neighbors and wall segments;
- deviation from the agent's current velocity.

The third term is not decoration. It is the primary defense against the
high-frequency direction oscillation that contract section 6.2 names as a
production blocker.

Neighbors are assumed to hold their current velocity, and avoidance
responsibility is shared half-and-half. That is the reciprocal-velocity insight
and it costs nothing to include.

Head-on symmetry is broken by a **fixed keep-left convention** evaluated in the
agent's own frame, not by an ID comparison. Two agents meeting head-on see
mirrored geometry, so if each asked "am I the lower ID?" they would derive
opposite answers — and in mirrored frames, opposite answers produce the *same*
world-space deflection, leaving them on a collision course. A fixed convention
produces opposite world deflections, which is what actually separates them.

Stable IDs still supply the asymmetry section 6.2 requires, applied where it is
genuinely needed: a perpendicular crossing conflict is symmetric under the
keep-left rule, so the higher-ID agent yields through a heavier collision
weight. Both agents compute that from data both already hold, so they never
both yield or both push.

Preferred speed is reduced by local density, per section 6.2's density-aware
speed reduction. When no candidate clears the collision threshold, the agent
brakes and is counted as stalled — the section 6.2 graceful fallback — rather
than being displaced out of the problem.

## 8. Benchmark scenes

| Scene | Purpose |
|---|---|
| `bidirectional_corridor` | The contract's two-flow benchmark; lane formation |
| `crossing` | Two perpendicular flows |
| `bottleneck` | Doorway throughput and congestion |
| `dense_flow` | High density converging on one exit |
| `circle` | Antipodal swap; the cheapest exposure of oscillation and deadlock |

Each runs at 1,000 agents, plus a 100/500/1,000/2,000 sweep for scaling curves.

## 9. Metrics and reporting

The subset of contract section 12.3 that applies without animation or a cache:

- destination completion rate, median and p95 travel time;
- agent-agent penetration count, maximum depth, and total duration;
- minimum predicted time to collision and near-miss count;
- static boundary violations;
- stalled agent count and stall duration;
- heading-reversal and abrupt-turn counts;
- doorway throughput;
- simulation wall time, ticks per second, peak allocated bytes;
- per-phase time shares for index, perceive, steer, and integrate.

Reports record CPU, RAM, OS, rustc version, build profile, scene hash, and tick
rate, per section 8.3. Blender version is not recorded because this slice does
not involve Blender.

Baselines are measured once and checked into `benchmarks/baselines/`. Each
baseline file declares a per-metric relative tolerance alongside its measured
value; the `check` command reruns the scene and fails when any metric drifts
beyond its declared tolerance. Per section 12.3, this slice asserts no absolute
quality thresholds.

## 10. Error model

Scene construction is fallible and returns diagnostics naming the offending
entity, per contract section 10.3: unreachable destination, spawn region outside
walkable bounds, disconnected waypoint graph, duplicate stable ID.

The tick loop itself is infallible. It cannot fail, only report. Non-finite
state is a panic in debug builds and a counted, reported anomaly in release,
because silently propagating a NaN through a bake is the worst available
outcome.

## 11. Testing

Development follows test-driven development: write the test, watch it fail, then
implement. The determinism work rewards this heavily.

Per contract section 15.1:

- **Unit tests** beside their modules: grid cell assignment, route following,
  candidate scoring, wall penetration resolution, ID derivation.
- **Property tests** via `proptest`, a dev-dependency only: stable identity,
  bounded motion, and grid neighbor queries equivalent to brute force.
- **Golden tests** for the scene definition and metrics report schemas.
- **Determinism tests**: run twice and compare per-tick state hashes bitwise;
  permute spawn input order and compare by ID; add one agent and confirm no
  other agent's attributes change.
- **Randomized fuzz** at high density, checked for NaN, escape through walls,
  and deadlock.

Microbenchmarking uses the scene runner's own per-phase timings rather than a
separate benchmark harness dependency.

## 12. Visual output

A hand-written SVG per scene: walls, waypoint graph, trajectory polylines, and a
density overlay, plus snapshot frames. No dependencies.

Metrics cannot tell you a crowd looks robotic. This makes avoidance quality
assessable well before the Blender bridge exists, which matters because contract
section 16 lists "avoidance looks robotic or deadlocks" as a top risk.

## 13. What this slice unlocks

At completion, the next slices in contract order become tractable:

1. The avoidance bake-off — two more `AvoidanceSolver` implementations measured
   against checked-in baselines on existing scenes.
2. Tiled navmesh and corridors — replacing the section 5 routing stand-in.
3. Cache v0 and the Blender bridge — with a simulator whose output is worth
   caching.
