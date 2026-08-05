//! Uniform grid spatial index.
//!
//! Rebuilt every tick by counting sort. Counting sort is chosen deliberately:
//! it is O(n), allocates into reused buffers, and — unlike a hash map —
//! produces one canonical ordering, so neighbor lists never depend on
//! insertion history.

use crate::geometry::Segment;
use crate::units::{Aabb, Vec2};

/// Agent broad-phase index, rebuilt each tick.
#[derive(Clone, Debug)]
pub struct UniformGrid {
    bounds: Aabb,
    cell_size: f32,
    inv_cell_size: f32,
    cols: u32,
    rows: u32,
    /// Prefix sums, length `cols * rows + 1`.
    cell_start: Vec<u32>,
    /// Agent slots grouped by cell, ascending within each cell.
    items: Vec<u32>,
    counts: Vec<u32>,
    /// Per-cell write cursor for the fill pass. A field rather than a local
    /// because `rebuild` runs every tick, and a local would heap-allocate on
    /// every one of them.
    cursor: Vec<u32>,
}

impl UniformGrid {
    pub fn new(bounds: Aabb, cell_size: f32) -> Self {
        assert!(cell_size > 0.0, "cell_size must be positive");
        let size = bounds.size();
        let cols = ((size.x / cell_size).ceil() as u32).max(1);
        let rows = ((size.y / cell_size).ceil() as u32).max(1);
        Self {
            bounds,
            cell_size,
            inv_cell_size: 1.0 / cell_size,
            cols,
            rows,
            cell_start: vec![0; (cols * rows + 1) as usize],
            items: Vec::new(),
            counts: vec![0; (cols * rows) as usize],
            cursor: vec![0; (cols * rows) as usize],
        }
    }

    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    /// Cell coordinates for a world position, clamped to the grid.
    ///
    /// Clamping rather than rejecting matters: an agent that escapes the
    /// bounds through a solver failure must stay findable, or it becomes
    /// invisible to every other agent and the failure compounds silently.
    fn cell_of(&self, x: f32, y: f32) -> (u32, u32) {
        let cx = ((x - self.bounds.min.x) * self.inv_cell_size).floor();
        let cy = ((y - self.bounds.min.y) * self.inv_cell_size).floor();
        let cx = if cx.is_finite() { cx } else { 0.0 };
        let cy = if cy.is_finite() { cy } else { 0.0 };
        (
            (cx.max(0.0) as u32).min(self.cols - 1),
            (cy.max(0.0) as u32).min(self.rows - 1),
        )
    }

    fn cell_index(&self, cx: u32, cy: u32) -> usize {
        (cy * self.cols + cx) as usize
    }

    pub fn rebuild(&mut self, pos_x: &[f32], pos_y: &[f32]) {
        debug_assert_eq!(pos_x.len(), pos_y.len());
        let n = pos_x.len();

        self.counts.iter_mut().for_each(|c| *c = 0);
        for i in 0..n {
            let (cx, cy) = self.cell_of(pos_x[i], pos_y[i]);
            let cell = self.cell_index(cx, cy);
            self.counts[cell] += 1;
        }

        let mut running = 0u32;
        for (cell, count) in self.counts.iter().enumerate() {
            self.cell_start[cell] = running;
            running += count;
        }
        *self.cell_start.last_mut().expect("non-empty prefix sums") = running;

        // Second pass fills each cell in ascending slot order, because slots
        // are visited in ascending order and each cell's cursor advances
        // monotonically. That ordering is what makes queries reproducible.
        self.items.clear();
        self.items.resize(n, 0);
        // Refill the cursor in place. `to_vec()` here would allocate on every
        // tick, which is exactly what this index is built to avoid.
        self.cursor.clear();
        self.cursor
            .extend_from_slice(&self.cell_start[..self.counts.len()]);
        for i in 0..n {
            let (cx, cy) = self.cell_of(pos_x[i], pos_y[i]);
            let cell = self.cell_index(cx, cy);
            self.items[self.cursor[cell] as usize] = i as u32;
            self.cursor[cell] += 1;
        }
    }

