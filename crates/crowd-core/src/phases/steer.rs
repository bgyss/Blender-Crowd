//! Tick phase 6: steer.
//!
//! Turns the decide phase's preferred velocity into a solved velocity by
//! running the avoidance solver against perceived neighbors and nearby walls.
//! Reads only previous-tick state, so the result does not depend on the order
//! agents are visited.

use crate::arena::NeighborArena;
use crate::avoidance::{AvoidanceInput, AvoidanceSolver, NeighborState};
use crate::geometry::{time_to_collision_disc, time_to_collision_segment, Segment};
use crate::scene::CompiledScene;
use crate::units::Vec2;
use crate::world::{SolverStatus, World};

/// Horizon used only for the raw, pre-avoidance wall threat estimate below.
/// Generous on purpose: this is a metrics figure, not a safety cutoff, so
/// erring toward reporting a threat is preferable to missing one.
const RAW_WALL_HORIZON: f32 = 10.0;

#[derive(Clone, Copy, Debug)]
pub struct SteerConfig {
    /// How far to look for walls. Should exceed the solver's wall horizon
    /// times the maximum speed, or an agent can turn into a wall it never saw.
    pub wall_query_radius: f32,
}

impl Default for SteerConfig {
    fn default() -> Self {
        Self {
            wall_query_radius: 3.0,
        }
    }
}

/// Reused buffers so the phase does not allocate after warmup.
#[derive(Clone, Debug, Default)]
pub struct SteerScratch {
    neighbors: Vec<NeighborState>,
    wall_indices: Vec<u32>,
    walls: Vec<Segment>,
}

/// Tick-level aggregates the metrics layer would otherwise recompute.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SteerReport {
    pub min_time_to_collision: f32,
    pub braking_agents: u32,
}

