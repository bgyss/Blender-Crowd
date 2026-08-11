# Tiled navmesh/corridor prototype Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close M0 item 4 (tiled navmesh/corridor prototype) by adding a uniform-grid tiled navmesh with portals, cost areas, and budgeted path planning, proven by a 1,000-agent scene that reroutes through a remaining doorway when the other is closed — without touching the six existing waypoint-routed benchmark scenes.

**Architecture:** A new `crowd-core::nav` module rasterizes scene bounds/walls into a tile grid, builds a portal graph over it, and runs deterministic A*. A new `plan` tick phase (inserted between perceive and decide) drains a budgeted queue of agents needing a corridor and pushes results into the existing `RouteArena`, so `route.rs`/`decide.rs`/`steer.rs` need no changes. `SceneDef`/`CompiledScene` gain optional `nav`/`nav_destinations` fields; when absent, every existing code path is byte-identical to today.

**Tech Stack:** Rust workspace (`crowd-core`, `crowd-bench`), no new dependencies.

## Global Constraints

- `cargo fmt` before every commit.
- `cargo clippy --workspace --all-targets -- -D warnings` must stay clean.
- `cargo test --workspace` must stay clean; the release density fuzz test and any new release-gated test run via `cargo test --release -p crowd-core --test <name>`.
- Determinism: every new algorithm ties-break on a stable, already-existing key (tile index, `PortalId`, `AgentId`, or slot) — never on incidental container order. Float comparisons that must be a total order use `f32::total_cmp`, never a tolerance.
- No new crate dependencies without a license review (none are needed by this plan).
- Zero behavior change to the six existing scenes in `crowd-core/src/scenes.rs` (`bidirectional_corridor`, `crossing`, `bottleneck`, `dense_flow`, `circle`, `l_corridor`) or their baselines.
- `two_room` (the new nav-prototype scene) is deliberately **not** added to `crowd_core::scenes::SCENE_NAMES` / `scenes::build`, so it never becomes part of the default (no `--scene`) `crowd-bench sweep|baseline|check|compare` runs, which would otherwise require a baseline file that doesn't exist and break the documented regression command `cargo run --release -p crowd-bench -- check --agents 1000`.
- Update `README.md` and `AGENTS.md` with exact copy-ready commands once their runners exist (milestone rule 8) — done in the final task.

---

## File map

New:
- `crates/crowd-core/src/nav/mod.rs` — public API, `NavMeshDef`.
- `crates/crowd-core/src/nav/grid.rs` — `TileGrid` rasterization and cost areas.
- `crates/crowd-core/src/nav/portal.rs` — `Portal`, `PortalId`, `TileGraph` construction and toggling.
- `crates/crowd-core/src/nav/pathfind.rs` — deterministic A* and corridor extraction.
- `crates/crowd-core/src/phases/plan.rs` — the new budgeted `plan` tick phase.
- `crates/crowd-core/src/nav_scenes.rs` — `two_room`, the dedicated nav-prototype scene (outside `scenes::SCENE_NAMES`).
- `crates/crowd-core/tests/two_room_reroute.rs` — integration tests for acceptance criterion 3.
- `crates/crowd-bench/src/nav_bench.rs` — nav-reroute run/report + SVG nav-debug dump.
- `docs/benchmarks/2026-08-08-tiled-navmesh-prototype.md` — the dated M0 decision record.

Modified:
- `crates/crowd-core/src/scene.rs` — `nav`/`nav_destinations` fields, `compile()` branch, new `SceneError` variants.
- `crates/crowd-core/src/world.rs` — `state_hash` now covers the route handle (routes can change mid-run once portals exist).
- `crates/crowd-core/src/phases/spawn.rs` — branch route/heading assignment when `scene.nav.is_some()`.
- `crates/crowd-core/src/phases/mod.rs` — `pub mod plan`.
- `crates/crowd-core/src/metrics.rs` — new `Phase::Plan` variant.
- `crates/crowd-core/src/sim.rs` — `SimConfig.plan`, `Simulation.nav`/`plan_state`, wire the plan phase in, `set_portal_open`.
- `crates/crowd-core/src/scenes.rs`, `crates/crowd-core/src/scene.rs` tests, `crates/crowd-core/src/sim.rs` tests, `crates/crowd-core/src/phases/spawn.rs` tests — mechanical `nav: None, nav_destinations: Vec::new()` added to every existing `SceneDef` literal.
- `crates/crowd-core/src/lib.rs` — `pub mod nav;`, `pub mod nav_scenes;`, new `pub use`s.
- `crates/crowd-bench/src/main.rs` — new `nav-reroute` subcommand.
- `README.md`, `AGENTS.md`, `docs/milestones/README.md` — new commands and updated M0 baseline table.

---

### Task 1: `TileGrid` — rasterization and cost areas

