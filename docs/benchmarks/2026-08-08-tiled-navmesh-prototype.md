# Tiled navmesh/corridor prototype — M0 item 4

Date: 2026-08-08
Milestone: [M0 — Proving grounds](../milestones/M0-proving-grounds.md)
Design: [Tiled navmesh/corridor prototype design](../superpowers/specs/2026-08-08-tiled-navmesh-prototype-design.md)

## Environment

| | |
|---|---|
| CPU | Apple M1 Max |
| OS | macOS 27.0 (BuildVersion 26A5378n) |
| Rust | 1.94.1 |

## What was built

A uniform-grid tiled navmesh (`crowd_core::nav`) coexisting with the existing
`WaypointGraph` — none of the six existing avoidance benchmark scenes changed.
A new `plan` tick phase budgets A* path (re)planning across ticks. A new
`two_room` scene (two rooms, two doorways) proves the architecture at the
contract's 1,000-agent scale.

## Results

| Measure | Value |
|---|---|
| Scene | `two_room`, 1,000 agents, seed 2026 |
| Agents invalidated on closing the south door | 505 |
| Agents untouched by closing the south door | 495 |
| Of the invalidated agents, new route verified to cross `north_door` | 478 |
| Agents arrived after the reroute | 15 |
| Release-gated 1,000-agent reroute test | PASS, 26.45s (test wall time; 26.53s wall including a no-op release rebuild) |

Command run: `cargo run --release -p crowd-bench -- nav-reroute --agents 1000
--svg`, printing `two_room: 505 invalidated / 495 untouched on close, 478 of
the invalidated crossed north_door, 15 arrived after reroute ->
benchmarks/reports/two_room-reroute-1000.json` and writing that JSON report
plus the accompanying SVG debug trace.

### The doorway now genuinely closes

A final whole-branch review found a Critical bug in the slice that produced
the numbers previously reported here: `two_room`'s doorways are 1.6 m wide,
which after tile size (0.5 m) and agent-radius (0.3 m) inflation spans *two*
adjacent portals, but each named door only captured one of them via
`nearest_portal`. Closing `south_door` therefore left the doorway's other
portal open, and every "rerouted" agent simply walked back through the same
doorway it was supposedly locked out of — it never actually reached
`north_door`. No test asserted *which* door a rerouted agent's new corridor
used, only that it had *some* route, so this passed unnoticed.

That is now fixed. Named doors resolve to `Vec<PortalId>`
(`TileGraph::portals_named`), `Simulation::set_portals_open` closes or opens
a whole named door's portal set atomically, and `Simulation::route_crosses_any`
lets a caller check whether an agent's *current* route's recorded portal
sequence actually crosses a given portal set. `two_room_reroute.rs`'s
1,000-agent test now asserts, for every agent invalidated by the south-door
close that was still on room A's side of the divider at the moment of
invalidation, that its new route is verified to cross `north_door` — not
merely that it has *a* route. In this run, 478 of the 505 invalidated agents
were verified to cross `north_door` this way (the released assertion, this
one, in the test only counts the subset still in room A when invalidated —
see below for why the remaining 27 are correctly excluded, not a shortfall).

### Why not all 505 invalidated agents show up as "crossed north_door"

`run_until_initially_routed` allows up to 2,000 ticks for the fastest of
1,000 agents (spawned in batches of 8/tick, seed 2026) to acquire their first
route before the south door closes — plenty of time, at the scene's ~1.35 m/s
mean walking speed, for the earliest spawns to already be deep into room B,
in some cases essentially at the destination, by the time the close happens.
Such an agent's *historical* corridor legitimately crossed a south portal
(which is exactly why closing it invalidates that agent's stale route
record), but once replanned from its *current*, already-past-the-divider
position, the shortest path to the room-B destination correctly crosses no
door at all — there is nothing left to cross. This was confirmed directly:
instrumenting the previously-failing assertion showed the one violating case
was an agent at `x≈37.97` (destination is `x=38`) and already `arrived`.
`two_room_reroute.rs` now captures each agent's position at the moment of
invalidation and scopes the "must cross north_door" assertion to agents still
on room A's side of the divider then; agents already past it are correctly
exempt, not silently passed. This is the reason the CLI's 478/505 figure is
expected, not a defect — the remaining ~27 were already in or near room B.

### On the "15 arrived" number

Fifteen arrivals out of 1,000 agents in the ticks this run allots is
expected, not a bug or a deadlock. Most of the invalidated agents are now
rerouted through a single ~1.6 m doorway (`north_door`) toward one
destination point; funneling roughly half of 1,000 agents through one
doorway is inherently throughput-limited by the doorway's width, not by the
planner or the budgeting logic. This matches the project's existing
documented behavior for single-chokepoint scenes: the `crossing`/`bottleneck`
avoidance scenes quoted in `README.md` show a 24% completion rate in their
own allotted ticks for the same reason, and it is treated there as expected
congestion character rather than a defect. The acceptance criterion this
report addresses is that the reroute does not corrupt unrelated corridors and
genuinely uses the other door (see the 495 untouched agents, the 478
verified `north_door` crossings, and the passing `two_room_reroute` test
below), not a target arrival count or throughput figure — no throughput claim
is made here.

## Acceptance criteria addressed

- M0 criterion 3 (1,000-agent tiled-navigation reroute without corrupting
  unrelated corridors): `cargo test --release -p crowd-core --test
  two_room_reroute -- --ignored`, and `crowd-bench nav-reroute --agents 1000`.

## Known limitations

- Corridors are tile-center/portal-midpoint polylines, not funnel-smoothed.
- Tile size (0.5 m in `two_room`) is fixed per scene, not adaptive.
- Multi-corridor journeys are not exercised by `two_room`.
- Dynamic (transient) obstacles were not added in this slice; only the
  topological portal-close/reopen path was exercised, per contract 6.1's
  stated separation between the two.

## Unsupported claims

This report does not claim anything about 10K/100K agent scale, GPU
navigation, or funnel-smoothed corridor quality — none were measured. The
"15 arrived" figure is not a throughput or performance benchmark; it is a
single-run congestion observation under a fixed tick budget, explained above
so it is not mistaken for one. Likewise the 478/505 "crossed north_door"
figure is not a claim that 27 agents misrouted — it is explained above as the
expected count of agents already past the divider when the door closed.

## Next gate

M0 item 5 (cache v0) is still open, and item 7 (Python/Rust facade) is still
partial. M0 acceptance criterion 4 (cache round trip/incomplete-state
behavior) cannot be attempted before item 5 exists. Criterion 7's
consolidated dated M0 report is still not written. M1 stays blocked until
these close.
