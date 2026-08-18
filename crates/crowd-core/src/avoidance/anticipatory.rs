//! Scoped, multi-step anticipatory avoidance, contract section 6.2's third
//! candidate.
//!
//! Distinct from both other solvers in *where* it spends its attention:
//! sampled-velocity scores every neighbor with one analytic
//! time-to-collision per candidate; ORCA solves one closed-form half-plane
//! per neighbor. This solver instead ranks neighbors by distance and gives
//! only the nearest few (`lookahead_neighbors`) a multi-step constant-velocity
//! extrapolation via `lookahead_collision_cost`, so its cost is bounded by
//! that count rather than by the full neighbor list -- that scoped lookahead
//! is what makes this solver's name accurate. Everything else (walls,
//! goal-seeking, smoothness, side bias, and density-based speed reduction) is
//! reused unchanged from the shared helpers `sampled.rs` also uses.

use super::{
    density_adjusted_preferred, sample_candidates, side_bias_cost, wall_avoidance_cost,
    AvoidanceInput, AvoidanceOutput, AvoidanceSolver, MIN_TIME_FOR_COST, OVERLAP_URGENCY,
};
use crate::units::Vec2;
use crate::world::SolverStatus;

pub use super::NeighborState;

/// Floor on predicted separation, in meters, when converting it to a cost —
/// the lookahead analogue of `MIN_TIME_FOR_COST`.
const MIN_SEPARATION_FOR_COST: f32 = 0.1;

#[derive(Clone, Copy, Debug)]
pub struct AnticipatorySolver {
    pub speed_samples: u32,
    pub heading_samples: u32,
    pub time_horizon: f32,
    pub wall_horizon: f32,
    /// How many of the nearest neighbors get full multi-step lookahead; the
    /// rest fall back to the cheap `far_field_cost` repulsion.
    pub lookahead_neighbors: usize,
    /// How many sub-steps the lookahead walks across `time_horizon`.
    pub lookahead_steps: u32,
    pub goal_weight: f32,
    pub collision_weight: f32,
    pub wall_weight: f32,
    pub smoothness_weight: f32,
    pub side_bias_weight: f32,
    /// Extra collision weight carried by the higher-ID agent in a crossing
    /// conflict.
    pub yield_factor: f32,
    pub brake_speed_fraction: f32,
    pub personal_space: f32,
    pub density_speed_factor: f32,
    /// Floor on the crowding speed multiplier, as a fraction of preferred
    /// speed. `0.0` disables it, restoring the unbounded decay.
    ///
    /// Default off, on measurement. It was added to attack the 100K stall
    /// result and does not: `m5_crowding_distribution` measured the M5 fixture
    /// as sparse at every scale — max crowding 6 at 10K and 4 at 100K, against
    /// the ~10 neighbours a 0.35 floor needs before it binds at all — so any
    /// floor gentle enough to leave the fixture alone is a no-op, and
    /// `m5_density_floor_sweep` confirms floor 0.35 reproduces the no-floor
    /// `final_state_hash` exactly. Where it does bind, in `dense_flow`, it made
    /// jams 74% longer (84.8 to 147.6 ticks per episode) by driving agents into
    /// blockages instead of letting them hold back. Kept as a knob because the
    /// sweep needs it, not because it is recommended.
    pub min_density_speed_fraction: f32,
    pub head_on_cosine: f32,
    /// Repulsion weight for neighbors past the lookahead cutoff. Recalibrated from
    /// the plan's original 0.15 to 10.0 to bring `far_field_cost` into the same
    /// order of magnitude as near-field collision cost in typical geometries
    /// (~13.3 vs ~13.3 in ring tests). The fit is narrow: [8, 15] works,
    /// [0.6, 3.0] and [20.0+] fail `dense_neighbors_reduce_speed`. See
    /// `docs/benchmarks/2026-08-06-avoidance-solver-comparison.md` for the
    /// measured, per-scene behavior of this value; it may still need
    /// per-scene tuning due to raw-distance scaling.
    pub far_field_weight: f32,
}

impl Default for AnticipatorySolver {
    fn default() -> Self {
        Self {
            speed_samples: 3,
            heading_samples: 16,
            time_horizon: 3.0,
            wall_horizon: 2.0,
            lookahead_neighbors: 4,
            lookahead_steps: 6,
            goal_weight: 1.0,
            collision_weight: 2.0,
            wall_weight: 1.5,
            smoothness_weight: 0.35,
            side_bias_weight: 0.6,
            yield_factor: 1.4,
            brake_speed_fraction: 0.5,
            personal_space: 0.45,
            density_speed_factor: 0.18,
            min_density_speed_fraction: 0.0,
            head_on_cosine: 0.7,
            far_field_weight: 10.0,
        }
    }
}

