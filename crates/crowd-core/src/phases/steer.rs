//! Tick phase 6: steer.
//!
//! Turns the decide phase's preferred velocity into a solved velocity by
//! running the avoidance solver against perceived neighbors and nearby walls.
//! Reads only previous-tick state, so the result does not depend on the order
//! agents are visited.

use crate::arena::NeighborArena;
use crate::avoidance::{AvoidanceInput, AvoidanceSolver, NeighborState};
use crate::fidelity::{FidelityPolicy, SimulationTier};
use crate::geometry::{time_to_collision_disc, time_to_collision_segment, Segment};
use crate::scene::CompiledScene;
use crate::units::Vec2;
use crate::world::{SolverStatus, World};

/// Horizon for the risk measurement below. Generous on purpose: this is a
/// metrics figure, not a safety cutoff, so erring toward reporting a threat is
/// preferable to missing one.
const RISK_HORIZON: f32 = 10.0;

#[derive(Clone, Copy, Debug)]
pub struct SteerConfig {
    /// How far to look for walls. Should exceed the solver's wall horizon
    /// times the maximum speed, or an agent can turn into a wall it never saw.
    pub wall_query_radius: f32,
    /// Predicted time to collision below which an agent-tick is a near miss.
    pub near_miss_time: f32,
    /// Agents below this count steer on one thread: thread setup costs more
    /// than it saves on a small scene.
    ///
    /// Purely a performance knob. Each agent's solution is a pure function of
    /// the previous-tick snapshot and the reduction runs in slot order, so the
    /// two paths agree bitwise and this value cannot change a result — which
    /// `parallel_and_sequential_steering_agree_bitwise` asserts by running the
    /// same scene both ways.
    pub parallel_min_agents: usize,
}

impl Default for SteerConfig {
    fn default() -> Self {
        Self {
            wall_query_radius: 3.0,
            near_miss_time: 0.5,
            parallel_min_agents: 2_048,
        }
    }
}

/// Reused buffers so the phase does not allocate after warmup.
#[derive(Clone, Debug, Default)]
pub struct SteerScratch {
    /// One buffer set per worker, retained across ticks so neither path
    /// allocates per tick. The sequential path is simply the one-worker case.
    workers: Vec<WorkerScratch>,
    /// Per-slot solver results, filled in parallel and applied in slot order.
    outcomes: Vec<Option<SolvedOutcome>>,
}

#[derive(Clone, Debug, Default)]
struct WorkerScratch {
    neighbors: Vec<NeighborState>,
    wall_indices: Vec<u32>,
    walls: Vec<Segment>,
}

/// What solving one agent produced, before any of it is written back.
///
/// Solving is separated from applying so the compute pass can run across
/// threads while touching nothing mutable: every field here is a pure function
/// of the previous-tick snapshot. The apply pass then walks slots in order, so
/// the world writes and the float reduction happen in exactly the sequence the
/// single-threaded path used — which is what makes the two bitwise identical.
#[derive(Clone, Copy, Debug)]
struct SolvedOutcome {
    velocity: Vec2,
    status: SolverStatus,
    /// Predicted time to collision against the velocity actually chosen.
    time_to_collision: f32,
    /// False for agents that have left the scene and must not accrue risk.
    contributes_risk: bool,
}

/// Tick-level aggregates the metrics layer would otherwise recompute.
///
/// Risk is measured against the velocity each agent will actually use, not
/// against the solver's internal reciprocal construction, and it is reported
/// as a distribution rather than a single global minimum. A global minimum
/// over a thousand agents is pinned at zero by any one overlapping pair, so it
/// cannot distinguish a good solver from a bad one — which is the only thing
/// the next slice's bake-off needs it to do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SteerReport {
    /// Smallest predicted time to collision anywhere this tick. Retained for
    /// completeness; expect it to saturate at zero in a dense scene.
    pub min_time_to_collision: f32,
    /// Sum of per-agent predicted times to collision, capped at the horizon.
    /// With `risk_samples`, yields an unsaturated mean.
    pub time_to_collision_sum: f32,
    pub risk_samples: u32,
    /// Agents whose predicted time to collision was under the near-miss
    /// threshold this tick. Scales with the population instead of saturating.
    pub near_miss_agents: u32,
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
    steer_with_schedule(world, arena, scene, solver, None, config, scratch, None)
}

