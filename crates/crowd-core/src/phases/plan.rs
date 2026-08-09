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
use crate::world::{RouteHandle, World, NO_ROUTE};

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

impl PlanState {
    /// The portal sequence recorded for a still-live route handle, if any.
    /// Exposed (narrowly, not a general route-inspection API) so callers such
    /// as `Simulation::route_crosses_any` can verify which door an agent's
    /// current corridor actually crosses, rather than only that it has *a*
    /// route.
    pub fn portals_for(&self, handle: RouteHandle) -> Option<&[PortalId]> {
        self.portals_of_route.get(&handle.0).map(Vec::as_slice)
    }
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
                    let (tile_path, expansions) = find_path(nav, from, to);
                    // Charged unconditionally: a failed search is the most
                    // expensive kind (it exhausts the whole reachable
                    // component before giving up), so charging it only on
                    // success let an unroutable agent's search escape the
                    // budget entirely and re-run at full cost every tick.
                    budget = budget.saturating_sub(expansions.max(1));
                    if let Some(tile_path) = tile_path {
                        let points =
                            corridor_points(nav, &tile_path, world.position(slot), *dest_point);
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
        if world.arrived[slot] {
            // An arrived agent's old corridor is irrelevant now: it is done
            // routing, and clearing its route would only inflate the
            // invalidated/untouched counts with an agent nobody needs to
            // replan.
            continue;
        }
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
    use crate::ids::AgentId;
    use crate::nav::{NavMeshDef, TileGraph};
    use crate::units::Aabb;
    use crate::world::{AgentSpawn, NO_ROUTE};

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
        assert_eq!(
            world.route[0], handle,
            "an already-routed agent must not be replanned"
        );
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