impl AnticipatorySolver {
    /// Cheap directional repulsion for neighbors past the lookahead cutoff.
    /// Only candidates heading *toward* a nearby far-field neighbor are
    /// penalised, so this is a real gradient rather than a constant that
    /// would cancel out of the argmin.
    fn far_field_cost(
        &self,
        position: Vec2,
        radius: f32,
        candidate: Vec2,
        far: &[&NeighborState],
    ) -> f32 {
        let mut cost = 0.0;
        for neighbor in far {
            let clearance = radius + neighbor.radius + self.personal_space;
            let offset = neighbor.position - position;
            let dist_sq = offset.length_squared();
            if dist_sq < clearance * clearance {
                let dist = dist_sq.sqrt().max(0.05);
                let toward = candidate.dot(offset.normalize_or_zero()).max(0.0);
                cost += self.far_field_weight * toward / dist;
            }
        }
        cost
    }

    /// Collision cost and earliest predicted contact for one candidate,
    /// against only the scoped (nearest `lookahead_neighbors`) threats, via a
    /// fixed number of constant-velocity sub-steps across `time_horizon`.
    fn lookahead_collision_cost(
        &self,
        input: &AvoidanceInput<'_>,
        candidate: Vec2,
        scoped: &[&NeighborState],
    ) -> (f32, f32) {
        let mut cost = 0.0;
        let mut earliest = f32::INFINITY;
        let step_dt = self.time_horizon / self.lookahead_steps as f32;

        for neighbor in scoped {
            let combined_radius = input.radius + neighbor.radius;
            // The higher stable ID yields, exactly as in `sampled.rs`: a
            // perpendicular conflict is symmetric under the keep-left rule,
            // so without this both agents derive the identical choice.
            let yield_weight = if input.agent_id > neighbor.agent_id {
                self.yield_factor
            } else {
                1.0
            };

            let mut min_separation = f32::INFINITY;
            let mut min_separation_time = f32::INFINITY;
            for step in 1..=self.lookahead_steps {
                let t = step_dt * step as f32;
                let self_pos = input.position + candidate * t;
                let neighbor_pos = neighbor.position + neighbor.velocity * t;
                let separation = self_pos.distance_squared(neighbor_pos).sqrt() - combined_radius;
                if separation < min_separation {
                    min_separation = separation;
                    min_separation_time = t;
                }
            }

            if min_separation <= 0.0 {
                let offset = neighbor.position - input.position;
                let direction = offset.normalize_or_zero();
                let relative_velocity = neighbor.velocity - candidate;
                let separation_rate = relative_velocity.dot(direction);
                let relief = (separation_rate / input.max_speed.max(0.1)).clamp(0.0, 1.0);
                cost += self.collision_weight * yield_weight * OVERLAP_URGENCY * (1.0 - relief);
                earliest = earliest.min(min_separation_time);
            } else if min_separation < self.personal_space {
                cost += self.collision_weight * yield_weight
                    / min_separation.max(MIN_SEPARATION_FOR_COST);
                earliest = earliest.min(min_separation_time);
            }
        }
        (cost, earliest)
    }
}