    /// Broad-phase query: append every slot in a cell overlapping the circle.
    ///
    /// Callers must still filter by exact distance.
    pub fn query(&self, center: Vec2, radius: f32, out: &mut Vec<u32>) {
        out.clear();
        let (min_cx, min_cy) = self.cell_of(center.x - radius, center.y - radius);
        let (max_cx, max_cy) = self.cell_of(center.x + radius, center.y + radius);
        for cy in min_cy..=max_cy {
            for cx in min_cx..=max_cx {
                let cell = self.cell_index(cx, cy);
                let start = self.cell_start[cell] as usize;
                let end = self.cell_start[cell + 1] as usize;
                out.extend_from_slice(&self.items[start..end]);
            }
        }
    }
}

/// Static wall broad-phase, built once at scene compile time.
#[derive(Clone, Debug)]
pub struct SegmentIndex {
    grid: UniformGrid,
    cell_items: Vec<Vec<u32>>,
}

impl SegmentIndex {
    pub fn build(bounds: Aabb, cell_size: f32, segments: &[Segment]) -> Self {
        let grid = UniformGrid::new(bounds, cell_size);
        let mut cell_items = vec![Vec::new(); (grid.cols * grid.rows) as usize];
        for (index, seg) in segments.iter().enumerate() {
            let b = seg.bounds();
            let (min_cx, min_cy) = grid.cell_of(b.min.x, b.min.y);
            let (max_cx, max_cy) = grid.cell_of(b.max.x, b.max.y);
            for cy in min_cy..=max_cy {
                for cx in min_cx..=max_cx {
                    cell_items[grid.cell_index(cx, cy)].push(index as u32);
                }
            }
        }
        Self { grid, cell_items }
    }

