//! Local avoidance, contract section 6.2.
//!
//! The trait exists so the ORCA-style and scoped time-to-collision candidates
//! can be measured against this baseline in the next slice without touching
//! any tick phase.

pub mod sampled;

pub use sampled::SampledVelocitySolver;

use crate::geometry::Segment;
use crate::ids::AgentId;
use crate::units::Vec2;
use crate::world::SolverStatus;

/// One neighbor as the solver sees it: a disc with a velocity and an ID.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NeighborState {
    pub position: Vec2,
    pub velocity: Vec2,
    pub radius: f32,
    pub agent_id: AgentId,
}

/// Everything one agent's avoidance decision depends on.
///
/// Deliberately a plain snapshot rather than a reference to the world: it
/// makes the solver trivially testable in isolation and keeps solvers from
/// reaching into state they should not read.
#[derive(Clone, Copy, Debug)]
pub struct AvoidanceInput<'a> {
    pub agent_id: AgentId,
    pub position: Vec2,
    pub velocity: Vec2,
    /// Goal-seeking velocity from the decide phase, before avoidance.
    pub preferred: Vec2,
    pub radius: f32,
    pub max_speed: f32,
    pub neighbors: &'a [NeighborState],
    pub walls: &'a [Segment],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvoidanceOutput {
    pub velocity: Vec2,
    pub status: SolverStatus,
}

// A solver deliberately does NOT report a predicted time to collision.
//
// Its internal figure is computed against the reciprocal construction —
// `candidate * 2 - velocity` — which is right as a cost heuristic but is a
// velocity the agent never actually has. Surfacing it would put a fictitious
// kinematic state into the quality metrics. The steer phase measures the real
// one against the velocity the agent will actually use.

pub trait AvoidanceSolver {
    fn name(&self) -> &'static str;
    fn solve(&self, input: &AvoidanceInput<'_>) -> AvoidanceOutput;
}