/// M5 steering schedule. S2 reuses its last solved target between scheduled
/// evaluations; S3 follows its coarse desired flow without invoking the
/// individual avoidance solver. Neither path freezes root motion.
#[allow(clippy::too_many_arguments)]
pub fn steer_scheduled(
    world: &mut World,
    arena: &NeighborArena,
    scene: &CompiledScene,
    solver: &dyn AvoidanceSolver,
    background_solver: Option<&dyn AvoidanceSolver>,
    config: &SteerConfig,
    scratch: &mut SteerScratch,
    tick: u64,
) -> SteerReport {
    steer_with_schedule(
        world,
        arena,
        scene,
        solver,
        background_solver,
        config,
        scratch,
        Some(tick),
    )
}

#[allow(clippy::too_many_arguments)]
fn steer_with_schedule(
    world: &mut World,
    arena: &NeighborArena,
    scene: &CompiledScene,
    solver: &dyn AvoidanceSolver,
    background_solver: Option<&dyn AvoidanceSolver>,
    config: &SteerConfig,
    scratch: &mut SteerScratch,
    scheduled_tick: Option<u64>,
) -> SteerReport {
    let len = world.len();
    scratch.outcomes.clear();
    scratch.outcomes.resize(len, None);

    // Phase one: solve. Nothing mutable in the world is touched, so this can
    // be spread across threads without any hazard.
    if len >= config.parallel_min_agents {
        let SteerScratch {
            workers, outcomes, ..
        } = scratch;
        // Reborrowed as shared for the compute pass. This is the borrow that
        // makes the parallelism sound: while it lives, nothing can mutate the
        // world, so every worker reads the same consistent snapshot.
        let world: &World = world;
        let worker_count = std::thread::available_parallelism().map_or(1, |n| n.get());
        workers.resize_with(worker_count, WorkerScratch::default);
        let chunk = len.div_ceil(worker_count).max(1);

        // Contiguous chunks in slot order. Any chunking produces the same
        // outcomes — each is a pure function of the snapshot — so this is a
        // scheduling decision, not a semantic one.
        std::thread::scope(|scope| {
            for (index, (worker, slice)) in workers
                .iter_mut()
                .zip(outcomes.chunks_mut(chunk))
                .enumerate()
            {
                let base = index * chunk;
                scope.spawn(move || {
                    for (offset, outcome) in slice.iter_mut().enumerate() {
                        let slot = base + offset;
                        if !should_solve(world, slot, scheduled_tick) {
                            continue;
                        }
                        *outcome = Some(solve_slot(
                            world,
                            arena,
                            scene,
                            solver,
                            background_solver,
                            config,
                            worker,
                            slot,
                        ));
                    }
                });
            }
        });
    } else {
        let SteerScratch {
            workers, outcomes, ..
        } = scratch;
        // The sequential path is the parallel one with a single worker, so it
        // reuses the same retained buffers rather than keeping a second set.
        workers.resize_with(1, WorkerScratch::default);
        let worker = &mut workers[0];
        for (slot, outcome) in outcomes.iter_mut().enumerate() {
            if !should_solve(world, slot, scheduled_tick) {
                continue;
            }
            *outcome = Some(solve_slot(
                world,
                arena,
                scene,
                solver,
                background_solver,
                config,
                worker,
                slot,
            ));
        }
    }

    // Phase two: apply, in slot order. Both the world writes and the float
    // reduction happen in exactly the order the single-threaded implementation
    // used, which is what keeps the parallel and sequential paths bitwise
    // identical rather than merely close.
    let mut min_time_to_collision = f32::INFINITY;
    let mut time_to_collision_sum = 0.0;
    let mut risk_samples = 0;
    let mut near_miss_agents = 0;
    let mut braking_agents = 0;

    for slot in 0..len {
        let Some(outcome) = scratch.outcomes[slot] else {
            if world.simulation_tier[slot] == SimulationTier::S2 {
                // `decide` has just written a new preferred velocity into
                // des_vel; restore the target from the last sparse solve.
                // Reusing the current velocity turns each sparse interval
                // into a stop-start acceleration sawtooth and adds heading
                // jitter without any new avoidance evidence.
                world.des_vel_x[slot] = world.scheduled_target_vel_x[slot];
                world.des_vel_y[slot] = world.scheduled_target_vel_y[slot];
                // The retained target also retains its solver result. Marking
                // this tick `Free` would let intermittent braking accumulate
                // in `stall_ticks` without representing consecutive braking;
                // conversely, preserving `Braking` without incrementing would
                // undercount the time spent following a braking target.
                if world.solver_status[slot] == SolverStatus::Braking {
                    braking_agents += 1;
                    world.stall_ticks[slot] = world.stall_ticks[slot].saturating_add(1);
                } else {
                    world.stall_ticks[slot] = 0;
                }
            } else {
                world.solver_status[slot] = SolverStatus::Free;
                world.stall_ticks[slot] = 0;
            }
            continue;
        };

        world.des_vel_x[slot] = outcome.velocity.x;
        world.des_vel_y[slot] = outcome.velocity.y;
        world.scheduled_target_vel_x[slot] = outcome.velocity.x;
        world.scheduled_target_vel_y[slot] = outcome.velocity.y;
        world.solver_status[slot] = outcome.status;

        if outcome.status == SolverStatus::Braking {
            braking_agents += 1;
            world.stall_ticks[slot] = world.stall_ticks[slot].saturating_add(1);
        } else {
            world.stall_ticks[slot] = 0;
        }

        if !outcome.contributes_risk {
            continue;
        }
        min_time_to_collision = min_time_to_collision.min(outcome.time_to_collision);
        // Cap before summing so an agent in the clear does not contribute
        // infinity and destroy the mean.
        time_to_collision_sum += outcome.time_to_collision.min(RISK_HORIZON);
        risk_samples += 1;
        if outcome.time_to_collision < config.near_miss_time {
            near_miss_agents += 1;
        }
    }

    SteerReport {
        min_time_to_collision,
        time_to_collision_sum,
        risk_samples,
        near_miss_agents,
        braking_agents,
    }
}

