//! Authored waypoint routing — a deliberate stand-in for the tiled navmesh.
//!
//! With analytic walls, straight-line steering to a goal deadlocks in the
//! corner beside a doorway, so agents need a global route before the navmesh
//! exists (contract section 6.1).
//!
//! The point is the interface, not the implementation. A route exposes exactly
//! one operation — given my position, what is the next steering target? — which
//! is precisely what a navmesh polygon corridor will implement. When real
//! navigation lands, it replaces this module and touches no agent state.

use crate::units::Vec2;
use crate::world::{RouteHandle, NO_ROUTE};

/// A small hand-authored navigation graph.
#[derive(Clone, Debug, Default)]
pub struct WaypointGraph {
    nodes: Vec<Vec2>,
    /// Adjacency, each inner list kept sorted so traversal order is fixed.
    adjacency: Vec<Vec<u32>>,
}

impl WaypointGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, p: Vec2) -> u32 {
        self.nodes.push(p);
        self.adjacency.push(Vec::new());
        (self.nodes.len() - 1) as u32
    }

    /// Add an undirected edge. Ignores duplicates and self-loops.
    pub fn add_edge(&mut self, a: u32, b: u32) {
        if a == b {
            return;
        }
        for (from, to) in [(a, b), (b, a)] {
            let list = &mut self.adjacency[from as usize];
            if let Err(insert_at) = list.binary_search(&to) {
                list.insert(insert_at, to);
            }
        }
    }

    pub fn node_count(&self) -> u32 {
        self.nodes.len() as u32
    }

    pub fn position(&self, node: u32) -> Vec2 {
        self.nodes[node as usize]
    }

    /// The nearest node to `p`, breaking exact ties by lower node index.
    pub fn nearest_node(&self, p: Vec2) -> Option<u32> {
        let mut best: Option<(f32, u32)> = None;
        for (index, node) in self.nodes.iter().enumerate() {
            let d = node.distance_squared(p);
            // Strict `<` means the first (lowest-index) node wins a tie.
            if best.is_none_or(|(best_d, _)| d < best_d) {
                best = Some((d, index as u32));
            }
        }
        best.map(|(_, index)| index)
    }

    /// Dijkstra over Euclidean edge lengths.
    ///
    /// Deliberately the O(V^2) scan rather than a binary heap: these graphs
    /// have tens of nodes, and a heap would need a total order over `f32`
    /// costs, which is exactly the kind of subtle ordering dependency the
    /// determinism contract forbids. The linear scan breaks ties by node
    /// index, which is unambiguous.
    pub fn shortest_path(&self, from: u32, to: u32) -> Option<Vec<u32>> {
        let n = self.nodes.len();
        if from as usize >= n || to as usize >= n {
            return None;
        }
        if from == to {
            return Some(vec![from]);
        }

        let mut dist = vec![f32::INFINITY; n];
        let mut prev = vec![u32::MAX; n];
        let mut visited = vec![false; n];
        dist[from as usize] = 0.0;

        loop {
            let mut current: Option<usize> = None;
            for i in 0..n {
                if visited[i] || !dist[i].is_finite() {
                    continue;
                }
                if current.is_none_or(|c| dist[i] < dist[c]) {
                    current = Some(i);
                }
            }
            let Some(current) = current else { break };
            if current == to as usize {
                break;
            }
            visited[current] = true;

            for &next in &self.adjacency[current] {
                let next = next as usize;
                if visited[next] {
                    continue;
                }
                let step = self.nodes[current]
                    .distance_squared(self.nodes[next])
                    .sqrt();
                let candidate = dist[current] + step;
                if candidate < dist[next] {
                    dist[next] = candidate;
                    prev[next] = current as u32;
                }
            }
        }

        if !dist[to as usize].is_finite() {
            return None;
        }

        let mut path = vec![to];
        let mut cursor = to;
        while cursor != from {
            cursor = prev[cursor as usize];
            debug_assert_ne!(cursor, u32::MAX, "reachable node must have a predecessor");
            path.push(cursor);
        }
        path.reverse();
        Some(path)
    }

    /// True when every node is reachable from node 0.
    ///
    /// Scene compilation rejects disconnected graphs, because an agent routed
    /// into an isolated component would stall forever with no diagnostic.
    pub fn is_connected(&self) -> bool {
        if self.nodes.is_empty() {
            return true;
        }
        let mut seen = vec![false; self.nodes.len()];
        let mut stack = vec![0usize];
        seen[0] = true;
        while let Some(current) = stack.pop() {
            for &next in &self.adjacency[current] {
                if !seen[next as usize] {
                    seen[next as usize] = true;
                    stack.push(next as usize);
                }
            }
        }
        seen.iter().all(|s| *s)
    }
}

/// Pooled storage for resolved routes.
#[derive(Clone, Debug, Default)]
pub struct RouteArena {
    points: Vec<Vec2>,
    start: Vec<u32>,
    len: Vec<u32>,
}

impl RouteArena {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_route(&mut self, points: &[Vec2]) -> RouteHandle {
        let handle = RouteHandle(self.start.len() as u32);
        self.start.push(self.points.len() as u32);
        self.len.push(points.len() as u32);
        self.points.extend_from_slice(points);
        handle
    }

    pub fn points(&self, handle: RouteHandle) -> &[Vec2] {
        if handle == NO_ROUTE || handle.0 as usize >= self.start.len() {
            return &[];
        }
        let start = self.start[handle.0 as usize] as usize;
        let len = self.len[handle.0 as usize] as usize;
        &self.points[start..start + len]
    }

