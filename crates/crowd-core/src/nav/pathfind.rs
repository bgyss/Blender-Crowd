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
            let step = graph
                .grid()
                .tile_center(tile)
                .distance_squared(graph.grid().tile_center(next))
                .sqrt()
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
    graph
        .grid()
        .tile_center(tile)
        .distance_squared(graph.grid().tile_center(goal))
        .sqrt()
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
    use crate::geometry::Segment;
    use crate::nav::grid::TileGrid;
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
        let from = graph
            .grid()
            .nearest_walkable_tile(Vec2::new(0.5, 1.5))
            .unwrap();
        let to = graph
            .grid()
            .nearest_walkable_tile(Vec2::new(5.5, 1.5))
            .unwrap();
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
