//! Deterministic crowd simulation kernel.
//!
//! See `docs/superpowers/specs/2026-08-04-crowd-sim-kernel-design.md`.

pub mod units;

pub use units::{wrap_angle, Aabb, Vec2, DEFAULT_TICKS_PER_SECOND, WORLD_TO_METER};
