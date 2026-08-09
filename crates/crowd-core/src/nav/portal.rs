//! Portals: the tile-adjacency edges a topology change (a closed doorway)
//! removes or restores.

use std::collections::HashMap;

use crate::nav::grid::TileGrid;
use crate::units::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PortalId(pub u32);

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
    named: HashMap<String, PortalId>,
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
            let col = tile % cols;
            let _row = tile / cols;
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

    pub fn name_portal(&mut self, name: String, id: PortalId) {
        self.named.insert(name, id);
    }

    pub fn portal_named(&self, name: &str) -> Option<PortalId> {
        self.named.get(name).copied()
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
        graph.name_portal("east_door".into(), graph.nearest_portal(midpoint).unwrap());
        assert_eq!(graph.portal_named("east_door"), Some(id));
        assert_eq!(graph.portal_named("no_such_door"), None);
    }
}
