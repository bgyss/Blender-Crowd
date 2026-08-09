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
    use crate::ids::AgentId;
    use crate::nav::NavMeshDef;
    use crate::units::Aabb;
    use crate::world::{AgentSpawn, NO_ROUTE};

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
