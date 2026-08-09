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
use crate::units::{Aabb, Vec2};

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
