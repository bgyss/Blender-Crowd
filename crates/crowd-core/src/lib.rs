//! Deterministic crowd simulation kernel.
//!
//! See `docs/superpowers/specs/2026-08-04-crowd-sim-kernel-design.md`.

pub mod arena;
pub mod clock;
pub mod geometry;
pub mod grid;
pub mod ids;
pub mod rng;
pub mod units;
pub mod world;

pub use arena::{Neighbor, NeighborArena};
pub use clock::Clock;
pub use geometry::Segment;
pub use grid::{SegmentIndex, UniformGrid};
pub use ids::{derive_agent_id, AgentId};
pub use rng::{Purpose, StableRng};
pub use units::{wrap_angle, Aabb, Vec2, DEFAULT_TICKS_PER_SECOND, WORLD_TO_METER};
pub use world::{AgentSpawn, RouteHandle, SolverStatus, SpawnError, World, NO_ROUTE};
