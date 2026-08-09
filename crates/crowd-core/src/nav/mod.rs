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
    /// within the given capture radius *and* sharing the given crossing
    /// axis, once the graph is built. E.g.
    /// `("north_door", Vec2::new(20.0, 20.0), 1.0, CrossingAxis::EastWest)`.
    ///
    /// A doorway wider than one tile (after agent-radius inflation) crosses
    /// the dividing wall through more than one adjacent tile row, so it has
    /// more than one portal. The radius must be wide enough to capture every
    /// portal that actually spans the doorway (a good default is the
    /// doorway's half-width plus one tile size of margin) — but proximity
    /// alone is not sufficient: a radius wide enough to span a multi-tile
    /// doorway can also reach ordinary in-room portals that merely sit near
    /// the doorway point without ever crossing the wall (e.g. north-south
    /// portals near a doorway in a vertical wall, both fully on one side of
    /// the divider, or even an east-west portal entirely inside one room
    /// close to the wall). `CrossingAxis` filters the radius-based
    /// candidates down to only the portals whose orientation is
    /// perpendicular to the wall the door sits in, and `portals_within_axis`
    /// additionally requires the doorway point to sit *between* the
    /// portal's two tile centers along that axis — i.e. the portal's tiles
    /// are genuinely on opposite sides of the wall, not merely nearby it.
    /// Pick `EastWest` for a doorway in a wall that runs north-south
    /// (constant x), `NorthSouth` for a doorway in a wall that runs
    /// east-west (constant y), and author the point on the wall's
    /// centerline (e.g. `(divider_x, door_y)`) so the straddle check works.
    pub named_portals: Vec<(String, Vec2, f32, CrossingAxis)>,
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
        for (name, point, radius, axis) in &self.named_portals {
            let ids = graph.portals_within_axis(*point, *radius, *axis);
            graph.name_portals(name.clone(), ids);
        }
        graph
    }
}
