//! Unit and coordinate contract.
//!
//! Meters, seconds, radians. Z-up right-handed to match Blender exactly.
//! Pedestrian kinematics are planar in XY; ground height Z is resolved from
//! the environment rather than integrated. Orientation is a yaw scalar about Z.

use std::ops::{Add, Mul, Neg, Sub};

use serde::{Deserialize, Serialize};

/// Default simulation rate in ticks per second.
pub const DEFAULT_TICKS_PER_SECOND: u32 = 30;

/// Scene scale factor. This slice asserts 1.0; scaling has no test yet.
pub const WORLD_TO_METER: f32 = 1.0;

/// A planar vector in the XY ground plane, in meters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn dot(self, other: Vec2) -> f32 {
        self.x * other.x + self.y * other.y
    }

    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    pub fn distance_squared(self, other: Vec2) -> f32 {
        (other - self).length_squared()
    }

    /// Unit vector, or exactly zero when the input is degenerate.
    ///
    /// Returning zero rather than NaN keeps a stalled agent's state finite,
    /// which the tick loop relies on.
    pub fn normalize_or_zero(self) -> Vec2 {
        let len_sq = self.length_squared();
        if len_sq <= f32::MIN_POSITIVE {
            Vec2::ZERO
        } else {
            let inv = 1.0 / len_sq.sqrt();
            Vec2::new(self.x * inv, self.y * inv)
        }
    }

    /// Rotate 90 degrees counter-clockwise (to the agent's left, Z-up).
    pub fn perp(self) -> Vec2 {
        Vec2::new(-self.y, self.x)
    }

    pub fn clamp_length(self, max_length: f32) -> Vec2 {
        let len_sq = self.length_squared();
        if len_sq <= max_length * max_length {
            self
        } else {
            self.normalize_or_zero() * max_length
        }
    }

    /// Yaw about Z, in radians, for the direction this vector points.
    pub fn to_yaw(self) -> f32 {
        self.y.atan2(self.x)
    }

    pub fn from_yaw(yaw: f32) -> Vec2 {
        Vec2::new(yaw.cos(), yaw.sin())
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, rhs: f32) -> Vec2 {
        Vec2::new(self.x * rhs, self.y * rhs)
    }
}

impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

/// An axis-aligned bounding box in the XY ground plane.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Aabb {
    pub min: Vec2,
    pub max: Vec2,
}

impl Aabb {
    pub const fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    pub fn expanded(&self, margin: f32) -> Aabb {
        Aabb::new(
            Vec2::new(self.min.x - margin, self.min.y - margin),
            Vec2::new(self.max.x + margin, self.max.y + margin),
        )
    }
}

