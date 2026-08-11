# Tiled navmesh/corridor prototype (M0 item 4) — design

Date: 2026-08-08
Status: approved design, ready for implementation planning
Parent contract: [Blender Crowd 1.0 architecture and MVP](../../blender-crowd-1.0.md), section 6.1
Owning milestone: [M0 — Proving grounds](../../milestones/M0-proving-grounds.md)
Prior slices: [Deterministic crowd simulation kernel](2026-08-04-crowd-sim-kernel-design.md),
[Avoidance solver comparison](2026-08-06-avoidance-solver-comparison-design.md),
[Blender bridge slice](2026-08-07-blender-bridge-slice-design.md)

## 1. Scope

The kernel slice shipped `WaypointGraph`, an explicitly-named stand-in
(`route.rs`'s own doc comment: "a deliberate stand-in for the tiled
navmesh... when real navigation lands, it replaces this module"). This slice
adds real tiled navigation without touching that module or the six existing
avoidance benchmark scenes, closing M0 item 4 and acceptance criterion 3.

### 1.1 In scope

- `crates/crowd-core/src/nav/` — a uniform-grid tiled navmesh: rasterization
  of a scene's bounds/walls into walkable/blocked tiles, per-tile cost areas,
  portal edges with a stable ID and an open/closed flag, and deterministic A*
  over the tile graph.
- A new `phases/plan.rs` tick phase implementing path budgeting: a bounded
  per-tick expansion budget that amortizes (re)planning across ticks instead
  of solving every agent's path in one tick.
- Dynamic obstacle policy: transient obstacles affect local avoidance only
  (already true today); only a portal toggle invalidates corridors, and only
  for agents whose corridor used that portal.
- A `NavDebugSnapshot` — tile grid, cost, walkable/portal state, and
  per-agent corridor — dumped through the same SVG-trace machinery
  `crowd-bench --svg` already uses.
- One new checked-in benchmark scene (`two_room`) built specifically to host
  a 1,000-agent portal-reroute case: two rooms joined by two doorways, so
  closing one doorway must reroute only the agents that were using it.
- A `crowd-bench` mode that runs `two_room`, closes a portal partway through,
  and asserts/report the reroute behavior acceptance criterion 3 requires.
- A dated decision record under `docs/benchmarks/` reporting the reroute
  test, path-budget cost, and known limitations.

### 1.2 Explicitly out of scope

- Removing or migrating `WaypointGraph`. The six existing avoidance scenes
  keep using it unchanged; this slice adds a second, independent route
  *producer* that pushes into the same `RouteArena`.
- Polygon navmesh, triangulation, or funnel/string-pulling corridor
  smoothing. Tile-center/portal-midpoint corridors feed into the existing
  `next_target` lookahead+centreline-pull steering unchanged, which already
  handles polyline following and lane formation.
- Recast/Detour or any third-party navmesh/pathfinding library — no
  dependency/license review has been done, and the contract does not require
  one for a proving-grounds prototype.
- Multi-floor, non-uniform tile sizes, or off-axis geometry. Tiles are
  square and axis-aligned, matching every existing scene's axis-aligned
  walls.
- Changing the avoidance solvers, cache, Blender bridge, or facade
  (M0 items 3, 5, 6, 7).
- Full production path-request API (priorities, async cancellation
  semantics beyond what budgeting already implies) — the plan phase here
  proves the budgeting *mechanism*, not a scheduler product.

### 1.3 Success criteria

1. A 1,000-agent `two_room` run reroutes agents through the remaining open
   doorway after the other is closed, and agents whose corridor never used
   the closed doorway are unaffected (their `route` handle and
   `route_index` are untouched by the toggle) — acceptance criterion 3,
   verified by an automated test, not just a visual check.
2. Cost areas measurably change path choice: a cheaper detour tile route is
   preferred over a more-expensive direct route when the direct route's
   tiles carry a high cost multiplier, and vice versa.
3. Path budgeting bounds per-tick planning cost: a scene that spawns all
   1,000 agents in one tick does not spend an entire tick's budget doing
   1,000 A* searches in that tick; the report measures ticks-to-fully-routed
   against the configured budget.
4. `NavDebugSnapshot` output round-trips into the existing SVG trace tooling
   and visibly shows the closed portal and the rerouted corridors.
5. `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D
   warnings` stay clean throughout.
6. No change to any of the six existing avoidance scenes' behavior,
   verified by their existing tests and baselines passing unmodified.

## 2. Tile grid and rasterization

```rust
pub struct TileGrid {
    origin: Vec2,
    tile_size: f32,
    cols: u32,
    rows: u32,
    walkable: Vec<bool>,      // cols * rows, row-major
    cost: Vec<f32>,           // per-tile cost multiplier, default 1.0
}
```

- `tile_size` defaults to 1.0m — coarse enough that a 40m×40m scene (the
  existing scenes' scale) stays a few thousand tiles, fine enough to route
  through a ~2m doorway without the doorway vanishing at rasterization.
- A tile is walkable when its center, inflated by the largest agent-radius
  preset in the scene (mirrors "agent-radius presets" from contract 6.1),
  does not intersect any wall `Segment`. Rasterization reuses
  `Segment::distance_to`, already used elsewhere in `geometry.rs`.
- Cost areas are authored as `Vec<(Aabb, f32)>` on the scene def; any tile
  whose center falls inside a cost-area AABB gets that multiplier (last
  writer wins on overlap, applied in authored order — deterministic and
  simple, matching `WaypointGraph`'s existing "authored order is the fixed
  order" convention).
- Rasterization runs once at scene compile time (mirrors `WaypointGraph`
  construction), not per tick.

## 3. Portals and the tile graph

```rust
pub struct PortalId(pub u32);

pub struct Portal {
    pub id: PortalId,
    pub tile_a: u32,
    pub tile_b: u32,
    pub open: bool,
}

pub struct TileGraph {
    grid: TileGrid,
    portals: Vec<Portal>,          // one per walkable-tile adjacency
    portals_by_tile: Vec<Vec<u32>>, // adjacency list, index into `portals`
}
```

- Every adjacent pair of walkable tiles (4-connected: N/E/S/W, not diagonal —
  diagonal movement would let a corridor clip a blocked tile's corner) gets
  exactly one portal, open by default.
- Portals are addressed by `PortalId`, not by tile pair, because the
  runtime API for "close this portal" (tests, and later authoring) should
  not require recomputing which tiles border which. Scene authoring assigns
  IDs to specific doorway-crossing portals by naming them (position-based
  lookup at compile time — "the portal nearest this point" — same pattern
  `WaypointGraph::nearest_node` already uses for spawn/destination
  resolution) so a test can say "close the north doorway" without hardcoding
  a raw tile index.
- `TileGraph::set_portal_open(id, bool)` is the one mutation entry point.
  It does not touch tile walkability or cost — a closed portal is a missing
  edge, not a blocked tile, matching contract 6.1: "Dynamic obstacle policy"
  and "topological change" are named separately from "cost areas."

## 4. Pathfinding

```rust
pub fn find_path(graph: &TileGraph, from_tile: u32, to_tile: u32) -> Option<Vec<u32>>
```

- A* over the tile graph, edge cost = destination tile's cost multiplier ×
  center-to-center distance, heuristic = Euclidean distance to goal tile
  center (admissible since cost multipliers are >= the scene's declared
  minimum, enforced at compile time same as `MIN_PREFERRED_SPEED`).
- Tie-breaking: `WaypointGraph::shortest_path` uses an O(V²) linear scan
  specifically to avoid needing a total order over `f32` costs in a binary
  heap. The tile graph is larger (low thousands of tiles vs. tens of nodes),
  so this slice uses a binary heap keyed on a **bit-pattern-ordered** cost
  (`f32::total_cmp`, which is a real total order, not a heuristic one) with
  ties broken by tile index — deterministic and no linear-scan blowup at
  tile-graph scale. This is a deliberate, documented deviation from
  `WaypointGraph`'s approach, justified by scale, not a contradiction of the
  determinism rule (`total_cmp` is exact, not tolerance-based).
- Output is a tile-index path; corridor extraction turns it into `Vec<Vec2>`
  by taking each crossed portal's edge midpoint, plus the exact start and
  goal points, then pushes that into the existing `RouteArena` via
  `push_route` — identical to how `spawn.rs` currently builds a route from
  `WaypointGraph::shortest_path` output. **No change to `route.rs`,
  `decide.rs`, or `steer.rs`.**

## 5. Path budgeting (`phases/plan.rs`)

This fills the pipeline's phase 5 ("Plan"), which today is folded into spawn
(routes are computed once, eagerly, at spawn time) because `WaypointGraph`
paths are cheap. Tile-graph A* is not free at 1,000-agent spawn bursts, so it
gets its own phase and a budget.

```rust
pub struct PlanConfig {
    pub max_expansions_per_tick: u32,   // A* node-expansion budget
}

pub struct PlanState {
    pending: VecDeque<u32>,             // agent slots awaiting a (re)plan
}
```

- An agent enters `pending` when: it spawns onto a nav-mesh-routed scene with
  no route yet, or a portal toggle invalidates its current corridor (section
  6), or `decide.rs` reports corridor exhaustion but the agent has not
  reached its true destination tile (multi-corridor journeys — out of scope
  for the `two_room` scene, but the queueing mechanism does not assume a
  single corridor covers a whole journey, so it is not a wall this slice
  builds itself into).
- Each tick, `plan()` pops from the front of `pending` and runs `find_path`,
  charging its actual node-expansion count against
  `max_expansions_per_tick`, until the budget is spent or the queue is
  empty. Order is FIFO by slot-enqueue order, which is deterministic because
  enqueue order is itself deterministic (spawn order, then portal-toggle
  invalidation in stable ID order).
- An agent with no route yet (still queued) does not move: `decide.rs`
  already treats "no route" as `unrouted`, which existing code paths handle
  as a diagnosable non-arrival, not a crash. An agent with a *stale but
  still geometrically valid* route (not yet invalidated) keeps using it
  until its replan completes — only a portal toggle actually invalidates a
  route, so "stale" only happens in that case, and the invalidated route is
  simply not used for steering until the requeue finishes (falls back to
  `unrouted` handling for that window, same as "no route").

## 6. Portal toggle → selective invalidation

- `TileGraph::set_portal_open` returns the set of tile-graph edges that
  changed (in this design, always exactly one portal, but the return shape
  is a set so a future multi-portal toggle is not a signature break).
- Invalidation scan: an agent is invalidated only if the closed portal's ID
  appears in the *portal sequence* its current corridor actually crossed —
  recorded alongside the corridor when it was built (`Vec<PortalId>` stored
  next to the route in a small side table keyed by `RouteHandle`, not
  reconstructed by re-deriving portals from points). This is what makes
  "reroutes without corrupting unrelated corridors" a checked property
  instead of an accident of scene geometry: the invalidation test does not
  rely on "the other room happens to be far away," it relies on the closed
  portal's ID simply not being in those agents' recorded portal sequence.
- Invalidated agents: `route` handle is cleared to `NO_ROUTE`,
  `route_index` reset to 0, `unrouted` stays false (they are mid-journey,
  not undestined), and they are pushed onto `plan.rs`'s `pending` queue for
  a fresh `find_path` from their current tile to the same destination tile.

## 7. `NavDebugSnapshot`

```rust
pub struct NavDebugSnapshot {
    pub grid: TileGrid,               // walkable + cost, for rendering
    pub portals: Vec<(Portal, Vec2)>, // portal + its midpoint, for rendering
    pub corridors: Vec<(u32, Vec<Vec2>)>, // (agent slot, corridor points)
}
```

- Produced on demand (not every tick — same "sampled every tick only under
  `--svg`/`--trace`" cost discipline the project guidance already states)
  by a `crowd-bench` flag, and rendered through the existing SVG writer used
  by `--svg`/`--frames`, adding tile fill (walkable/blocked/cost-tinted),
  portal markers (open/closed), and corridor polylines as new SVG layers
  alongside the existing agent-disc rendering. No new rendering dependency.

## 8. The `two_room` scene

- Two rectangular rooms sharing one wall, with two doorway gaps (two
  portals of interest, named `north_door` and `south_door` at compile time
  via nearest-portal lookup). Spawn regions in room A, destination in room
  B, split roughly evenly across both doors by proximity (agents path
  through whichever door their A* solve prefers, which is "nearest door"
  under uniform cost — cost areas are exercised by a *second*, smaller cost
  scenario within the same scene file, not by inventing a second scene).
- Scaled to 1,000 agents to match acceptance criterion 3's stated scale,
  following the existing `scaled(scale)` pattern scenes already use.
- The reroute test: run the scene, close `south_door` partway through,
  continue simulating, and assert (a) every agent that had crossed
  `south_door` in its recorded portal sequence gets a new corridor that
  uses `north_door` instead (or reports `unrouted` only transiently, never
  permanently, since `north_door` is always reachable), and (b) every agent
  whose corridor never touched `south_door` has an unchanged `route` handle
  across the toggle tick.

## 9. Testing

- `nav/grid.rs` unit tests: rasterization walkability for a known wall
  layout, cost-area assignment, tie-break determinism for equidistant
  tile-center cases (mirrors `WaypointGraph::nearest_node`'s existing tie
  test).
- `nav/portal.rs` unit tests: portal generation is symmetric and complete
  (every walkable 4-adjacency has exactly one portal), closing a portal
  removes exactly one edge, reopening restores it.
- `nav/pathfind.rs` unit tests: shortest path on a known small grid,
  cost-area detour preference (criterion 2), unreachable-goal returns
  `None`, determinism (same input twice produces identical tile sequence).
- `phases/plan.rs` unit tests: budget cap is respected (queue of N agents,
  budget for M < N expansions, only M/expansions-worth get routed this
  tick), FIFO order, an agent mid-queue never moves before it has a route.
- Integration test in `tests/` (or `crowd-core/tests/`, matching existing
  layout): the full `two_room` portal-close-and-reroute scenario at reduced
  scale (e.g. 50 agents) for fast CI, plus the 1,000-agent version gated
  behind the same `--release` discipline as the existing density fuzz test.
- Existing determinism suite (bitwise per-tick hash, spawn-order
  permutation, add-one-agent) parametrized to include a `two_room` run,
  proving the new plan phase does not break the kernel's existing
  determinism guarantees.

## 10. Validation

```sh
cargo test --workspace
cargo test --release -p crowd-core --test two_room_reroute   # 1,000-agent portal reroute
cargo clippy --workspace --all-targets -- -D warnings
cargo run --release -p crowd-bench -- run --scene two_room --agents 1000 --nav-debug-svg
```

The last command's exact flag name is finalized during implementation
planning; it is recorded here as the intended shape so `README.md`/`AGENTS.md`
get the real command once the runner exists, per milestone rule 8.

## 11. Known limitations carried into the M0 report

- Corridors are tile-center/portal-midpoint polylines, not funnel-smoothed —
  visually blockier than a polygon navmesh at tight turns. Acceptable for a
  proving-grounds prototype; `next_target`'s existing lookahead already
  prevents literal zig-zag stepping.
- Tile size is fixed per scene, not adaptive; a doorway narrower than
  `tile_size` after agent-radius inflation would rasterize as blocked. The
  `two_room` scene's doorways are authored wide enough to avoid this, and
  the limitation is stated rather than solved.
- Multi-corridor journeys (a route that must cross more destination tiles
  than one `find_path` call covers) are not exercised by `two_room`; the
  plan-phase queueing mechanism does not preclude them, but nothing in this
  slice tests them.
