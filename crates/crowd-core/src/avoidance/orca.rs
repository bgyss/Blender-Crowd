//! From-scratch reciprocal velocity obstacle (ORCA) avoidance, contract
//! section 6.2's second candidate.
//!
//! Unlike the sampled-velocity solver, this does not score many candidate
//! velocities: it builds one half-plane constraint per neighbor and per wall
//! directly in velocity space (the standard ORCA construction), then solves
//! for the feasible velocity closest to the preferred one via the sequential
//! linear program in `super::linear_program`.

use super::linear_program::{self, Line};
use super::{density_adjusted_preferred, is_head_on_encounter, rotate};
use super::{AvoidanceInput, AvoidanceOutput, AvoidanceSolver, NeighborState};
use crate::geometry::Segment;
use crate::ids::AgentId;
use crate::units::Vec2;
use crate::world::SolverStatus;

/// Assumed step, in seconds, used only when two discs already overlap: the
/// standard ORCA treatment of an existing penetration replaces the time
/// horizon with a short fixed step so the correction is proportionally more
/// urgent. Matches the kernel's default tick rate; it does not need to track
/// the actual tick length; it only shapes how sharply an overlap is
/// penalised.
const COLLISION_TIME_STEP: f32 = 1.0 / 30.0;

/// Fixed rotation applied to a head-on neighbor's constraint line so the
/// perfectly symmetric case resolves to one side rather than leaving both
/// mirrored agents to settle it independently (and possibly identically in
/// world space). A convention, like `sampled.rs`'s keep-left bias -- not an
/// ID comparison, which would fail for the same mirrored-geometry reason
/// documented there.
const HEAD_ON_TIE_BREAK_RADIANS: f32 = 0.03;

#[derive(Clone, Copy, Debug)]
/// Reciprocal velocity obstacle (ORCA) avoidance solver.
///
/// # Known limitation: no ID-based tie-break for symmetric crossing conflicts
///
/// `OrcaSolver` has no asymmetry-breaking mechanism for perpendicular crossing
/// conflicts—only for head-on encounters. `HEAD_ON_TIE_BREAK_RADIANS` applies
/// only to meetings detected as head-on; general perpendicular crossings receive
/// no ID-based correction. Unlike `sampled_velocity` and `anticipatory`, which
/// both apply a `yield_factor` scaled by stable ID to break symmetry in
/// crossing conflicts, `OrcaSolver` offers no equivalent tie-break. Two agents
/// in a perfectly symmetric perpendicular crossing may compute identical
/// corrections and fail to separate via identity alone. This gap does not block
/// this slice's conclusion—`sampled_velocity` is the selected default, not
/// `orca`—but should be resolved before `OrcaSolver` is considered for promotion
/// to default.
pub struct OrcaSolver {
    /// How far ahead, in seconds, a neighbor's velocity obstacle is built.
    pub time_horizon: f32,
    /// Wall lookahead, in seconds. Shorter than `time_horizon` for the same
    /// reason `sampled.rs` gives: static geometry is avoided by turning, not
    /// long-range planning.
    pub wall_horizon: f32,
    pub head_on_cosine: f32,
    pub brake_speed_fraction: f32,
    pub personal_space: f32,
    pub density_speed_factor: f32,
}

impl Default for OrcaSolver {
    fn default() -> Self {
        Self {
            time_horizon: 3.0,
            wall_horizon: 2.0,
            head_on_cosine: 0.7,
            brake_speed_fraction: 0.5,
            personal_space: 0.45,
            density_speed_factor: 0.18,
        }
    }
}