/// Wrap an angle to `(-pi, pi]` so differences take the short way round.
///
/// Shared rather than duplicated: the integrate phase uses it to limit turn
/// rate and the metrics layer uses it to measure heading change. Two copies of
/// one convention is two chances for them to disagree about what a reversal is.
pub fn wrap_angle(angle: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let mut a = angle % TAU;
    if a > PI {
        a -= TAU;
    } else if a <= -PI {
        a += TAU;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_or_zero_handles_zero_vector() {
        assert_eq!(Vec2::ZERO.normalize_or_zero(), Vec2::ZERO);
    }

    #[test]
    fn normalize_or_zero_produces_unit_length() {
        let v = Vec2::new(3.0, 4.0).normalize_or_zero();
        assert!((v.length() - 1.0).abs() < 1e-6);
        assert!((v.x - 0.6).abs() < 1e-6);
        assert!((v.y - 0.8).abs() < 1e-6);
    }

    #[test]
    fn clamp_length_leaves_short_vectors_untouched() {
        let v = Vec2::new(1.0, 0.0);
        assert_eq!(v.clamp_length(2.0), v);
    }

    #[test]
    fn clamp_length_shortens_long_vectors() {
        let v = Vec2::new(10.0, 0.0).clamp_length(2.0);
        assert!((v.length() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn perp_is_ninety_degrees_left() {
        let v = Vec2::new(1.0, 0.0).perp();
        assert!((v.x - 0.0).abs() < 1e-6);
        assert!((v.y - 1.0).abs() < 1e-6);
    }

    #[test]
    fn aabb_contains_only_points_inside() {
        let b = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(2.0, 2.0));
        assert!(b.contains(Vec2::new(1.0, 1.0)));
        assert!(!b.contains(Vec2::new(3.0, 1.0)));
    }

    #[test]
    fn operators_have_the_expected_signs() {
        // An inverted `Sub` or `Neg` would corrupt every integration step and
        // every relative-position calculation, silently and everywhere.
        let a = Vec2::new(3.0, 5.0);
        let b = Vec2::new(1.0, 2.0);
        assert_eq!(a + b, Vec2::new(4.0, 7.0));
        assert_eq!(a - b, Vec2::new(2.0, 3.0), "Sub must be self minus other");
        assert_eq!(-a, Vec2::new(-3.0, -5.0));
        assert_eq!(a * 2.0, Vec2::new(6.0, 10.0));
        assert_eq!(a - a, Vec2::ZERO);
    }

    #[test]
    fn dot_and_distance_agree_with_hand_computation() {
        let a = Vec2::new(3.0, 4.0);
        let b = Vec2::new(1.0, 2.0);
        assert_eq!(a.dot(b), 11.0);
        assert_eq!(a.length_squared(), 25.0);
        assert_eq!(a.length(), 5.0);
        // (3-1)^2 + (4-2)^2 = 8
        assert_eq!(a.distance_squared(b), 8.0);
        // Symmetric, and zero against itself.
        assert_eq!(b.distance_squared(a), 8.0);
        assert_eq!(a.distance_squared(a), 0.0);
    }

    #[test]
    fn yaw_round_trips_through_a_direction() {
        for step in 0..8 {
            let yaw = std::f32::consts::TAU * step as f32 / 8.0 - std::f32::consts::PI;
            let back = Vec2::from_yaw(yaw).to_yaw();
            assert!(
                (wrap_angle(back - yaw)).abs() < 1e-5,
                "yaw {yaw} round-tripped to {back}"
            );
        }
        // +X is yaw zero, +Y is a quarter turn — the Z-up convention.
        assert!((Vec2::new(1.0, 0.0).to_yaw()).abs() < 1e-6);
        assert!((Vec2::new(0.0, 1.0).to_yaw() - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    }

    #[test]
    fn is_finite_rejects_nan_and_infinity() {
        assert!(Vec2::new(1.0, 2.0).is_finite());
        assert!(!Vec2::new(f32::NAN, 0.0).is_finite());
        assert!(!Vec2::new(0.0, f32::INFINITY).is_finite());
    }

    #[test]
    fn aabb_accessors_match_their_names() {
        let b = Aabb::new(Vec2::new(2.0, 4.0), Vec2::new(6.0, 10.0));
        assert_eq!(b.center(), Vec2::new(4.0, 7.0));
        assert_eq!(b.size(), Vec2::new(4.0, 6.0));
        let grown = b.expanded(1.0);
        assert_eq!(grown.min, Vec2::new(1.0, 3.0));
        assert_eq!(grown.max, Vec2::new(7.0, 11.0));
        assert!(
            grown.contains(b.min),
            "expanding must not exclude the original"
        );
    }

    #[test]
    fn wrap_angle_leaves_small_angles_alone() {
        assert!((wrap_angle(0.5) - 0.5).abs() < 1e-6);
        assert!((wrap_angle(-0.5) + 0.5).abs() < 1e-6);
    }

    #[test]
    fn wrap_angle_takes_the_short_way_round() {
        use std::f32::consts::PI;
        // Just past +pi must come out just past -pi, not as a near-full turn.
        let wrapped = wrap_angle(PI + 0.1);
        assert!(wrapped < 0.0, "got {wrapped}");
        assert!((wrapped + PI - 0.1).abs() < 1e-4, "got {wrapped}");
    }

    #[test]
    fn wrap_angle_output_is_always_within_half_turn() {
        use std::f32::consts::PI;
        for step in -50..50 {
            let angle = step as f32 * 0.7;
            let wrapped = wrap_angle(angle);
            assert!(
                wrapped > -PI - 1e-5 && wrapped <= PI + 1e-5,
                "got {wrapped}"
            );
        }
    }
}
