//! Sampled-velocity avoidance, the 1.0 baseline solver.
//!
//! Each agent scores a fixed, fixed-order set of candidate velocities and
//! takes the cheapest. The cost has four terms:
//!
//! - distance from the preferred velocity (make progress);
//! - predicted time to collision with neighbors and walls (stay safe);
//! - deviation from the current velocity (stay smooth);
//! - a fixed keep-left preference for head-on encounters.
//!
//! The smoothness term is not decoration: without it, a candidate set
//! re-evaluated every tick flips between near-equal options and produces the
//! high-frequency oscillation contract section 6.2 names as a production
//! blocker.
//!
//! # Why head-on side choice is a convention, not an ID comparison
//!
//! Two agents meeting head-on see mirrored geometry. If each asked "am I the
//! lower ID?" they would get *opposite* answers, and because their frames are
//! mirrored, opposite answers produce the **same** world-space deflection —
//! both drift the same way and stay on a collision course. A fixed convention
//! evaluated in each agent's own frame produces opposite world deflections,
//! which is what actually separates them. Real pedestrians solve it the same
//! way, with a cultural side convention rather than a negotiation.
//!
//! Stable IDs still supply the asymmetry contract section 6.2 requires, but
//! where it is actually needed: a perpendicular crossing conflict is
//! symmetric under the keep-left rule, so the higher ID yields via a heavier
//! collision weight. Both agents can compute that from data both already
//! have, so they never both yield or both push.

use super::{AvoidanceInput, AvoidanceOutput, AvoidanceSolver};
use crate::geometry::{time_to_collision_disc, time_to_collision_segment};
use crate::units::Vec2;
use crate::world::SolverStatus;

pub use super::NeighborState;

/// Cost scale for an existing overlap.
///
/// Deliberately comparable to the other terms rather than dominating them.
/// An unbounded `1/t` made a single touching neighbour cost ~200 against a
/// goal term of ~1.4, so once any pair touched the solver stopped steering
/// toward the destination at all and only jostled to separate -- a crowd that
/// freezes on contact.
const OVERLAP_URGENCY: f32 = 8.0;

/// Floor on predicted time-to-collision when converting it to a cost.
///
/// Bounds the `1/t` term for the same reason: an imminent collision should
/// dominate, not annihilate, every other consideration.
const MIN_TIME_FOR_COST: f32 = 0.25;

#[derive(Clone, Copy, Debug)]
pub struct SampledVelocitySolver {
    /// Speed rings sampled below the preferred speed, plus a stop candidate.
    pub speed_samples: u32,
    /// Headings sampled around the full circle.
    pub heading_samples: u32,
    /// Ignore predicted collisions further out than this, in seconds.
    pub time_horizon: f32,
    /// Wall lookahead, in seconds. Shorter than `time_horizon` because static
    /// geometry is avoided by turning, not by long-range planning.
    pub wall_horizon: f32,
    pub goal_weight: f32,
    pub collision_weight: f32,
    /// Walls do not yield, so they weigh more than a neighbor at equal range.
    pub wall_weight: f32,
    pub smoothness_weight: f32,
    pub side_bias_weight: f32,
    /// Extra collision weight carried by the higher-ID agent in a conflict,
    /// which is what makes a symmetric crossing resolve.
    pub yield_factor: f32,
    /// Choosing a speed below this fraction of the preferred speed counts as
    /// braking.
    ///
    /// Braking cannot be defined by the chosen candidate's time to collision:
    /// stopping has an *infinite* one by construction, so the graceful
    /// fallback would never report itself. What actually distinguishes it is
    /// that the solver gave up on making progress.
    pub brake_speed_fraction: f32,
    /// Comfortable clearance beyond touching radii, in meters.
    pub personal_space: f32,
    /// How strongly local crowding reduces preferred speed.
    pub density_speed_factor: f32,
    /// Cosine threshold for treating an encounter as head-on.
    pub head_on_cosine: f32,
}