pub fn steer(
    world: &mut World,
    arena: &NeighborArena,
    scene: &CompiledScene,
    solver: &dyn AvoidanceSolver,
    config: &SteerConfig,
    scratch: &mut SteerScratch,
) -> SteerReport {
    let mut min_time_to_collision = f32::INFINITY;
    let mut braking_agents = 0;

    for slot in 0..world.len() {
        let position = Vec2::new(world.pos_x[slot], world.pos_y[slot]);

        scratch.neighbors.clear();
        for neighbor in arena.neighbors(slot) {
            let other = neighbor.slot as usize;
            scratch.neighbors.push(NeighborState {
                position: Vec2::new(world.pos_x[other], world.pos_y[other]),
                velocity: Vec2::new(world.vel_x[other], world.vel_y[other]),
                radius: world.radius[other],
                agent_id: world.agent_id[other],
            });
        }

        scene.wall_index.query(
            position,
            config.wall_query_radius,
            &mut scratch.wall_indices,
        );
        scratch.walls.clear();
        for &index in &scratch.wall_indices {
            scratch.walls.push(scene.walls[index as usize]);
        }

        // `des_vel` carries the decide phase's preferred velocity on the way
        // in and this phase's solution on the way out. Read it before the
        // overwrite below.
        let preferred = Vec2::new(world.des_vel_x[slot], world.des_vel_y[slot]);
        let velocity_now = Vec2::new(world.vel_x[slot], world.vel_y[slot]);

        let output = solver.solve(&AvoidanceInput {
            agent_id: world.agent_id[slot],
            position,
            velocity: velocity_now,
            preferred,
            radius: world.radius[slot],
            max_speed: world.max_speed[slot],
            neighbors: &scratch.neighbors,
            walls: &scratch.walls,
        });

        // A non-finite solution would propagate into position and poison the
        // whole bake, so refuse it here rather than letting it escape.
        debug_assert!(output.velocity.is_finite(), "solver produced {output:?}");
        let velocity = if output.velocity.is_finite() {
            output.velocity
        } else {
            Vec2::ZERO
        };

        world.des_vel_x[slot] = velocity.x;
        world.des_vel_y[slot] = velocity.y;

        world.solver_status[slot] = output.status;

        if output.status == SolverStatus::Braking {
            braking_agents += 1;
            world.stall_ticks[slot] = world.stall_ticks[slot].saturating_add(1);
        } else {
            world.stall_ticks[slot] = 0;
        }

        // `output.min_time_to_collision` is the chosen candidate's own risk,
        // which is legitimately infinite when avoidance fully succeeds. That
        // alone would make this report unable to see a near miss the solver
        // just steered around, so also probe the raw, pre-avoidance risk the
        // agent would have run into on its original preferred velocity.
        let mut raw_ttc = f32::INFINITY;
        for neighbor in &scratch.neighbors {
            let relative_position = neighbor.position - position;
            let relative_velocity = neighbor.velocity - preferred;
            let combined_radius = world.radius[slot] + neighbor.radius;
            if let Some(t) =
                time_to_collision_disc(relative_position, relative_velocity, combined_radius)
            {
                raw_ttc = raw_ttc.min(t);
            }
        }
        for wall in &scratch.walls {
            if let Some(t) = time_to_collision_segment(
                position,
                preferred,
                world.radius[slot],
                wall,
                RAW_WALL_HORIZON,
            ) {
                raw_ttc = raw_ttc.min(t);
            }
        }

        min_time_to_collision = min_time_to_collision
            .min(output.min_time_to_collision)
            .min(raw_ttc);
    }

    SteerReport {
        min_time_to_collision,
        braking_agents,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avoidance::SampledVelocitySolver;
    use crate::grid::UniformGrid;
    use crate::ids::AgentId;
    use crate::phases::perceive::{perceive, PerceiveConfig, PerceiveScratch};
    use crate::route::WaypointGraph;
    use crate::scene::{Destination, PopulationParams, SceneDef, SpawnRegion};
    use crate::units::{Aabb, Vec2};
    use crate::world::{AgentSpawn, SolverStatus, World, NO_ROUTE};

    fn open_scene(walls: Vec<Segment>) -> CompiledScene {
        let mut waypoints = WaypointGraph::new();
        let a = waypoints.add_node(Vec2::new(1.0, 5.0));
        let b = waypoints.add_node(Vec2::new(9.0, 5.0));
        waypoints.add_edge(a, b);
        SceneDef {
            name: "steer_test".into(),
            bounds: Aabb::new(Vec2::new(-20.0, -20.0), Vec2::new(20.0, 20.0)),
            walls,
            waypoints,
            destinations: vec![Destination {
                name: "exit".into(),
                node: b,
            }],
            spawns: vec![SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(0.5, 4.0), Vec2::new(1.5, 6.0)),
                count: 1,
                per_tick: 1,
                destination: 0,
            }],
            populations: vec![PopulationParams::default()],
            project_seed: 1,
            ticks_per_second: 30,
            duration_ticks: 10,
        }
        .compile()
        .unwrap()
    }

    fn world_with(agents: &[(u64, Vec2, Vec2)]) -> World {
        let mut world = World::new();
        for (id, position, desired) in agents {
            let slot = world
                .spawn(
                    AgentSpawn {
                        agent_id: AgentId(*id),
                        population_id: 0,
                        position: *position,
                        yaw: 0.0,
                        radius: 0.3,
                        max_speed: 2.0,
                        preferred_speed: 1.35,
                        route: NO_ROUTE,
                        destination: 0,
                    },
                    0,
                )
                .unwrap() as usize;
            world.des_vel_x[slot] = desired.x;
            world.des_vel_y[slot] = desired.y;
        }
        world
    }

    /// Run one tick's worth of perceive-then-steer.
    ///
    /// `preferred` is re-applied on every call because `steer` overwrites
    /// `des_vel` with its solution. In a real tick the decide phase supplies a
    /// fresh preferred velocity first; a test that skipped that would be
    /// feeding the solver its own previous output, which never happens in
    /// production.
    fn run_steer(world: &mut World, scene: &CompiledScene, preferred: &[Vec2]) -> SteerReport {
        for (slot, want) in preferred.iter().enumerate() {
            world.des_vel_x[slot] = want.x;
            world.des_vel_y[slot] = want.y;
        }
        let mut grid = UniformGrid::new(scene.bounds, 5.0);
        grid.rebuild(&world.pos_x, &world.pos_y);
        let mut perceive_scratch = PerceiveScratch::default();
        let mut arena = NeighborArena::new();
        perceive(
            world,
            &grid,
            &PerceiveConfig::default(),
            &mut perceive_scratch,
            &mut arena,
        );
        let solver = SampledVelocitySolver::default();
        let mut scratch = SteerScratch::default();
        steer(
            world,
            &arena,
            scene,
            &solver,
            &SteerConfig::default(),
            &mut scratch,
        )
    }

    #[test]
    fn an_isolated_agent_keeps_its_preferred_velocity() {
        let scene = open_scene(Vec::new());
        let mut world = world_with(&[(1, Vec2::ZERO, Vec2::new(1.35, 0.0))]);
        run_steer(&mut world, &scene, &[Vec2::new(1.35, 0.0)]);
        assert!((world.desired_velocity(0) - Vec2::new(1.35, 0.0)).length() < 1e-4);
        assert_eq!(world.solver_status[0], SolverStatus::Free);
        assert_eq!(world.stall_ticks[0], 0);
    }

    #[test]
    fn converging_agents_are_deflected() {
        let scene = open_scene(Vec::new());
        let mut world = world_with(&[
            (1, Vec2::new(0.0, 0.0), Vec2::new(1.35, 0.0)),
            (2, Vec2::new(3.0, 0.0), Vec2::new(-1.35, 0.0)),
        ]);
        run_steer(
            &mut world,
            &scene,
            &[Vec2::new(1.35, 0.0), Vec2::new(-1.35, 0.0)],
        );
        assert!(world.desired_velocity(0).y.abs() > 0.01);
        assert!(world.desired_velocity(1).y.abs() > 0.01);
    }

    #[test]
    fn walls_are_supplied_to_the_solver() {
        let walls = vec![Segment::new(Vec2::new(2.0, -5.0), Vec2::new(2.0, 5.0))];
        let scene = open_scene(walls);
        let mut world = world_with(&[(1, Vec2::ZERO, Vec2::new(1.35, 0.0))]);
        run_steer(&mut world, &scene, &[Vec2::new(1.35, 0.0)]);
        assert_ne!(world.solver_status[0], SolverStatus::Free);
    }

    #[test]
    fn braking_increments_the_stall_counter() {
        let walls = vec![
            Segment::new(Vec2::new(0.7, -2.0), Vec2::new(0.7, 2.0)),
            Segment::new(Vec2::new(-2.0, 0.7), Vec2::new(2.0, 0.7)),
            Segment::new(Vec2::new(-2.0, -0.7), Vec2::new(2.0, -0.7)),
            Segment::new(Vec2::new(-0.7, -2.0), Vec2::new(-0.7, 2.0)),
        ];
        let scene = open_scene(walls);
        let mut world = world_with(&[(1, Vec2::ZERO, Vec2::new(1.35, 0.0))]);
        run_steer(&mut world, &scene, &[Vec2::new(1.35, 0.0)]);
        assert_eq!(world.solver_status[0], SolverStatus::Braking);
        assert_eq!(world.stall_ticks[0], 1);
        run_steer(&mut world, &scene, &[Vec2::new(1.35, 0.0)]);
        assert_eq!(world.stall_ticks[0], 2);
    }

    #[test]
    fn leaving_a_stall_resets_the_counter() {
        let scene = open_scene(Vec::new());
        let mut world = world_with(&[(1, Vec2::ZERO, Vec2::new(1.35, 0.0))]);
        world.stall_ticks[0] = 9;
        run_steer(&mut world, &scene, &[Vec2::new(1.35, 0.0)]);
        assert_eq!(world.stall_ticks[0], 0);
    }

    #[test]
    fn the_report_aggregates_the_worst_time_to_collision() {
        let scene = open_scene(Vec::new());
        let mut world = world_with(&[
            (1, Vec2::new(0.0, 0.0), Vec2::new(1.35, 0.0)),
            (2, Vec2::new(2.0, 0.0), Vec2::new(-1.35, 0.0)),
        ]);
        let report = run_steer(
            &mut world,
            &scene,
            &[Vec2::new(1.35, 0.0), Vec2::new(-1.35, 0.0)],
        );
        assert!(report.min_time_to_collision.is_finite());
    }

    #[test]
    fn steering_an_empty_world_is_a_no_op() {
        let scene = open_scene(Vec::new());
        let mut world = World::new();
        let report = run_steer(&mut world, &scene, &[]);
        assert_eq!(report.braking_agents, 0);
        assert!(report.min_time_to_collision.is_infinite());
    }
}
