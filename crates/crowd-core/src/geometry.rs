//! Analytic static geometry and predictive collision math.
//!
//! The navmesh is deferred (contract section 6.1), so this slice represents
//! the environment as line segments. They serve two roles: constraints for the
//! avoidance solver, and hard boundaries the integrate phase resolves against.

use crate::units::{Aabb, Vec2};

/// A wall, as a line segment in the XY ground plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub a: Vec2,
    pub b: Vec2,
}

impl Segment {
    pub const fn new(a: Vec2, b: Vec2) -> Self {
        Self { a, b }
    }

    /// The point on the segment nearest `p`, clamped to the endpoints.
    pub fn closest_point(&self, p: Vec2) -> Vec2 {
        let ab = self.b - self.a;
        let len_sq = ab.length_squared();
        if len_sq <= f32::MIN_POSITIVE {
            return self.a;
        }
        let t = ((p - self.a).dot(ab) / len_sq).clamp(0.0, 1.0);
        self.a + ab * t
    }

    pub fn distance_to(&self, p: Vec2) -> f32 {
        (p - self.closest_point(p)).length()
    }

    pub fn bounds(&self) -> Aabb {
        Aabb::new(
            Vec2::new(self.a.x.min(self.b.x), self.a.y.min(self.b.y)),
            Vec2::new(self.a.x.max(self.b.x), self.a.y.max(self.b.y)),
        )
    }
}

/// Time until two moving discs touch, or `None` if they never do.
///
/// `rel_pos` is other minus self, `rel_vel` is other's velocity minus self's,
/// and `combined_radius` is the sum of both radii. Returns `Some(0.0)` when
/// they already overlap, so callers treat interpenetration as maximally urgent
/// rather than as "no collision".
pub fn time_to_collision_disc(rel_pos: Vec2, rel_vel: Vec2, combined_radius: f32) -> Option<f32> {
    let dist_sq = rel_pos.length_squared();
    let radius_sq = combined_radius * combined_radius;
    if dist_sq <= radius_sq {
        return Some(0.0);
    }

    // Solve |rel_pos + rel_vel * t| = combined_radius for the smaller root.
    let a = rel_vel.length_squared();
    if a <= f32::MIN_POSITIVE {
        return None;
    }
    let b = rel_pos.dot(rel_vel);
    if b >= 0.0 {
        return None; // Separating or parallel.
    }
    let c = dist_sq - radius_sq;
    let discriminant = b * b - a * c;
    if discriminant <= 0.0 {
        return None; // Passes by without touching.
    }
    let t = (-b - discriminant.sqrt()) / a;
    if t < 0.0 {
        None
    } else {
        Some(t)
    }
}

/// Time until a moving disc touches a static segment, within `horizon`.
///
/// Sampled rather than solved in closed form: the exact swept-capsule test is
/// three cases (two endpoints plus the edge), and at the sample counts the
/// solver uses, a short bisection is both simpler to keep correct and fast
/// enough. Determinism is unaffected because the sample count is fixed.
pub fn time_to_collision_segment(
    pos: Vec2,
    vel: Vec2,
    radius: f32,
    seg: &Segment,
    horizon: f32,
) -> Option<f32> {
    if seg.distance_to(pos) <= radius {
        return Some(0.0);
    }
    if vel.length_squared() <= f32::MIN_POSITIVE {
        return None;
    }

    const COARSE_STEPS: u32 = 8;
    const REFINE_STEPS: u32 = 12;

    let mut lo = 0.0f32;
    let mut hit = false;
    let mut hi = horizon;
    for i in 1..=COARSE_STEPS {
        let t = horizon * i as f32 / COARSE_STEPS as f32;
        if seg.distance_to(pos + vel * t) <= radius {
            hi = t;
            hit = true;
            break;
        }
        lo = t;
    }
    if !hit {
        return None;
    }

    for _ in 0..REFINE_STEPS {
        let mid = 0.5 * (lo + hi);
        if seg.distance_to(pos + vel * mid) <= radius {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some(hi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Vec2;

    #[test]
    fn closest_point_clamps_to_the_segment_endpoints() {
        let seg = Segment::new(Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0));
        assert_eq!(seg.closest_point(Vec2::new(-5.0, 1.0)), Vec2::new(0.0, 0.0));
        assert_eq!(seg.closest_point(Vec2::new(9.0, 1.0)), Vec2::new(2.0, 0.0));
        assert_eq!(seg.closest_point(Vec2::new(1.0, 3.0)), Vec2::new(1.0, 0.0));
    }

    #[test]
    fn distance_to_measures_perpendicular_distance() {
        let seg = Segment::new(Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0));
        assert!((seg.distance_to(Vec2::new(1.0, 3.0)) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn degenerate_segment_behaves_like_a_point() {
        let seg = Segment::new(Vec2::new(1.0, 1.0), Vec2::new(1.0, 1.0));
        assert_eq!(seg.closest_point(Vec2::new(4.0, 5.0)), Vec2::new(1.0, 1.0));
    }

    #[test]
    fn head_on_approach_has_a_finite_time_to_collision() {
        // Two unit-radius discs 10m apart closing at 1 m/s each: surfaces meet
        // after 8m of closure, so t = 8.0.
        let t = time_to_collision_disc(Vec2::new(10.0, 0.0), Vec2::new(-2.0, 0.0), 2.0);
        assert!((t.unwrap() - 4.0).abs() < 1e-4, "got {t:?}");
    }

    #[test]
    fn separating_discs_never_collide() {
        assert_eq!(
            time_to_collision_disc(Vec2::new(10.0, 0.0), Vec2::new(2.0, 0.0), 2.0),
            None
        );
    }

    #[test]
    fn parallel_passing_discs_never_collide() {
        assert_eq!(
            time_to_collision_disc(Vec2::new(0.0, 5.0), Vec2::new(-2.0, 0.0), 2.0),
            None
        );
    }

    #[test]
    fn already_overlapping_discs_report_zero() {
        let t = time_to_collision_disc(Vec2::new(0.5, 0.0), Vec2::new(-1.0, 0.0), 2.0);
        assert_eq!(t, Some(0.0));
    }

    #[test]
    fn stationary_discs_never_collide() {
        assert_eq!(
            time_to_collision_disc(Vec2::new(10.0, 0.0), Vec2::ZERO, 2.0),
            None
        );
    }

    #[test]
    fn agent_walking_into_a_wall_has_a_finite_time_to_collision() {
        let wall = Segment::new(Vec2::new(5.0, -5.0), Vec2::new(5.0, 5.0));
        let t = time_to_collision_segment(Vec2::ZERO, Vec2::new(1.0, 0.0), 0.5, &wall, 10.0);
        assert!((t.unwrap() - 4.5).abs() < 1e-2, "got {t:?}");
    }

    #[test]
    fn agent_walking_away_from_a_wall_has_no_collision() {
        let wall = Segment::new(Vec2::new(5.0, -5.0), Vec2::new(5.0, 5.0));
        assert_eq!(
            time_to_collision_segment(Vec2::ZERO, Vec2::new(-1.0, 0.0), 0.5, &wall, 10.0),
            None
        );
    }

    #[test]
    fn wall_collision_beyond_the_horizon_is_ignored() {
        let wall = Segment::new(Vec2::new(50.0, -5.0), Vec2::new(50.0, 5.0));
        assert_eq!(
            time_to_collision_segment(Vec2::ZERO, Vec2::new(1.0, 0.0), 0.5, &wall, 10.0),
            None
        );
    }
}
