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
/// # Why this is solved exactly rather than sampled
///
/// Any sampled search tunnels, and no step bound fixes it. The disc's overlap
/// window with a segment endpoint has width `2·sqrt(r² − d²)` where `d` is the
/// closest-approach distance — which goes to *zero* as `d` approaches `r`. A
/// grazing contact can therefore be arbitrarily brief, so for any fixed or
/// derived step size there exists a real collision that falls entirely between
/// two samples and is reported as no collision at all. That is silent, and it
/// happens exactly at the corner and doorframe geometry a crowd meets
/// constantly.
///
/// The exact swept-capsule test is three cases and is also *cheaper* than the
/// twenty-odd distance evaluations a bisection needs: two endpoint quadratics
/// (reusing `time_to_collision_disc`, since a capsule cap is a zero-radius
/// disc) plus one linear crossing of the edge offset by `radius`.
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

    let mut earliest = f32::INFINITY;

    // Endpoint caps. A cap is a stationary zero-radius disc, so the relative
    // position is endpoint-minus-self and the relative velocity is -vel, per
    // `time_to_collision_disc`'s convention.
    for endpoint in [seg.a, seg.b] {
        if let Some(t) = time_to_collision_disc(endpoint - pos, -vel, radius) {
            if t <= horizon && t < earliest {
                earliest = t;
            }
        }
    }

    // Flat side: cross the line offset by `radius` toward the side we start on.
    let along = seg.b - seg.a;
    let len_sq = along.length_squared();
    if len_sq > f32::MIN_POSITIVE {
        let normal = along.perp() * (1.0 / len_sq.sqrt());
        let offset = (pos - seg.a).dot(normal);
        let approach_rate = vel.dot(normal);
        if approach_rate.abs() > f32::MIN_POSITIVE {
            let target = if offset >= 0.0 { radius } else { -radius };
            let t = (target - offset) / approach_rate;
            if (0.0..=horizon).contains(&t) && t < earliest {
                // Only a flat-side hit if contact lands between the endpoints.
                // Beyond them the caps above are the real geometry.
                let contact = pos + vel * t;
                let s = (contact - seg.a).dot(along) / len_sq;
                if (0.0..=1.0).contains(&s) {
                    earliest = t;
                }
            }
        }
    }

    if earliest.is_finite() {
        Some(earliest)
    } else {
        None
    }
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

    #[test]
    fn an_agent_already_inside_a_wall_reports_zero() {
        // The overlap early return is load-bearing: the solver's escape
        // gradient depends on distinguishing "already inside" from "will
        // never touch".
        let wall = Segment::new(Vec2::new(0.0, -5.0), Vec2::new(0.0, 5.0));
        let inside =
            time_to_collision_segment(Vec2::new(0.1, 0.0), Vec2::new(1.0, 0.0), 0.5, &wall, 10.0);
        assert_eq!(inside, Some(0.0));
        // ... and it does not depend on which way the agent is moving.
        let leaving =
            time_to_collision_segment(Vec2::new(0.1, 0.0), Vec2::new(-1.0, 0.0), 0.5, &wall, 10.0);
        assert_eq!(leaving, Some(0.0));
    }

    #[test]
    fn a_stationary_agent_clear_of_a_wall_never_reaches_it() {
        let wall = Segment::new(Vec2::new(5.0, -5.0), Vec2::new(5.0, 5.0));
        assert_eq!(
            time_to_collision_segment(Vec2::ZERO, Vec2::ZERO, 0.5, &wall, 10.0),
            None
        );
    }

    #[test]
    fn a_brief_grazing_collision_is_not_stepped_over() {
        // Regression: this endpoint sits 0.49 from the path against a radius of
        // 0.5, so the true overlap window is only about 0.2s wide. Any sampled
        // search coarse enough to be affordable steps straight over it and
        // reports no collision.
        let wall = Segment::new(Vec2::new(3.125, 0.49), Vec2::new(3.125, 6.0));
        let t = time_to_collision_segment(Vec2::ZERO, Vec2::new(1.0, 0.0), 0.5, &wall, 10.0);
        assert!(t.is_some(), "grazing collision was missed");
        let t = t.unwrap();
        assert!(
            (3.0..3.1).contains(&t),
            "collision found at the wrong time: {t}"
        );
    }

    #[test]
    fn an_arbitrarily_brief_graze_is_still_detected() {
        // The window narrows without limit as the miss distance approaches the
        // radius: at 0.4999 against 0.5 it is under 0.03s. This is the case
        // that proves sampling cannot work here at any step size.
        let wall = Segment::new(Vec2::new(4.0, 0.4999), Vec2::new(4.0, 6.0));
        let t = time_to_collision_segment(Vec2::ZERO, Vec2::new(1.0, 0.0), 0.5, &wall, 10.0);
        assert!(t.is_some(), "near-tangential graze was missed");
    }

    #[test]
    fn contact_past_the_end_of_a_segment_uses_the_endpoint_not_the_edge() {
        // Travelling parallel to the wall's line but level with its end: the
        // infinite line would say "never", the capsule cap says otherwise.
        let wall = Segment::new(Vec2::new(0.0, 5.0), Vec2::new(0.0, 10.0));
        let t =
            time_to_collision_segment(Vec2::new(0.0, 0.0), Vec2::new(0.0, 1.0), 0.5, &wall, 10.0);
        assert!((t.unwrap() - 4.5).abs() < 1e-4, "got {t:?}");
    }

    #[test]
    fn a_fast_agent_does_not_tunnel_through_a_thin_gap() {
        // 20 m of travel against a 0.2 m radius: a constant step count would
        // advance many radii per sample.
        let wall = Segment::new(Vec2::new(10.0, -1.0), Vec2::new(10.0, 1.0));
        let t = time_to_collision_segment(Vec2::ZERO, Vec2::new(10.0, 0.0), 0.2, &wall, 2.0);
        assert!(t.is_some(), "fast agent tunnelled through the wall");
        assert!((t.unwrap() - 0.98).abs() < 0.02, "got {t:?}");
    }

    #[test]
    fn segment_bounds_cover_both_endpoints_in_either_order() {
        let ascending = Segment::new(Vec2::new(1.0, 2.0), Vec2::new(4.0, 6.0));
        let descending = Segment::new(Vec2::new(4.0, 6.0), Vec2::new(1.0, 2.0));
        for seg in [ascending, descending] {
            let b = seg.bounds();
            assert_eq!(b.min, Vec2::new(1.0, 2.0));
            assert_eq!(b.max, Vec2::new(4.0, 6.0));
        }
    }
}