/// Whether this agent runs the avoidance solver this tick.
fn should_solve(world: &World, slot: usize, scheduled_tick: Option<u64>) -> bool {
    match scheduled_tick {
        None => true,
        Some(_)
            if matches!(
                world.simulation_tier[slot],
                SimulationTier::S0 | SimulationTier::S1
            ) =>
        {
            true
        }
        Some(tick) if world.simulation_tier[slot] == SimulationTier::S2 => {
            FidelityPolicy::s2_update_due(world.agent_id[slot], tick)
        }
        Some(_) => false,
    }
}

/// Solve one agent against the previous-tick snapshot.
///
/// Takes `&World` rather than `&mut World` deliberately: the immutable borrow
/// is what proves this is safe to run concurrently for many slots at once.
#[allow(clippy::too_many_arguments)]
fn solve_slot(
    world: &World,
    arena: &NeighborArena,
    scene: &CompiledScene,
    solver: &dyn AvoidanceSolver,
    background_solver: Option<&dyn AvoidanceSolver>,
    config: &SteerConfig,
    scratch: &mut WorkerScratch,
    slot: usize,
) -> SolvedOutcome {
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

    // `des_vel` carries the decide phase's preferred velocity on the way in
    // and this phase's solution on the way out. Read it before the apply pass
    // overwrites it.
    let preferred = Vec2::new(world.des_vel_x[slot], world.des_vel_y[slot]);
    let velocity_now = Vec2::new(world.vel_x[slot], world.vel_y[slot]);

    // Tier selects the solver, not just the cadence. M5's fidelity model
    // gives background agents a coarse *representation*, not merely a
    // sparser schedule. The choice is a pure function of the committed tier,
    // so it cannot vary between runs.
    let active_solver = match (background_solver, world.simulation_tier[slot]) {
        (Some(coarse), SimulationTier::S2 | SimulationTier::S3) => coarse,
        _ => solver,
    };
    let output = active_solver.solve(&AvoidanceInput {
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

    // Risk is measured here, not taken from the solver. The solver's own
    // figure comes from the reciprocal construction (`candidate * 2 -
    // velocity`), which is correct as a cost heuristic but describes a
    // velocity the agent never has. This uses the velocity the agent will
    // actually move with.
    let mut agent_ttc = f32::INFINITY;
    for neighbor in &scratch.neighbors {
        let relative_position = neighbor.position - position;
        let relative_velocity = neighbor.velocity - velocity;
        let combined_radius = world.radius[slot] + neighbor.radius;
        if let Some(t) =
            time_to_collision_disc(relative_position, relative_velocity, combined_radius)
        {
            agent_ttc = agent_ttc.min(t);
        }
    }
    for wall in &scratch.walls {
        if let Some(t) =
            time_to_collision_segment(position, velocity, world.radius[slot], wall, RISK_HORIZON)
        {
            agent_ttc = agent_ttc.min(t);
        }
    }

    SolvedOutcome {
        velocity,
        status: output.status,
        time_to_collision: agent_ttc,
        // Agents that have left the scene do not contribute risk. They are
        // parked on a destination with zero velocity, and every later agent
        // routes to that same point, so counting them would accrue phantom
        // risk in proportion to how *well* the solver does — a solver landing
        // 500 agents would look worse than one landing 200. That is a perverse
        // gradient in the metrics meant to compare solvers.
        contributes_risk: !(world.arrived[slot] || world.unrouted[slot]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avoidance::SampledVelocitySolver;
    use crate::fidelity::S2_UPDATE_INTERVAL_TICKS;
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
            nav: None,
            nav_destinations: Vec::new(),
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
    fn the_report_samples_risk_for_every_agent() {
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
        // Risk is measured against the velocity each agent will actually use.
        // These two successfully avoid, so their predicted collision times are
        // legitimately large — the point is that every agent is sampled, and
        // the aggregate stays finite and bounded.
        assert_eq!(report.risk_samples, 2, "every agent must be sampled");
        assert!(report.time_to_collision_sum.is_finite());
        let mean = report.time_to_collision_sum / report.risk_samples as f32;
        assert!(mean > 0.0 && mean <= RISK_HORIZON, "mean was {mean}");
    }

    #[test]
    fn a_genuine_near_miss_is_counted_per_agent() {
        // Already overlapping, so no choice of velocity avoids it and the
        // measurement cannot be steered away. Unlike a global minimum, which
        // one bad pair pins for a whole run, this count scales with how many
        // agents are actually in trouble.
        //
        // A merely *close* pair is deliberately not used here: the solver
        // avoids that successfully, and reporting no near miss is the correct
        // answer in that case.
        let scene = open_scene(Vec::new());
        let mut world = world_with(&[
            (1, Vec2::new(0.0, 0.0), Vec2::new(1.35, 0.0)),
            (2, Vec2::new(0.4, 0.0), Vec2::new(-1.35, 0.0)),
        ]);
        let report = run_steer(
            &mut world,
            &scene,
            &[Vec2::new(1.35, 0.0), Vec2::new(-1.35, 0.0)],
        );
        assert!(
            report.near_miss_agents > 0,
            "an imminent contact was not counted: {report:?}"
        );
        assert!(report.near_miss_agents <= 2);
    }

    #[test]
    fn steering_an_empty_world_is_a_no_op() {
        let scene = open_scene(Vec::new());
        let mut world = World::new();
        let report = run_steer(&mut world, &scene, &[]);
        assert_eq!(report.braking_agents, 0);
        assert!(report.min_time_to_collision.is_infinite());
    }

    #[test]
    fn sparse_s2_tick_reuses_last_solved_target_not_current_velocity() {
        let scene = open_scene(Vec::new());
        let mut world = world_with(&[(1, Vec2::ZERO, Vec2::new(1.35, 0.0))]);
        world.simulation_tier[0] = SimulationTier::S2;
        world.vel_x[0] = 0.2;
        world.vel_y[0] = -0.1;
        world.scheduled_target_vel_x[0] = 1.1;
        world.scheduled_target_vel_y[0] = 0.4;
        let mut scratch = SteerScratch::default();
        let skipped_tick = (0..S2_UPDATE_INTERVAL_TICKS)
            .find(|tick| !FidelityPolicy::s2_update_due(world.agent_id[0], *tick))
            .unwrap();

        steer_scheduled(
            &mut world,
            &NeighborArena::new(),
            &scene,
            &SampledVelocitySolver::default(),
            None,
            &SteerConfig::default(),
            &mut scratch,
            skipped_tick,
        );

        assert_eq!(world.desired_velocity(0), Vec2::new(1.1, 0.4));
    }

    #[test]
    fn sparse_s2_braking_is_counted_as_continuous_not_accumulated_samples() {
        let scene = open_scene(Vec::new());
        let mut world = world_with(&[(1, Vec2::ZERO, Vec2::new(1.35, 0.0))]);
        world.simulation_tier[0] = SimulationTier::S2;
        world.solver_status[0] = SolverStatus::Braking;
        world.stall_ticks[0] = 7;
        let mut scratch = SteerScratch::default();
        let skipped_tick = (0..S2_UPDATE_INTERVAL_TICKS)
            .find(|tick| !FidelityPolicy::s2_update_due(world.agent_id[0], *tick))
            .unwrap();

        let report = steer_scheduled(
            &mut world,
            &NeighborArena::new(),
            &scene,
            &SampledVelocitySolver::default(),
            None,
            &SteerConfig::default(),
            &mut scratch,
            skipped_tick,
        );

        assert_eq!(world.solver_status[0], SolverStatus::Braking);
        assert_eq!(world.stall_ticks[0], 8);
        assert_eq!(report.braking_agents, 1);
    }
}