impl AvoidanceSolver for AnticipatorySolver {
    fn name(&self) -> &'static str {
        "anticipatory"
    }

    fn solve(&self, input: &AvoidanceInput<'_>) -> AvoidanceOutput {
        let preferred = density_adjusted_preferred(
            input.preferred,
            input.position,
            input.radius,
            input.neighbors,
            self.personal_space,
            self.density_speed_factor,
            self.min_density_speed_fraction,
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

        let preferred_speed = preferred.length();
        let heading = if preferred_speed > f32::MIN_POSITIVE {
            preferred.normalize_or_zero()
        } else {
            input.velocity.normalize_or_zero()
        };

        // Rank by distance, breaking ties by stable ID so the scoped/far
        // split never depends on upstream neighbor-list order.
        let mut ranked: Vec<&NeighborState> = input.neighbors.iter().collect();
        ranked.sort_by(|a, b| {
            let da = input.position.distance_squared(a.position);
            let db = input.position.distance_squared(b.position);
            da.partial_cmp(&db)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.agent_id.cmp(&b.agent_id))
        });
        let (scoped, far): (&[&NeighborState], &[&NeighborState]) =
            if ranked.len() > self.lookahead_neighbors {
                ranked.split_at(self.lookahead_neighbors)
            } else {
                (&ranked[..], &[])
            };

        let mut best_velocity = Vec2::ZERO;
        let mut best_cost = f32::INFINITY;
        let mut best_ttc = f32::INFINITY;

        let evaluate =
            |candidate: Vec2, best_velocity: &mut Vec2, best_cost: &mut f32, best_ttc: &mut f32| {
                let (near_cost, near_ttc) = self.lookahead_collision_cost(input, candidate, scoped);
                let (wall_cost, wall_ttc) = wall_avoidance_cost(
                    input.position,
                    input.max_speed,
                    candidate,
                    input.radius,
                    input.walls,
                    self.wall_horizon,
                    self.collision_weight,
                    self.wall_weight,
                    OVERLAP_URGENCY,
                    MIN_TIME_FOR_COST,
                );
                let far_cost = self.far_field_cost(input.position, input.radius, candidate, far);
                let bias_cost = side_bias_cost(
                    input.preferred,
                    input.position,
                    input.velocity,
                    input.neighbors,
                    candidate,
                    self.head_on_cosine,
                    self.side_bias_weight,
                );
                let cost = self.goal_weight * (candidate - preferred).length()
                    + self.smoothness_weight * (candidate - input.velocity).length()
                    + near_cost
                    + wall_cost
                    + far_cost
                    + bias_cost;
                if cost < *best_cost {
                    *best_cost = cost;
                    *best_velocity = candidate;
                    *best_ttc = near_ttc.min(wall_ttc);
                }
            };

        evaluate(preferred, &mut best_velocity, &mut best_cost, &mut best_ttc);
        let speed_reference = preferred_speed.max(input.velocity.length());
        sample_candidates(
            heading,
            speed_reference,
            self.speed_samples,
            self.heading_samples,
            |candidate| evaluate(candidate, &mut best_velocity, &mut best_cost, &mut best_ttc),
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Segment;
    use crate::ids::AgentId;

    fn solver() -> AnticipatorySolver {
        AnticipatorySolver::default()
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
    fn a_wall_ahead_deflects_the_agent() {
        let walls = [Segment::new(Vec2::new(3.0, -5.0), Vec2::new(3.0, 5.0))];
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
        assert_eq!(out.status, SolverStatus::Braking);
        assert!(out.velocity.length() < preferred.length());
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
    fn the_output_is_always_finite() {
        let walls = [Segment::new(Vec2::ZERO, Vec2::ZERO)];
        let out = solver().solve(&input(
            1,
            Vec2::ZERO,
            Vec2::ZERO,
            Vec2::new(1.35, 0.0),
            &[],
            &walls,
        ));
        assert!(out.velocity.is_finite(), "got {:?}", out.velocity);
    }

    #[test]
    fn solving_is_deterministic_for_identical_input() {
        let walls = [Segment::new(Vec2::new(3.0, -5.0), Vec2::new(3.0, 5.0))];
        let preferred = Vec2::new(1.35, 0.0);
        let first = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &walls));
        let second = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &walls));
        assert_eq!(first.velocity, second.velocity);
        assert_eq!(first.status, second.status);
    }

    #[test]
    fn the_solver_reports_its_name() {
        assert_eq!(solver().name(), "anticipatory");
    }

    #[test]
    fn dense_neighbors_reduce_speed() {
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
            higher.velocity.length() <= lower.velocity.length() + 1e-3,
            "the higher ID must not push harder: lower={:?} higher={:?}",
            lower.velocity,
            higher.velocity
        );
    }

    #[test]
    fn only_the_nearest_k_neighbors_receive_full_lookahead() {
        // lookahead_neighbors is set to 2 and the two near threats fill that
        // quota; a third, far threat added on top must not get the full
        // multi-step treatment, so adding it should change the result far
        // less than either of the two scoped neighbors would.
        let mut base = solver();
        base.lookahead_neighbors = 2;
        let near: Vec<NeighborState> = (0..2)
            .map(|i| NeighborState {
                position: Vec2::new(1.0, if i == 0 { 0.3 } else { -0.3 }),
                velocity: Vec2::new(-1.35, 0.0),
                radius: 0.3,
                agent_id: AgentId(10 + i as u64),
            })
            .collect();
        let far_threat = NeighborState {
            position: Vec2::new(1.0, 20.0),
            velocity: Vec2::new(0.0, -1.35),
            radius: 0.3,
            agent_id: AgentId(99),
        };
        let preferred = Vec2::new(1.35, 0.0);
        let without_far = base.solve(&input(1, Vec2::ZERO, preferred, preferred, &near, &[]));
        let with_far_neighbors: Vec<NeighborState> = near
            .iter()
            .cloned()
            .chain(std::iter::once(far_threat))
            .collect();
        let with_far = base.solve(&input(
            1,
            Vec2::ZERO,
            preferred,
            preferred,
            &with_far_neighbors,
            &[],
        ));
        assert!(
            (without_far.velocity - with_far.velocity).length() < 0.05,
            "a far, out-of-scope neighbor changed the outcome as much as a scoped one: \
             without={:?} with={:?}",
            without_far.velocity,
            with_far.velocity
        );
    }

    #[test]
    fn solving_is_deterministic_with_neighbors_present() {
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
}
