//! Portals: the tile-adjacency edges a topology change (a closed doorway)
//! removes or restores.

use std::collections::HashMap;

use crate::nav::grid::TileGrid;
use crate::units::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PortalId(pub u32);

/// Which pair of tile neighbors a portal connects, matching the two cases
/// `TileGraph::build` constructs: an east-west portal joins column-adjacent
/// tiles in the same row (`tile_b == tile_a + 1`), a north-south portal joins
/// row-adjacent tiles in the same column (`tile_b == tile_a + cols`). A
/// doorway in a wall running along one axis is only ever crossed by portals
/// of the *other* axis — proximity in 2D space alone cannot distinguish "this
/// portal actually crosses the doorway" from "this portal happens to sit
/// near the doorway point but lies entirely on one side of the wall."
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrossingAxis {
    /// Connects column-adjacent tiles in the same row — crosses a wall that
    /// runs north-south (vertical), such as a divider at a fixed x.
    EastWest,
    /// Connects row-adjacent tiles in the same column — crosses a wall that
    /// runs east-west (horizontal), such as a divider at a fixed y.
    NorthSouth,
}

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
    /// A door name can span more than one portal: a doorway wider than one
    /// tile after agent-radius inflation crosses the room divider through
    /// several adjacent tile rows, each with its own portal. Every portal
    /// resolved for a name is kept, ascending by `PortalId`, so closing a
    /// door by name closes the whole doorway rather than one crossing point
    /// of it.
    named: HashMap<String, Vec<PortalId>>,
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

        let push_portal =
            |a: u32, b: u32, portals: &mut Vec<Portal>, adjacency: &mut Vec<Vec<u32>>| {
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
            let (col, _row) = grid.col_row(tile);
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

    /// Every portal (open or closed) whose midpoint lies within `radius` of
    /// `point`, ascending by `PortalId` — a fixed, deterministic order
    /// regardless of portal creation order or hash-map iteration.
    ///
    /// A proximity radius alone cannot tell a portal that actually crosses a
    /// doorway from an ordinary in-room portal that merely happens to sit
    /// near the doorway point: for a doorway in a vertical wall, both the
    /// genuine crossing portals *and* nearby north-south portals fully on
    /// one side of the wall can have midpoints within any radius wide enough
    /// to span a multi-tile-row doorway. Prefer `portals_crossing` for
    /// named-door resolution; this method is kept for callers (and the tests
    /// below) that want raw proximity without axis filtering.
    pub fn portals_within(&self, point: Vec2, radius: f32) -> Vec<PortalId> {
        let radius_sq = radius * radius;
        let mut ids: Vec<PortalId> = self
            .portals
            .iter()
            .filter(|p| self.portal_midpoint(p.id).distance_squared(point) <= radius_sq)
            .map(|p| p.id)
            .collect();
        ids.sort();
        ids
    }

    /// A portal's crossing axis, derived from whether its two tiles differ
    /// in column (same row: east-west) or row (same column: north-south).
    pub fn portal_axis(&self, id: PortalId) -> CrossingAxis {
        let p = self.portal(id);
        let (col_a, row_a) = self.grid.col_row(p.tile_a);
        let (col_b, row_b) = self.grid.col_row(p.tile_b);
        if row_a == row_b {
            debug_assert_ne!(col_a, col_b, "portal connects a tile to itself");
            CrossingAxis::EastWest
        } else {
            debug_assert_eq!(col_a, col_b, "portal is neither east-west nor north-south");
            CrossingAxis::NorthSouth
        }
    }

    /// Whether a portal runs along `axis` and straddles `point`'s coordinate
    /// along that axis — its two tile centers lie on opposite sides of
    /// `point.x` (`EastWest`) or `point.y` (`NorthSouth`). This is the exact
    /// per-portal test for "does this portal actually cross the doorway
    /// authored at `point`" — it works because a named door's point is
    /// authored on the wall's centerline (e.g. `(divider_x, door_y)` for a
    /// doorway in a vertical wall at `x = divider_x`), so only a portal
    /// whose two tiles are on opposite sides of that line can straddle it.
    fn straddles(&self, id: PortalId, point: Vec2, axis: CrossingAxis) -> bool {
        if self.portal_axis(id) != axis {
            return false;
        }
        let p = self.portal(id);
        let a = self.grid.tile_center(p.tile_a);
        let b = self.grid.tile_center(p.tile_b);
        match axis {
            CrossingAxis::EastWest => (a.x - point.x) * (b.x - point.x) <= 0.0,
            CrossingAxis::NorthSouth => (a.y - point.y) * (b.y - point.y) <= 0.0,
        }
    }

    /// A portal's row (`EastWest`) or column (`NorthSouth`) index — the
    /// coordinate that varies as you walk along the wall the portal crosses,
    /// as opposed to the coordinate the portal crosses *through*. An
    /// `EastWest` portal connects same-row tiles, so its row index is fixed;
    /// a `NorthSouth` portal connects same-column tiles, so its column index
    /// is fixed.
    fn portal_lane(&self, id: PortalId) -> u32 {
        let p = self.portal(id);
        let (col, row) = self.grid.col_row(p.tile_a);
        match self.portal_axis(id) {
            CrossingAxis::EastWest => row,
            CrossingAxis::NorthSouth => col,
        }
    }

    /// Every portal that actually crosses the doorway named by `point` and
    /// `axis`, ascending by `PortalId`.
    ///
    /// A named door has no author-chosen radius: instead, every portal in
    /// the whole graph that (a) runs along `axis` and (b) straddles
    /// `point`'s coordinate along that axis is a *candidate* ("crosses the
    /// wall's line somewhere"), and then only the candidates in the
    /// connected run of lanes (rows for `EastWest`, columns for
    /// `NorthSouth`) immediately adjacent to the lane nearest `point` are
    /// kept — found by walking outward, one lane at a time, until a lane
    /// with no straddling portal is hit.
    ///
    /// This makes silent under-capture structurally impossible: a doorway
    /// gap in a wall is bounded on both sides by tiles the wall makes
    /// non-walkable, so no portal exists on those bounding lanes at all,
    /// which means the walk always stops exactly at the doorway's true
    /// edges — however many lanes wide the doorway is. It also can't
    /// over-capture into a *different* doorway further down the same wall
    /// line, because reaching it would require crossing at least one lane
    /// with no straddling portal (the solid wall between the two doorways),
    /// which halts the walk. A fixed Euclidean radius could not guarantee
    /// both properties at once: wide enough to span an arbitrarily wide
    /// doorway risks reaching a neighboring doorway on the same line; narrow
    /// enough to stay clear of a neighbor risks stopping short of a wide
    /// doorway's far edge. Walking the connected run has no such tradeoff.
    pub fn portals_crossing(&self, point: Vec2, axis: CrossingAxis) -> Vec<PortalId> {
        let mut by_lane: HashMap<u32, PortalId> = HashMap::new();
        for portal in &self.portals {
            if self.straddles(portal.id, point, axis) {
                by_lane.insert(self.portal_lane(portal.id), portal.id);
            }
        }
        if by_lane.is_empty() {
            return Vec::new();
        }

        // Seed lane: whichever straddling candidate's own tile-center
        // coordinate along the wall (y for EastWest, x for NorthSouth) is
        // nearest point's coordinate along that same direction.
        let perpendicular = |id: PortalId| -> f32 {
            let mid = self.portal_midpoint(id);
            match axis {
                CrossingAxis::EastWest => mid.y,
                CrossingAxis::NorthSouth => mid.x,
            }
        };
        let target = match axis {
            CrossingAxis::EastWest => point.y,
            CrossingAxis::NorthSouth => point.x,
        };
        let seed_lane = *by_lane
            .iter()
            .min_by(|(_, &a), (_, &b)| {
                (perpendicular(a) - target)
                    .abs()
                    .total_cmp(&(perpendicular(b) - target).abs())
            })
            .map(|(lane, _)| lane)
            .expect("by_lane is non-empty");

        let mut lanes = vec![seed_lane];
        let mut lane = seed_lane;
        while let Some(next) = lane.checked_sub(1) {
            if !by_lane.contains_key(&next) {
                break;
            }
            lanes.push(next);
            lane = next;
        }
        let mut lane = seed_lane;
        loop {
            let next = lane + 1;
            if !by_lane.contains_key(&next) {
                break;
            }
            lanes.push(next);
            lane = next;
        }

        let mut ids: Vec<PortalId> = lanes.into_iter().map(|l| by_lane[&l]).collect();
        ids.sort();
        ids
    }

    /// Name a set of portals as one door. Deduplicates and sorts so repeated
    /// or out-of-order input still yields a stable, comparable set.
    pub fn name_portals(&mut self, name: String, mut ids: Vec<PortalId>) {
        ids.sort();
        ids.dedup();
        self.named.insert(name, ids);
    }

    /// Every portal resolved for `name`, ascending by `PortalId`. Empty (not
    /// panicking) for an unknown name, matching `open_portals_of`'s style for
    /// an empty result.
    pub fn portals_named(&self, name: &str) -> &[PortalId] {
        self.named.get(name).map_or(&[], |ids| ids.as_slice())
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
        graph.name_portals("east_door".into(), graph.portals_within(midpoint, 0.01));
        assert_eq!(graph.portals_named("east_door"), &[id]);
        assert_eq!(graph.portals_named("no_such_door"), &[] as &[PortalId]);
    }

    #[test]
    fn portals_within_captures_every_portal_in_radius_deterministically() {
        // A 5x1 open strip has portals between every adjacent pair. A radius
        // wide enough to span two adjacent portal midpoints must return both,
        // sorted ascending by PortalId regardless of which one is nearer.
        let graph = TileGraph::build(TileGrid::build(
            Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(5.0, 1.0)),
            1.0,
            &[],
            0.3,
            &[],
        ));
        let a = graph.portal_between(1, 2).unwrap();
        let b = graph.portal_between(2, 3).unwrap();
        let center = graph.portal_midpoint(a);
        let wide = graph.portals_within(center, 1.5);
        assert!(wide.contains(&a));
        assert!(wide.contains(&b));
        let mut sorted = wide.clone();
        sorted.sort();
        assert_eq!(
            wide, sorted,
            "portals_within must return ascending PortalId order"
        );
    }

    /// A 12x12 grid (tile_size 0.5, agent_radius 0.3 — the same ratio
    /// `two_room` uses, so a wall at a column boundary blocks both tiles
    /// immediately either side of it) split by a vertical wall at x=3 with
    /// an open gap in `open_y`, everywhere else solid.
    fn vertical_divider_grid(open_y: (f32, f32)) -> TileGrid {
        let mut walls = Vec::new();
        if open_y.0 > 0.0 {
            walls.push(crate::geometry::Segment::new(
                Vec2::new(3.0, 0.0),
                Vec2::new(3.0, open_y.0),
            ));
        }
        if open_y.1 < 6.0 {
            walls.push(crate::geometry::Segment::new(
                Vec2::new(3.0, open_y.1),
                Vec2::new(3.0, 6.0),
            ));
        }
        TileGrid::build(
            Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(6.0, 6.0)),
            0.5,
            &walls,
            0.3,
            &[],
        )
    }

    /// Same layout, rotated 90 degrees: a horizontal wall at y=3 with an
    /// open gap in `open_x`.
    fn horizontal_divider_grid(open_x: (f32, f32)) -> TileGrid {
        let mut walls = Vec::new();
        if open_x.0 > 0.0 {
            walls.push(crate::geometry::Segment::new(
                Vec2::new(0.0, 3.0),
                Vec2::new(open_x.0, 3.0),
            ));
        }
        if open_x.1 < 6.0 {
            walls.push(crate::geometry::Segment::new(
                Vec2::new(open_x.1, 3.0),
                Vec2::new(6.0, 3.0),
            ));
        }
        TileGrid::build(
            Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(6.0, 6.0)),
            0.5,
            &walls,
            0.3,
            &[],
        )
    }

    #[test]
    fn portal_axis_classifies_east_west_and_north_south_correctly() {
        let graph = TileGraph::build(open_grid());
        // open_grid is 3 tiles wide, 1 tall: only east-west adjacency exists.
        let ew = graph.portal_between(0, 1).unwrap();
        assert_eq!(graph.portal_axis(ew), CrossingAxis::EastWest);

        // A taller grid adds a north (same-column) neighbor.
        let tall = TileGraph::build(TileGrid::build(
            Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(1.0, 2.0)),
            1.0,
            &[],
            0.3,
            &[],
        ));
        let ns = tall.portal_between(0, 1).unwrap();
        assert_eq!(tall.portal_axis(ns), CrossingAxis::NorthSouth);
    }

    #[test]
    fn portals_crossing_captures_a_north_south_axis_door() {
        // A horizontal wall at y=3 with a 2-column gap in x=[1,2] — the
        // NorthSouth-axis case, never exercised by any checked-in test
        // before this one (only `two_room`'s two EastWest doors were).
        let graph = TileGraph::build(horizontal_divider_grid((1.0, 2.0)));
        let door = Vec2::new(1.5, 3.0);
        let ids = graph.portals_crossing(door, CrossingAxis::NorthSouth);
        assert_eq!(
            ids.len(),
            2,
            "expected exactly 2 north-south crossings of the 2-column gap: {ids:?}"
        );
        let grid = graph.grid();
        for &id in &ids {
            assert_eq!(graph.portal_axis(id), CrossingAxis::NorthSouth);
            let p = graph.portal(id);
            let a = grid.tile_center(p.tile_a);
            let b = grid.tile_center(p.tile_b);
            assert!(
                (a.y < 3.0) != (b.y < 3.0),
                "portal {id:?} does not straddle y=3: a={a:?} b={b:?}"
            );
        }
    }

    #[test]
    fn portals_crossing_excludes_wrong_axis_and_non_straddling_portals() {
        // Vertical wall at x=3, gap in y=[2,4] (4 rows: tile rows 4..7 at
        // this grid's 0.5 m tile size, centers y=2.25..3.75).
        let graph = TileGraph::build(vertical_divider_grid((2.0, 4.0)));
        let grid = graph.grid();
        let cols = grid.cols();
        let door = Vec2::new(3.0, 3.0);
        let correct = graph.portals_crossing(door, CrossingAxis::EastWest);
        assert!(!correct.is_empty());
        for &id in &correct {
            let p = graph.portal(id);
            let a = grid.tile_center(p.tile_a);
            let b = grid.tile_center(p.tile_b);
            assert!(
                (a.x < 3.0) != (b.x < 3.0),
                "portal {id:?} returned by portals_crossing does not straddle x=3: a={a:?} b={b:?}"
            );
        }

        // A north-south portal sitting right at the doorway (same row range,
        // same columns straddling x=3) is on the wrong axis for this
        // east-west doorway and must not appear, even though it is exactly
        // at the door point.
        let col_at_wall = 5; // center x=2.75, immediately left of the wall
        let row_in_gap = 5; // center y=2.75, inside the open gap
        let tile = row_in_gap * cols + col_at_wall;
        let tile_north = tile + cols;
        let wrong_axis_portal = graph.portal_between(tile, tile_north).unwrap();
        assert_eq!(
            graph.portal_axis(wrong_axis_portal),
            CrossingAxis::NorthSouth
        );
        assert!(
            !correct.contains(&wrong_axis_portal),
            "a north-south portal at the doorway must not be captured by an \
             east-west door query: {wrong_axis_portal:?}"
        );

        // An east-west portal deep inside room A, in the same row range as
        // the doorway (so spatially close to the door point), but between
        // two tiles that are both left of x=3 — it never straddles the
        // wall, so it must be excluded even though it shares axis and row.
        let non_straddling_portal = graph
            .portal_between(row_in_gap * cols, row_in_gap * cols + 1)
            .unwrap();
        assert_eq!(
            graph.portal_axis(non_straddling_portal),
            CrossingAxis::EastWest
        );
        assert!(
            !correct.contains(&non_straddling_portal),
            "an east-west, non-straddling in-room portal must not be captured: \
             {non_straddling_portal:?}"
        );
    }

    #[test]
    fn portals_crossing_captures_an_arbitrarily_wide_doorway_without_a_radius() {
        // Regression for the under-capture gap: an 8-tile-row-wide doorway
        // (4 m at this grid's 0.5 m tile size) — wide enough that a
        // radius-based capture picked "generously" (e.g. the previous
        // round's half-width-plus-one-tile heuristic) would still miss some
        // of its crossings, exactly as a re-review demonstrated concretely
        // for a 4 m doorway with radius 1.3 (6 of 8 genuine crossings
        // captured, silently). `portals_crossing` takes no radius at all —
        // it walks the connected run of straddling portals outward until
        // the doorway's own bounding wall stops it — so under-capture here
        // is structurally impossible, not just unlikely.
        let graph = TileGraph::build(vertical_divider_grid((1.0, 5.0)));
        let door = Vec2::new(3.0, 3.0);
        let ids = graph.portals_crossing(door, CrossingAxis::EastWest);
        assert_eq!(
            ids.len(),
            8,
            "an 8-row-wide doorway must resolve to all 8 crossing portals, not a \
             radius-limited subset: got {} portals: {ids:?}",
            ids.len()
        );
    }

    #[test]
    fn portals_crossing_does_not_spill_into_a_neighboring_doorway() {
        // Two separate gaps in the same vertical wall line (x=3), like
        // two_room's north/south doors: y=[1,2] and y=[4,5], separated by a
        // solid stretch y=[2,4]. A door point named at the first gap's
        // center must resolve only to that gap's portals — the walk halts
        // at the solid wall between the two gaps.
        let walls = vec![
            crate::geometry::Segment::new(Vec2::new(3.0, 0.0), Vec2::new(3.0, 1.0)),
            crate::geometry::Segment::new(Vec2::new(3.0, 2.0), Vec2::new(3.0, 4.0)),
            crate::geometry::Segment::new(Vec2::new(3.0, 5.0), Vec2::new(3.0, 6.0)),
        ];
        let graph = TileGraph::build(TileGrid::build(
            Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(6.0, 6.0)),
            0.5,
            &walls,
            0.3,
            &[],
        ));
        let south = graph.portals_crossing(Vec2::new(3.0, 1.5), CrossingAxis::EastWest);
        let north = graph.portals_crossing(Vec2::new(3.0, 4.5), CrossingAxis::EastWest);
        assert!(!south.is_empty());
        assert!(!north.is_empty());
        assert!(
            south.iter().all(|s| !north.contains(s)),
            "the two doorways must not share a portal: south={south:?} north={north:?}"
        );
        let grid = graph.grid();
        for &id in &south {
            let p = graph.portal(id);
            let y = grid
                .tile_center(p.tile_a)
                .y
                .min(grid.tile_center(p.tile_b).y);
            assert!(
                y < 2.0,
                "south portal {id:?} leaked past the solid wall at y=2"
            );
        }
        for &id in &north {
            let p = graph.portal(id);
            let y = grid
                .tile_center(p.tile_a)
                .y
                .max(grid.tile_center(p.tile_b).y);
            assert!(
                y > 4.0,
                "north portal {id:?} leaked past the solid wall at y=4"
            );
        }
    }

    #[test]
    fn name_portals_deduplicates_and_sorts() {
        let mut graph = TileGraph::build(open_grid());
        let a = graph.portal_between(0, 1).unwrap();
        let b = graph.portal_between(1, 2).unwrap();
        graph.name_portals("both".into(), vec![b, a, a]);
        assert_eq!(graph.portals_named("both"), &[a, b]);
    }
}
