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
pub use portal::{CrossingAxis, Portal, PortalId, TileGraph};

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
    /// that actually crosses the named doorway once the graph is built. E.g.
    /// `("north_door", Vec2::new(20.0, 20.0), CrossingAxis::EastWest)`.
    ///
    /// A doorway wider than one tile (after agent-radius inflation) crosses
    /// the dividing wall through more than one adjacent tile row, so it has
    /// more than one portal. There is no radius to tune: `portals_crossing`
    /// walks the connected run of straddling portals outward from `point`
    /// until it runs out of doorway (a lane with no straddling portal, which
    /// only happens where the wall is solid), so it captures exactly the
    /// doorway's full width regardless of how many tile rows/columns it
    /// spans, and never spills into a different doorway further down the
    /// same wall line. `CrossingAxis` picks which portal orientation can
    /// cross this wall: `EastWest` for a doorway in a wall that runs
    /// north-south (constant x), `NorthSouth` for a doorway in a wall that
    /// runs east-west (constant y). Author the point on the wall's
    /// centerline (e.g. `(divider_x, door_y)`) so the straddle check works.
    pub named_portals: Vec<(String, Vec2, CrossingAxis)>,
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
        for (name, point, axis) in &self.named_portals {
            let ids = graph.portals_crossing(*point, *axis);
            graph.name_portals(name.clone(), ids);
        }
        graph
    }
}