impl Default for SampledVelocitySolver {
    fn default() -> Self {
        Self {
            speed_samples: 3,
            heading_samples: 16,
            time_horizon: 3.0,
            wall_horizon: 2.0,
            goal_weight: 1.0,
            collision_weight: 2.0,
            wall_weight: 1.5,
            smoothness_weight: 0.35,
            side_bias_weight: 0.6,
            yield_factor: 1.4,
            brake_speed_fraction: 0.5,
            personal_space: 0.45,
            density_speed_factor: 0.18,
            head_on_cosine: 0.7,
        }
    }
}

impl SampledVelocitySolver {
    /// Collision penalty and earliest predicted collision for one candidate.
    ///
    /// Neighbors use the reciprocal construction: assuming the neighbor holds
    /// its velocity while this agent takes on the full correction produces
    /// mutual over-correction, so each side is credited with half the change.
    ///
    /// Penalties are summed across threats rather than taken from the single
    /// worst one, so an agent squeezed between two neighbors feels both.
    fn collision_cost(&self, input: &AvoidanceInput<'_>, candidate: Vec2) -> (f32, f32) {
        let mut cost = 0.0;
        let mut earliest = f32::INFINITY;

        for neighbor in input.neighbors {
            let reciprocal_velocity = candidate * 2.0 - input.velocity;
            let relative_position = neighbor.position - input.position;
            let relative_velocity = neighbor.velocity - reciprocal_velocity;
            let combined_radius = input.radius + neighbor.radius;
            let Some(t) =
                time_to_collision_disc(relative_position, relative_velocity, combined_radius)
            else {
                continue;
            };
            if t < earliest {
                earliest = t;
            }
            // The higher stable ID yields. A perpendicular conflict is
            // symmetric under the keep-left rule, so without this both
            // agents would make the identical choice and collide.
            let yield_weight = if input.agent_id > neighbor.agent_id {
                self.yield_factor
            } else {
                1.0
            };

            if t <= 0.0 {
                // Already overlapping, and `time_to_collision_disc` reports
                // zero for *every* candidate in that state. A penalty derived
                // from `t` alone would therefore score "pull apart" exactly
                // like "drive deeper" — the agent gets no gradient telling it
                // which way is out, and a dense cluster locks solid. This is
                // the frozen-robot trap, and a crowd meets it constantly at
                // doorways and spawn points.
                //
                // Penalise by how fast the overlap is closing instead, so
                // separating is always strictly cheaper than compressing.
                let direction = relative_position.normalize_or_zero();
                let separation_rate = relative_velocity.dot(direction);
                let relief = (separation_rate / input.max_speed.max(0.1)).clamp(0.0, 1.0);
                cost += self.collision_weight * yield_weight * OVERLAP_URGENCY * (1.0 - relief);
            } else if t < self.time_horizon {
                cost += self.collision_weight * yield_weight / t.max(MIN_TIME_FOR_COST);
            }
        }

        for wall in input.walls {
            let Some(t) = time_to_collision_segment(
                input.position,
                candidate,
                input.radius,
                wall,
                self.wall_horizon,
            ) else {
                continue;
            };
            if t < earliest {
                earliest = t;
            }
            if t <= 0.0 {
                // Already inside the wall. As with an overlapping neighbour,
                // `t` is zero for every candidate, so a penalty derived from
                // it is a constant that cancels out of the argmin — the wall
                // becomes invisible for this tick and the agent may steer
                // deeper in. Penalise by whether the candidate is closing on
                // the wall or escaping it.
                let closest = wall.closest_point(input.position);
                let outward = (input.position - closest).normalize_or_zero();
                // Exactly on the surface: use the wall's own normal, chosen by
                // a fixed rule so the result stays deterministic.
                let outward = if outward == Vec2::ZERO {
                    (wall.b - wall.a).normalize_or_zero().perp()
                } else {
                    outward
                };
                let escape_rate = candidate.dot(outward);
                let relief = (escape_rate / input.max_speed.max(0.1)).clamp(0.0, 1.0);
                cost += self.collision_weight * self.wall_weight * OVERLAP_URGENCY * (1.0 - relief);
            } else {
                // Walls never yield, so they are weighted more heavily than a
                // neighbor at the same predicted range.
                cost += self.collision_weight * self.wall_weight / t.max(MIN_TIME_FOR_COST);
            }
        }

        (cost, earliest)
    }