    pub fn len(&self) -> usize {
        self.start.len()
    }

    pub fn is_empty(&self) -> bool {
        self.start.is_empty()
    }
}

/// The next steering target along a route, advancing `index` past any
/// waypoints already reached.
///
/// Returns `None` once the final waypoint is consumed, which the decide phase
/// reads as arrival. This signature is the contract a navmesh corridor will
/// inherit.
pub fn next_target(
    points: &[Vec2],
    index: &mut u16,
    pos: Vec2,
    arrive_radius: f32,
) -> Option<Vec2> {
    let arrive_sq = arrive_radius * arrive_radius;
    while (*index as usize) < points.len() {
        let i = *index as usize;
        let target = points[i];
        // Already closer to the next waypoint than to this one: the agent
        // overshot this waypoint (a large step, or waypoints colinear with
        // the path) and it counts as passed regardless of arrive_radius.
        if i + 1 < points.len()
            && points[i + 1].distance_squared(pos) < target.distance_squared(pos)
        {
            *index += 1;
            continue;
        }
        if target.distance_squared(pos) > arrive_sq {
            return Some(target);
        }
        *index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Vec2;

    /// 0 -- 1 -- 2  with a detour 0 -- 3 -- 2 that is longer.
    fn diamond() -> WaypointGraph {
        let mut g = WaypointGraph::new();
        let n0 = g.add_node(Vec2::new(0.0, 0.0));
        let n1 = g.add_node(Vec2::new(1.0, 0.0));
        let n2 = g.add_node(Vec2::new(2.0, 0.0));
        let n3 = g.add_node(Vec2::new(1.0, 5.0));
        g.add_edge(n0, n1);
        g.add_edge(n1, n2);
        g.add_edge(n0, n3);
        g.add_edge(n3, n2);
        g
    }

    #[test]
    fn shortest_path_prefers_the_shorter_route() {
        assert_eq!(diamond().shortest_path(0, 2), Some(vec![0, 1, 2]));
    }

    #[test]
    fn shortest_path_to_self_is_a_single_node() {
        assert_eq!(diamond().shortest_path(1, 1), Some(vec![1]));
    }

    #[test]
    fn shortest_path_returns_none_when_unreachable() {
        let mut g = diamond();
        let isolated = g.add_node(Vec2::new(99.0, 99.0));
        assert_eq!(g.shortest_path(0, isolated), None);
    }

    #[test]
    fn nearest_node_picks_the_closest_and_breaks_ties_by_index() {
        let g = diamond();
        assert_eq!(g.nearest_node(Vec2::new(1.9, 0.0)), Some(2));
        // Equidistant from nodes 0 and 1: lower index wins.
        assert_eq!(g.nearest_node(Vec2::new(0.5, 0.0)), Some(0));
    }

    #[test]
    fn is_connected_detects_an_isolated_node() {
        assert!(diamond().is_connected());
        let mut g = diamond();
        g.add_node(Vec2::new(99.0, 99.0));
        assert!(!g.is_connected());
    }

    #[test]
    fn empty_graph_has_no_nearest_node() {
        assert_eq!(WaypointGraph::new().nearest_node(Vec2::ZERO), None);
    }

    #[test]
    fn route_arena_round_trips_points() {
        let mut arena = RouteArena::new();
        let a = arena.push_route(&[Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0)]);
        let b = arena.push_route(&[Vec2::new(5.0, 5.0)]);
        assert_eq!(arena.points(a).len(), 2);
        assert_eq!(arena.points(b), &[Vec2::new(5.0, 5.0)]);
        assert_eq!(arena.len(), 2);
    }

    #[test]
    fn no_route_handle_yields_no_points() {
        let arena = RouteArena::new();
        assert!(arena.points(NO_ROUTE).is_empty());
    }

    #[test]
    fn next_target_returns_the_current_waypoint_when_far_away() {
        let points = [Vec2::new(10.0, 0.0), Vec2::new(20.0, 0.0)];
        let mut index = 0;
        assert_eq!(
            next_target(&points, &mut index, Vec2::ZERO, 0.5),
            Some(Vec2::new(10.0, 0.0))
        );
        assert_eq!(index, 0);
    }

    #[test]
    fn next_target_advances_when_the_waypoint_is_reached() {
        let points = [Vec2::new(10.0, 0.0), Vec2::new(20.0, 0.0)];
        let mut index = 0;
        let target = next_target(&points, &mut index, Vec2::new(10.1, 0.0), 0.5);
        assert_eq!(target, Some(Vec2::new(20.0, 0.0)));
        assert_eq!(index, 1);
    }

    #[test]
    fn next_target_skips_multiple_waypoints_in_one_call() {
        let points = [
            Vec2::new(1.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(9.0, 0.0),
        ];
        let mut index = 0;
        let target = next_target(&points, &mut index, Vec2::new(2.0, 0.0), 0.5);
        assert_eq!(target, Some(Vec2::new(9.0, 0.0)));
        assert_eq!(index, 2);
    }

    #[test]
    fn next_target_reports_none_after_the_final_waypoint() {
        let points = [Vec2::new(1.0, 0.0)];
        let mut index = 0;
        assert_eq!(
            next_target(&points, &mut index, Vec2::new(1.0, 0.0), 0.5),
            None
        );
    }

    #[test]
    fn next_target_on_an_empty_route_is_none() {
        let mut index = 0;
        assert_eq!(next_target(&[], &mut index, Vec2::ZERO, 0.5), None);
    }
}
