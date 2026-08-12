//! Deterministic crowd simulation kernel.
//!
//! See `docs/superpowers/specs/2026-08-04-crowd-sim-kernel-design.md`.

pub mod arena;
pub mod assets;
pub mod authoring;
pub mod avoidance;
pub mod behavior;
pub mod clock;
pub mod commuter;
pub mod concourse;
pub mod geometry;
pub mod grid;
pub mod ids;
pub mod metrics;
pub mod nav;
pub mod nav_scenes;
pub mod phases;
pub mod presentation;
pub mod project;
pub mod rng;
pub mod route;
pub mod runtime_behavior;
pub mod scene;
pub mod scenes;
pub mod sim;
pub mod social;
pub mod units;
pub mod world;

pub use arena::{Neighbor, NeighborArena};
pub use avoidance::{
    AvoidanceInput, AvoidanceOutput, AvoidanceSolver, NeighborState, SampledVelocitySolver,
};
pub use clock::Clock;
pub use commuter::{
    AgentSnapshot, ClipState, CommuterState, DecisionReason, FrameSnapshot, PortalControlError,
    RuntimeAgentSpec, RuntimeAnimationSettings, TimedPortalInput,
};
pub use concourse::compile_concourse;
pub use geometry::Segment;
pub use grid::{SegmentIndex, UniformGrid};
pub use ids::{derive_agent_id, AgentId};
pub use metrics::{Metrics, MetricsConfig, MetricsSummary, Phase};
pub use nav::{NavMeshDef, PortalId, TileGraph};
pub use phases::{animate, AnimateConfig, IDLE_CLIP_ID, JOG_CLIP_ID, WALK_CLIP_ID};
pub use project::{
    compile_project, CompiledAgentSpawn, CompiledProject, Diagnostic, DiagnosticCode, ProjectIrV1,
    PROJECT_IR_SCHEMA_VERSION,
};
pub use rng::{Purpose, StableRng};
pub use route::{next_target, RouteArena, WaypointGraph};
pub use runtime_behavior::{BehaviorRuntimeEvent, BehaviorRuntimeEventKind};
pub use scene::{CompiledScene, Destination, PopulationParams, SceneDef, SceneError, SpawnRegion};
pub use sim::{SimConfig, Simulation};
pub use units::{wrap_angle, Aabb, Vec2, DEFAULT_TICKS_PER_SECOND, WORLD_TO_METER};
pub use world::{AgentSpawn, RouteHandle, SolverStatus, SpawnError, World, NO_ROUTE};
