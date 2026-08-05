//! Deterministic crowd simulation kernel.
//!
//! See `docs/superpowers/specs/2026-08-04-crowd-sim-kernel-design.md`.

pub mod arena;
pub mod avoidance;
pub mod clock;
pub mod geometry;
pub mod grid;
pub mod ids;
pub mod metrics;
pub mod phases;
pub mod rng;
pub mod route;
pub mod scene;
pub mod sim;
pub mod units;
pub mod world;

pub use arena::{Neighbor, NeighborArena};
pub use avoidance::{
    AvoidanceInput, AvoidanceOutput, AvoidanceSolver, NeighborState, SampledVelocitySolver,
};
pub use clock::Clock;
pub use geometry::Segment;
pub use grid::{SegmentIndex, UniformGrid};
pub use ids::{derive_agent_id, AgentId};
pub use metrics::{Metrics, MetricsConfig, MetricsSummary, Phase};
pub use rng::{Purpose, StableRng};
pub use route::{next_target, RouteArena, WaypointGraph};
pub use scene::{CompiledScene, Destination, PopulationParams, SceneDef, SceneError, SpawnRegion};
pub use sim::{SimConfig, Simulation};
pub use units::{wrap_angle, Aabb, Vec2, DEFAULT_TICKS_PER_SECOND, WORLD_TO_METER};
pub use world::{AgentSpawn, RouteHandle, SolverStatus, SpawnError, World, NO_ROUTE};
