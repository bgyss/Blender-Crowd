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

/// Cost scale for an existing overlap. Shared by every solver so "already
/// touching" reads as comparably urgent everywhere, not tuned per solver.
pub(crate) const OVERLAP_URGENCY: f32 = 8.0;

/// Floor on predicted time-to-collision when converting it to a cost.
pub(crate) const MIN_TIME_FOR_COST: f32 = 0.25;

pub(crate) fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

/// Enumerate velocity-space candidates on fixed speed rings around `heading`,
/// in a fixed order, so every solver that samples candidates produces the
/// identical sequence for identical inputs. Does not include the preferred
/// velocity or the stop candidate -- callers evaluate those explicitly around
/// this call, since where they sit in evaluation order is solver bookkeeping,
/// not enumeration.
pub(crate) fn sample_candidates(
    heading: Vec2,
    speed_reference: f32,
    speed_samples: u32,
    heading_samples: u32,
    mut visit: impl FnMut(Vec2),
) {
    for speed_index in 1..=speed_samples {
        let speed = speed_reference * speed_index as f32 / speed_samples as f32;
        for heading_index in 0..heading_samples {
            let angle = std::f32::consts::TAU * heading_index as f32 / heading_samples as f32;
            visit(rotate(heading, angle) * speed);
        }
    }
}

/// Preferred velocity scaled down by local crowding.
pub(crate) fn density_adjusted_preferred(
    preferred: Vec2,
    position: Vec2,
    radius: f32,
    neighbors: &[NeighborState],
    personal_space: f32,
    density_speed_factor: f32,
) -> Vec2 {
    let crowding = neighbors
        .iter()
        .filter(|n| {
            let clearance = radius + n.radius + personal_space;
            (n.position - position).length_squared() < clearance * clearance
        })
        .count() as f32;
    preferred * (1.0 / (1.0 + density_speed_factor * crowding))
}

/// Whether `neighbor` is closing on `position` from roughly ahead, by
/// `heading`. A fixed, ID-independent test: two agents meeting head-on see
/// mirrored geometry, so deriving this from an ID comparison would make both
/// derive opposite answers and, in mirrored frames, deflect the same way in
/// world space -- staying on a collision course rather than separating.
pub(crate) fn is_head_on_encounter(
    heading: Vec2,
    position: Vec2,
    velocity: Vec2,
    neighbor: &NeighborState,
    head_on_cosine: f32,
) -> bool {
    if heading == Vec2::ZERO {
        return false;
    }
    let to_neighbor = (neighbor.position - position).normalize_or_zero();
    if to_neighbor == Vec2::ZERO || heading.dot(to_neighbor) < head_on_cosine {
        return false;
    }
    (neighbor.velocity - velocity).dot(to_neighbor) < 0.0
}

/// Extra cost for passing a head-on neighbor on the wrong side: a fixed
/// keep-left convention evaluated in the agent's own frame.
pub(crate) fn side_bias_cost(
    preferred: Vec2,
    position: Vec2,
    velocity: Vec2,
    neighbors: &[NeighborState],
    candidate: Vec2,
    head_on_cosine: f32,
    side_bias_weight: f32,
) -> f32 {
    let heading = preferred.normalize_or_zero();
    if heading == Vec2::ZERO {
        return 0.0;
    }
    let mut cost = 0.0;
    for neighbor in neighbors {
        if !is_head_on_encounter(heading, position, velocity, neighbor, head_on_cosine) {
            continue;
        }
        let to_neighbor = (neighbor.position - position).normalize_or_zero();
        // Positive cross product means the candidate passes to the left as
        // the agent looks at the neighbor -- its own left, in its own frame.
        let candidate_side = to_neighbor.x * candidate.y - to_neighbor.y * candidate.x;
        if candidate_side < 0.0 {
            cost += side_bias_weight;
        }
    }
    cost
}

/// Collision cost and earliest predicted time-to-collision against every
/// wall, for one candidate velocity. Walls never yield, so this is always the
/// full (non-reciprocal) correction, never halved.
#[allow(clippy::too_many_arguments)]
pub(crate) fn wall_avoidance_cost(
    position: Vec2,
    max_speed: f32,
    candidate: Vec2,
    radius: f32,
    walls: &[Segment],
    wall_horizon: f32,
    collision_weight: f32,
    wall_weight: f32,
    overlap_urgency: f32,
    min_time_for_cost: f32,
) -> (f32, f32) {
    let mut cost = 0.0;
    let mut earliest = f32::INFINITY;
    for wall in walls {
        let Some(t) =
            crate::geometry::time_to_collision_segment(position, candidate, radius, wall, wall_horizon)
        else {
            continue;
        };
        if t < earliest {
            earliest = t;
        }
        if t <= 0.0 {
            // Already inside the wall. A penalty derived from `t` alone
            // would be a constant that cancels out of the argmin -- see
            // `sampled.rs`'s original comment on this, which this code
            // preserves verbatim in behavior.
            let closest = wall.closest_point(position);
            let outward = (position - closest).normalize_or_zero();
            let outward = if outward == Vec2::ZERO {
                (wall.b - wall.a).normalize_or_zero().perp()
            } else {
                outward
            };
            let escape_rate = candidate.dot(outward);
            let relief = (escape_rate / max_speed.max(0.1)).clamp(0.0, 1.0);
            cost += collision_weight * wall_weight * overlap_urgency * (1.0 - relief);
        } else {
            cost += collision_weight * wall_weight / t.max(min_time_for_cost);
        }
    }
    (cost, earliest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_candidates_enumerates_speed_rings_in_a_fixed_order() {
        let mut seen = Vec::new();
        sample_candidates(Vec2::new(1.0, 0.0), 2.0, 2, 4, |v| seen.push(v));
        assert_eq!(seen.len(), 8, "2 speed samples x 4 headings");
        // First ring at half the reference speed, heading 0 first.
        assert!((seen[0].length() - 1.0).abs() < 1e-4);
        assert!((seen[0].x - 1.0).abs() < 1e-4 && seen[0].y.abs() < 1e-4);
        // Second ring at the full reference speed.
        assert!((seen[4].length() - 2.0).abs() < 1e-4);
    }
}
