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

/// How far ahead along the corridor to aim, in metres.
///
/// Long enough that agents commit to the lane rather than oscillating toward
/// a point under their feet; short enough that they still track a turn.
const LOOKAHEAD: f32 = 2.0;

/// Fraction of an agent's offset from the corridor centreline corrected per
/// steering query. Small on purpose — see `next_target`.
const CENTRELINE_PULL: f32 = 0.35;

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

    /// Adjacent nodes, ascending. Empty for an unknown index.
    ///
    /// Exposed so the scene hash can cover topology: two graphs with identical
    /// node positions but different edges route differently.
    pub fn neighbors(&self, node: u32) -> &[u32] {
        self.adjacency
            .get(node as usize)
            .map_or(&[], |list| list.as_slice())
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

/// The next steering target along a route.
///
/// Returns `None` once the agent reaches the final waypoint, which the decide
/// phase reads as arrival. This signature is the contract a navmesh corridor
/// will inherit.
///
/// # Corridor-following, not node-chasing
///
/// The target is a point a lookahead distance *along the polyline*, measured
/// from the agent's own projection onto it — not the next node.
///
/// Steering at the node itself makes every waypoint a mandatory pass-through
/// point: a population spread across a wide corridor all converges on one
/// spot, and since a crowd cannot fit inside an arrival radius, it jams there
/// permanently. Projecting instead lets agents spread across the corridor
/// converge onto the *line* and flow along it, which is both what a real
/// crowd does and what a navmesh polygon corridor computes.
pub fn next_target(
    points: &[Vec2],
    index: &mut u16,
    pos: Vec2,
    arrive_radius: f32,
) -> Option<Vec2> {
    if points.is_empty() {
        return None;
    }
    let last = points.len() - 1;

    // Consume any leg the agent's projection has already run off the end of.
    while (*index as usize) < last {
        let i = *index as usize;
        let along = points[i + 1] - points[i];
        let len_sq = along.length_squared();
        if len_sq <= f32::MIN_POSITIVE {
            *index += 1;
            continue;
        }
        if (pos - points[i]).dot(along) / len_sq >= 1.0 {
            *index += 1;
        } else {
            break;
        }
    }

    let i = *index as usize;
    if i >= last {
        // Final waypoint is a destination, not a corridor: steer at it
        // directly and report arrival inside the radius.
        let goal = points[last];
        return if goal.distance_squared(pos) <= arrive_radius * arrive_radius {
            None
        } else {
            Some(goal)
        };
    }

    let a = points[i];
    let along = points[i + 1] - a;
    let len = along.length();
    let direction = along.normalize_or_zero();
    let projected = ((pos - a).dot(along) / (len * len)).clamp(0.0, 1.0) * len;
    let ahead = (projected + LOOKAHEAD).min(len);

    // Keep most of the agent's offset from the centreline. Aiming everyone at
    // the centre would collapse a whole population onto one line: two
    // opposing flows would then meet head-on in a single lane and jam solid,
    // which is exactly what real crowds avoid by forming lanes. Correcting
    // only a fraction per query lets agents hold a lane while still
    // converging on the corridor over a few seconds.
    // The perpendicular rejection specifically. Using `pos - (a + dir *
    // projected)` would fold the along-axis component back in whenever the
    // projection clamps at a leg's end, aiming a trailing agent backwards.
    let offset = pos - a;
    let lateral = offset - direction * offset.dot(direction);
    Some(a + direction * ahead + lateral * (1.0 - CENTRELINE_PULL))
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
    fn the_target_is_a_point_ahead_along_the_corridor_not_the_node() {
        // Node-chasing would return (10,0) and make every agent converge on
        // that one spot. Corridor-following aims a lookahead further on.
        let points = [Vec2::new(10.0, 0.0), Vec2::new(20.0, 0.0)];
        let mut index = 0;
        let target = next_target(&points, &mut index, Vec2::ZERO, 0.5).unwrap();
        assert!(target.x > 10.0, "aimed at the node itself: {target:?}");
        assert!((target.y).abs() < 1e-5);
        assert_eq!(index, 0);
    }

    #[test]
    fn an_agent_beside_the_corridor_converges_without_abandoning_its_lane() {
        // Two competing requirements. Agents must converge on the corridor,
        // or they drift into walls. But they must NOT all be aimed at the
        // centreline, because two opposing flows would then meet head-on in
        // a single lane instead of forming lanes either side of it.
        let points = [Vec2::new(0.0, 0.0), Vec2::new(20.0, 0.0)];
        let mut high_index = 0;
        let mut low_index = 0;
        let high = next_target(&points, &mut high_index, Vec2::new(5.0, 3.0), 0.6).unwrap();
        let low = next_target(&points, &mut low_index, Vec2::new(5.0, -3.0), 0.6).unwrap();

        assert!(high.x > 5.0, "target must lead the agent forward: {high:?}");
        assert!(high.y < 3.0 && high.y > 0.0, "did not converge: {high:?}");
        assert!(low.y > -3.0 && low.y < 0.0, "did not converge: {low:?}");
        assert!(
            high.y > low.y,
            "agents on opposite sides were collapsed onto one line"
        );
    }

    #[test]
    fn the_target_advances_as_the_agent_moves_along_the_corridor() {
        let points = [Vec2::new(0.0, 0.0), Vec2::new(20.0, 0.0)];
        let mut index = 0;
        let early = next_target(&points, &mut index, Vec2::new(2.0, 0.0), 0.6).unwrap();
        let later = next_target(&points, &mut index, Vec2::new(9.0, 0.0), 0.6).unwrap();
        assert!(later.x > early.x, "{early:?} then {later:?}");
    }

    #[test]
    fn the_target_never_runs_past_the_end_of_a_leg() {
        let points = [
            Vec2::new(0.0, 0.0),
            Vec2::new(5.0, 0.0),
            Vec2::new(5.0, 20.0),
        ];
        let mut index = 0;
        let target = next_target(&points, &mut index, Vec2::new(4.5, 0.0), 0.6).unwrap();
        assert!(target.x <= 5.0 + 1e-5, "overshot the corner: {target:?}");
    }

    #[test]
    fn passing_the_end_of_a_leg_advances_to_the_next() {
        let points = [
            Vec2::new(1.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(9.0, 0.0),
        ];
        let mut index = 0;
        next_target(&points, &mut index, Vec2::new(2.0, 0.0), 0.5);
        assert_eq!(index, 1, "leg 0 was fully traversed");
    }

    #[test]
    fn the_final_waypoint_is_targeted_directly() {
        // A destination is a point, not a corridor, so the agent aims at it.
        let points = [Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)];
        let mut index = 1;
        let target = next_target(&points, &mut index, Vec2::new(9.0, 0.0), 0.5);
        assert_eq!(target, Some(Vec2::new(10.0, 0.0)));
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

    #[test]
    fn an_agent_off_the_path_does_not_skip_a_corner_waypoint() {
        // An L-shaped route whose corner exists to steer around a wall. An
        // agent that has not traversed the first leg must be drawn back onto
        // that leg, never sent straight at the leg beyond the corner.
        let points = [
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 10.0),
            Vec2::new(10.0, 10.0),
        ];
        let mut index = 0;
        let target = next_target(&points, &mut index, Vec2::new(6.0, 5.0), 0.6).unwrap();
        assert_eq!(index, 0, "advanced past a leg it had not traversed");
        assert!(
            target.y > 5.0,
            "not progressing along the first leg: {target:?}"
        );
        assert!(
            target.x < 6.0,
            "not converging back toward the first leg: {target:?}"
        );
    }

    #[test]
    fn an_agent_past_the_corner_follows_the_next_leg() {
        // The other side of the same rule: once the first leg really is
        // behind the agent, walking back to the corner would be absurd.
        let points = [
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 10.0),
            Vec2::new(10.0, 10.0),
        ];
        let mut index = 0;
        let target = next_target(&points, &mut index, Vec2::new(1.0, 10.5), 0.6).unwrap();
        assert_eq!(index, 1);
        assert!(target.x > 1.0, "did not follow the second leg: {target:?}");
    }
}
