//! Deterministic crowd simulation kernel.
//!
//! See `docs/superpowers/specs/2026-08-04-crowd-sim-kernel-design.md`.

pub mod clock;
pub mod ids;
pub mod rng;
pub mod units;

pub use clock::Clock;
pub use ids::{derive_agent_id, AgentId};
pub use rng::{Purpose, StableRng};
pub use units::{wrap_angle, Aabb, Vec2, DEFAULT_TICKS_PER_SECOND, WORLD_TO_METER};
