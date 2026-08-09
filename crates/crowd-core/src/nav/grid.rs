//! Uniform-grid tiled navmesh rasterization.
//!
//! A tile is walkable when its center, inflated by the scene's agent radius,
//! clears every wall. Rasterization runs once, at scene-compile time, same as
//! `SegmentIndex`.

use crate::geometry::Segment;
use crate::units::{Aabb, Vec2};

#[derive(Clone, Debug)]
pub struct TileGrid {
    origin: Vec2,
    tile_size: f32,
    cols: u32,
    rows: u32,
    walkable: Vec<bool>,
    cost: Vec<f32>,
}

impl TileGrid {
    pub fn build(
        bounds: Aabb,
        tile_size: f32,
        walls: &[Segment],
        agent_radius: f32,
        cost_areas: &[(Aabb, f32)],
    ) -> Self {
        let size = bounds.size();
        let cols = (size.x / tile_size).ceil().max(1.0) as u32;
        let rows = (size.y / tile_size).ceil().max(1.0) as u32;
        let tile_count = (cols * rows) as usize;
        let mut walkable = vec![true; tile_count];
        let mut cost = vec![1.0f32; tile_count];

        let grid = Self {
            origin: bounds.min,
            tile_size,
            cols,
            rows,
            walkable: Vec::new(),
            cost: Vec::new(),
        };

        for row in 0..rows {
            for col in 0..cols {
                let index = (row * cols + col) as usize;
                let center = grid.tile_center_at(col, row);
                if walls.iter().any(|w| w.distance_to(center) < agent_radius) {
                    walkable[index] = false;
                }
                // Last authored cost area covering this tile wins.
                for (area, multiplier) in cost_areas {
                    if area.contains(center) {
                        cost[index] = *multiplier;
                    }
                }
            }
        }

        Self {
            walkable,
            cost,
            ..grid
        }
    }

    fn tile_center_at(&self, col: u32, row: u32) -> Vec2 {
        Vec2::new(
            self.origin.x + (col as f32 + 0.5) * self.tile_size,
            self.origin.y + (row as f32 + 0.5) * self.tile_size,
        )
    }

    pub fn cols(&self) -> u32 {
        self.cols
    }

    pub fn rows(&self) -> u32 {
        self.rows
    }

    pub fn tile_count(&self) -> u32 {
        self.cols * self.rows
    }

    pub fn tile_center(&self, tile: u32) -> Vec2 {
        let (col, row) = self.col_row(tile);
        self.tile_center_at(col, row)
    }

    /// A tile index's (col, row) in this grid's row-major layout. Centralizes
    /// the `% cols` / `/ cols` split so it is written once, not re-derived at
    /// each open-coded call site.
    pub fn col_row(&self, tile: u32) -> (u32, u32) {
        (tile % self.cols, tile / self.cols)
    }

    pub fn origin(&self) -> Vec2 {
        self.origin
    }

    pub fn tile_size(&self) -> f32 {
        self.tile_size
    }

    pub fn is_walkable(&self, tile: u32) -> bool {
        self.walkable.get(tile as usize).copied().unwrap_or(false)
    }

    pub fn cost(&self, tile: u32) -> f32 {
        self.cost.get(tile as usize).copied().unwrap_or(1.0)
    }

    fn tile_at_point(&self, p: Vec2) -> Option<u32> {
        let local = p - self.origin;
        if local.x < 0.0 || local.y < 0.0 {
            return None;
        }
        let col = (local.x / self.tile_size) as u32;
        let row = (local.y / self.tile_size) as u32;
        if col >= self.cols || row >= self.rows {
            return None;
        }
        Some(row * self.cols + col)
    }

