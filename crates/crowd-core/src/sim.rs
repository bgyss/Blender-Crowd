//! The fixed-step tick loop.
//!
//! Phases run in the fixed order of contract section 6, with `commit` at the
//! end publishing staged next-state. Until then every phase reads a consistent
//! previous-tick snapshot, which is what makes results independent of the
//! order agents are visited.
//!
//! `Animate` is omitted rather than stubbed: there is no clip data to select
//! from in this slice.

use std::time::Instant;

use crate::arena::NeighborArena;
use crate::avoidance::AvoidanceSolver;
use crate::clock::Clock;
use crate::geometry::Segment;
use crate::grid::UniformGrid;
use crate::metrics::{Metrics, MetricsConfig, Phase};
use crate::phases::decide::{decide, DecideConfig};
use crate::phases::integrate::{integrate, IntegrateConfig, IntegrateScratch};
use crate::phases::perceive::{perceive, PerceiveConfig, PerceiveScratch};
use crate::phases::spawn::{apply_spawns, SpawnState};
use crate::phases::steer::{steer, SteerConfig, SteerScratch};
use crate::route::RouteArena;
use crate::scene::CompiledScene;
use crate::world::{SpawnError, World};

#[derive(Clone, Debug, Default)]
pub struct SimConfig {
    pub perceive: PerceiveConfig,
    pub decide: DecideConfig,
    pub steer: SteerConfig,
    pub integrate: IntegrateConfig,
    pub metrics: MetricsConfig,
    /// Grid cell size. Zero means "derive from the perception radius", which
    /// is the right default: cells much smaller than the query radius make
    /// every query touch many cells.
    pub grid_cell_size: f32,
}

pub struct Simulation {
    scene: CompiledScene,
    solver: Box<dyn AvoidanceSolver>,
    config: SimConfig,

    world: World,
    clock: Clock,
    routes: RouteArena,
    spawn_state: SpawnState,
    spawn_errors: Vec<SpawnError>,

    grid: UniformGrid,
    neighbors: NeighborArena,
    perceive_scratch: PerceiveScratch,
    steer_scratch: SteerScratch,
    integrate_scratch: IntegrateScratch,

    metrics: Metrics,
}

impl Simulation {
    pub fn new(
        scene: CompiledScene,
        solver: Box<dyn AvoidanceSolver>,
        config: SimConfig,
    ) -> Self {
        let cell_size = if config.grid_cell_size > 0.0 {
            config.grid_cell_size
        } else {
            config.perceive.query_radius
        };
        // Expand the grid past the scene bounds so an agent that slips outside
        // still lands in a real cell rather than being clamped onto the edge
        // with every other escapee.
        let grid = UniformGrid::new(scene.bounds.expanded(cell_size * 2.0), cell_size);
        let spawn_state = SpawnState::new(&scene);
        let clock = Clock::new(scene.ticks_per_second);

        Self {
            scene,
            solver,
            config,
            world: World::new(),
            clock,
            routes: RouteArena::new(),
            spawn_state,
            spawn_errors: Vec::new(),
            grid,
            neighbors: NeighborArena::new(),
            perceive_scratch: PerceiveScratch::default(),
            steer_scratch: SteerScratch::default(),
            integrate_scratch: IntegrateScratch::default(),
            metrics: Metrics::new(),
        }
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    pub fn scene(&self) -> &CompiledScene {
        &self.scene
    }

    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    pub fn routes(&self) -> &RouteArena {
        &self.routes
    }

    pub fn solver_name(&self) -> &'static str {
        self.solver.name()
    }

    pub fn spawn_errors(&self) -> &[SpawnError] {
        &self.spawn_errors
    }

    pub fn state_hash(&self) -> u64 {
        self.world.state_hash()
    }