    /// Extra cost for passing a head-on neighbor on the wrong side.
    ///
    /// A **fixed** keep-left convention, evaluated in the agent's own frame.
    /// Deriving the side from an ID comparison would be actively wrong here:
    /// mirrored agents would derive opposite answers, which in mirrored frames
    /// means the same world-space deflection, and they would fail to separate.
    fn side_bias_cost(&self, input: &AvoidanceInput<'_>, candidate: Vec2) -> f32 {
        let mut cost = 0.0;
        let heading = input.preferred.normalize_or_zero();
        if heading == Vec2::ZERO {
            return 0.0;
        }

        for neighbor in input.neighbors {
            let to_neighbor = (neighbor.position - input.position).normalize_or_zero();
            if to_neighbor == Vec2::ZERO || heading.dot(to_neighbor) < self.head_on_cosine {
                continue;
            }
            // Only bias against neighbors actually closing on us.
            if (neighbor.velocity - input.velocity).dot(to_neighbor) >= 0.0 {
                continue;
            }

            // Positive cross product means the candidate passes to the left as
            // the agent looks at the neighbor — its own left, in its own frame.
            let candidate_side = to_neighbor.x * candidate.y - to_neighbor.y * candidate.x;
            if candidate_side < 0.0 {
                cost += self.side_bias_weight;
            }
        }
        cost
    }

    /// Preferred velocity scaled down by local crowding.
    fn density_adjusted_preferred(&self, input: &AvoidanceInput<'_>) -> Vec2 {
        let crowding = input
            .neighbors
            .iter()
            .filter(|n| {
                let clearance = input.radius + n.radius + self.personal_space;
                (n.position - input.position).length_squared() < clearance * clearance
            })
            .count() as f32;
        input.preferred * (1.0 / (1.0 + self.density_speed_factor * crowding))
    }
}