fn det(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// One ORCA half-plane induced by `neighbor`, in this agent's own velocity
/// space, with each side credited half the necessary correction.
fn orca_line_for_neighbor(
    position: Vec2,
    self_velocity: Vec2,
    radius: f32,
    neighbor: &NeighborState,
    time_horizon: f32,
) -> Line {
    let relative_position = neighbor.position - position;
    let relative_velocity = self_velocity - neighbor.velocity;
    let combined_radius = radius + neighbor.radius;
    let dist_sq = relative_position.length_squared();
    let combined_radius_sq = combined_radius * combined_radius;

    let (direction, u) = if dist_sq > combined_radius_sq {
        let inv_time_horizon = 1.0 / time_horizon;
        let w = relative_velocity - relative_position * inv_time_horizon;
        let w_length_sq = w.length_squared();
        let dot1 = w.dot(relative_position);

        if dot1 < 0.0 && dot1 * dot1 > combined_radius_sq * w_length_sq {
            // Relative velocity is inside the truncated cone's near side:
            // project onto the cutoff circle.
            let w_length = w_length_sq.sqrt();
            let unit_w = w * (1.0 / w_length);
            let direction = Vec2::new(unit_w.y, -unit_w.x);
            let u = unit_w * (combined_radius * inv_time_horizon - w_length);
            (direction, u)
        } else {
            // Project onto whichever leg of the cone is nearer.
            let leg = (dist_sq - combined_radius_sq).sqrt();
            let direction = if det(relative_position, w) > 0.0 {
                Vec2::new(
                    relative_position.x * leg - relative_position.y * combined_radius,
                    relative_position.x * combined_radius + relative_position.y * leg,
                ) * (1.0 / dist_sq)
            } else {
                -Vec2::new(
                    relative_position.x * leg + relative_position.y * combined_radius,
                    -relative_position.x * combined_radius + relative_position.y * leg,
                ) * (1.0 / dist_sq)
            };
            let u = direction * relative_velocity.dot(direction) - relative_velocity;
            (direction, u)
        }
    } else {
        // Already overlapping: use the short collision time step instead of
        // the full horizon, matching an existing penetration's urgency.
        let inv_step = 1.0 / COLLISION_TIME_STEP;
        let w = relative_velocity - relative_position * inv_step;
        let w_length = w.length();
        let unit_w = if w_length > f32::MIN_POSITIVE {
            w * (1.0 / w_length)
        } else {
            Vec2::new(0.0, 1.0)
        };
        let direction = Vec2::new(unit_w.y, -unit_w.x);
        let u = unit_w * (combined_radius * inv_step - w_length);
        (direction, u)
    };

    Line {
        point: self_velocity + u * 0.5,
        direction,
    }
}

/// A wall's ORCA half-plane, built by treating the wall's closest point to
/// the agent as a zero-radius, zero-velocity neighbor (the agent's own
/// radius supplies the clearance), then undoing the reciprocal halving --
/// walls never yield, so the agent's own constraint carries the full
/// correction.
fn orca_line_for_wall(
    position: Vec2,
    self_velocity: Vec2,
    radius: f32,
    wall: &Segment,
    wall_horizon: f32,
) -> Option<Line> {
    let closest = wall.closest_point(position);
    if (closest - position).length_squared() <= f32::MIN_POSITIVE {
        return None;
    }
    let proxy = NeighborState {
        position: closest,
        velocity: Vec2::ZERO,
        radius: 0.0,
        agent_id: AgentId(u64::MAX),
    };
    let line = orca_line_for_neighbor(position, self_velocity, radius, &proxy, wall_horizon);
    Some(Line {
        point: self_velocity + (line.point - self_velocity) * 2.0,
        direction: line.direction,
    })
}

impl AvoidanceSolver for OrcaSolver {
    fn name(&self) -> &'static str {
        "orca"
    }

    fn solve(&self, input: &AvoidanceInput<'_>) -> AvoidanceOutput {
        let preferred = density_adjusted_preferred(
            input.preferred,
            input.position,
            input.radius,
            input.neighbors,
            self.personal_space,
            self.density_speed_factor,
        )
        .clamp_length(input.max_speed);

        if preferred.length_squared() <= f32::MIN_POSITIVE
            && input.velocity.length_squared() <= f32::MIN_POSITIVE
        {
            return AvoidanceOutput {
                velocity: Vec2::ZERO,
                status: SolverStatus::Free,
            };
        }

        let heading = input.preferred.normalize_or_zero();

        // Fixed order by stable ID: the LP's infeasible-fallback path
        // depends on constraint order, so this must not depend on upstream
        // neighbor-list order.
        let mut ordered: Vec<&NeighborState> = input.neighbors.iter().collect();
        ordered.sort_by_key(|n| n.agent_id);

        let mut lines = Vec::with_capacity(ordered.len() + input.walls.len());
        for neighbor in ordered {
            let mut line = orca_line_for_neighbor(
                input.position,
                input.velocity,
                input.radius,
                neighbor,
                self.time_horizon,
            );
            if is_head_on_encounter(
                heading,
                input.position,
                input.velocity,
                neighbor,
                self.head_on_cosine,
            ) {
                line.direction = rotate(line.direction, HEAD_ON_TIE_BREAK_RADIANS);
            }
            lines.push(line);
        }
        for wall in input.walls {
            if let Some(line) = orca_line_for_wall(
                input.position,
                input.velocity,
                input.radius,
                wall,
                self.wall_horizon,
            ) {
                lines.push(line);
            }
        }

        let solved = linear_program::solve(&lines, input.max_speed, preferred);
        let preferred_speed = preferred.length();

        let status = if solved.length() < preferred_speed * self.brake_speed_fraction {
            SolverStatus::Braking
        } else if (solved - preferred).length() > 1e-3 {
            SolverStatus::Avoiding
        } else {
            SolverStatus::Free
        };

        AvoidanceOutput {
            velocity: solved.clamp_length(input.max_speed),
            status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Vec2;

    fn solver() -> OrcaSolver {
        OrcaSolver::default()
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
        assert!((out.velocity - preferred).length() < 1e-3);
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
            out.velocity.y.abs() > 0.01,
            "no lateral deflection: {out:?}"
        );
    }

    #[test]
    fn head_on_agents_choose_opposite_sides() {
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
    fn a_wall_ahead_deflects_the_agent() {
        // Wall at x = 3.0 (the brief's literal value) sits at an exact ORCA
        // tangent boundary for this preferred speed, radius, and
        // `wall_horizon`: 1.35 * 2.0 + 0.3 == 3.0 to the millimeter, so the
        // agent's relative velocity lands exactly on the truncated cone's
        // cutoff circle. The required correction is then mathematically
        // zero (not merely small), and gets rounded away entirely by f32,
        // leaving the agent on its preferred velocity -- correct ORCA
        // behavior for an exact tangent, but not what this test means to
        // exercise. x = 2.5 gives the same "wall clearly ahead" scenario
        // with real margin inside the collision cone instead of sitting on
        // its boundary.
        let walls = [Segment::new(Vec2::new(2.5, -5.0), Vec2::new(2.5, 5.0))];
        let preferred = Vec2::new(1.35, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &walls));
        assert_ne!(out.status, SolverStatus::Free);
        assert!(out.velocity.x < preferred.x, "agent drove into the wall");
    }

    #[test]
    fn a_boxed_in_agent_brakes_rather_than_escaping() {
        let walls = [
            Segment::new(Vec2::new(0.8, -2.0), Vec2::new(0.8, 2.0)),
            Segment::new(Vec2::new(-2.0, 0.8), Vec2::new(2.0, 0.8)),
            Segment::new(Vec2::new(-2.0, -0.8), Vec2::new(2.0, -0.8)),
            Segment::new(Vec2::new(-0.8, -2.0), Vec2::new(-0.8, 2.0)),
        ];
        let preferred = Vec2::new(1.35, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &walls));
        assert!(out.velocity.is_finite());
        assert!(
            out.velocity.length() < preferred.length(),
            "did not brake in a fully boxed-in space: {out:?}"
        );
    }

    #[test]
    fn an_agent_inside_a_wall_is_given_a_way_out() {
        let wall = [Segment::new(Vec2::new(0.0, -5.0), Vec2::new(0.0, 5.0))];
        let position = Vec2::new(0.1, 0.0);
        let out = solver().solve(&input(
            1,
            position,
            Vec2::ZERO,
            Vec2::new(-1.35, 0.0),
            &[],
            &wall,
        ));
        assert!(
            out.velocity.x >= -1e-3,
            "steered deeper into the wall: {:?}",
            out.velocity
        );
    }

    #[test]
    fn the_solution_never_exceeds_max_speed() {
        let preferred = Vec2::new(100.0, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, Vec2::ZERO, preferred, &[], &[]));
        assert!(
            out.velocity.length() <= 2.0 + 1e-3,
            "got {}",
            out.velocity.length()
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
        assert_eq!(solver().name(), "orca");
    }
}
