//! Tiled navmesh prototype (M0 item 4).
//!
//! See `docs/superpowers/specs/2026-08-08-tiled-navmesh-prototype-design.md`.

pub mod debug;
pub mod grid;
pub mod pathfind;
pub mod portal;

pub use debug::NavDebugSnapshot;
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
    /// Author-friendly portal lookup points, resolved to every `PortalId`
    /// within the given capture radius once the graph is built. E.g.
    /// `("north_door", Vec2::new(20.0, 20.0), 1.0)`.
    ///
    /// A doorway wider than one tile (after agent-radius inflation) crosses
    /// the dividing wall through more than one adjacent tile row, so it has
    /// more than one portal. The radius must be wide enough to capture every
    /// portal that actually spans the doorway (a good default is the
    /// doorway's half-width plus one tile size of margin) and narrow enough
    /// to stay clear of any other named door's portals.
    pub named_portals: Vec<(String, Vec2, f32)>,
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
        for (name, point, radius) in &self.named_portals {
            let ids = graph.portals_within(*point, *radius);
            graph.name_portals(name.clone(), ids);
        }
        graph
    }
}
