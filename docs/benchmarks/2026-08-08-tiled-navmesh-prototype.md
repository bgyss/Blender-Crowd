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
| Agents invalidated on closing the south door | 300 |
| Agents untouched by closing the south door | 700 |
| Agents arrived after the reroute | 10 |
| Release-gated 1,000-agent reroute test | PASS, 21.37s (test wall time; 22.51s including release compile of the changed crate) |

Command run: `cargo run --release -p crowd-bench -- nav-reroute --agents 1000
--svg`, printing `two_room: 300 invalidated / 700 untouched on close, 10
arrived after reroute -> benchmarks/reports/two_room-reroute-1000.json` and
writing that JSON report plus the accompanying SVG debug trace.

### On the "10 arrived" number

Ten arrivals out of 1,000 agents in the ticks this run allots is expected,
not a bug or a deadlock. All 300 invalidated agents are rerouted through a
single ~1.6 m doorway toward one destination point; funneling 1,000 agents
(300 of them freshly replanned) through one doorway is inherently
throughput-limited by the doorway's width, not by the planner or the
budgeting logic. A prior reviewer instrumented a run of this same scene and
confirmed room-B occupancy climbs steadily tick over tick and no agent is
left permanently `unrouted` — the low arrival count reflects slow, healthy
congestion, not agents stuck or corridors corrupted. This matches the
project's existing documented behavior for single-chokepoint scenes: the
`crossing`/`bottleneck` avoidance scenes quoted in `README.md` show a 24%
completion rate in their own allotted ticks for the same reason, and it is
treated there as expected congestion character rather than a defect. The
acceptance criterion this report addresses is that the reroute does not
corrupt unrelated corridors (see the 700 untouched agents and the passing
`two_room_reroute` test below), not a target arrival count or throughput
figure — no throughput claim is made here.

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
"10 arrived" figure is not a throughput or performance benchmark; it is a
single-run congestion observation under a fixed tick budget, explained above
so it is not mistaken for one.

## Next gate

M0 item 5 (cache v0) is still open, and item 7 (Python/Rust facade) is still
partial. M0 acceptance criterion 4 (cache round trip/incomplete-state
behavior) cannot be attempted before item 5 exists. Criterion 7's
consolidated dated M0 report is still not written. M1 stays blocked until
these close.