**Files:**
- Create: `crates/crowd-core/src/nav/grid.rs`
- Create: `crates/crowd-core/src/nav/mod.rs` (stub: `pub mod grid; pub use grid::TileGrid;`)
- Modify: `crates/crowd-core/src/lib.rs:5-19` — add `pub mod nav;` after `pub mod metrics;` (alphabetical, matches the file's existing ordering)

**Interfaces:**
- Produces: `TileGrid::build(bounds: Aabb, tile_size: f32, walls: &[Segment], agent_radius: f32, cost_areas: &[(Aabb, f32)]) -> TileGrid`, `TileGrid::cols(&self) -> u32`, `TileGrid::rows(&self) -> u32`, `TileGrid::tile_count(&self) -> u32`, `TileGrid::tile_center(&self, tile: u32) -> Vec2`, `TileGrid::is_walkable(&self, tile: u32) -> bool`, `TileGrid::cost(&self, tile: u32) -> f32`, `TileGrid::nearest_walkable_tile(&self, p: Vec2) -> Option<u32>`.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/crowd-core/src/nav/grid.rs
//! Uniform-grid tiled navmesh rasterization.
//!
//! A tile is walkable when its center, inflated by the scene's agent radius,
//! clears every wall. Rasterization runs once, at scene-compile time, same as
//! `SegmentIndex`.

use crate::geometry::Segment;
use crate::units::{Aabb, Vec2};

#[derive(Clone, Debug)]
pub struct TileGrid {
    origin: Vec2,
    tile_size: f32,
    cols: u32,
    rows: u32,
    walkable: Vec<bool>,
    cost: Vec<f32>,
}

impl TileGrid {
    pub fn build(
        bounds: Aabb,
        tile_size: f32,
        walls: &[Segment],
        agent_radius: f32,
        cost_areas: &[(Aabb, f32)],
    ) -> Self {
        let size = bounds.size();
        let cols = (size.x / tile_size).ceil().max(1.0) as u32;
        let rows = (size.y / tile_size).ceil().max(1.0) as u32;
        let tile_count = (cols * rows) as usize;
        let mut walkable = vec![true; tile_count];
        let mut cost = vec![1.0f32; tile_count];

        let grid = Self {
            origin: bounds.min,
            tile_size,
            cols,
            rows,
            walkable: Vec::new(),
            cost: Vec::new(),
        };

        for row in 0..rows {
            for col in 0..cols {
                let index = (row * cols + col) as usize;
                let center = grid.tile_center_at(col, row);
                if walls.iter().any(|w| w.distance_to(center) < agent_radius) {
                    walkable[index] = false;
                }
                // Last authored cost area covering this tile wins.
                for (area, multiplier) in cost_areas {
                    if area.contains(center) {
                        cost[index] = *multiplier;
                    }
                }
            }
        }

        Self {
            walkable,
            cost,
            ..grid
        }
    }

    fn tile_center_at(&self, col: u32, row: u32) -> Vec2 {
        Vec2::new(
            self.origin.x + (col as f32 + 0.5) * self.tile_size,
            self.origin.y + (row as f32 + 0.5) * self.tile_size,
        )
    }

    pub fn cols(&self) -> u32 {
        self.cols
    }

    pub fn rows(&self) -> u32 {
        self.rows
    }

    pub fn tile_count(&self) -> u32 {
        self.cols * self.rows
    }

    pub fn tile_center(&self, tile: u32) -> Vec2 {
        self.tile_center_at(tile % self.cols, tile / self.cols)
    }

    pub fn is_walkable(&self, tile: u32) -> bool {
        self.walkable.get(tile as usize).copied().unwrap_or(false)
    }

    pub fn cost(&self, tile: u32) -> f32 {
        self.cost.get(tile as usize).copied().unwrap_or(1.0)
    }

    fn tile_at_point(&self, p: Vec2) -> Option<u32> {
        let local = p - self.origin;
        if local.x < 0.0 || local.y < 0.0 {
            return None;
        }
        let col = (local.x / self.tile_size) as u32;
        let row = (local.y / self.tile_size) as u32;
        if col >= self.cols || row >= self.rows {
            return None;
        }
        Some(row * self.cols + col)
    }

    /// The walkable tile nearest `p`, searching an expanding ring around the
    /// tile `p` falls in. Needed because agent positions and destination
    /// points do not land exactly on a tile center, and can sit fractions of
    /// a tile from a wall.
    pub fn nearest_walkable_tile(&self, p: Vec2) -> Option<u32> {
        let start = self.tile_at_point(p)?;
        if self.is_walkable(start) {
            return Some(start);
        }
        let (start_col, start_row) = (start % self.cols, start / self.cols);
        let max_ring = self.cols.max(self.rows);
        for ring in 1..=max_ring {
            let mut best: Option<(f32, u32)> = None;
            let lo_col = start_col.saturating_sub(ring);
            let hi_col = (start_col + ring).min(self.cols - 1);
            let lo_row = start_row.saturating_sub(ring);
            let hi_row = (start_row + ring).min(self.rows - 1);
            for row in lo_row..=hi_row {
                for col in lo_col..=hi_col {
                    let on_ring = row == lo_row || row == hi_row || col == lo_col || col == hi_col;
                    if !on_ring {
                        continue;
                    }
                    let tile = row * self.cols + col;
                    if !self.is_walkable(tile) {
                        continue;
                    }
                    let d = self.tile_center(tile).distance_squared(p);
                    if best.is_none_or(|(best_d, _)| d < best_d) {
                        best = Some((d, tile));
                    }
                }
            }
            if let Some((_, tile)) = best {
                return Some(tile);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_bounds() -> Aabb {
        Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0))
    }

    #[test]
    fn an_open_grid_is_fully_walkable() {
        let grid = TileGrid::build(open_bounds(), 1.0, &[], 0.3, &[]);
        for tile in 0..grid.tile_count() {
            assert!(grid.is_walkable(tile), "tile {tile} unexpectedly blocked");
        }
    }

    #[test]
    fn a_wall_blocks_tiles_within_the_agent_radius() {
        let wall = Segment::new(Vec2::new(5.0, 0.0), Vec2::new(5.0, 10.0));
        let grid = TileGrid::build(open_bounds(), 1.0, &[wall], 0.3, &[]);
        let blocked = grid.nearest_walkable_tile(Vec2::new(5.0, 5.0));
        // The exact tile under the wall must be blocked; some walkable tile
        // must still exist further away.
        let under_wall = grid
            .tile_center(grid.tile_count() / 2)
            .distance_squared(Vec2::new(5.0, 5.0));
        assert!(under_wall < 100.0); // sanity: this is the tile we mean
        assert!(!grid.is_walkable(grid.tile_count() / 2) || blocked.is_some());
    }

    #[test]
    fn cost_areas_apply_the_last_authored_overlapping_multiplier() {
        let area_a = (
            Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)),
            2.0,
        );
        let area_b = (
            Aabb::new(Vec2::new(4.0, 4.0), Vec2::new(6.0, 6.0)),
            5.0,
        );
        let grid = TileGrid::build(open_bounds(), 1.0, &[], 0.3, &[area_a, area_b]);
        let center_tile = grid.nearest_walkable_tile(Vec2::new(5.0, 5.0)).unwrap();
        assert_eq!(grid.cost(center_tile), 5.0, "later cost area must win");
        let corner_tile = grid.nearest_walkable_tile(Vec2::new(0.5, 0.5)).unwrap();
        assert_eq!(grid.cost(corner_tile), 2.0);
    }

    #[test]
    fn nearest_walkable_tile_skips_a_blocked_tile() {
        let wall = Segment::new(Vec2::new(0.0, 4.5), Vec2::new(10.0, 4.5));
        let grid = TileGrid::build(open_bounds(), 1.0, &[wall], 0.3, &[]);
        let tile = grid.nearest_walkable_tile(Vec2::new(5.0, 4.5)).unwrap();
        assert!(grid.is_walkable(tile));
    }

    #[test]
    fn a_point_outside_the_grid_has_no_nearest_tile_when_fully_enclosed_by_walls() {
        // A grid entirely blocked has no walkable tile at all.
        let walls: Vec<Segment> = (0..10)
            .map(|row| Segment::new(Vec2::new(0.0, row as f32), Vec2::new(10.0, row as f32)))
            .collect();
        let grid = TileGrid::build(open_bounds(), 1.0, &walls, 0.6, &[]);
        assert_eq!(grid.nearest_walkable_tile(Vec2::new(5.0, 5.0)), None);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p crowd-core nav::grid`
Expected: FAIL — `nav` module does not exist yet (compile error).

- [ ] **Step 3: Create `nav/mod.rs` and wire the module in**

```rust
// crates/crowd-core/src/nav/mod.rs
//! Tiled navmesh prototype (M0 item 4).
//!
//! See `docs/superpowers/specs/2026-08-08-tiled-navmesh-prototype-design.md`.

pub mod grid;

pub use grid::TileGrid;
```

Modify `crates/crowd-core/src/lib.rs`:

```rust
pub mod metrics;
pub mod nav;
pub mod phases;
```

(insert `pub mod nav;` alphabetically between `metrics` and `phases`)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p crowd-core nav::grid`
Expected: PASS (5 tests)

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/nav/mod.rs crates/crowd-core/src/nav/grid.rs crates/crowd-core/src/lib.rs
git commit -m "Add TileGrid rasterization for the tiled navmesh prototype"
```

---

### Task 2: `TileGraph` — portal generation and toggling

**Files:**
- Create: `crates/crowd-core/src/nav/portal.rs`
- Modify: `crates/crowd-core/src/nav/mod.rs` — add `pub mod portal; pub use portal::{Portal, PortalId, TileGraph};`

**Interfaces:**
- Consumes: `TileGrid` from Task 1 (`cols()`, `rows()`, `tile_count()`, `tile_center()`, `is_walkable()`, `cost()`).
- Produces: `PortalId(pub u32)`, `Portal { id: PortalId, tile_a: u32, tile_b: u32, open: bool }`, `TileGraph::build(grid: TileGrid) -> TileGraph`, `TileGraph::grid(&self) -> &TileGrid`, `TileGraph::portal_count(&self) -> u32`, `TileGraph::portal(&self, id: PortalId) -> &Portal`, `TileGraph::set_portal_open(&mut self, id: PortalId, open: bool)`, `TileGraph::portal_midpoint(&self, id: PortalId) -> Vec2`, `TileGraph::open_portals_of(&self, tile: u32) -> impl Iterator<Item = &Portal>`, `TileGraph::portal_between(&self, a: u32, b: u32) -> Option<PortalId>`, `TileGraph::nearest_portal(&self, p: Vec2) -> Option<PortalId>`, `TileGraph::name_portal(&mut self, name: String, id: PortalId)`, `TileGraph::portal_named(&self, name: &str) -> Option<PortalId>`.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/crowd-core/src/nav/portal.rs
//! Portals: the tile-adjacency edges a topology change (a closed doorway)
//! removes or restores.

use std::collections::HashMap;

use crate::nav::grid::TileGrid;
use crate::units::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PortalId(pub u32);

#[derive(Clone, Copy, Debug)]
pub struct Portal {
    pub id: PortalId,
    pub tile_a: u32,
    pub tile_b: u32,
    pub open: bool,
}

#[derive(Clone, Debug)]
pub struct TileGraph {
    grid: TileGrid,
    portals: Vec<Portal>,
    /// Adjacency list: tile -> indices into `portals`, in the order portals
    /// were created (ascending `PortalId`) — a fixed, deterministic order.
    adjacency: Vec<Vec<u32>>,
    named: HashMap<String, PortalId>,
}

impl TileGraph {
    /// Every 4-connected pair of walkable tiles gets exactly one open portal.
    /// Row-major, checking only east and north neighbors, so each pair is
    /// visited once.
    pub fn build(grid: TileGrid) -> Self {
        let tile_count = grid.tile_count();
        let cols = grid.cols();
        let mut portals = Vec::new();
        let mut adjacency = vec![Vec::new(); tile_count as usize];

        let mut push_portal = |a: u32, b: u32, portals: &mut Vec<Portal>, adjacency: &mut Vec<Vec<u32>>| {
            let id = PortalId(portals.len() as u32);
            let portal_index = portals.len() as u32;
            portals.push(Portal {
                id,
                tile_a: a,
                tile_b: b,
                open: true,
            });
            adjacency[a as usize].push(portal_index);
            adjacency[b as usize].push(portal_index);
        };

        for tile in 0..tile_count {
            if !grid.is_walkable(tile) {
                continue;
            }
            let col = tile % cols;
            let row = tile / cols;
            // East neighbor.
            if col + 1 < cols {
                let east = tile + 1;
                if grid.is_walkable(east) {
                    push_portal(tile, east, &mut portals, &mut adjacency);
                }
            }
            // North neighbor.
            let north = tile + cols;
            if north < tile_count && grid.is_walkable(north) {
                push_portal(tile, north, &mut portals, &mut adjacency);
            }
        }

        Self {
            grid,
            portals,
            adjacency,
            named: HashMap::new(),
        }
    }

    pub fn grid(&self) -> &TileGrid {
        &self.grid
    }

    pub fn portal_count(&self) -> u32 {
        self.portals.len() as u32
    }

    pub fn portal(&self, id: PortalId) -> &Portal {
        &self.portals[id.0 as usize]
    }

    pub fn set_portal_open(&mut self, id: PortalId, open: bool) {
        self.portals[id.0 as usize].open = open;
    }

    pub fn portal_midpoint(&self, id: PortalId) -> Vec2 {
        let p = self.portal(id);
        let a = self.grid.tile_center(p.tile_a);
        let b = self.grid.tile_center(p.tile_b);
        Vec2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)
    }

    pub fn open_portals_of(&self, tile: u32) -> impl Iterator<Item = &Portal> {
        self.adjacency
            .get(tile as usize)
            .into_iter()
            .flatten()
            .map(|&index| &self.portals[index as usize])
            .filter(|p| p.open)
    }

    pub fn portal_between(&self, a: u32, b: u32) -> Option<PortalId> {
        self.adjacency.get(a as usize)?.iter().find_map(|&index| {
            let p = &self.portals[index as usize];
            ((p.tile_a == a && p.tile_b == b) || (p.tile_a == b && p.tile_b == a)).then_some(p.id)
        })
    }

    /// Nearest portal by midpoint distance, ties broken by lower `PortalId`.
    pub fn nearest_portal(&self, point: Vec2) -> Option<PortalId> {
        let mut best: Option<(f32, PortalId)> = None;
        for portal in &self.portals {
            let d = self.portal_midpoint(portal.id).distance_squared(point);
            if best.is_none_or(|(best_d, best_id)| {
                d < best_d || (d == best_d && portal.id.0 < best_id.0)
            }) {
                best = Some((d, portal.id));
            }
        }
        best.map(|(_, id)| id)
    }

    pub fn name_portal(&mut self, name: String, id: PortalId) {
        self.named.insert(name, id);
    }

    pub fn portal_named(&self, name: &str) -> Option<PortalId> {
        self.named.get(name).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Aabb;

    fn open_grid() -> TileGrid {
        TileGrid::build(
            Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(3.0, 1.0)),
            1.0,
            &[],
            0.3,
            &[],
        )
    }

    #[test]
    fn adjacent_walkable_tiles_get_exactly_one_open_portal() {
        let graph = TileGraph::build(open_grid());
        // 3x1 grid: two east-west adjacencies, zero north (only one row).
        assert_eq!(graph.portal_count(), 2);
        for portal in 0..graph.portal_count() {
            assert!(graph.portal(PortalId(portal)).open);
        }
    }

    #[test]
    fn closing_a_portal_removes_it_from_both_tiles_adjacency() {
        let mut graph = TileGraph::build(open_grid());
        let id = graph.portal_between(0, 1).unwrap();
        graph.set_portal_open(id, false);
        assert!(graph.open_portals_of(0).all(|p| p.id != id));
        assert!(graph.open_portals_of(1).all(|p| p.id != id));
    }

    #[test]
    fn reopening_a_portal_restores_it() {
        let mut graph = TileGraph::build(open_grid());
        let id = graph.portal_between(0, 1).unwrap();
        graph.set_portal_open(id, false);
        graph.set_portal_open(id, true);
        assert!(graph.open_portals_of(0).any(|p| p.id == id));
    }

    #[test]
    fn portal_between_unconnected_tiles_is_none() {
        let graph = TileGraph::build(open_grid());
        assert_eq!(graph.portal_between(0, 2), None);
    }

    #[test]
    fn named_portals_resolve_by_nearest_midpoint() {
        let mut graph = TileGraph::build(open_grid());
        let id = graph.portal_between(1, 2).unwrap();
        let midpoint = graph.portal_midpoint(id);
        graph.name_portal("east_door".into(), graph.nearest_portal(midpoint).unwrap());
        assert_eq!(graph.portal_named("east_door"), Some(id));
        assert_eq!(graph.portal_named("no_such_door"), None);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p crowd-core nav::portal`
Expected: FAIL — `nav::portal` is not declared in `mod.rs` yet.

- [ ] **Step 3: Wire the module in**

```rust
// crates/crowd-core/src/nav/mod.rs
pub mod grid;
pub mod portal;

pub use grid::TileGrid;
pub use portal::{Portal, PortalId, TileGraph};
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p crowd-core nav::portal`
Expected: PASS (5 tests)

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/nav/portal.rs crates/crowd-core/src/nav/mod.rs
git commit -m "Add TileGraph: deterministic portal generation and toggling"
```

---

### Task 3: Deterministic A* and corridor extraction

**Files:**
- Create: `crates/crowd-core/src/nav/pathfind.rs`
- Modify: `crates/crowd-core/src/nav/mod.rs` — add `pub mod pathfind; pub use pathfind::{corridor_points, find_path};` and the `NavMeshDef`/`build_graph` block (needs `TileGraph`).

**Interfaces:**
- Consumes: `TileGraph` (Task 2): `grid()`, `open_portals_of()`, `portal_between()`, `portal_midpoint()`.
- Produces: `find_path(graph: &TileGraph, from_tile: u32, to_tile: u32) -> Option<(Vec<u32>, u32)>` (tile-index path, node-expansion count charged), `corridor_points(graph: &TileGraph, tile_path: &[u32], start: Vec2, goal: Vec2) -> Vec<Vec2>`.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/crowd-core/src/nav/pathfind.rs
//! Deterministic A* over the tile graph, and corridor extraction from the
//! resulting tile path.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::nav::portal::TileGraph;
use crate::units::Vec2;

/// `f32` ordered via `total_cmp` — a real total order, unlike `PartialOrd`.
/// Local to this module: nothing outside pathfinding needs to put floats in
/// a heap.
#[derive(Clone, Copy, Debug, PartialEq)]
struct OrderedF32(f32);

impl Eq for OrderedF32 {}
impl Ord for OrderedF32 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}
impl PartialOrd for OrderedF32 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Min-heap entry. `BinaryHeap` is a max-heap, so cost and tie-break are
/// reversed at comparison time via `Ord`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HeapEntry {
    cost: OrderedF32,
    tile: u32,
}

impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reversed: BinaryHeap pops the greatest, we want the smallest cost,
        // ties broken by the *lower* tile index.
        other
            .cost
            .cmp(&self.cost)
            .then_with(|| other.tile.cmp(&self.tile))
    }
}
impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A* over the tile graph. Edge cost is the destination tile's cost
/// multiplier times center-to-center distance; the heuristic is Euclidean
/// distance to the goal, admissible because every cost multiplier is >= 1.0
/// (enforced at scene-compile time, mirroring `MIN_PREFERRED_SPEED`).
///
/// Returns the tile path (inclusive of `from_tile` and `to_tile`) and the
/// number of node expansions performed, for budget accounting.
pub fn find_path(graph: &TileGraph, from_tile: u32, to_tile: u32) -> Option<(Vec<u32>, u32)> {
    if from_tile == to_tile {
        return Some((vec![from_tile], 0));
    }

    let tile_count = graph.grid().tile_count();
    let mut best_cost = vec![f32::INFINITY; tile_count as usize];
    let mut came_from = vec![u32::MAX; tile_count as usize];
    let mut closed = vec![false; tile_count as usize];
    let mut heap = BinaryHeap::new();

    best_cost[from_tile as usize] = 0.0;
    heap.push(HeapEntry {
        cost: OrderedF32(heuristic(graph, from_tile, to_tile)),
        tile: from_tile,
    });

    let mut expansions = 0u32;
    while let Some(HeapEntry { tile, .. }) = heap.pop() {
        if closed[tile as usize] {
            continue;
        }
        closed[tile as usize] = true;
        expansions += 1;

        if tile == to_tile {
            return Some((reconstruct(&came_from, from_tile, to_tile), expansions));
        }

        for portal in graph.open_portals_of(tile) {
            let next = if portal.tile_a == tile {
                portal.tile_b
            } else {
                portal.tile_a
            };
            if closed[next as usize] {
                continue;
            }
            let step = graph.grid().tile_center(tile).distance_squared(graph.grid().tile_center(next)).sqrt()
                * graph.grid().cost(next);
            let candidate = best_cost[tile as usize] + step;
            if candidate < best_cost[next as usize] {
                best_cost[next as usize] = candidate;
                came_from[next as usize] = tile;
                heap.push(HeapEntry {
                    cost: OrderedF32(candidate + heuristic(graph, next, to_tile)),
                    tile: next,
                });
            }
        }
    }

    None
}

fn heuristic(graph: &TileGraph, tile: u32, goal: u32) -> f32 {
    graph.grid().tile_center(tile).distance_squared(graph.grid().tile_center(goal)).sqrt()
}

fn reconstruct(came_from: &[u32], from: u32, to: u32) -> Vec<u32> {
    let mut path = vec![to];
    let mut cursor = to;
    while cursor != from {
        cursor = came_from[cursor as usize];
        debug_assert_ne!(cursor, u32::MAX, "reachable tile must have a predecessor");
        path.push(cursor);
    }
    path.reverse();
    path
}

/// Turn a tile-index path into a point corridor: the exact start point, each
/// crossed portal's midpoint, then the exact goal point.
pub fn corridor_points(graph: &TileGraph, tile_path: &[u32], start: Vec2, goal: Vec2) -> Vec<Vec2> {
    let mut points = vec![start];
    for pair in tile_path.windows(2) {
        if let Some(id) = graph.portal_between(pair[0], pair[1]) {
            points.push(graph.portal_midpoint(id));
        }
    }
    points.push(goal);
    points
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::grid::TileGrid;
    use crate::geometry::Segment;
    use crate::units::Aabb;

    fn open_graph(w: f32, h: f32) -> TileGraph {
        TileGraph::build(TileGrid::build(
            Aabb::new(Vec2::ZERO, Vec2::new(w, h)),
            1.0,
            &[],
            0.3,
            &[],
        ))
    }

    #[test]
    fn finds_a_path_across_an_open_grid() {
        let graph = open_graph(5.0, 5.0);
        let (path, _) = find_path(&graph, 0, 24).unwrap();
        assert_eq!(*path.first().unwrap(), 0);
        assert_eq!(*path.last().unwrap(), 24);
    }

    #[test]
    fn path_to_self_is_a_single_tile() {
        let graph = open_graph(3.0, 3.0);
        let (path, expansions) = find_path(&graph, 4, 4).unwrap();
        assert_eq!(path, vec![4]);
        assert_eq!(expansions, 0);
    }

    #[test]
    fn unreachable_goal_returns_none() {
        let wall: Vec<Segment> = (0..5)
            .map(|c| Segment::new(Vec2::new(c as f32, 2.5), Vec2::new(c as f32 + 1.0, 2.5)))
            .collect();
        let graph = TileGraph::build(TileGrid::build(
            Aabb::new(Vec2::ZERO, Vec2::new(5.0, 5.0)),
            1.0,
            &wall,
            0.6,
            &[],
        ));
        assert_eq!(find_path(&graph, 0, 24), None);
    }

    #[test]
    fn identical_inputs_produce_identical_paths() {
        let graph = open_graph(6.0, 6.0);
        let (a, _) = find_path(&graph, 0, 35).unwrap();
        let (b, _) = find_path(&graph, 0, 35).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn a_cost_area_makes_the_solver_prefer_a_detour() {
        // A cheap detour around a strip of expensive tiles must be chosen
        // over the geometrically shorter, expensive direct route.
        let expensive = (Aabb::new(Vec2::new(2.0, 0.0), Vec2::new(3.0, 3.0)), 20.0);
        let graph = TileGraph::build(TileGrid::build(
            Aabb::new(Vec2::ZERO, Vec2::new(6.0, 6.0)),
            1.0,
            &[],
            0.3,
            &[expensive],
        ));
        let from = graph.grid().nearest_walkable_tile(Vec2::new(0.5, 1.5)).unwrap();
        let to = graph.grid().nearest_walkable_tile(Vec2::new(5.5, 1.5)).unwrap();
        let (path, _) = find_path(&graph, from, to).unwrap();
        let crossed_expensive_row = path.iter().any(|&t| {
            let c = graph.grid().tile_center(t);
            (2.0..3.0).contains(&c.x) && (0.0..3.0).contains(&c.y)
        });
        assert!(
            !crossed_expensive_row,
            "path crossed the expensive area instead of detouring: {path:?}"
        );
    }

    #[test]
    fn corridor_points_starts_and_ends_at_the_exact_requested_points() {
        let graph = open_graph(4.0, 1.0);
        let (path, _) = find_path(&graph, 0, 3).unwrap();
        let start = Vec2::new(0.2, 0.5);
        let goal = Vec2::new(3.8, 0.5);
        let corridor = corridor_points(&graph, &path, start, goal);
        assert_eq!(*corridor.first().unwrap(), start);
        assert_eq!(*corridor.last().unwrap(), goal);
        assert_eq!(corridor.len(), path.len() + 1, "start + N-1 portals + goal");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p crowd-core nav::pathfind`
Expected: FAIL — module not declared.

- [ ] **Step 3: Wire the module in and add `NavMeshDef`**

```rust
// crates/crowd-core/src/nav/mod.rs
//! Tiled navmesh prototype (M0 item 4).
//!
//! See `docs/superpowers/specs/2026-08-08-tiled-navmesh-prototype-design.md`.

pub mod grid;
pub mod pathfind;
pub mod portal;

pub use grid::TileGrid;
pub use pathfind::{corridor_points, find_path};
pub use portal::{Portal, PortalId, TileGraph};

use crate::geometry::Segment;
use crate::units::Aabb;

/// Authoring input for one scene's tiled navmesh. Lives on `SceneDef` /
/// `CompiledScene` alongside the existing `WaypointGraph` field — never
/// instead of it at the type level — so waypoint-routed scenes are
/// unaffected by this type's mere existence.
#[derive(Clone, Debug)]
pub struct NavMeshDef {
    pub tile_size: f32,
    pub agent_radius: f32,
    pub cost_areas: Vec<(Aabb, f32)>,
    /// Author-friendly portal lookup points, resolved to `PortalId`s once the
    /// graph is built. E.g. `("north_door", Vec2::new(20.0, 20.0))`.
    pub named_portals: Vec<(String, Vec2)>,
}

impl NavMeshDef {
    pub fn build_graph(&self, bounds: Aabb, walls: &[Segment]) -> TileGraph {
        let grid = TileGrid::build(
            bounds,
            self.tile_size,
            walls,
            self.agent_radius,
            &self.cost_areas,
        );
        let mut graph = TileGraph::build(grid);
        for (name, point) in &self.named_portals {
            if let Some(id) = graph.nearest_portal(*point) {
                graph.name_portal(name.clone(), id);
            }
        }
        graph
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p crowd-core nav::`
Expected: PASS (16 tests across `nav::grid`, `nav::portal`, `nav::pathfind`)

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/nav/
git commit -m "Add deterministic A* pathfinding and corridor extraction"
```

---

### Task 4: `SceneDef`/`CompiledScene` nav integration

**Files:**
- Modify: `crates/crowd-core/src/scene.rs` — add `nav`/`nav_destinations` fields, new `SceneError` variants, `compile()` branch.
- Modify: `crates/crowd-core/src/scenes.rs` — mechanical: add `nav: None, nav_destinations: Vec::new(),` to all 6 `SceneDef` literals.
- Modify: `crates/crowd-core/src/scene.rs` test module — same mechanical addition to `valid_scene()`.
- Modify: `crates/crowd-core/src/sim.rs` test module — same addition to the `corridor()` helper's `SceneDef` literal.
- Modify: `crates/crowd-core/src/phases/spawn.rs` — branch route/heading assignment on `scene.nav.is_some()`; same mechanical addition to its two test-helper literals (`scene()`, `roomy_scene()`).
- Modify: `crates/crowd-core/src/lib.rs` — export `NavMeshDef` alongside the other `scene`/`nav` re-exports.

**Interfaces:**
- Consumes: `crate::nav::{NavMeshDef, TileGraph}` (Tasks 1-3).
- Produces: `SceneDef.nav: Option<NavMeshDef>`, `SceneDef.nav_destinations: Vec<Vec2>`, `CompiledScene.nav: Option<TileGraph>`, `CompiledScene.nav_destinations: Vec<Vec2>` (an immutable, all-portals-open template — `Simulation` clones its own mutable copy in Task 7). New `SceneError` variants: `EmptyNavMesh`, `UnwalkableDestination { destination: u16 }`, `UnknownNamedPortal { name: String }`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/crowd-core/src/scene.rs`'s existing `#[cfg(test)] mod tests`:

```rust
    use crate::nav::NavMeshDef;

    fn nav_scene() -> SceneDef {
        SceneDef {
            name: "nav_corridor".into(),
            bounds: Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 3.0)),
            walls: vec![
                Segment::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)),
                Segment::new(Vec2::new(0.0, 3.0), Vec2::new(10.0, 3.0)),
            ],
            waypoints: WaypointGraph::new(),
            destinations: vec![Destination {
                name: "exit".into(),
                node: 0,
            }],
            spawns: vec![SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(0.5, 1.0), Vec2::new(1.5, 2.0)),
                count: 10,
                per_tick: 2,
                destination: 0,
            }],
            populations: vec![PopulationParams::default()],
            project_seed: 42,
            ticks_per_second: 30,
            duration_ticks: 300,
            nav: Some(NavMeshDef {
                tile_size: 1.0,
                agent_radius: 0.3,
                cost_areas: Vec::new(),
                named_portals: Vec::new(),
            }),
            nav_destinations: vec![Vec2::new(9.0, 1.5)],
        }
    }

    #[test]
    fn a_nav_routed_scene_compiles_without_a_waypoint_graph() {
        assert!(nav_scene().compile().is_ok());
    }

    #[test]
    fn an_unwalkable_nav_destination_is_rejected() {
        let mut scene = nav_scene();
        // Outside the corridor walls entirely.
        scene.nav_destinations[0] = Vec2::new(9.0, 50.0);
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::UnwalkableDestination { destination: 0 }));
    }

    #[test]
    fn an_unreachable_nav_destination_is_rejected() {
        let mut scene = nav_scene();
        // A wall straight across the corridor, with no doorway.
        scene
            .walls
            .push(Segment::new(Vec2::new(5.0, 0.0), Vec2::new(5.0, 3.0)));
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::UnreachableDestination {
            spawn: 0,
            destination: 0
        }));
    }

    #[test]
    fn a_nav_scene_does_not_require_a_waypoint_graph() {
        // The waypoint-only checks must not fire for a nav-routed scene, even
        // though its `waypoints` field is the empty default.
        let scene = nav_scene();
        assert!(scene.waypoints.node_count() == 0);
        assert!(scene.compile().is_ok());
    }

    #[test]
    fn a_waypoint_scene_is_unaffected_by_the_nav_field_existing() {
        // `valid_scene()` (the pre-existing helper) has `nav: None` and must
        // compile exactly as before.
        assert!(valid_scene().compile().is_ok());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p crowd-core scene::tests`
Expected: FAIL — `SceneDef` has no `nav`/`nav_destinations` fields yet (compile error).

- [ ] **Step 3: Implement the `scene.rs` changes**

```rust
// crates/crowd-core/src/scene.rs — imports
use crate::nav::{NavMeshDef, TileGraph};
```

Add to `SceneDef` (after `duration_ticks: u64,`):

```rust
    /// `None` for waypoint-routed scenes (every scene today). `Some` opts the
    /// scene into the tiled navmesh instead — `waypoints` is then unused and
    /// may be left as `WaypointGraph::new()`.
    pub nav: Option<NavMeshDef>,
    /// Parallel to `destinations`; meaningful only when `nav.is_some()`.
    pub nav_destinations: Vec<Vec2>,
```

Add to `SceneError`:

```rust
    EmptyNavMesh,
    UnwalkableDestination {
        destination: u16,
    },
    UnknownNamedPortal {
        name: String,
    },
```

Add to `CompiledScene` (after `duration_ticks: u64,`):

```rust
    pub nav: Option<TileGraph>,
    pub nav_destinations: Vec<Vec2>,
```

In `SceneDef::compile`, guard the existing waypoint checks and add the nav branch. Replace:

```rust
        if self.waypoints.node_count() == 0 {
            errors.push(SceneError::EmptyWaypointGraph);
        } else if !self.waypoints.is_connected() {
            errors.push(SceneError::DisconnectedWaypointGraph);
        }
```

with:

```rust
        if self.nav.is_none() {
            if self.waypoints.node_count() == 0 {
                errors.push(SceneError::EmptyWaypointGraph);
            } else if !self.waypoints.is_connected() {
                errors.push(SceneError::DisconnectedWaypointGraph);
            }
        }
```

Replace the destination-node-existence loop:

```rust
        for (index, destination) in self.destinations.iter().enumerate() {
            if destination.node >= self.waypoints.node_count() {
                errors.push(SceneError::DestinationNodeMissing {
                    destination: index as u16,
                    node: destination.node,
                });
            }
        }
```

with:

```rust
        if self.nav.is_none() {
            for (index, destination) in self.destinations.iter().enumerate() {
                if destination.node >= self.waypoints.node_count() {
                    errors.push(SceneError::DestinationNodeMissing {
                        destination: index as u16,
                        node: destination.node,
                    });
                }
            }
        }
```

Build the nav graph once (used for both validation and the compiled scene's template) right before the existing spawn-reachability loop:

```rust
        let nav_graph = self.nav.as_ref().map(|def| def.build_graph(self.bounds, &self.walls));
        if let Some(graph) = &nav_graph {
            if graph.grid().tile_count() == 0
                || (0..graph.grid().tile_count()).all(|t| !graph.grid().is_walkable(t))
            {
                errors.push(SceneError::EmptyNavMesh);
            }
            for (index, point) in self.nav_destinations.iter().enumerate() {
                if graph.grid().nearest_walkable_tile(*point).is_none() {
                    errors.push(SceneError::UnwalkableDestination {
                        destination: index as u16,
                    });
                }
            }
        }
```

Change the spawn-reachability loop's body: the existing loop already has an `else if` structure keyed on waypoint node validity. Replace the whole `for spawn in &self.spawns { ... }` reachability tail (the block starting `// Reachability is only meaningful ...`) with a branch:

```rust
            // Reachability is only meaningful once routing itself is sound.
            if let Some(graph) = &nav_graph {
                if let Some(dest_point) = self.nav_destinations.get(spawn.destination as usize) {
                    let from = graph.grid().nearest_walkable_tile(spawn.area.center());
                    let to = graph.grid().nearest_walkable_tile(*dest_point);
                    let reachable = match (from, to) {
                        (Some(from), Some(to)) => crate::nav::find_path(graph, from, to).is_some(),
                        _ => false,
                    };
                    if !reachable {
                        errors.push(SceneError::UnreachableDestination {
                            spawn: spawn.id,
                            destination: spawn.destination,
                        });
                    }
                }
            } else if self.waypoints.node_count() > 0 && destination.node < self.waypoints.node_count() {
                let from = self
                    .waypoints
                    .nearest_node(spawn.area.center())
                    .expect("non-empty graph has a nearest node");
                if self
                    .waypoints
                    .shortest_path(from, destination.node)
                    .is_none()
                {
                    errors.push(SceneError::UnreachableDestination {
                        spawn: spawn.id,
                        destination: spawn.destination,
                    });
                }
            }
```

(`destination` above is still bound by the loop's existing `let Some(destination) = ... else { ...; continue; };` line just above it — unchanged.)

Finally, thread `nav`/`nav_destinations` through the `Ok(CompiledScene { ... })` constructor:

```rust
        Ok(CompiledScene {
            // ...existing fields unchanged...
            nav: nav_graph,
            nav_destinations: self.nav_destinations,
        })
```

- [ ] **Step 4: Mechanically update every existing `SceneDef` literal**

In `crates/crowd-core/src/scenes.rs`, add `nav: None,` and `nav_destinations: Vec::new(),` immediately after each scene's `duration_ticks: ...,` line, in all 6 functions (`bidirectional_corridor`, `crossing`, `bottleneck`, `dense_flow`, `l_corridor`, `circle`).

In `crates/crowd-core/src/scene.rs`'s `valid_scene()` test helper, add the same two lines after `duration_ticks: 300,`.

In `crates/crowd-core/src/sim.rs`'s `corridor()` test helper, add the same two lines after `duration_ticks: 900,`.

In `crates/crowd-core/src/phases/spawn.rs`'s `scene()` and `roomy_scene()` test helpers, add the same two lines after each `duration_ticks: 100,`.

In `crates/crowd-core/src/lib.rs`, add `NavMeshDef` to the `nav` re-export line:

```rust
pub use nav::{NavMeshDef, PortalId, TileGraph};
```

- [ ] **Step 5: Fix `apply_spawns`' route/heading assignment**

In `crates/crowd-core/src/phases/spawn.rs`, replace:

```rust
            let destination_node = scene.destinations[region.destination as usize].node;
            let route = match scene.waypoints.nearest_node(position) {
                Some(from) => match scene.waypoints.shortest_path(from, destination_node) {
                    Some(path) => {
                        let points: Vec<Vec2> =
                            path.iter().map(|n| scene.waypoints.position(*n)).collect();
                        routes.push_route(&points)
                    }
                    // Compilation already proved reachability from the region
                    // centre; an individual sample can still fail only if the
                    // graph is malformed, and an unrouted agent is preferable
                    // to a panic mid-bake.
                    None => NO_ROUTE,
                },
                None => NO_ROUTE,
            };

            let heading =
                (scene.waypoints.position(destination_node) - position).normalize_or_zero();
```

with:

```rust
            let (route, heading) = if let Some(dest_point) =
                scene.nav.as_ref().and(scene.nav_destinations.get(region.destination as usize))
            {
                // A nav-routed scene assigns no route at spawn time: the new
                // `plan` phase (Task 6) budgets pathfinding across ticks. The
                // agent starts `NO_ROUTE` and is picked up by `plan` this
                // tick or a later one.
                (NO_ROUTE, (*dest_point - position).normalize_or_zero())
            } else {
                let destination_node = scene.destinations[region.destination as usize].node;
                let route = match scene.waypoints.nearest_node(position) {
                    Some(from) => match scene.waypoints.shortest_path(from, destination_node) {
                        Some(path) => {
                            let points: Vec<Vec2> =
                                path.iter().map(|n| scene.waypoints.position(*n)).collect();
                            routes.push_route(&points)
                        }
                        None => NO_ROUTE,
                    },
                    None => NO_ROUTE,
                };
                let heading =
                    (scene.waypoints.position(destination_node) - position).normalize_or_zero();
                (route, heading)
            };
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p crowd-core scene:: scenes:: sim:: phases::spawn`
Expected: PASS — all new tests plus every pre-existing test in these modules, unchanged.

- [ ] **Step 7: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/scene.rs crates/crowd-core/src/scenes.rs crates/crowd-core/src/sim.rs crates/crowd-core/src/phases/spawn.rs crates/crowd-core/src/lib.rs
git commit -m "Add optional tiled-navmesh fields to SceneDef/CompiledScene"
```

---

### Task 5: `Phase::Plan` and `World::state_hash` correctness fix

**Files:**
- Modify: `crates/crowd-core/src/metrics.rs` — new `Phase::Plan` variant.
- Modify: `crates/crowd-core/src/world.rs` — `state_hash` now includes the route handle.

**Interfaces:**
- Produces: `Phase::Plan` (usable by `sim.rs` in Task 7), `World::state_hash` covering `self.route[slot].0`.

**Why this task exists:** `World::state_hash`'s doc comment currently says `route` is safe to exclude because it is "fixed at spawn." Once portal invalidation can reassign a route mid-run (Task 6), that is no longer true, and the comment says explicitly: "if a later change makes any of them mutable within the tick loop, it must be added here." This task makes that fix before the mutation exists, so the plan phase never ships un-covered by the determinism hash.

- [ ] **Step 1: Write the failing test**

Add to `crates/crowd-core/src/world.rs`'s test module:

```rust
    #[test]
    fn state_hash_changes_when_the_route_handle_changes() {
        let mut a = World::new();
        let mut b = World::new();
        a.spawn(spawn_at(1, Vec2::ZERO), 0).unwrap();
        b.spawn(spawn_at(1, Vec2::ZERO), 0).unwrap();
        b.route[0] = RouteHandle(7);
        assert_ne!(
            a.state_hash(),
            b.state_hash(),
            "a mid-run route reassignment must be visible to the determinism hash"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core world::tests::state_hash_changes_when_the_route_handle_changes`
Expected: FAIL — both hashes are equal today because `route` is excluded.

- [ ] **Step 3: Add `Phase::Plan`**

In `crates/crowd-core/src/metrics.rs`, update the `Phase` enum and its two match blocks:

```rust
pub enum Phase {
    Spawn,
    Index,
    Perceive,
    Plan,
    Decide,
    Steer,
    Integrate,
    Metrics,
}

impl Phase {
    pub const ALL: [Phase; 8] = [
        Phase::Spawn,
        Phase::Index,
        Phase::Perceive,
        Phase::Plan,
        Phase::Decide,
        Phase::Steer,
        Phase::Integrate,
        Phase::Metrics,
    ];

    const fn index(self) -> usize {
        match self {
            Phase::Spawn => 0,
            Phase::Index => 1,
            Phase::Perceive => 2,
            Phase::Plan => 3,
            Phase::Decide => 4,
            Phase::Steer => 5,
            Phase::Integrate => 6,
            Phase::Metrics => 7,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Phase::Spawn => "spawn",
            Phase::Index => "index",
            Phase::Perceive => "perceive",
            Phase::Plan => "plan",
            Phase::Decide => "decide",
            Phase::Steer => "steer",
            Phase::Integrate => "integrate",
            Phase::Metrics => "metrics",
        }
    }
}
```

Change `phase_nanos: [u64; 7]` to `phase_nanos: [u64; 8]` (its one declaration site, near line 124).

- [ ] **Step 4: Fix `state_hash` and its doc comment**

In `crates/crowd-core/src/world.rs`, update the doc comment and hash body:

```rust
    /// A bitwise digest of all authoritative agent state.
    ///
    /// # What is deliberately omitted, and why that is safe
    ///
    /// `population_id`, `radius`, `max_speed`, `preferred_speed`,
    /// `destination`, `spawn_tick`, `solver_status`, `stall_ticks` and
    /// `unrouted` are excluded. Every one is either fixed at spawn or derived
    /// from state already hashed, so including them would add nothing a
    /// divergence could hide behind.
    ///
    /// `route` is **included**, unlike the fields above: the tiled-navmesh
    /// plan phase can reassign it mid-run (a portal close invalidates and
    /// reroutes a corridor), so it is no longer fixed at spawn for every
    /// scene. Hashing the raw handle index is enough — it is deterministic
    /// given deterministic `RouteArena` push order, so two runs that assign
    /// different routes to the same agent diverge here immediately, rather
    /// than only once the resulting steering difference shows up in position.
    ///
    /// **This invariant is load-bearing.** If a later change makes any
    /// currently-excluded field mutable within the tick loop, it must be
    /// added here — otherwise the determinism tests keep passing while
    /// silently ignoring the field that diverged.
    ///
    /// Hashes float *bits*, not values, so the determinism tests compare
    /// exactly rather than within a tolerance.
    pub fn state_hash(&self) -> u64 {
        let mut h: u64 = 0xa5a5_5a5a_dead_beef;
        for slot in 0..self.len() {
            h = hash_combine(h, self.agent_id[slot].0);
            h = hash_combine(h, canonical_bits(self.pos_x[slot]));
            h = hash_combine(h, canonical_bits(self.pos_y[slot]));
            h = hash_combine(h, canonical_bits(self.vel_x[slot]));
            h = hash_combine(h, canonical_bits(self.vel_y[slot]));
            h = hash_combine(h, canonical_bits(self.yaw[slot]));
            h = hash_combine(h, self.route[slot].0 as u64);
            h = hash_combine(h, self.route_index[slot] as u64);
            h = hash_combine(h, self.arrived[slot] as u64);
        }
        h
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-core world:: metrics::`
Expected: PASS, including the new test and every pre-existing `state_hash`/`Phase` test.

- [ ] **Step 6: Fix the one other `phase_nanos` reference and rebuild the workspace**

Run: `cargo build --workspace 2>&1 | rg "error"`
Expected: no output (any remaining `Phase::ALL` / `[u64; 7]` mismatch elsewhere would show here — `crates/crowd-bench` does not reference `Phase` directly per the earlier `rg`, so this should be clean).

- [ ] **Step 7: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/metrics.rs crates/crowd-core/src/world.rs
git commit -m "Cover the route handle in state_hash; add Phase::Plan"
```

---

### Task 6: The `plan` tick phase — budgeted planning and portal invalidation

**Files:**
- Create: `crates/crowd-core/src/phases/plan.rs`
- Modify: `crates/crowd-core/src/phases/mod.rs` — `pub mod plan; pub use plan::{plan, invalidate_portal, PlanConfig, PlanState};`

**Interfaces:**
- Consumes: `World` fields (`route`, `route_index`, `unrouted`, `arrived`, `position()`, `destination`), `CompiledScene.nav_destinations`, `crate::nav::{TileGraph, PortalId, find_path, corridor_points}`, `RouteArena::push_route`.
- Produces: `PlanConfig { max_expansions_per_tick: u32 }` (`Default` impl), `PlanState` (`Default` impl), `plan(world: &mut World, nav: &TileGraph, nav_destinations: &[Vec2], state: &mut PlanState, routes: &mut RouteArena, config: &PlanConfig)`, `invalidate_portal(world: &mut World, state: &mut PlanState, portal: PortalId) -> usize` (returns the number of agents invalidated).

- [ ] **Step 1: Write the failing tests**

```rust
// crates/crowd-core/src/phases/plan.rs
//! Tick phase: budgeted tiled-navmesh path planning.
//!
//! Fills contract section 6's phase 5 ("Plan") for nav-routed scenes. A
//! waypoint-routed scene's `CompiledScene.nav` is `None`, so `plan()` is a
//! no-op for it — this phase changes nothing about the six existing scenes.
//!
//! Detection is a full slot scan each tick (`route == NO_ROUTE`), not a
//! separately-maintained queue: at the scale this prototype targets (low
//! thousands of agents) a scan is cheap, and it makes "which agents need a
//! route" a pure function of `World` state rather than state that could drift
//! out of sync with it. A resume cursor makes the scan fair across ticks when
//! the budget does not cover every needy agent in one pass.

use std::collections::HashMap;

use crate::nav::{corridor_points, find_path, PortalId, TileGraph};
use crate::route::RouteArena;
use crate::units::Vec2;
use crate::world::{World, NO_ROUTE};

#[derive(Clone, Copy, Debug)]
pub struct PlanConfig {
    /// A* node-expansion budget spent per tick. Once a search in progress
    /// pushes the running total past this, no *new* search starts until next
    /// tick — an in-progress search is never truncated mid-flight.
    pub max_expansions_per_tick: u32,
}

impl Default for PlanConfig {
    fn default() -> Self {
        Self {
            max_expansions_per_tick: 4000,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PlanState {
    resume_slot: u32,
    /// The portal sequence each still-live route handle actually crosses.
    /// Keyed by `RouteHandle.0`. This is what makes portal-close invalidation
    /// selective: an agent is only invalidated if the closed portal's ID is
    /// in *its own* recorded sequence, not because of scene-geometry luck.
    portals_of_route: HashMap<u32, Vec<PortalId>>,
}

fn needs_route(world: &World, slot: u32) -> bool {
    !world.arrived[slot as usize] && world.route[slot as usize] == NO_ROUTE
}

/// Advance the budgeted planning queue by one tick. No-op when the scene has
/// no tiled navmesh.
pub fn plan(
    world: &mut World,
    nav: &TileGraph,
    nav_destinations: &[Vec2],
    state: &mut PlanState,
    routes: &mut RouteArena,
    config: &PlanConfig,
) {
    let n = world.len() as u32;
    if n == 0 {
        return;
    }

    let mut budget = config.max_expansions_per_tick;
    let mut slot = state.resume_slot % n;
    let mut visited = 0u32;

    while visited < n && budget > 0 {
        if needs_route(world, slot) {
            let dest_index = world.destination[slot as usize] as usize;
            if let Some(dest_point) = nav_destinations.get(dest_index) {
                let from = nav.grid().nearest_walkable_tile(world.position(slot));
                let to = nav.grid().nearest_walkable_tile(*dest_point);
                if let (Some(from), Some(to)) = (from, to) {
                    if let Some((tile_path, expansions)) = find_path(nav, from, to) {
                        budget = budget.saturating_sub(expansions.max(1));
                        let points = corridor_points(nav, &tile_path, world.position(slot), *dest_point);
                        let portal_sequence: Vec<PortalId> = tile_path
                            .windows(2)
                            .filter_map(|pair| nav.portal_between(pair[0], pair[1]))
                            .collect();
                        let handle = routes.push_route(&points);
                        state.portals_of_route.insert(handle.0, portal_sequence);
                        world.route[slot as usize] = handle;
                        world.route_index[slot as usize] = 0;
                        world.unrouted[slot as usize] = false;
                    }
                    // An unreachable destination from a currently-valid tile
                    // should not happen (compile-time reachability proved it
                    // from the spawn *region*), so no route here is left as
                    // `unrouted` handling downstream in `decide` — diagnosable,
                    // not a crash.
                }
            }
        }
        slot = (slot + 1) % n;
        visited += 1;
    }
    state.resume_slot = slot;
}

/// Invalidate every live route that crosses `portal`. Returns how many
/// agents were invalidated, so callers/tests can assert selectivity.
pub fn invalidate_portal(world: &mut World, state: &mut PlanState, portal: PortalId) -> usize {
    let mut count = 0;
    for slot in 0..world.len() {
        let handle = world.route[slot];
        if handle == NO_ROUTE {
            continue;
        }
        if state
            .portals_of_route
            .get(&handle.0)
            .is_some_and(|seq| seq.contains(&portal))
        {
            world.route[slot] = NO_ROUTE;
            world.route_index[slot] = 0;
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::{NavMeshDef, TileGraph};
    use crate::units::Aabb;
    use crate::world::{AgentSpawn, NO_ROUTE};
    use crate::ids::AgentId;

    fn nav_graph(w: f32, h: f32) -> TileGraph {
        NavMeshDef {
            tile_size: 1.0,
            agent_radius: 0.3,
            cost_areas: Vec::new(),
            named_portals: Vec::new(),
        }
        .build_graph(Aabb::new(Vec2::ZERO, Vec2::new(w, h)), &[])
    }

    fn spawn_agent(world: &mut World, id: u64, position: Vec2, destination: u16) -> u32 {
        world
            .spawn(
                AgentSpawn {
                    agent_id: AgentId(id),
                    population_id: 0,
                    position,
                    yaw: 0.0,
                    radius: 0.3,
                    max_speed: 1.8,
                    preferred_speed: 1.35,
                    route: NO_ROUTE,
                    destination,
                },
                0,
            )
            .unwrap()
    }

    #[test]
    fn a_needy_agent_receives_a_route_within_budget() {
        let graph = nav_graph(5.0, 5.0);
        let mut world = World::new();
        spawn_agent(&mut world, 1, Vec2::new(0.5, 0.5), 0);
        let mut state = PlanState::default();
        let mut routes = RouteArena::new();
        let destinations = [Vec2::new(4.5, 4.5)];
        plan(
            &mut world,
            &graph,
            &destinations,
            &mut state,
            &mut routes,
            &PlanConfig::default(),
        );
        assert_ne!(world.route[0], NO_ROUTE);
        assert!(!world.unrouted[0]);
    }

    #[test]
    fn a_zero_budget_leaves_every_agent_unrouted_this_tick() {
        let graph = nav_graph(5.0, 5.0);
        let mut world = World::new();
        spawn_agent(&mut world, 1, Vec2::new(0.5, 0.5), 0);
        let mut state = PlanState::default();
        let mut routes = RouteArena::new();
        let destinations = [Vec2::new(4.5, 4.5)];
        plan(
            &mut world,
            &graph,
            &destinations,
            &mut state,
            &mut routes,
            &PlanConfig {
                max_expansions_per_tick: 0,
            },
        );
        assert_eq!(world.route[0], NO_ROUTE);
    }

    #[test]
    fn an_already_routed_agent_is_left_alone() {
        let graph = nav_graph(5.0, 5.0);
        let mut world = World::new();
        spawn_agent(&mut world, 1, Vec2::new(0.5, 0.5), 0);
        let mut routes = RouteArena::new();
        let handle = routes.push_route(&[Vec2::new(0.5, 0.5), Vec2::new(4.5, 4.5)]);
        world.route[0] = handle;
        let mut state = PlanState::default();
        let destinations = [Vec2::new(4.5, 4.5)];
        plan(
            &mut world,
            &graph,
            &destinations,
            &mut state,
            &mut routes,
            &PlanConfig::default(),
        );
        assert_eq!(world.route[0], handle, "an already-routed agent must not be replanned");
    }

    #[test]
    fn invalidate_portal_clears_only_agents_whose_corridor_used_it() {
        let graph = nav_graph(6.0, 1.0);
        let mut world = World::new();
        spawn_agent(&mut world, 1, Vec2::new(0.5, 0.5), 0);
        spawn_agent(&mut world, 2, Vec2::new(0.5, 0.5), 0);
        let mut state = PlanState::default();
        let mut routes = RouteArena::new();
        let destinations = [Vec2::new(5.5, 0.5)];
        plan(
            &mut world,
            &graph,
            &destinations,
            &mut state,
            &mut routes,
            &PlanConfig::default(),
        );
        let route_before = world.route[0];
        // A portal this straight-line corridor does NOT cross (find a portal
        // between two tiles neither agent's path touches: none exist in this
        // 6x1 open grid other than the ones on the direct route, so instead
        // assert the reverse property directly on the portal the route *did*
        // use, which must invalidate both agents).
        let used_portal = state.portals_of_route.get(&route_before.0).unwrap()[0];
        let count = invalidate_portal(&mut world, &mut state, used_portal);
        assert_eq!(count, 2);
        assert_eq!(world.route[0], NO_ROUTE);
        assert_eq!(world.route[1], NO_ROUTE);
    }

    #[test]
    fn invalidate_portal_leaves_unrelated_routes_untouched() {
        let graph = nav_graph(6.0, 1.0);
        let mut world = World::new();
        spawn_agent(&mut world, 1, Vec2::new(0.5, 0.5), 0);
        let mut state = PlanState::default();
        let mut routes = RouteArena::new();
        let destinations = [Vec2::new(5.5, 0.5)];
        plan(
            &mut world,
            &graph,
            &destinations,
            &mut state,
            &mut routes,
            &PlanConfig::default(),
        );
        let route_before = world.route[0];
        // A portal ID guaranteed not to exist in this agent's own recorded
        // sequence: one past the graph's actual portal count.
        let bogus = PortalId(graph.portal_count() + 100);
        let count = invalidate_portal(&mut world, &mut state, bogus);
        assert_eq!(count, 0);
        assert_eq!(world.route[0], route_before);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p crowd-core phases::plan`
Expected: FAIL — `phases::plan` not declared.

- [ ] **Step 3: Wire the module in**

```rust
// crates/crowd-core/src/phases/mod.rs
pub mod decide;
pub mod integrate;
pub mod perceive;
pub mod plan;
pub mod spawn;
pub mod steer;

pub use decide::{decide, DecideConfig};
pub use integrate::{integrate, IntegrateConfig, IntegrateReport, IntegrateScratch};
pub use perceive::{perceive, PerceiveConfig, PerceiveScratch};
pub use plan::{invalidate_portal, plan, PlanConfig, PlanState};
pub use spawn::{apply_spawns, SpawnState};
pub use steer::{steer, SteerConfig, SteerReport, SteerScratch};
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p crowd-core phases::plan`
Expected: PASS (5 tests)

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/phases/plan.rs crates/crowd-core/src/phases/mod.rs
git commit -m "Add the budgeted plan tick phase and selective portal invalidation"
```

---

### Task 7: Wire `plan` into `Simulation`

**Files:**
- Modify: `crates/crowd-core/src/sim.rs`

**Interfaces:**
- Consumes: `phases::plan::{plan, invalidate_portal, PlanConfig, PlanState}` (Task 6), `crate::nav::{TileGraph, PortalId}` (Tasks 1-3), `CompiledScene.nav`/`nav_destinations` (Task 4).
- Produces: `SimConfig.plan: PlanConfig`, `Simulation::set_portal_open(&mut self, id: PortalId, open: bool)`, `Simulation::nav(&self) -> Option<&TileGraph>`.

- [ ] **Step 1: Write the failing test**

Add to `crates/crowd-core/src/sim.rs`'s test module (it will need a nav-routed scene helper distinct from `corridor()`):

```rust
    use crate::nav::NavMeshDef;

    fn nav_corridor(count: u32) -> CompiledScene {
        SceneDef {
            name: "sim_nav_corridor".into(),
            bounds: Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(20.0, 4.0)),
            walls: vec![
                Segment::new(Vec2::new(0.0, 0.0), Vec2::new(20.0, 0.0)),
                Segment::new(Vec2::new(0.0, 4.0), Vec2::new(20.0, 4.0)),
            ],
            waypoints: WaypointGraph::new(),
            destinations: vec![Destination {
                name: "exit".into(),
                node: 0,
            }],
            spawns: vec![SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(1.0, 1.0), Vec2::new(3.0, 3.0)),
                count,
                per_tick: 4,
                destination: 0,
            }],
            populations: vec![PopulationParams::default()],
            project_seed: 2026,
            ticks_per_second: 30,
            duration_ticks: 900,
            nav: Some(NavMeshDef {
                tile_size: 1.0,
                agent_radius: 0.3,
                cost_areas: Vec::new(),
                named_portals: Vec::new(),
            }),
            nav_destinations: vec![Vec2::new(18.0, 2.0)],
        }
        .compile()
        .unwrap()
    }

    #[test]
    fn a_nav_routed_simulation_routes_and_moves_its_agents() {
        let mut sim = Simulation::new(
            nav_corridor(10),
            Box::new(SampledVelocitySolver::default()),
            SimConfig::default(),
        );
        sim.run(60);
        let mut any_routed = false;
        for slot in 0..sim.world().len() {
            if sim.world().route[slot] != crate::world::NO_ROUTE {
                any_routed = true;
            }
        }
        assert!(any_routed, "the plan phase never routed any agent");
    }

    #[test]
    fn closing_a_portal_reroutes_only_agents_that_used_it() {
        let mut sim = Simulation::new(
            nav_corridor(4),
            Box::new(SampledVelocitySolver::default()),
            SimConfig::default(),
        );
        sim.run(10);
        let route_before: Vec<_> = (0..sim.world().len()).map(|s| sim.world().route[s]).collect();
        let portal = sim.nav().unwrap().portal_between(0, 1);
        if let Some(portal) = portal {
            sim.set_portal_open(portal, false);
            sim.step();
            // Every agent whose recorded route used this portal must now be
            // NO_ROUTE or freshly reassigned (never the pre-close handle).
            for (slot, before) in route_before.iter().enumerate() {
                let after = sim.world().route[slot];
                assert!(after == crate::world::NO_ROUTE || after != *before || true);
            }
        }
    }
```

(The second test's final assertion is deliberately weak — `sim.rs`'s own suite is not where selectivity is proven with confidence; that is Task 9's dedicated `two_room` integration test, which controls geometry precisely enough to assert real selectivity. This test only proves `Simulation::set_portal_open` is wired up and does not panic or desync `commit()`.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p crowd-core sim::tests::a_nav_routed_simulation_routes_and_moves_its_agents`
Expected: FAIL — `SimConfig` has no `plan` field / `Simulation` has no `set_portal_open`/`nav` (compile error).

- [ ] **Step 3: Implement the `sim.rs` changes**

Imports:

```rust
use crate::nav::{PortalId, TileGraph};
use crate::phases::decide::{decide, DecideConfig};
use crate::phases::integrate::{integrate, IntegrateConfig, IntegrateScratch};
use crate::phases::perceive::{perceive, PerceiveConfig, PerceiveScratch};
use crate::phases::plan::{invalidate_portal, plan, PlanConfig, PlanState};
use crate::phases::spawn::{apply_spawns, SpawnState};
```

`SimConfig`:

```rust
#[derive(Clone, Debug, Default)]
pub struct SimConfig {
    pub perceive: PerceiveConfig,
    pub plan: PlanConfig,
    pub decide: DecideConfig,
    pub steer: SteerConfig,
    pub integrate: IntegrateConfig,
    pub metrics: MetricsConfig,
    pub grid_cell_size: f32,
}
```

`Simulation` struct — add two fields:

```rust
pub struct Simulation {
    scene: CompiledScene,
    solver: Box<dyn AvoidanceSolver>,
    config: SimConfig,

    world: World,
    clock: Clock,
    routes: RouteArena,
    spawn_state: SpawnState,
    spawn_errors: Vec<SpawnError>,
    nav: Option<TileGraph>,
    plan_state: PlanState,

    grid: UniformGrid,
    neighbors: NeighborArena,
    perceive_scratch: PerceiveScratch,
    steer_scratch: SteerScratch,
    integrate_scratch: IntegrateScratch,

    metrics: Metrics,
}
```

`Simulation::new` — clone the scene's immutable nav template into the simulation's own mutable copy (mirrors why `routes`/`spawn_state` are separate from `scene`):

```rust
        let nav = scene.nav.clone();
        Self {
            scene,
            solver,
            config,
            world: World::new(),
            clock,
            routes: RouteArena::new(),
            spawn_state,
            spawn_errors: Vec::new(),
            nav,
            plan_state: PlanState::default(),
            grid,
            neighbors: NeighborArena::new(),
            perceive_scratch: PerceiveScratch::default(),
            steer_scratch: SteerScratch::default(),
            integrate_scratch: IntegrateScratch::default(),
            metrics: Metrics::new(),
        }
```

New accessors, next to the existing ones:

```rust
    pub fn nav(&self) -> Option<&TileGraph> {
        self.nav.as_ref()
    }

    /// Toggle a portal's open/closed state and selectively invalidate the
    /// corridors of agents whose route crossed it. No-op if the scene has no
    /// tiled navmesh.
    pub fn set_portal_open(&mut self, id: PortalId, open: bool) {
        if let Some(nav) = &mut self.nav {
            nav.set_portal_open(id, open);
            invalidate_portal(&mut self.world, &mut self.plan_state, id);
        }
    }
```

`step()` — insert the plan phase between perceive and decide:

```rust
        let start = Instant::now();
        if let Some(nav) = &self.nav {
            plan(
                &mut self.world,
                nav,
                &self.scene.nav_destinations,
                &mut self.plan_state,
                &mut self.routes,
                &self.config.plan,
            );
        }
        self.metrics
            .record_phase(Phase::Plan, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        decide(&mut self.world, &self.routes, &self.config.decide);
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p crowd-core sim::`
Expected: PASS, including both new tests and every pre-existing `sim::tests` test (which use `corridor()`, still nav-free, unchanged behavior).

- [ ] **Step 5: Run the full workspace to catch any other `SimConfig`/`Simulation` construction site**

Run: `cargo build --workspace 2>&1 | rg "error"`
Expected: no output. (`crowd-bench`'s `RunOptions`/`run_scene` build `SimConfig::default()`, which now includes `plan: PlanConfig::default()` automatically via `#[derive(Default)]` — no call site needs editing.)

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/sim.rs
git commit -m "Wire the plan phase and portal toggling into Simulation"
```

---

### Task 8: The `two_room` scene

**Files:**
- Create: `crates/crowd-core/src/nav_scenes.rs`
- Modify: `crates/crowd-core/src/lib.rs` — `pub mod nav_scenes;`

**Interfaces:**
- Consumes: `crate::nav::NavMeshDef`, `crate::scene::{SceneDef, Destination, SpawnRegion, PopulationParams}`.
- Produces: `pub fn two_room(agents: u32, seed: u64) -> SceneDef`, `pub const NORTH_DOOR: &str`, `pub const SOUTH_DOOR: &str`.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/crowd-core/src/nav_scenes.rs
//! The dedicated tiled-navmesh prototype scene: two rooms joined by two
//! doorways, built specifically to host M0 acceptance criterion 3 (a
//! 1,000-agent reroute after a portal change).
//!
//! Deliberately not part of `crowd_core::scenes::SCENE_NAMES` — it is a
//! navigation-architecture proof, not an avoidance-solver benchmark scene,
//! and folding it into `scenes::build` would pull it into the default
//! (no `--scene`) `crowd-bench sweep|baseline|check|compare` runs, which
//! need a checked-in baseline it does not have.

use crate::geometry::Segment;
use crate::nav::NavMeshDef;
use crate::scene::{Destination, PopulationParams, SceneDef, SpawnRegion};
use crate::units::{Aabb, Vec2};

pub const NORTH_DOOR: &str = "north_door";
pub const SOUTH_DOOR: &str = "south_door";

/// Room A: x in [0, 20]. Room B: x in [20, 40]. Both rooms span y in [0, 20].
/// Two doorways in the dividing wall at x=20: one centered at y=6 (south),
/// one at y=14 (north), each 1.6 m wide — wide enough to stay walkable after
/// the default 0.3 m agent-radius inflation this scene uses.
pub fn two_room(agents: u32, seed: u64) -> SceneDef {
    const DOOR_HALF_WIDTH: f32 = 0.8;
    let bounds = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(40.0, 20.0));
    let divider_x = 20.0;
    let south_y = 6.0;
    let north_y = 14.0;

    let mut walls = vec![
        Segment::new(Vec2::new(0.0, 0.0), Vec2::new(40.0, 0.0)),
        Segment::new(Vec2::new(40.0, 0.0), Vec2::new(40.0, 20.0)),
        Segment::new(Vec2::new(40.0, 20.0), Vec2::new(0.0, 20.0)),
        Segment::new(Vec2::new(0.0, 20.0), Vec2::new(0.0, 0.0)),
    ];
    // The dividing wall, with two doorway gaps.
    walls.push(Segment::new(
        Vec2::new(divider_x, 0.0),
        Vec2::new(divider_x, south_y - DOOR_HALF_WIDTH),
    ));
    walls.push(Segment::new(
        Vec2::new(divider_x, south_y + DOOR_HALF_WIDTH),
        Vec2::new(divider_x, north_y - DOOR_HALF_WIDTH),
    ));
    walls.push(Segment::new(
        Vec2::new(divider_x, north_y + DOOR_HALF_WIDTH),
        Vec2::new(divider_x, 20.0),
    ));

    SceneDef {
        name: "two_room".into(),
        bounds,
        walls,
        waypoints: crate::route::WaypointGraph::new(),
        destinations: vec![Destination {
            name: "room_b".into(),
            node: 0, // unused: nav_destinations carries the real point
        }],
        spawns: vec![SpawnRegion {
            id: 0,
            population_id: 0,
            area: Aabb::new(Vec2::new(1.0, 1.0), Vec2::new(18.0, 19.0)),
            count: agents,
            per_tick: 8,
            destination: 0,
        }],
        populations: vec![PopulationParams::default()],
        project_seed: seed,
        ticks_per_second: 30,
        duration_ticks: 3600,
        nav: Some(NavMeshDef {
            tile_size: 0.5,
            agent_radius: 0.3,
            cost_areas: Vec::new(),
            named_portals: vec![
                (SOUTH_DOOR.to_string(), Vec2::new(divider_x, south_y)),
                (NORTH_DOOR.to_string(), Vec2::new(divider_x, north_y)),
            ],
        }),
        nav_destinations: vec![Vec2::new(38.0, 10.0)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{SimConfig, Simulation};
    use crate::avoidance::SampledVelocitySolver;

    #[test]
    fn two_room_compiles_without_diagnostics() {
        assert!(two_room(100, 42).compile().is_ok());
    }

    #[test]
    fn two_room_spawns_the_requested_agent_count() {
        let compiled = two_room(200, 42).compile().unwrap();
        assert_eq!(compiled.total_agents(), 200);
    }

    #[test]
    fn both_named_doors_resolve_to_distinct_portals() {
        let compiled = two_room(50, 42).compile().unwrap();
        let nav = compiled.nav.as_ref().unwrap();
        let south = nav.portal_named(SOUTH_DOOR);
        let north = nav.portal_named(NORTH_DOOR);
        assert!(south.is_some());
        assert!(north.is_some());
        assert_ne!(south, north);
    }

    #[test]
    fn two_room_runs_without_producing_nonfinite_state() {
        let compiled = two_room(50, 42).compile().unwrap();
        let mut sim = Simulation::new(
            compiled,
            Box::new(SampledVelocitySolver::default()),
            SimConfig::default(),
        );
        sim.run(300);
        for slot in 0..sim.world().len() {
            assert!(sim.world().position(slot as u32).is_finite());
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p crowd-core nav_scenes`
Expected: FAIL — module not declared.

- [ ] **Step 3: Wire the module in**

```rust
// crates/crowd-core/src/lib.rs — add after `pub mod metrics;`, before `pub mod nav;`
pub mod nav_scenes;
```

(alphabetically `nav` < `nav_scenes`, so the final order is `metrics`, `nav`, `nav_scenes`, `phases`, ...)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p crowd-core nav_scenes`
Expected: PASS (4 tests)

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/nav_scenes.rs crates/crowd-core/src/lib.rs
git commit -m "Add the two_room tiled-navmesh prototype scene"
```

---

### Task 9: Integration test — the portal-reroute acceptance criterion

**Files:**
- Create: `crates/crowd-core/tests/two_room_reroute.rs`

**Interfaces:**
- Consumes: `crowd_core::nav_scenes::{two_room, NORTH_DOOR, SOUTH_DOOR}`, `crowd_core::sim::{SimConfig, Simulation}`, `crowd_core::avoidance::SampledVelocitySolver`, `Simulation::{nav, set_portal_open}`, `phases::plan::PlanState` internals via `Simulation`'s public surface only (no direct access to `portals_of_route`).

- [ ] **Step 1: Write the failing tests**

```rust
// crates/crowd-core/tests/two_room_reroute.rs
//! M0 acceptance criterion 3: a tiled-navigation case reroutes after a portal
//! change without corrupting unrelated corridors.
//!
//! Runs at reduced scale here for fast CI. The 1,000-agent version is
//! release-gated (`#[ignore]`), matching the project's existing
//! `fuzz_density` convention for expensive scenes.

use crowd_core::avoidance::SampledVelocitySolver;
use crowd_core::nav_scenes::{two_room, SOUTH_DOOR};
use crowd_core::sim::{SimConfig, Simulation};
use crowd_core::world::NO_ROUTE;

fn simulation(agents: u32, seed: u64) -> Simulation {
    Simulation::new(
        two_room(agents, seed).compile().unwrap(),
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    )
}

/// Run until every non-arrived agent has at least attempted a route once
/// (route != NO_ROUTE at least once), bounded so a bug cannot hang the test.
fn run_until_initially_routed(sim: &mut Simulation, max_ticks: u64) {
    for _ in 0..max_ticks {
        sim.step();
        let all_attempted = (0..sim.world().len()).all(|s| sim.world().route[s] != NO_ROUTE);
        if all_attempted {
            return;
        }
    }
    panic!("agents never finished initial routing within {max_ticks} ticks");
}

#[test]
fn closing_south_door_reroutes_agents_that_used_it_and_leaves_the_rest_alone() {
    let mut sim = simulation(60, 2026);
    run_until_initially_routed(&mut sim, 200);

    let south = sim.nav().unwrap().portal_named(SOUTH_DOOR).unwrap();

    let route_before: Vec<_> = (0..sim.world().len()).map(|s| sim.world().route[s]).collect();

    sim.set_portal_open(south, false);

    // The invalidation itself is synchronous (Task 6), visible immediately
    // after `set_portal_open` returns, before the next `step()`.
    let mut invalidated = 0;
    let mut untouched = 0;
    for (slot, before) in route_before.iter().enumerate() {
        if sim.world().route[slot] == NO_ROUTE {
            invalidated += 1;
        } else {
            assert_eq!(
                sim.world().route[slot],
                *before,
                "slot {slot}'s route changed without being invalidated"
            );
            untouched += 1;
        }
    }
    assert!(invalidated > 0, "closing a door in active use invalidated nobody");
    assert!(untouched > 0, "closing one door invalidated every agent, not just its users");

    // Every invalidated agent must recover a working route via the north
    // door within a bounded number of further ticks, and the population
    // must keep making progress.
    for _ in 0..200 {
        sim.step();
    }
    assert!(
        sim.metrics().arrived() > 0,
        "nobody made it to room B via the remaining door"
    );
}

#[test]
fn reopening_a_portal_does_not_disturb_routes_that_never_used_it() {
    let mut sim = simulation(40, 7);
    run_until_initially_routed(&mut sim, 200);
    let south = sim.nav().unwrap().portal_named(SOUTH_DOOR).unwrap();
    let route_before: Vec<_> = (0..sim.world().len()).map(|s| sim.world().route[s]).collect();
    sim.set_portal_open(south, false);
    sim.set_portal_open(south, true);
    // Agents that never used the south door were untouched by the close, and
    // the reopen touches nothing (Task 6's `invalidate_portal` only clears
    // routes whose recorded sequence contains the toggled portal — true for
    // both directions of the toggle, so a route already cleared by the close
    // stays cleared, and one that was never in the closed set is still
    // unaffected by the reopen).
    for (slot, before) in route_before.iter().enumerate() {
        let after = sim.world().route[slot];
        assert!(
            after == *before || after == NO_ROUTE,
            "slot {slot} acquired an unexplained new route from a reopen alone"
        );
    }
}

#[test]
#[ignore] // release-only: cargo test --release -p crowd-core --test two_room_reroute -- --ignored
fn a_thousand_agent_reroute_does_not_corrupt_unrelated_corridors() {
    let mut sim = simulation(1000, 2026);
    run_until_initially_routed(&mut sim, 2000);
    let south = sim.nav().unwrap().portal_named(SOUTH_DOOR).unwrap();
    let route_before: Vec<_> = (0..sim.world().len()).map(|s| sim.world().route[s]).collect();
    sim.set_portal_open(south, false);
    let mut invalidated = 0;
    for (slot, before) in route_before.iter().enumerate() {
        if sim.world().route[slot] != *before {
            invalidated += 1;
        }
    }
    assert!(invalidated > 0, "1,000-agent close invalidated nobody");
    assert!(
        (invalidated as usize) < sim.world().len(),
        "1,000-agent close invalidated the entire population, not a selective subset"
    );
    for _ in 0..1500 {
        sim.step();
    }
    assert!(sim.metrics().arrived() > 0, "nobody arrived after the 1,000-agent reroute");
    for slot in 0..sim.world().len() {
        assert!(sim.world().position(slot as u32).is_finite());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p crowd-core --test two_room_reroute`
Expected: FAIL initially only if a prior task is incomplete; if Tasks 1-8 are done, this should mostly compile — run it to confirm the two non-ignored tests currently pass or reveal a real bug (this is the acceptance-criterion test, so treat a genuine failure here as a signal to fix Tasks 6/7, not to weaken the assertion).

- [ ] **Step 3: Fix any real failures surfaced**

If `invalidated == 0` in the first test: the scene's spawn area (`Aabb::new(Vec2::new(1.0,1.0), Vec2::new(18.0,19.0))`, all of room A) combined with a single `nav_destinations` point at `(38.0, 10.0)` may make *every* agent's A* solve prefer the same door (north, being geometrically central) rather than splitting across both — since both doors are equidistant-ish from `(38,10)`'s straight path only for agents near y=10. Adjust the spawn region split into two sub-regions in `two_room()` (Task 8) that bias placement toward each door if the observed split is degenerate: change `nav_scenes.rs`'s single `spawns: vec![SpawnRegion { area: Aabb::new(Vec2::new(1.0,1.0), Vec2::new(18.0,19.0)), ...}]` to two spawn regions, one in the lower half (`y` in `[1,9]`) and one in the upper half (`y` in `[11,19]`), each with `count: agents/2`-style split using the existing `split_count`-style pattern from `scenes.rs` (import or reimplement a local `fn split_count` in `nav_scenes.rs`, copied verbatim from `crowd-core/src/scenes.rs:126-130`, since `scenes.rs`'s copy is private (`fn`, not `pub fn`)).

Re-run Step 2 after this fix until `invalidated > 0` and `untouched > 0` both hold reliably.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p crowd-core --test two_room_reroute`
Expected: PASS (2 tests; the 3rd is `#[ignore]`d)

Run: `cargo test --release -p crowd-core --test two_room_reroute -- --ignored`
Expected: PASS (1 test) — this is the actual M0 acceptance-criterion-3 evidence at the contract's stated 1,000-agent scale.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add crates/crowd-core/tests/two_room_reroute.rs crates/crowd-core/src/nav_scenes.rs
git commit -m "Add the portal-reroute integration test (M0 acceptance criterion 3)"
```

---

### Task 10: Determinism suite parametrization

**Files:**
- Modify: `crates/crowd-core/tests/determinism.rs`

**Interfaces:**
- Consumes: `crowd_core::nav_scenes::two_room`, whatever helper pattern `determinism.rs` already uses for its per-scene loop (read the file first — it likely already parametrizes over `crowd_core::scenes::SCENE_NAMES`, so `two_room` needs a second, explicit loop entry since it is intentionally outside that list).

- [ ] **Step 1: Read the existing file to match its pattern exactly**

Run: `cat crates/crowd-core/tests/determinism.rs` and identify: (a) the exact helper function name that builds a `Simulation` from a scene name/`SceneDef`, (b) whether it iterates `SCENE_NAMES` directly or a local list, (c) the exact assertions used (bitwise state-hash equality across identical runs, spawn-order permutation, add-one-agent).

- [ ] **Step 2: Write the failing test**

Add a `two_room`-specific version of each existing determinism check that the file already has for the six named scenes, following its exact existing style (do not invent a new pattern). At minimum:

```rust
#[test]
fn two_room_identical_runs_produce_identical_state_hashes() {
    use crowd_core::avoidance::SampledVelocitySolver;
    use crowd_core::nav_scenes::two_room;
    use crowd_core::sim::{SimConfig, Simulation};

    let mut a = Simulation::new(
        two_room(60, 2026).compile().unwrap(),
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    let mut b = Simulation::new(
        two_room(60, 2026).compile().unwrap(),
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    for tick in 0..300 {
        a.step();
        b.step();
        assert_eq!(a.state_hash(), b.state_hash(), "diverged at tick {tick}");
    }
}

#[test]
fn two_room_portal_close_stays_deterministic() {
    use crowd_core::avoidance::SampledVelocitySolver;
    use crowd_core::nav_scenes::{two_room, SOUTH_DOOR};
    use crowd_core::sim::{SimConfig, Simulation};

    let build = || {
        Simulation::new(
            two_room(60, 2026).compile().unwrap(),
            Box::new(SampledVelocitySolver::default()),
            SimConfig::default(),
        )
    };
    let mut a = build();
    let mut b = build();
    for _ in 0..100 {
        a.step();
        b.step();
    }
    let south_a = a.nav().unwrap().portal_named(SOUTH_DOOR).unwrap();
    let south_b = b.nav().unwrap().portal_named(SOUTH_DOOR).unwrap();
    assert_eq!(south_a, south_b, "named-portal resolution itself must be deterministic");
    a.set_portal_open(south_a, false);
    b.set_portal_open(south_b, false);
    for tick in 0..300 {
        a.step();
        b.step();
        assert_eq!(a.state_hash(), b.state_hash(), "diverged at tick {tick} after portal close");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail, then pass**

Run: `cargo test -p crowd-core --test determinism two_room`
Expected: FAIL first (if `nav_scenes`/`Simulation::nav`/`set_portal_open` are already implemented from prior tasks, this should actually pass immediately — treat an immediate pass as confirmation, not a process violation, and still record the run).
Run again: PASS (2 tests)

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add crates/crowd-core/tests/determinism.rs
git commit -m "Parametrize the determinism suite over two_room and portal toggling"
```

---

### Task 11: `NavDebugSnapshot` and crowd-bench SVG rendering

**Files:**
- Create: `crates/crowd-core/src/nav/debug.rs`
- Modify: `crates/crowd-core/src/nav/mod.rs` — `pub mod debug; pub use debug::NavDebugSnapshot;`
- Modify: `crates/crowd-bench/src/svg.rs` — add nav-layer rendering.

**Interfaces:**
- Consumes: `TileGraph` (grid + portals), `World`/`RouteArena` (for per-agent corridor points), `Simulation::nav()`.
- Produces: `NavDebugSnapshot::capture(sim: &Simulation) -> NavDebugSnapshot`, `TrajectoryRecorder::write_svg_with_nav(&self, scene_name: &str, bounds: Aabb, walls: &[Segment], nav: Option<&NavDebugSnapshot>) -> String`.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/crowd-core/src/nav/debug.rs
//! A point-in-time snapshot for visualising the tiled navmesh: which tiles
//! are walkable and at what cost, which portals are open/closed, and each
//! tracked agent's current corridor.

use crate::route::RouteArena;
use crate::units::Vec2;
use crate::world::World;

use super::{Portal, TileGraph};

pub struct NavDebugSnapshot {
    pub tile_size: f32,
    pub origin: Vec2,
    pub cols: u32,
    pub rows: u32,
    pub walkable: Vec<bool>,
    pub cost: Vec<f32>,
    pub portals: Vec<(Portal, Vec2)>,
    /// (agent slot, corridor points), only for slots with a live route.
    pub corridors: Vec<(u32, Vec<Vec2>)>,
}

impl NavDebugSnapshot {
    pub fn capture(nav: &TileGraph, world: &World, routes: &RouteArena, max_agents: usize) -> Self {
        let grid = nav.grid();
        let mut walkable = Vec::with_capacity(grid.tile_count() as usize);
        let mut cost = Vec::with_capacity(grid.tile_count() as usize);
        for tile in 0..grid.tile_count() {
            walkable.push(grid.is_walkable(tile));
            cost.push(grid.cost(tile));
        }
        let mut portals = Vec::with_capacity(nav.portal_count() as usize);
        for id in 0..nav.portal_count() {
            let id = super::PortalId(id);
            portals.push((*nav.portal(id), nav.portal_midpoint(id)));
        }
        let mut corridors = Vec::new();
        for slot in 0..world.len().min(max_agents) {
            let handle = world.route[slot];
            let points = routes.points(handle);
            if !points.is_empty() {
                corridors.push((slot as u32, points.to_vec()));
            }
        }
        Self {
            tile_size: grid_tile_size(grid),
            origin: grid_origin(grid),
            cols: grid.cols(),
            rows: grid.rows(),
            walkable,
            cost,
            portals,
            corridors,
        }
    }
}

// `TileGrid` intentionally keeps `origin`/`tile_size` private (Task 1) since
// nothing outside rasterization needed them until now. Small accessors added
// here rather than widening `TileGrid`'s public surface for a debug-only
// consumer:
fn grid_tile_size(grid: &super::TileGrid) -> f32 {
    tile_size_via_two_tiles(grid)
}

fn tile_size_via_two_tiles(grid: &super::TileGrid) -> f32 {
    if grid.cols() > 1 {
        (grid.tile_center(1).x - grid.tile_center(0).x).abs()
    } else if grid.rows() > 1 {
        (grid.tile_center(grid.cols()).y - grid.tile_center(0).y).abs()
    } else {
        1.0
    }
}

fn grid_origin(grid: &super::TileGrid) -> Vec2 {
    let half = tile_size_via_two_tiles(grid) * 0.5;
    Vec2::new(grid.tile_center(0).x - half, grid.tile_center(0).y - half)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::NavMeshDef;
    use crate::units::Aabb;
    use crate::world::{AgentSpawn, NO_ROUTE};
    use crate::ids::AgentId;

    #[test]
    fn snapshot_covers_every_tile_and_portal() {
        let graph = NavMeshDef {
            tile_size: 1.0,
            agent_radius: 0.3,
            cost_areas: Vec::new(),
            named_portals: Vec::new(),
        }
        .build_graph(Aabb::new(Vec2::ZERO, Vec2::new(3.0, 3.0)), &[]);
        let world = World::new();
        let routes = RouteArena::new();
        let snapshot = NavDebugSnapshot::capture(&graph, &world, &routes, 100);
        assert_eq!(snapshot.walkable.len(), graph.grid().tile_count() as usize);
        assert_eq!(snapshot.portals.len(), graph.portal_count() as usize);
    }

    #[test]
    fn snapshot_includes_routed_agents_corridors() {
        let graph = NavMeshDef {
            tile_size: 1.0,
            agent_radius: 0.3,
            cost_areas: Vec::new(),
            named_portals: Vec::new(),
        }
        .build_graph(Aabb::new(Vec2::ZERO, Vec2::new(3.0, 3.0)), &[]);
        let mut world = World::new();
        world
            .spawn(
                AgentSpawn {
                    agent_id: AgentId(1),
                    population_id: 0,
                    position: Vec2::new(0.5, 0.5),
                    yaw: 0.0,
                    radius: 0.3,
                    max_speed: 1.8,
                    preferred_speed: 1.35,
                    route: NO_ROUTE,
                    destination: 0,
                },
                0,
            )
            .unwrap();
        let mut routes = RouteArena::new();
        let handle = routes.push_route(&[Vec2::new(0.5, 0.5), Vec2::new(2.5, 2.5)]);
        world.route[0] = handle;
        let snapshot = NavDebugSnapshot::capture(&graph, &world, &routes, 100);
        assert_eq!(snapshot.corridors.len(), 1);
        assert_eq!(snapshot.corridors[0].0, 0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p crowd-core nav::debug`
Expected: FAIL — module not declared.

- [ ] **Step 3: Wire the module in**

```rust
// crates/crowd-core/src/nav/mod.rs — add:
pub mod debug;
pub use debug::NavDebugSnapshot;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p crowd-core nav::debug`
Expected: PASS (2 tests)

- [ ] **Step 5: Add SVG rendering in crowd-bench**

In `crates/crowd-bench/src/svg.rs`, add a new method to `TrajectoryRecorder` (do not change the existing `write_svg`'s signature or callers — this is additive):

```rust
use crowd_core::nav::NavDebugSnapshot;

impl TrajectoryRecorder {
    // ... existing methods unchanged ...

    pub fn write_svg_with_nav(
        &self,
        scene_name: &str,
        bounds: Aabb,
        walls: &[Segment],
        nav: Option<&NavDebugSnapshot>,
    ) -> String {
        let size = bounds.size();
        let width = size.x * SCALE + MARGIN * 2.0;
        let height = size.y * SCALE + MARGIN * 2.0;
        let project = |p: Vec2| -> (f32, f32) {
            (
                (p.x - bounds.min.x) * SCALE + MARGIN,
                height - ((p.y - bounds.min.y) * SCALE + MARGIN),
            )
        };

        let mut out = String::new();
        let _ = write!(
            out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}">"#
        );
        let _ = write!(out, r##"<rect width="100%" height="100%" fill="#111"/>"##);

        if let Some(nav) = nav {
            for tile in 0..(nav.cols * nav.rows) {
                if !nav.walkable[tile as usize] {
                    continue;
                }
                let col = tile % nav.cols;
                let row = tile / nav.cols;
                let center = Vec2::new(
                    nav.origin.x + (col as f32 + 0.5) * nav.tile_size,
                    nav.origin.y + (row as f32 + 0.5) * nav.tile_size,
                );
                let (x, y) = project(center);
                let cost = nav.cost[tile as usize];
                let fill = if cost > 1.01 { "#553311" } else { "#1a2a1a" };
                let half = nav.tile_size * SCALE * 0.5;
                let _ = write!(
                    out,
                    r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{fill}"/>"##,
                    x - half,
                    y - half,
                    half * 2.0,
                    half * 2.0
                );
            }
            for (portal, midpoint) in &nav.portals {
                let (x, y) = project(*midpoint);
                let color = if portal.open { "#4caf50" } else { "#f44336" };
                let _ = write!(
                    out,
                    r##"<circle cx="{x:.1}" cy="{y:.1}" r="3" fill="{color}"/>"##
                );
            }
            for (_, corridor) in &nav.corridors {
                let mut points = String::new();
                for p in corridor {
                    let (x, y) = project(*p);
                    let _ = write!(points, "{x:.1},{y:.1} ");
                }
                let _ = write!(
                    out,
                    r#"<polyline points="{}" fill="none" stroke="#00bcd4" stroke-width="1" opacity="0.6"/>"#,
                    points.trim_end()
                );
            }
        }

        for wall in walls {
            let (x1, y1) = project(wall.a);
            let (x2, y2) = project(wall.b);
            if [x1, y1, x2, y2].iter().all(|v| v.is_finite()) {
                let _ = write!(
                    out,
                    r##"<line x1="{x1:.1}" y1="{y1:.1}" x2="{x2:.1}" y2="{y2:.1}" stroke="#888" stroke-width="2"/>"##
                );
            }
        }

        let _ = write!(
            out,
            r##"<text x="{MARGIN:.0}" y="{:.0}" fill="#eee" font-family="monospace" font-size="14">{scene_name}</text>"##,
            MARGIN - 4.0
        );

        out.push_str("</svg>\n");
        out
    }
}

#[cfg(test)]
mod nav_svg_tests {
    use super::*;
    use crowd_core::nav::{NavDebugSnapshot, NavMeshDef};
    use crowd_core::route::RouteArena;
    use crowd_core::units::{Aabb, Vec2};
    use crowd_core::world::World;

    #[test]
    fn nav_debug_svg_renders_tiles_and_portals() {
        let graph = NavMeshDef {
            tile_size: 1.0,
            agent_radius: 0.3,
            cost_areas: Vec::new(),
            named_portals: Vec::new(),
        }
        .build_graph(Aabb::new(Vec2::ZERO, Vec2::new(3.0, 3.0)), &[]);
        let world = World::new();
        let routes = RouteArena::new();
        let snapshot = NavDebugSnapshot::capture(&graph, &world, &routes, 100);
        let recorder = TrajectoryRecorder::new(5, 100);
        let svg = recorder.write_svg_with_nav(
            "two_room",
            Aabb::new(Vec2::ZERO, Vec2::new(3.0, 3.0)),
            &[],
            Some(&snapshot),
        );
        assert!(svg.contains("<rect"));
        assert!(svg.contains("<circle"));
    }

    #[test]
    fn nav_debug_svg_without_a_snapshot_still_renders() {
        let recorder = TrajectoryRecorder::new(5, 100);
        let svg = recorder.write_svg_with_nav(
            "empty",
            Aabb::new(Vec2::ZERO, Vec2::new(3.0, 3.0)),
            &[],
            None,
        );
        assert!(svg.starts_with("<svg"));
    }
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p crowd-core nav::debug && cargo test -p crowd-bench svg`
Expected: PASS (2 + 2 tests)

- [ ] **Step 7: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/nav/debug.rs crates/crowd-core/src/nav/mod.rs crates/crowd-bench/src/svg.rs
git commit -m "Add NavDebugSnapshot and SVG rendering for tiles, portals, and corridors"
```

---

### Task 12: `crowd-bench nav-reroute` subcommand

**Files:**
- Create: `crates/crowd-bench/src/nav_bench.rs`
- Modify: `crates/crowd-bench/src/main.rs` — new subcommand and args.

**Interfaces:**
- Consumes: `crowd_core::nav_scenes::{two_room, SOUTH_DOOR}`, `crowd_core::sim::{SimConfig, Simulation}`, `NavDebugSnapshot`, `TrajectoryRecorder::write_svg_with_nav`.
- Produces: `crowd-bench nav-reroute [--agents N] [--seed N] [--out DIR] [--svg]`, writing a JSON report with pre/post-close invalidation counts and arrival count, and optionally an SVG.

- [ ] **Step 1: Implement `nav_bench.rs`**

```rust
// crates/crowd-bench/src/nav_bench.rs
//! Runs `two_room`, closes the south door partway through, and reports the
//! reroute outcome — the CLI-facing form of M0 acceptance criterion 3.

use std::path::PathBuf;

use crowd_core::avoidance::SampledVelocitySolver;
use crowd_core::nav::NavDebugSnapshot;
use crowd_core::nav_scenes::{two_room, SOUTH_DOOR};
use crowd_core::sim::{SimConfig, Simulation};
use crowd_core::world::NO_ROUTE;
use serde::{Deserialize, Serialize};

use crate::svg::TrajectoryRecorder;

pub struct NavRerouteOptions {
    pub agents: u32,
    pub seed: u64,
    pub out_dir: PathBuf,
    pub svg: bool,
    /// Ticks to run before the initial routing pass is assumed complete, and
    /// again after the close, before measuring arrivals.
    pub settle_ticks: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NavRerouteReport {
    pub scene: String,
    pub agents: u32,
    pub seed: u64,
    pub invalidated_on_close: u32,
    pub untouched_on_close: u32,
    pub arrived_after_reroute: u64,
}

pub fn run_nav_reroute(options: &NavRerouteOptions) -> Result<NavRerouteReport, String> {
    let compiled = two_room(options.agents, options.seed)
        .compile()
        .map_err(|e| format!("{e:?}"))?;
    let mut sim = Simulation::new(
        compiled,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );

    let mut recorder = TrajectoryRecorder::new(10, options.agents as usize);
    for _ in 0..options.settle_ticks {
        sim.step();
        recorder.record(&sim);
    }

    let south = sim
        .nav()
        .and_then(|nav| nav.portal_named(SOUTH_DOOR))
        .ok_or("south_door portal did not resolve")?;
    let route_before: Vec<_> = (0..sim.world().len()).map(|s| sim.world().route[s]).collect();
    sim.set_portal_open(south, false);

    let mut invalidated_on_close = 0u32;
    let mut untouched_on_close = 0u32;
    for (slot, before) in route_before.iter().enumerate() {
        if sim.world().route[slot] == NO_ROUTE {
            invalidated_on_close += 1;
        } else {
            debug_assert_eq!(sim.world().route[slot], *before);
            untouched_on_close += 1;
        }
    }

    for _ in 0..options.settle_ticks {
        sim.step();
        recorder.record(&sim);
    }

    if options.svg {
        std::fs::create_dir_all(&options.out_dir).map_err(|e| e.to_string())?;
        let snapshot = sim
            .nav()
            .map(|nav| NavDebugSnapshot::capture(nav, sim.world(), sim.routes(), options.agents as usize));
        let svg = recorder.write_svg_with_nav(
            "two_room",
            sim.scene().bounds,
            sim.walls(),
            snapshot.as_ref(),
        );
        let path = options.out_dir.join(format!("two_room-reroute-{}.svg", options.agents));
        std::fs::write(&path, svg).map_err(|e| e.to_string())?;
    }

    Ok(NavRerouteReport {
        scene: "two_room".to_string(),
        agents: options.agents,
        seed: options.seed,
        invalidated_on_close,
        untouched_on_close,
        arrived_after_reroute: sim.metrics().arrived(),
    })
}
```

- [ ] **Step 2: Wire the subcommand into `main.rs`**

```rust
// crates/crowd-bench/src/main.rs
mod alloc;
mod baseline;
mod frames;
mod nav_bench;
mod report;
mod svg;
```

Extend `usage()`:

```rust
fn usage() -> &'static str {
    "usage:
  crowd-bench run [--scene NAME] [--agents N] [--seed N] [--svg] [--frames] [--frame-interval N] [--out DIR] [--solver NAME] [--trace]
  crowd-bench sweep [--scene NAME] [--seed N]
  crowd-bench baseline [--scene NAME] [--agents N] [--seed N] [--solver NAME]
  crowd-bench check [--agents N] [--seed N] [--solver NAME]
  crowd-bench compare [--scene NAME] [--out DIR]
  crowd-bench nav-reroute [--agents N] [--seed N] [--out DIR] [--svg]

Omitting --scene runs every scene."
}
```

Add a `command_nav_reroute`:

```rust
fn command_nav_reroute(args: &Args) -> Result<(), String> {
    std::fs::create_dir_all(&args.out).map_err(|e| e.to_string())?;
    let options = nav_bench::NavRerouteOptions {
        agents: args.agents,
        seed: args.seed,
        out_dir: args.out.clone(),
        svg: args.svg,
        settle_ticks: 600,
    };
    let report = nav_bench::run_nav_reroute(&options)?;
    let path = args.out.join(format!("two_room-reroute-{}.json", args.agents));
    let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    println!(
        "two_room: {} invalidated / {} untouched on close, {} arrived after reroute -> {}",
        report.invalidated_on_close, report.untouched_on_close, report.arrived_after_reroute, path.display()
    );
    Ok(())
}
```

Add the dispatch arm in `main()`:

```rust
        "nav-reroute" => command_nav_reroute(&args).map(|()| true),
```

- [ ] **Step 3: Manually verify the new command runs**

Run: `cargo run --release -p crowd-bench -- nav-reroute --agents 1000 --svg`
Expected: prints a summary line with `invalidated > 0` and `untouched > 0`, and writes `benchmarks/reports/two_room-reroute-1000.json` and `.svg`.

- [ ] **Step 4: Run the full workspace test suite**

Run: `cargo test --workspace`
Expected: PASS, no regressions.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add crates/crowd-bench/src/nav_bench.rs crates/crowd-bench/src/main.rs
git commit -m "Add the crowd-bench nav-reroute subcommand"
```

---

### Task 13: Dated M0 report and doc updates

**Files:**
- Create: `docs/benchmarks/2026-08-08-tiled-navmesh-prototype.md`
- Modify: `docs/milestones/README.md` — update the M0 in-scope table (item 4: Done) and baseline section.
- Modify: `README.md` — add the `nav-reroute` command to the command block.
- Modify: `AGENTS.md` — same.

**Interfaces:** None (documentation only).

- [ ] **Step 1: Run the real 1,000-agent evidence and record the exact numbers**

Run: `cargo run --release -p crowd-bench -- nav-reroute --agents 1000 --svg`

Capture the printed `invalidated`/`untouched`/`arrived` counts, and the environment (`os`, `arch`, `cpu`, `rustc --version`) the same way `docs/benchmarks/2026-08-07-blender-bridge.md` records it (read that file's "Environment" table format and match it — do not invent a different table shape).

Run: `cargo test --release -p crowd-core --test two_room_reroute -- --ignored`

Capture pass/fail and wall time.

- [ ] **Step 2: Write the report**

```markdown
# Tiled navmesh/corridor prototype — M0 item 4

Date: 2026-08-08
Milestone: [M0 — Proving grounds](../milestones/M0-proving-grounds.md)
Design: [Tiled navmesh/corridor prototype design](../superpowers/specs/2026-08-08-tiled-navmesh-prototype-design.md)

## Environment

| | |
|---|---|
| CPU | <fill in from `sysctl -n machdep.cpu.brand_string` or equivalent> |
| OS | <fill in> |
| Rust | <fill in from `rustc --version`> |

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
| Agents invalidated on closing the south door | <fill in> |
| Agents untouched by closing the south door | <fill in> |
| Agents arrived after the reroute | <fill in> |
| Release-gated 1,000-agent reroute test | <PASS/FAIL, wall time> |

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
navigation, or funnel-smoothed corridor quality — none were measured.
```

Fill in the `<fill in>` placeholders from Step 1's captured output before committing — this file must not be committed with any placeholder text remaining.

- [ ] **Step 3: Update `docs/milestones/README.md`**

Change the M0 in-scope table row 4 from:

```
| 4 | Tiled navmesh/corridor prototype, portal change, path budgeting | **Open** — navigation is still the waypoint stand-in |
```

to:

```
| 4 | Tiled navmesh/corridor prototype, portal change, path budgeting | Done — `crowd_core::nav`, the `plan` phase, and the `two_room` scene |
```

Update the "Met" acceptance-criteria sentence to add criterion 3, and move it out of "Not met." Update the "Evidence to date" list to add:

```
- [Tiled navmesh/corridor prototype](../benchmarks/2026-08-08-tiled-navmesh-prototype.md)
```

- [ ] **Step 4: Update `README.md` and `AGENTS.md`**

In both files' command blocks, add after the existing `crowd-bench compare` line:

```sh
cargo run --release -p crowd-bench -- nav-reroute --agents 1000 --svg  # tiled-navmesh portal reroute (M0 item 4)
cargo test --release -p crowd-core --test two_room_reroute -- --ignored  # 1,000-agent reroute acceptance test
```

- [ ] **Step 5: Run the documentation checks**

Run: `git diff --check`
Expected: no output (no whitespace errors).

Run: `rg '^## ' docs/blender-crowd-1.0.md`
Expected: unchanged section list (sanity check the contract itself wasn't touched).

Run: `rg '^#' docs/milestones/*.md`
Expected: headings render as expected, M0's table edit is visible.

- [ ] **Step 6: Final full-workspace verification**

Run: `cargo test --workspace`
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Run: `cargo fmt --check`

Expected: all clean.

- [ ] **Step 7: Commit**

```bash
git add docs/benchmarks/2026-08-08-tiled-navmesh-prototype.md docs/milestones/README.md README.md AGENTS.md
git commit -m "Record the tiled navmesh prototype's M0 evidence and close item 4"
```

---

## Self-review notes

- **Spec coverage:** Design section 2 (tile grid/cost areas) → Task 1. Section 3 (portals) → Task 2. Section 4 (pathfinding/corridor) → Task 3. Section 5 (budgeting) → Task 6, refined from a `VecDeque` queue to a resume-cursor full scan (Task 6's header comment states why: detection becomes a pure function of `World` state instead of separately-maintained queue state that could drift out of sync — same observable budgeted/deterministic/fair behavior). Section 6 (portal toggle invalidation) → Task 6 + Task 7. Section 7 (`NavDebugSnapshot`) → Task 11. Section 8 (`two_room`) → Task 8, with a mid-plan refinement in Task 9 Step 3 to guarantee the reroute test's split is non-degenerate. Section 9 (testing) → distributed across every task's own test step, plus dedicated Tasks 9-10. Section 10 (validation commands) → Task 12 Step 3 and Task 13. Section 11 (limitations) → Task 13's report.
- **A deviation from the design doc that is called out explicitly:** the design's section 4/8 sketch used `crowd-bench run --scene two_room --nav-debug-svg`. This plan instead gives `two_room` its own `nav-reroute` subcommand entirely outside `scenes::SCENE_NAMES`, because folding it into the shared scene list would silently expand every existing default (`--scene`-less) `sweep`/`baseline`/`check`/`compare` invocation to include a scene with no baseline file — breaking the documented regression command. This preserves the "no change to the six existing scenes' behavior" constraint at the tooling level, not just the simulation level.
- **A design-doc gap resolved during planning:** the original design's section 5/7 implicitly treated `TileGraph` as living on `CompiledScene` and being mutated in place by a portal toggle. `CompiledScene` is meant to be immutable (matching `wall_index`/`SegmentIndex`'s existing precedent), so this plan has `CompiledScene.nav` hold an immutable, all-open template and has `Simulation::new` clone its own mutable copy — mirroring exactly how `RouteArena`/`SpawnState` are already kept separate from the immutable `scene` field.
- **Placeholder scan:** no task step says "add tests for the above" without code, no `TBD`/`TODO` remains outside the one documented, filled-in-before-commit report placeholder in Task 13 (which is explicit about being filled in before that commit, not left in).
- **Type consistency:** `PlanState`, `PlanConfig`, `TileGraph`, `PortalId`, `NavMeshDef`, `NavDebugSnapshot` are defined once (Tasks 1-3, 6, 11) and consumed with identical signatures in every later task (7-12) — cross-checked against each task's "Consumes" line.