    /// Advance one tick through the fixed phase order.
    ///
    /// Timing uses `Instant`, which is wall-clock and therefore varies between
    /// runs. It only ever feeds the metrics report, never a simulation
    /// decision, so determinism is unaffected.
    pub fn step(&mut self) {
        self.metrics.begin_tick();

        let start = Instant::now();
        let errors = apply_spawns(
            &self.scene,
            &mut self.spawn_state,
            &mut self.world,
            &mut self.routes,
            self.clock.tick(),
        );
        self.spawn_errors.extend(errors);
        self.metrics
            .record_phase(Phase::Spawn, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        self.grid.rebuild(&self.world.pos_x, &self.world.pos_y);
        self.metrics
            .record_phase(Phase::Index, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        perceive(
            &self.world,
            &self.grid,
            &self.config.perceive,
            &mut self.perceive_scratch,
            &mut self.neighbors,
        );
        self.metrics
            .record_phase(Phase::Perceive, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        decide(&mut self.world, &self.routes, &self.config.decide);
        self.metrics
            .record_phase(Phase::Decide, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        let steer_report = steer(
            &mut self.world,
            &self.neighbors,
            &self.scene,
            self.solver.as_ref(),
            &self.config.steer,
            &mut self.steer_scratch,
        );
        self.metrics
            .record_phase(Phase::Steer, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        let integrate_report = integrate(
            &mut self.world,
            &self.scene,
            &self.config.integrate,
            self.clock.dt(),
            &mut self.integrate_scratch,
        );
        self.world.commit();
        self.metrics
            .record_phase(Phase::Integrate, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        self.metrics.record_steer(&steer_report);
        self.metrics.record_integrate(&integrate_report);
        self.metrics.observe_tick(
            &self.world,
            &self.neighbors,
            &self.clock,
            &self.config.metrics,
        );
        self.clock.advance();
        self.metrics.record_arrivals(&self.world, &self.clock);
        self.metrics
            .record_phase(Phase::Metrics, start.elapsed().as_nanos() as u64);
    }

    pub fn run(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.step();
        }
    }

    /// Run until the scene's declared duration elapses.
    pub fn run_to_completion(&mut self) {
        while self.clock.tick() < self.scene.duration_ticks {
            self.step();
        }
    }

    /// Wall segments, for the SVG dump.
    pub fn walls(&self) -> &[Segment] {
        &self.scene.walls
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avoidance::SampledVelocitySolver;
    use crate::route::WaypointGraph;
    use crate::scene::{Destination, PopulationParams, SceneDef, SpawnRegion};
    use crate::units::{Aabb, Vec2};

    fn corridor(count: u32) -> CompiledScene {
        let mut waypoints = WaypointGraph::new();
        let a = waypoints.add_node(Vec2::new(2.0, 5.0));
        let b = waypoints.add_node(Vec2::new(18.0, 5.0));
        waypoints.add_edge(a, b);
        SceneDef {
            name: "sim_corridor".into(),
            bounds: Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(20.0, 10.0)),
            walls: vec![
                Segment::new(Vec2::new(0.0, 2.0), Vec2::new(20.0, 2.0)),
                Segment::new(Vec2::new(0.0, 8.0), Vec2::new(20.0, 8.0)),
            ],
            waypoints,
            destinations: vec![Destination { name: "exit".into(), node: b }],
            spawns: vec![SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(1.0, 3.0), Vec2::new(3.0, 7.0)),
                count,
                per_tick: count.min(20),
                destination: 0,
            }],
            populations: vec![PopulationParams::default()],
            project_seed: 2026,
            ticks_per_second: 30,
            duration_ticks: 900,
        }
        .compile()
        .unwrap()
    }

    fn simulation(count: u32) -> Simulation {
        Simulation::new(
            corridor(count),
            Box::new(SampledVelocitySolver::default()),
            SimConfig::default(),
        )
    }

    #[test]
    fn a_new_simulation_starts_empty_at_tick_zero() {
        let sim = simulation(10);
        assert_eq!(sim.clock().tick(), 0);
        assert_eq!(sim.world().len(), 0);
    }

    #[test]
    fn stepping_spawns_agents_and_advances_the_clock() {
        let mut sim = simulation(10);
        sim.step();
        assert_eq!(sim.clock().tick(), 1);
        assert_eq!(sim.world().len(), 10);
        assert!(sim.spawn_errors().is_empty());
    }

    #[test]
    fn agents_make_progress_toward_their_destination() {
        let mut sim = simulation(20);
        sim.step();
        let start_x: f32 = sim.world().pos_x.iter().sum::<f32>() / 20.0;
        sim.run(120);
        let end_x: f32 = sim.world().pos_x.iter().sum::<f32>() / 20.0;
        assert!(end_x > start_x + 2.0, "agents did not advance: {start_x} to {end_x}");
    }

    #[test]
    fn agents_eventually_arrive() {
        let mut sim = simulation(20);
        sim.run_to_completion();
        assert!(sim.metrics().arrived() > 0, "nobody reached the destination");
    }

    #[test]
    fn agents_stay_inside_the_corridor_walls() {
        let mut sim = simulation(50);
        sim.run(300);
        for slot in 0..sim.world().len() {
            let y = sim.world().pos_y[slot];
            assert!(
                (1.5..=8.5).contains(&y),
                "slot {slot} escaped the corridor at y={y}"
            );
        }
    }

    #[test]
    fn all_state_stays_finite() {
        let mut sim = simulation(100);
        sim.run(300);
        for slot in 0..sim.world().len() {
            assert!(sim.world().position(slot as u32).is_finite());
            assert!(sim.world().velocity(slot as u32).is_finite());
            assert!(sim.world().yaw[slot].is_finite());
        }
    }

    #[test]
    fn identical_runs_produce_identical_state_hashes() {
        let mut a = simulation(50);
        let mut b = simulation(50);
        a.run(200);
        b.run(200);
        assert_eq!(a.state_hash(), b.state_hash());
    }

    #[test]
    fn state_hashes_match_at_every_tick() {
        // A single end-state comparison can hide a divergence that later
        // reconverges, so compare the whole trajectory.
        let mut a = simulation(30);
        let mut b = simulation(30);
        for tick in 0..150 {
            a.step();
            b.step();
            assert_eq!(a.state_hash(), b.state_hash(), "diverged at tick {tick}");
        }
    }

    #[test]
    fn run_to_completion_stops_at_the_scene_duration() {
        let mut sim = simulation(5);
        sim.run_to_completion();
        assert!(sim.clock().tick() <= sim.scene().duration_ticks);
    }

    #[test]
    fn phase_timings_are_recorded() {
        let mut sim = simulation(10);
        sim.run(10);
        assert!(sim.metrics().phase_nanos(Phase::Steer) > 0);
    }
}
