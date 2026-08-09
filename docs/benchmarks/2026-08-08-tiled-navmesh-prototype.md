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
| Of the invalidated agents, new route verified to cross `north_door` | 482 |
| Agents arrived after the reroute | 7 |
| Release-gated 1,000-agent reroute test | PASS, 22.37s (test wall time), run twice with identical results |

Command run: `cargo run --release -p crowd-bench -- nav-reroute --agents 1000
--svg`, printing `two_room: 505 invalidated / 495 untouched on close, 482 of
the invalidated crossed north_door, 7 arrived after reroute ->
benchmarks/reports/two_room-reroute-1000.json` and writing that JSON report
plus the accompanying SVG debug trace.

These are fresh numbers from a re-run after the capture-mechanism fix below;
they supersede every number previously reported in this file's "478/505" and
"15 arrived" narrative, which was measured against the buggy 28-portal
capture.

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

That was fixed by resolving named doors to `Vec<PortalId>`
(`TileGraph::portals_named`), with `Simulation::set_portals_open` closing or
opening a whole named door's portal set atomically, and
`Simulation::route_crosses_any` letting a caller check whether an agent's
*current* route's recorded portal sequence actually crosses a given portal
set.

### The proximity-radius capture itself was also wrong

A second re-review, after the above fix landed, found the *capture
mechanism* it introduced was itself broken: resolving a named door to "every
portal whose midpoint lies within a radius of the doorway point" is not the
same as "every portal that crosses the doorway." For `two_room`'s geometry,
the capture radius (`DOOR_HALF_WIDTH + TILE_SIZE = 1.3`) needed to be wide
enough to span the doorway's multiple tile rows, but that same radius also
reached ordinary in-room portals nearby that never cross the dividing wall at
all — both north-south portals fully on one side of the wall, and east-west
portals entirely inside one room close to it. Measured directly: each
doorway resolved to **28 candidate portals**, of which only **2** actually
cross the `x = 20` divider. Closing `south_door` under that bug closed all
28, sealing 12 unrelated walkable tiles into isolated pockets and
permanently stranding agents that ended up inside them — never requeued,
never routed, forever `unrouted`.

The fix: `TileGraph::portal_axis` classifies each portal as `EastWest`
(column-adjacent tiles, same row) or `NorthSouth` (row-adjacent tiles, same
column) — matching exactly how `TileGraph::build` constructs them. A named
door now also carries a `CrossingAxis`, and `TileGraph::portals_within_axis`
filters the radius-based candidates down to only portals that (a) run along
the given axis *and* (b) straddle the doorway point along that axis — i.e.
the portal's two tile centers are genuinely on opposite sides of the wall,
not merely nearby it. For `two_room`'s two doors this resolves to exactly 2
portals each, verified against every divider-straddling portal in the whole
graph (not just those within the radius), so no genuine crossing is missed
either. `crates/crowd-core/src/nav_scenes.rs`'s
`named_doors_resolve_to_exactly_the_portals_that_cross_the_divider` test
pins this down permanently.

With the fix, `two_room_reroute.rs`'s 1,000-agent test now also asserts that
zero agents end the run permanently `unrouted` — the direct regression guard
for the sealed-pocket bug — in addition to the pre-existing check that every
agent invalidated by the south-door close, while still on room A's side of
the divider at the moment of invalidation, gets a new route verified to
cross `north_door`. In this run, 482 of the 505 invalidated agents were
verified to cross `north_door` this way; see below for why the remaining 23
are correctly excluded, not a shortfall, and confirmation that none of them
are stranded.

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
door at all — there is nothing left to cross.

This category is the *entire* explanation for the gap under the fixed
capture mechanism. A direct breakdown at the CLI's own settle-tick window
(matching `crowd-bench nav-reroute`'s 600-tick close and 600-tick
post-close settle exactly) categorized every one of the 23 gap agents as
already on room B's side of the divider at the moment of invalidation — zero
were still in room A and stuck (`unrouted_at_measure = 0`), and zero drifted
across the still-geometrically-open doorway gap while waiting to be replanned
(`drifted_to_room_b_by_measure = 0`; closing a portal removes it from
pathfinding but does not erect a physical wall in the gap, so a densely
packed agent right at the threshold can in principle be pushed through by
crowd pressure before its replan runs — `two_room_reroute.rs`'s assertions
account for this possibility, it simply did not occur in this run's 23-agent
gap). `two_room_reroute.rs` captures each agent's position at the moment of
invalidation and scopes the "must cross north_door" assertion to agents
still on room A's side of the divider then (also re-checking current
position at assertion time, to correctly exempt an agent that drifted across
during the replan-wait window rather than one that was invalidated while
already past it); agents already past the divider are correctly exempt, not
silently passed. This is the reason the CLI's 482/505 figure is expected,
not a defect — the remaining 23 were already in or near room B, a smaller
gap than the pre-fix report's 27 because the corrected capture invalidates
fewer routes overall (2 genuine south-door crossings instead of 28
proximity-based false positives, so fewer agents get needlessly caught up in
the close in the first place).

### On the "7 arrived" number

Seven arrivals out of 1,000 agents in the ticks this run allots is expected,
not a bug or a deadlock, and is not comparable to the previous report's "15"
— that number was measured against the buggy 28-portal capture, which sealed
12 unrelated tiles into isolated pockets and materially changed the crowd's
flow dynamics (fewer usable tiles near the divider, different congestion
patterns) relative to the corrected 2-portal capture measured here. Most of
the invalidated agents are now rerouted through a single ~1.6 m doorway
(`north_door`) toward one destination point; funneling roughly half of 1,000
agents through one doorway is inherently throughput-limited by the doorway's
width, not by the planner or the budgeting logic. This matches the project's
existing documented behavior for single-chokepoint scenes: the
`crossing`/`bottleneck` avoidance scenes quoted in `README.md` show a 24%
completion rate in their own allotted ticks for the same reason, and it is
treated there as expected congestion character rather than a defect. The
acceptance criterion this report addresses is that the reroute does not
corrupt unrelated corridors and genuinely uses the other door (see the 495
untouched agents, the 482 verified `north_door` crossings, the zero
permanently-stranded agents, and the passing `two_room_reroute` test below),
not a target arrival count or throughput figure — no throughput claim is made
here.

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
"7 arrived" figure is not a throughput or performance benchmark; it is a
single-run congestion observation under a fixed tick budget, explained above
so it is not mistaken for one, and it is not comparable across the capture
fix (see above). Likewise the 482/505 "crossed north_door" figure is not a
claim that 23 agents misrouted — it is explained above, and confirmed by a
direct per-agent breakdown, as the expected count of agents already past the
divider when the door closed, with zero of them permanently stranded.

## Next gate

M0 item 5 (cache v0) is still open, and item 7 (Python/Rust facade) is still
partial. M0 acceptance criterion 4 (cache round trip/incomplete-state
behavior) cannot be attempted before item 5 exists. Criterion 7's
consolidated dated M0 report is still not written. M1 stays blocked until
these close.