impl AvoidanceSolver for SampledVelocitySolver {
    fn name(&self) -> &'static str {
        "sampled_velocity"
    }

    fn solve(&self, input: &AvoidanceInput<'_>) -> AvoidanceOutput {
        let preferred = self
            .density_adjusted_preferred(input)
            .clamp_length(input.max_speed);

        // A stationary agent with no goal has nothing to solve, and sampling
        // headings around a zero vector would be meaningless.
        if preferred.length_squared() <= f32::MIN_POSITIVE
            && input.velocity.length_squared() <= f32::MIN_POSITIVE
        {
            return AvoidanceOutput {
                velocity: Vec2::ZERO,
                status: SolverStatus::Free,
            };
        }

        let preferred_speed = preferred.length();
        let heading = if preferred_speed > f32::MIN_POSITIVE {
            preferred.normalize_or_zero()
        } else {
            input.velocity.normalize_or_zero()
        };

        let mut best_velocity = Vec2::ZERO;
        let mut best_cost = f32::INFINITY;
        let mut best_ttc = f32::INFINITY;

        // Candidate generation order is fixed, so ties resolve identically on
        // every run. The preferred velocity is evaluated first so an
        // unobstructed agent keeps it exactly.
        let evaluate =
            |candidate: Vec2, best_velocity: &mut Vec2, best_cost: &mut f32, best_ttc: &mut f32| {
                let (collision_cost, ttc) = self.collision_cost(input, candidate);
                let cost = self.goal_weight * (candidate - preferred).length()
                    + self.smoothness_weight * (candidate - input.velocity).length()
                    + collision_cost
                    + self.side_bias_cost(input, candidate);
                // Strict `<` means the first candidate evaluated wins a tie, and
                // candidate order is fixed, so ties resolve identically every run.
                if cost < *best_cost {
                    *best_cost = cost;
                    *best_velocity = candidate;
                    *best_ttc = ttc;
                }
            };

        evaluate(preferred, &mut best_velocity, &mut best_cost, &mut best_ttc);

        let speed_reference = preferred_speed.max(input.velocity.length());
        for speed_index in 1..=self.speed_samples {
            let speed = speed_reference * speed_index as f32 / self.speed_samples as f32;
            for heading_index in 0..self.heading_samples {
                let angle =
                    std::f32::consts::TAU * heading_index as f32 / self.heading_samples as f32;
                let direction = rotate(heading, angle);
                evaluate(
                    direction * speed,
                    &mut best_velocity,
                    &mut best_cost,
                    &mut best_ttc,
                );
            }
        }

        // Stopping is always available. It is the contract's graceful fallback
        // when no feasible velocity exists.
        evaluate(
            Vec2::ZERO,
            &mut best_velocity,
            &mut best_cost,
            &mut best_ttc,
        );

        let status = if best_velocity.length() < preferred_speed * self.brake_speed_fraction {
            SolverStatus::Braking
        } else if (best_velocity - preferred).length() > 1e-3 {
            SolverStatus::Avoiding
        } else {
            SolverStatus::Free
        };

        AvoidanceOutput {
            velocity: best_velocity.clamp_length(input.max_speed),
            status,
        }
    }
}

fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Segment;
    use crate::ids::AgentId;
    use crate::units::Vec2;

    fn solver() -> SampledVelocitySolver {
        SampledVelocitySolver::default()
    }

    fn input<'a>(
        agent_id: u64,
        position: Vec2,
        velocity: Vec2,
        preferred: Vec2,
        neighbors: &'a [NeighborState],
        walls: &'a [Segment],
    ) -> AvoidanceInput<'a> {
        AvoidanceInput {
            agent_id: AgentId(agent_id),
            position,
            velocity,
            preferred,
            radius: 0.3,
            max_speed: 2.0,
            neighbors,
            walls,
        }
    }

    #[test]
    fn an_unobstructed_agent_keeps_its_preferred_velocity() {
        let preferred = Vec2::new(1.35, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &[]));
        assert_eq!(out.status, SolverStatus::Free);
        assert!((out.velocity - preferred).length() < 1e-4);
    }

    #[test]
    fn a_stopped_agent_with_no_goal_stays_stopped() {
        let out = solver().solve(&input(1, Vec2::ZERO, Vec2::ZERO, Vec2::ZERO, &[], &[]));
        assert_eq!(out.velocity, Vec2::ZERO);
        assert_eq!(out.status, SolverStatus::Free);
    }

    #[test]
    fn a_head_on_neighbor_deflects_the_agent() {
        let neighbors = [NeighborState {
            position: Vec2::new(4.0, 0.0),
            velocity: Vec2::new(-1.35, 0.0),
            radius: 0.3,
            agent_id: AgentId(2),
        }];
        let preferred = Vec2::new(1.35, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &neighbors, &[]));
        assert_ne!(out.status, SolverStatus::Free);
        assert!(
            out.velocity.y.abs() > 0.05,
            "no lateral deflection: {out:?}"
        );
    }

    #[test]
    fn head_on_agents_choose_opposite_sides() {
        // Deterministic separation, contract section 6.2. Both agents run the
        // same solver on mirrored inputs; if they deflected the same way in
        // world space they would stay on a collision course.
        let a_neighbors = [NeighborState {
            position: Vec2::new(4.0, 0.0),
            velocity: Vec2::new(-1.35, 0.0),
            radius: 0.3,
            agent_id: AgentId(2),
        }];
        let b_neighbors = [NeighborState {
            position: Vec2::new(0.0, 0.0),
            velocity: Vec2::new(1.35, 0.0),
            radius: 0.3,
            agent_id: AgentId(1),
        }];
        let a = solver().solve(&input(
            1,
            Vec2::ZERO,
            Vec2::new(1.35, 0.0),
            Vec2::new(1.35, 0.0),
            &a_neighbors,
            &[],
        ));
        let b = solver().solve(&input(
            2,
            Vec2::new(4.0, 0.0),
            Vec2::new(-1.35, 0.0),
            Vec2::new(-1.35, 0.0),
            &b_neighbors,
            &[],
        ));
        assert!(
            a.velocity.y * b.velocity.y < 0.0,
            "agents chose the same world-space side: a={:?} b={:?}",
            a.velocity,
            b.velocity
        );
    }

    #[test]
    fn head_on_side_choice_does_not_depend_on_id_ordering() {
        // The head-on rule must be a *fixed* convention, not an ID
        // comparison. Two agents meeting head-on see mirrored geometry, so if
        // the side were derived from "am I the lower ID?", they would derive
        // opposite answers, deflect the same way in world space, and stay on
        // a collision course.
        let neighbors_low = [NeighborState {
            position: Vec2::new(4.0, 0.0),
            velocity: Vec2::new(-1.35, 0.0),
            radius: 0.3,
            agent_id: AgentId(99),
        }];
        let neighbors_high = [NeighborState {
            position: Vec2::new(4.0, 0.0),
            velocity: Vec2::new(-1.35, 0.0),
            radius: 0.3,
            agent_id: AgentId(1),
        }];
        let preferred = Vec2::new(1.35, 0.0);
        let lower_id = solver().solve(&input(
            5,
            Vec2::ZERO,
            preferred,
            preferred,
            &neighbors_low,
            &[],
        ));
        let higher_id = solver().solve(&input(
            5,
            Vec2::ZERO,
            preferred,
            preferred,
            &neighbors_high,
            &[],
        ));
        assert!(
            lower_id.velocity.y * higher_id.velocity.y > 0.0,
            "head-on side must be a fixed convention: {:?} vs {:?}",
            lower_id.velocity,
            higher_id.velocity
        );
    }

    #[test]
    fn the_higher_id_yields_more_in_a_crossing_conflict() {
        // A perpendicular conflict is symmetric, so the keep-left convention
        // is degenerate there. Stable IDs supply the asymmetry: the higher ID
        // gives way. This is contract section 6.2's deterministic
        // tie-breaking.
        let crossing_neighbor = |id: u64| {
            [NeighborState {
                position: Vec2::new(2.0, -2.0),
                velocity: Vec2::new(0.0, 1.35),
                radius: 0.3,
                agent_id: AgentId(id),
            }]
        };
        let preferred = Vec2::new(1.35, 0.0);
        let lower = solver().solve(&input(
            10,
            Vec2::ZERO,
            preferred,
            preferred,
            &crossing_neighbor(20),
            &[],
        ));
        let higher = solver().solve(&input(
            30,
            Vec2::ZERO,
            preferred,
            preferred,
            &crossing_neighbor(20),
            &[],
        ));
        assert!(
            higher.velocity.length() <= lower.velocity.length(),
            "the higher ID must not push harder: lower={:?} higher={:?}",
            lower.velocity,
            higher.velocity
        );
    }

    #[test]
    fn a_wall_ahead_deflects_the_agent() {
        let walls = [Segment::new(Vec2::new(3.0, -5.0), Vec2::new(3.0, 5.0))];
        let preferred = Vec2::new(1.35, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &walls));
        assert_ne!(out.status, SolverStatus::Free);
        assert!(out.velocity.x < preferred.x, "agent drove into the wall");
    }

    #[test]
    fn a_boxed_in_agent_brakes_rather_than_escaping() {
        // Walls on all four sides: no candidate is safe, so the contract's
        // graceful fallback applies.
        let walls = [
            Segment::new(Vec2::new(0.8, -2.0), Vec2::new(0.8, 2.0)),
            Segment::new(Vec2::new(-2.0, 0.8), Vec2::new(2.0, 0.8)),
            Segment::new(Vec2::new(-2.0, -0.8), Vec2::new(2.0, -0.8)),
            Segment::new(Vec2::new(-0.8, -2.0), Vec2::new(-0.8, 2.0)),
        ];
        let preferred = Vec2::new(1.35, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &walls));
        assert_eq!(out.status, SolverStatus::Braking);
        assert!(out.velocity.length() < preferred.length());
    }

    #[test]
    fn an_agent_inside_a_wall_is_given_a_way_out() {
        // Regression: `time_to_collision_segment` reports zero for every
        // candidate once the agent is inside the wall, so a penalty derived
        // from it is a constant that cancels out of the argmin. The wall goes
        // invisible and the agent can steer deeper in. The escape direction
        // must be strictly cheaper than driving further in.
        let wall = [Segment::new(Vec2::new(0.0, -5.0), Vec2::new(0.0, 5.0))];
        // Straddling the wall: 0.1 to its right against a radius of 0.3.
        let position = Vec2::new(0.1, 0.0);
        let out = solver().solve(&input(
            1,
            position,
            Vec2::ZERO,
            // Preferred velocity points *into* the wall, so only the wall term
            // can produce an outward answer.
            Vec2::new(-1.35, 0.0),
            &[],
            &wall,
        ));
        assert!(
            out.velocity.x >= 0.0,
            "steered deeper into the wall: {:?}",
            out.velocity
        );
    }

    #[test]
    fn the_solution_never_exceeds_max_speed() {
        let preferred = Vec2::new(100.0, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, Vec2::ZERO, preferred, &[], &[]));
        assert!(
            out.velocity.length() <= 2.0 + 1e-4,
            "got {}",
            out.velocity.length()
        );
    }

    #[test]
    fn dense_neighbors_reduce_speed() {
        // Contract section 6.2 density-aware speed reduction.
        let crowd: Vec<NeighborState> = (0..8)
            .map(|i| NeighborState {
                position: Vec2::from_yaw(i as f32) * 0.75,
                velocity: Vec2::ZERO,
                radius: 0.3,
                agent_id: AgentId(100 + i as u64),
            })
            .collect();
        let preferred = Vec2::new(1.35, 0.0);
        let sparse = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &[]));
        let dense = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &crowd, &[]));
        assert!(
            dense.velocity.length() < sparse.velocity.length(),
            "density did not slow the agent"
        );
    }

    #[test]
    fn the_output_is_always_finite() {
        let neighbors = [NeighborState {
            position: Vec2::ZERO,
            velocity: Vec2::ZERO,
            radius: 0.3,
            agent_id: AgentId(2),
        }];
        let walls = [Segment::new(Vec2::ZERO, Vec2::ZERO)];
        let out = solver().solve(&input(
            1,
            Vec2::ZERO,
            Vec2::ZERO,
            Vec2::new(1.35, 0.0),
            &neighbors,
            &walls,
        ));
        assert!(out.velocity.is_finite(), "got {:?}", out.velocity);
    }

    #[test]
    fn solving_is_deterministic_for_identical_input() {
        let neighbors = [NeighborState {
            position: Vec2::new(2.0, 0.5),
            velocity: Vec2::new(-1.0, 0.0),
            radius: 0.3,
            agent_id: AgentId(2),
        }];
        let preferred = Vec2::new(1.35, 0.0);
        let first = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &neighbors, &[]));
        let second = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &neighbors, &[]));
        assert_eq!(first.velocity, second.velocity);
        assert_eq!(first.status, second.status);
    }

    #[test]
    fn the_solver_reports_its_name() {
        assert_eq!(solver().name(), "sampled_velocity");
    }
}