    /// Append the indices of walls near `center`, each at most once.
    pub fn query(&self, center: Vec2, radius: f32, out: &mut Vec<u32>) {
        out.clear();
        let (min_cx, min_cy) = self.grid.cell_of(center.x - radius, center.y - radius);
        let (max_cx, max_cy) = self.grid.cell_of(center.x + radius, center.y + radius);
        for cy in min_cy..=max_cy {
            for cx in min_cx..=max_cx {
                out.extend_from_slice(&self.cell_items[self.grid.cell_index(cx, cy)]);
            }
        }
        // A long wall spans many cells, so dedupe. Sorting first keeps the
        // result in ascending index order, which callers rely on.
        out.sort_unstable();
        out.dedup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Segment;
    use crate::units::{Aabb, Vec2};

    fn test_bounds() -> Aabb {
        Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0))
    }

    #[test]
    fn query_finds_a_point_at_the_query_centre() {
        let mut grid = UniformGrid::new(test_bounds(), 1.0);
        grid.rebuild(&[5.0], &[5.0]);
        let mut out = Vec::new();
        grid.query(Vec2::new(5.0, 5.0), 0.5, &mut out);
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn query_excludes_points_far_outside_the_radius() {
        let mut grid = UniformGrid::new(test_bounds(), 1.0);
        grid.rebuild(&[1.0, 9.0], &[1.0, 9.0]);
        let mut out = Vec::new();
        grid.query(Vec2::new(1.0, 1.0), 0.5, &mut out);
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn query_clears_the_output_buffer_first() {
        let mut grid = UniformGrid::new(test_bounds(), 1.0);
        grid.rebuild(&[5.0], &[5.0]);
        let mut out = vec![999, 998];
        grid.query(Vec2::new(5.0, 5.0), 0.5, &mut out);
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn positions_outside_bounds_are_clamped_not_dropped() {
        let mut grid = UniformGrid::new(test_bounds(), 1.0);
        grid.rebuild(&[-50.0], &[-50.0]);
        let mut out = Vec::new();
        grid.query(Vec2::new(0.0, 0.0), 1.0, &mut out);
        assert_eq!(out, vec![0], "an escaped agent must remain findable");
    }

    #[test]
    fn broad_phase_is_a_superset_of_brute_force() {
        let xs: Vec<f32> = (0..200).map(|i| (i % 20) as f32 * 0.5).collect();
        let ys: Vec<f32> = (0..200).map(|i| (i / 20) as f32 * 0.5).collect();
        let mut grid = UniformGrid::new(test_bounds(), 1.0);
        grid.rebuild(&xs, &ys);

        let centre = Vec2::new(4.0, 4.0);
        let radius = 2.0;
        let mut out = Vec::new();
        grid.query(centre, radius, &mut out);

        for i in 0..xs.len() {
            let d = Vec2::new(xs[i], ys[i]).distance_squared(centre);
            if d <= radius * radius {
                assert!(out.contains(&(i as u32)), "grid missed slot {i}");
            }
        }
    }

    #[test]
    fn rebuild_produces_ascending_slot_order_within_a_query() {
        let xs: Vec<f32> = vec![5.0; 50];
        let ys: Vec<f32> = vec![5.0; 50];
        let mut grid = UniformGrid::new(test_bounds(), 1.0);
        grid.rebuild(&xs, &ys);
        let mut out = Vec::new();
        grid.query(Vec2::new(5.0, 5.0), 0.5, &mut out);
        let mut sorted = out.clone();
        sorted.sort_unstable();
        assert_eq!(out, sorted, "counting sort must preserve slot order");
    }

    #[test]
    fn rebuild_is_deterministic_across_runs() {
        let xs: Vec<f32> = (0..500).map(|i| (i % 37) as f32 * 0.25).collect();
        let ys: Vec<f32> = (0..500).map(|i| (i % 23) as f32 * 0.4).collect();
        let mut a = UniformGrid::new(test_bounds(), 1.0);
        let mut b = UniformGrid::new(test_bounds(), 1.0);
        a.rebuild(&xs, &ys);
        b.rebuild(&xs, &ys);
        let (mut oa, mut ob) = (Vec::new(), Vec::new());
        a.query(Vec2::new(3.0, 3.0), 2.0, &mut oa);
        b.query(Vec2::new(3.0, 3.0), 2.0, &mut ob);
        assert_eq!(oa, ob);
    }

    #[test]
    fn rebuild_does_not_allocate_once_warmed_up() {
        // `rebuild` runs every tick. If any buffer reallocates on a steady
        // agent count, the tick loop is allocating in its hot path.
        let xs: Vec<f32> = (0..300).map(|i| (i % 17) as f32 * 0.5).collect();
        let ys: Vec<f32> = (0..300).map(|i| (i % 13) as f32 * 0.6).collect();
        let mut grid = UniformGrid::new(test_bounds(), 1.0);
        grid.rebuild(&xs, &ys);

        let warm = (
            grid.items.capacity(),
            grid.cursor.capacity(),
            grid.counts.capacity(),
            grid.cell_start.capacity(),
        );
        for _ in 0..50 {
            grid.rebuild(&xs, &ys);
        }
        assert_eq!(
            (
                grid.items.capacity(),
                grid.cursor.capacity(),
                grid.counts.capacity(),
                grid.cell_start.capacity(),
            ),
            warm,
            "a buffer reallocated during steady-state rebuild"
        );
    }

    #[test]
    fn segment_index_finds_a_nearby_wall() {
        let walls = vec![
            Segment::new(Vec2::new(2.0, 0.0), Vec2::new(2.0, 10.0)),
            Segment::new(Vec2::new(9.0, 0.0), Vec2::new(9.0, 10.0)),
        ];
        let index = SegmentIndex::build(test_bounds(), 1.0, &walls);
        let mut out = Vec::new();
        index.query(Vec2::new(2.5, 5.0), 1.0, &mut out);
        assert!(out.contains(&0));
        assert!(!out.contains(&1));
    }

    #[test]
    fn segment_index_returns_each_wall_at_most_once() {
        let walls = vec![Segment::new(Vec2::new(0.0, 5.0), Vec2::new(10.0, 5.0))];
        let index = SegmentIndex::build(test_bounds(), 1.0, &walls);
        let mut out = Vec::new();
        index.query(Vec2::new(5.0, 5.0), 4.0, &mut out);
        assert_eq!(out, vec![0]);
    }
}