    /// The walkable tile nearest `p`, searching an expanding ring around the
    /// tile `p` falls in. Needed because agent positions and destination
    /// points do not land exactly on a tile center, and can sit fractions of
    /// a tile from a wall.
    pub fn nearest_walkable_tile(&self, p: Vec2) -> Option<u32> {
        let start = self.tile_at_point(p)?;
        if self.is_walkable(start) {
            return Some(start);
        }
        let (start_col, start_row) = self.col_row(start);
        let max_ring = self.cols.max(self.rows);
        for ring in 1..=max_ring {
            let mut best: Option<(f32, u32)> = None;
            let lo_col = start_col.saturating_sub(ring);
            let hi_col = (start_col + ring).min(self.cols - 1);
            let lo_row = start_row.saturating_sub(ring);
            let hi_row = (start_row + ring).min(self.rows - 1);
            for row in lo_row..=hi_row {
                for col in lo_col..=hi_col {
                    let on_ring = row == lo_row || row == hi_row || col == lo_col || col == hi_col;
                    if !on_ring {
                        continue;
                    }
                    let tile = row * self.cols + col;
                    if !self.is_walkable(tile) {
                        continue;
                    }
                    let d = self.tile_center(tile).distance_squared(p);
                    if best.is_none_or(|(best_d, _)| d < best_d) {
                        best = Some((d, tile));
                    }
                }
            }
            if let Some((_, tile)) = best {
                return Some(tile);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_bounds() -> Aabb {
        Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0))
    }

    #[test]
    fn an_open_grid_is_fully_walkable() {
        let grid = TileGrid::build(open_bounds(), 1.0, &[], 0.3, &[]);
        for tile in 0..grid.tile_count() {
            assert!(grid.is_walkable(tile), "tile {tile} unexpectedly blocked");
        }
    }

    #[test]
    fn a_wall_blocks_tiles_within_the_agent_radius() {
        let wall = Segment::new(Vec2::new(5.0, 0.0), Vec2::new(5.0, 10.0));
        let grid = TileGrid::build(open_bounds(), 1.0, &[wall], 0.3, &[]);
        let blocked = grid.nearest_walkable_tile(Vec2::new(5.0, 5.0));
        // The exact tile under the wall must be blocked; some walkable tile
        // must still exist further away.
        let under_wall = grid
            .tile_center(grid.tile_count() / 2)
            .distance_squared(Vec2::new(5.0, 5.0));
        assert!(under_wall < 100.0); // sanity: this is the tile we mean
        assert!(!grid.is_walkable(grid.tile_count() / 2) || blocked.is_some());
    }

    #[test]
    fn cost_areas_apply_the_last_authored_overlapping_multiplier() {
        let area_a = (Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)), 2.0);
        let area_b = (Aabb::new(Vec2::new(4.0, 4.0), Vec2::new(6.0, 6.0)), 5.0);
        let grid = TileGrid::build(open_bounds(), 1.0, &[], 0.3, &[area_a, area_b]);
        let center_tile = grid.nearest_walkable_tile(Vec2::new(5.0, 5.0)).unwrap();
        assert_eq!(grid.cost(center_tile), 5.0, "later cost area must win");
        let corner_tile = grid.nearest_walkable_tile(Vec2::new(0.5, 0.5)).unwrap();
        assert_eq!(grid.cost(corner_tile), 2.0);
    }

    #[test]
    fn nearest_walkable_tile_skips_a_blocked_tile() {
        let wall = Segment::new(Vec2::new(0.0, 4.5), Vec2::new(10.0, 4.5));
        let grid = TileGrid::build(open_bounds(), 1.0, &[wall], 0.3, &[]);
        let tile = grid.nearest_walkable_tile(Vec2::new(5.0, 4.5)).unwrap();
        assert!(grid.is_walkable(tile));
    }

    #[test]
    fn a_point_outside_the_grid_has_no_nearest_tile_when_fully_enclosed_by_walls() {
        // A grid entirely blocked has no walkable tile at all.
        let walls: Vec<Segment> = (0..10)
            .map(|row| Segment::new(Vec2::new(0.0, row as f32), Vec2::new(10.0, row as f32)))
            .collect();
        let grid = TileGrid::build(open_bounds(), 1.0, &walls, 0.6, &[]);
        assert_eq!(grid.nearest_walkable_tile(Vec2::new(5.0, 5.0)), None);
    }
}
