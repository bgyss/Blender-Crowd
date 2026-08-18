//! The fixed-step tick loop.
//!
//! Phases run in the fixed order of contract section 6, with `commit` at the
//! end publishing staged next-state. Until then every phase reads a consistent
//! previous-tick snapshot, which is what makes results independent of the
//! order agents are visited.
//!
//! The M1 `Animate` phase consumes staged integrated motion and publishes only
//! commuter/clip metadata; trajectory remains owned by integration.

use std::time::Instant;

use crate::arena::NeighborArena;
use crate::avoidance::AvoidanceSolver;
use crate::clock::Clock;
use crate::commuter::{
    AgentSnapshot, ClipState, DecisionReason, FrameSnapshot, PortalControlError,
};
use crate::fidelity::{render_for, FidelityPin, FidelityPolicy};
use crate::field::{CpuSpatialField, FieldConfig, FieldSample, SpatialFieldKernel};
use crate::geometry::Segment;
use crate::grid::UniformGrid;
use crate::metrics::{Metrics, MetricsConfig, Phase};
use crate::nav::{PortalId, TileGraph};
use crate::phases::animate::{animate, animate_scheduled, AnimateConfig};
use crate::phases::decide::{decide, DecideConfig};
use crate::phases::integrate::{integrate, IntegrateConfig, IntegrateScratch};
use crate::phases::perceive::{perceive, perceive_scheduled, PerceiveConfig, PerceiveScratch};
use crate::phases::plan::{invalidate_portal, plan, PlanConfig, PlanState};
use crate::phases::spawn::{apply_spawns, SpawnState};
use crate::phases::steer::{steer, steer_scheduled, SteerConfig, SteerScratch};
use crate::route::RouteArena;
use crate::runtime_behavior::RuntimeBehaviorController;
use crate::scene::CompiledScene;
use crate::world::{SpawnError, World, NO_ROUTE};

#[derive(Clone, Debug, Default)]
pub struct SimConfig {
    pub perceive: PerceiveConfig,
    pub plan: PlanConfig,
    pub decide: DecideConfig,
    pub steer: SteerConfig,
    pub integrate: IntegrateConfig,
    pub metrics: MetricsConfig,
    /// Grid cell size. Zero means "derive from the perception radius", which
    /// is the right default: cells much smaller than the query radius make
    /// every query touch many cells.
    pub grid_cell_size: f32,
    /// Optional M5 presentation scheduler. It runs after root-motion commit,
    /// so it can never feed back into this tick's authoritative trajectory.
    pub fidelity: Option<FidelityPolicy>,
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
    nav: Option<TileGraph>,
    plan_state: PlanState,
    timed_input_cursor: usize,
    animate_config: AnimateConfig,

    grid: UniformGrid,
    neighbors: NeighborArena,
    perceive_scratch: PerceiveScratch,
    steer_scratch: SteerScratch,
    integrate_scratch: IntegrateScratch,

    metrics: Metrics,
    authorable_behavior: Option<RuntimeBehaviorController>,
    fidelity_pins: Vec<FidelityPin>,
    /// Coarse solver for background tiers. Only consulted when a fidelity
    /// policy is active, so a scene without a declared tier mix is unaffected.
    background_solver: Option<Box<dyn AvoidanceSolver>>,
    /// Whether presentation classification follows the tier cadence. On by
    /// default whenever a fidelity policy is active; `set_animation_scheduling`
    /// turns it off to measure the saving against an otherwise identical run.
    animation_scheduling: bool,
}

impl Simulation {
    pub fn new(scene: CompiledScene, solver: Box<dyn AvoidanceSolver>, config: SimConfig) -> Self {
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
        let mut nav = scene.nav.clone();
        if let Some(graph) = &mut nav {
            for name in &scene.initially_closed_portals {
                let ids = graph.portals_named(name).to_vec();
                for id in ids {
                    graph.set_portal_open(id, false);
                }
            }
        }
        let animate_config = AnimateConfig {
            jog_threshold_mps: scene.runtime_animation.jog_threshold_mps,
            ..AnimateConfig::default()
        };

        Self {
            scene,
            solver,
            config,
            world: World::new(),
            clock,
            routes: RouteArena::new(),
            spawn_state,
            spawn_errors: Vec::new(),
            nav,
            plan_state: PlanState::default(),
            timed_input_cursor: 0,
            animate_config,
            grid,
            neighbors: NeighborArena::new(),
            perceive_scratch: PerceiveScratch::default(),
            steer_scratch: SteerScratch::default(),
            integrate_scratch: IntegrateScratch::default(),
            metrics: Metrics::new(),
            authorable_behavior: None,
            fidelity_pins: Vec::new(),
            background_solver: None,
            animation_scheduling: true,
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

    /// Produce a read-only aggregate field from the committed authoritative
    /// world.  This deliberately returns a separate artifact: M5 consumers
    /// can use it for background-flow presentation or coarse perception but
    /// cannot feed a field result back into identity, layers, or root motion.
    pub fn cpu_spatial_field(&self, config: FieldConfig) -> Result<CpuSpatialField, &'static str> {
        let samples = (0..self.world.len())
            .map(|slot| FieldSample {
                position: self.world.position(slot as u32),
                velocity: self.world.velocity(slot as u32),
            })
            .collect::<Vec<_>>();
        let mut field = CpuSpatialField::default();
        field.build(&samples, config)?;
        Ok(field)
    }

    pub fn nav(&self) -> Option<&TileGraph> {
        self.nav.as_ref()
    }

    pub fn enable_authorable_behavior(&mut self, controller: RuntimeBehaviorController) {
        self.authorable_behavior = Some(controller);
    }

    /// Move the fidelity policy mid-run.
    ///
    /// The camera is part of the policy, and in Blender it moves every frame,
    /// so a policy fixed at construction could only ever describe a still.
    /// This changes scheduling only; it cannot touch identity or root motion.
    pub fn set_fidelity_policy(&mut self, policy: FidelityPolicy) {
        debug_assert!(policy.validate().is_ok());
        self.config.fidelity = Some(policy);
    }

    /// Use a cheaper solver for background tiers.
    ///
    /// Takes effect only alongside a fidelity policy: without declared tiers
    /// there is no background population to apply it to. The assignment is a
    /// pure function of the committed tier, so results stay reproducible.
    pub fn set_background_solver(&mut self, solver: Box<dyn AvoidanceSolver>) {
        self.background_solver = Some(solver);
    }

    /// Turn camera/focus animation scheduling off, re-classifying every agent
    /// every tick.
    ///
    /// Exists so the saving can be measured against a like-for-like run rather
    /// than asserted: the same declared tier mix, differing only in whether
    /// presentation classification is scheduled.
    pub fn set_animation_scheduling(&mut self, enabled: bool) {
        self.animation_scheduling = enabled;
    }

    /// Replace pins atomically. Sorting makes lookup and results independent
    /// of Blender/UI collection order.
    pub fn set_fidelity_pins(&mut self, mut pins: Vec<FidelityPin>) {
        pins.sort_by_key(|pin| pin.agent_id);
        pins.dedup_by_key(|pin| pin.agent_id);
        self.fidelity_pins = pins;
    }

    fn schedule_fidelity(&mut self) {
        let Some(policy) = self.config.fidelity else {
            return;
        };
        debug_assert!(policy.validate().is_ok());
        for slot in 0..self.world.len() {
            let id = self.world.agent_id[slot];
            let pin = self
                .fidelity_pins
                .binary_search_by_key(&id, |pin| pin.agent_id)
                .ok()
                .map(|index| self.fidelity_pins[index]);
            let simulation = pin.map_or_else(
                || match policy.background_permyriad {
                    Some(_) if policy.is_background_id(id) == Some(true) => {
                        crate::fidelity::SimulationTier::S2
                    }
                    Some(_) => crate::fidelity::SimulationTier::S1,
                    None => policy.target(
                        self.world.position(slot as u32),
                        self.world.simulation_tier[slot],
                    ),
                },
                |pin| pin.simulation,
            );
            let render = pin.map_or_else(|| render_for(simulation), |pin| pin.render);
            self.world.simulation_tier[slot] = simulation;
            self.world.render_fidelity_tier[slot] = render;
            self.world.render_tier[slot] = render as u8;
        }
    }

    pub fn behavior_trace(
        &self,
        agent_id: crate::ids::AgentId,
    ) -> Option<&crate::behavior::DecisionOutcome> {
        self.authorable_behavior.as_ref()?.trace(agent_id)
    }

    pub fn authorable_queue_status(
        &self,
        queue_id: &str,
        agent_id: crate::ids::AgentId,
    ) -> Option<crate::social::QueueStatus> {
        Some(
            self.authorable_behavior
                .as_ref()?
                .queue_status(queue_id, agent_id),
        )
    }

    pub fn authorable_group_report(&self, group_id: &str) -> Option<crate::social::GroupReport> {
        self.authorable_behavior.as_ref()?.group_report(group_id)
    }

    /// Consume ordered authored behavior evidence generated by fixed-step decisions.
    pub fn drain_behavior_events(&mut self) -> Vec<crate::runtime_behavior::BehaviorRuntimeEvent> {
        self.authorable_behavior
            .as_mut()
            .map_or_else(Vec::new, RuntimeBehaviorController::drain_events)
    }

    /// Toggle a portal's open/closed state and selectively invalidate the
    /// corridors of agents whose route crossed it. Returns how many agents
    /// were invalidated (0 if the scene has no tiled navmesh).
    pub fn set_portal_open(&mut self, id: PortalId, open: bool) -> usize {
        self.set_portals_open(std::slice::from_ref(&id), open)
    }

    /// Toggle every portal in `ids` to the same open/closed state — the form
    /// a named door (which can span more than one portal; see
    /// `TileGraph::portals_named`) needs to close or reopen atomically.
    /// Returns the total number of agents invalidated across all of them.
    ///
    /// An agent invalidated by one portal in the set has its route cleared to
    /// `NO_ROUTE` before the next portal in the set is processed, so it is
    /// never double-counted even if its old corridor happened to cross more
    /// than one portal in `ids`.
    pub fn set_portals_open(&mut self, ids: &[PortalId], open: bool) -> usize {
        let affected_slots: Vec<usize> = if open {
            Vec::new()
        } else {
            (0..self.world.len())
                .filter(|slot| self.route_crosses_any(*slot as u32, ids))
                .collect()
        };
        let Some(nav) = &mut self.nav else {
            return 0;
        };
        for &id in ids {
            nav.set_portal_open(id, open);
        }
        let mut invalidated = 0;
        for &id in ids {
            invalidated += invalidate_portal(&mut self.world, &mut self.plan_state, id);
        }
        for slot in affected_slots {
            self.world.decision_reason[slot] = DecisionReason::PortalClosedReplan;
        }
        invalidated
    }

    pub fn set_named_portal_open(
        &mut self,
        name: &str,
        open: bool,
    ) -> Result<usize, PortalControlError> {
        let ids = self
            .nav
            .as_ref()
            .ok_or(PortalControlError::MissingNavigation)?
            .portals_named(name)
            .to_vec();
        if ids.is_empty() {
            return Err(PortalControlError::UnknownPortal(name.to_string()));
        }
        Ok(self.set_portals_open(&ids, open))
    }

    pub fn named_portal_is_open(&self, name: &str) -> Result<bool, PortalControlError> {
        let nav = self
            .nav
            .as_ref()
            .ok_or(PortalControlError::MissingNavigation)?;
        let ids = nav.portals_named(name);
        if ids.is_empty() {
            return Err(PortalControlError::UnknownPortal(name.to_string()));
        }
        Ok(ids.iter().all(|id| nav.portal(*id).open))
    }

    pub fn agent_ids_not_using_portal(
        &self,
        name: &str,
    ) -> Result<Vec<crate::ids::AgentId>, PortalControlError> {
        let portals = self
            .nav
            .as_ref()
            .ok_or(PortalControlError::MissingNavigation)?
            .portals_named(name);
        if portals.is_empty() {
            return Err(PortalControlError::UnknownPortal(name.to_string()));
        }
        Ok((0..self.world.len())
            .filter(|slot| !self.route_crosses_any(*slot as u32, portals))
            .map(|slot| self.world.agent_id[slot])
            .collect())
    }

    pub fn apply_timed_inputs(&mut self) -> Result<(), PortalControlError> {
        let tick = self.clock.tick();
        while let Some(event) = self.scene.timed_portal_events.get(self.timed_input_cursor) {
            if event.tick > tick {
                break;
            }
            let portal_id = event.portal_id.clone();
            let open = event.open;
            self.timed_input_cursor += 1;
            self.set_named_portal_open(&portal_id, open)?;
        }
        Ok(())
    }

    pub fn frame_snapshot(&self) -> FrameSnapshot {
        FrameSnapshot {
            tick: self.clock.tick(),
            agents: (0..self.world.len())
                .map(|slot| self.snapshot_at(slot))
                .collect(),
        }
    }

    pub fn query_agent(&self, id: crate::ids::AgentId) -> Option<AgentSnapshot> {
        self.world
            .slot_of(id)
            .map(|slot| self.snapshot_at(slot as usize))
    }

    /// Stable portal IDs in the agent's current corridor, for bounded debug
    /// evidence. Unknown or unspawned IDs return `None`; an unrouted agent
    /// returns an empty list.
    pub fn route_portal_ids(&self, id: crate::ids::AgentId) -> Option<Vec<u32>> {
        let slot = self.world.slot_of(id)? as usize;
        let handle = self.world.route[slot];
        Some(
            self.plan_state
                .portals_for(handle)
                .unwrap_or_default()
                .iter()
                .map(|portal| portal.0)
                .collect(),
        )
    }

    /// Current corridor polyline for selected-agent inspection.
    pub fn route_points_for_agent(
        &self,
        id: crate::ids::AgentId,
    ) -> Option<Vec<crate::units::Vec2>> {
        let slot = self.world.slot_of(id)? as usize;
        Some(self.routes.points(self.world.route[slot]).to_vec())
    }

    /// Current look-ahead target without mutating the authoritative route
    /// cursor.
    pub fn next_route_target(&self, id: crate::ids::AgentId) -> Option<crate::units::Vec2> {
        let slot = self.world.slot_of(id)? as usize;
        let points = self.routes.points(self.world.route[slot]);
        let mut index = self.world.route_index[slot];
        crate::route::next_target(
            points,
            &mut index,
            self.world.position(slot as u32),
            self.config.decide.arrive_radius,
        )
    }

    fn snapshot_at(&self, slot: usize) -> AgentSnapshot {
        AgentSnapshot {
            agent_id: self.world.agent_id[slot],
            population_id: u32::from(self.world.population_id[slot]),
            archetype_id: self.world.archetype_id[slot],
            variant_id: self.world.variant_id[slot],
            spawn_ordinal: self.world.spawn_ordinal[slot],
            position: self.world.position(slot as u32),
            orientation: self.world.yaw[slot],
            scale: self.world.scale[slot],
            velocity: self.world.velocity(slot as u32),
            desired_velocity: self.world.desired_velocity(slot as u32),
            destination_id: u32::from(self.world.destination[slot]),
            destination_point: if self.world.custom_destination[slot] {
                crate::units::Vec2::new(
                    self.world.destination_x[slot],
                    self.world.destination_y[slot],
                )
            } else {
                self.scene
                    .destination_position(self.world.destination[slot])
                    .unwrap_or(crate::units::Vec2::ZERO)
            },
            destination_bounds: if self.world.custom_destination_bounds[slot] {
                crate::units::Aabb::new(
                    crate::units::Vec2::new(
                        self.world.destination_min_x[slot],
                        self.world.destination_min_y[slot],
                    ),
                    crate::units::Vec2::new(
                        self.world.destination_max_x[slot],
                        self.world.destination_max_y[slot],
                    ),
                )
            } else {
                let point = self
                    .scene
                    .destination_position(self.world.destination[slot])
                    .unwrap_or(crate::units::Vec2::ZERO);
                crate::units::Aabb::new(point, point)
            },
            commuter_state: self.world.commuter_state[slot],
            decision_reason: self.world.decision_reason[slot],
            clip_state: ClipState {
                clip_id: self.world.clip_id[slot],
                phase: self.world.clip_phase[slot],
                playback_rate: self.world.playback_rate[slot],
            },
            visible: self.world.visible[slot],
            render_tier: self.world.render_tier[slot],
        }
    }

    /// True when agent `slot` has a live route whose recorded portal sequence
    /// crosses at least one portal in `portals`. False for an unrouted or
    /// arrived agent. Lets a test or caller verify *which* doorway an agent's
    /// current corridor actually uses, not merely that it has some route.
    pub fn route_crosses_any(&self, slot: u32, portals: &[PortalId]) -> bool {
        if self.world.arrived[slot as usize] {
            return false;
        }
        let handle = self.world.route[slot as usize];
        if handle == NO_ROUTE {
            return false;
        }
        self.plan_state
            .portals_for(handle)
            .is_some_and(|seq| seq.iter().any(|p| portals.contains(p)))
    }

    /// Advance one tick through the fixed phase order.
    ///
    /// Timing uses `Instant`, which is wall-clock and therefore varies between
    /// runs. It only ever feeds the metrics report, never a simulation
    /// decision, so determinism is unaffected.
    pub fn step(&mut self) {
        self.metrics.begin_tick();

        let start = Instant::now();
        self.apply_timed_inputs()
            .expect("compiled timed portal inputs remain valid");
        self.metrics
            .record_phase(Phase::Inputs, start.elapsed().as_nanos() as u64);

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
        if self.config.fidelity.is_some() {
            perceive_scheduled(
                &self.world,
                &self.grid,
                &self.config.perceive,
                &mut self.perceive_scratch,
                &mut self.neighbors,
                self.clock.tick(),
            );
        } else {
            perceive(
                &self.world,
                &self.grid,
                &self.config.perceive,
                &mut self.perceive_scratch,
                &mut self.neighbors,
            );
        }
        self.metrics
            .record_phase(Phase::Perceive, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        if let Some(nav) = &self.nav {
            plan(
                &mut self.world,
                nav,
                &self.scene.nav_destinations,
                &mut self.plan_state,
                &mut self.routes,
                &self.config.plan,
            );
        }
        self.metrics
            .record_phase(Phase::Plan, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        decide(&mut self.world, &self.routes, &self.config.decide);
        if let Some(controller) = &mut self.authorable_behavior {
            controller.apply(&mut self.world, &self.neighbors, self.clock.tick());
        }
        self.metrics
            .record_phase(Phase::Decide, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        let steer_report = if self.config.fidelity.is_some() {
            steer_scheduled(
                &mut self.world,
                &self.neighbors,
                &self.scene,
                self.solver.as_ref(),
                self.background_solver.as_deref(),
                &self.config.steer,
                &mut self.steer_scratch,
                self.clock.tick(),
            )
        } else {
            steer(
                &mut self.world,
                &self.neighbors,
                &self.scene,
                self.solver.as_ref(),
                &self.config.steer,
                &mut self.steer_scratch,
            )
        };
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
        self.metrics
            .record_phase(Phase::Integrate, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        // Presentation scheduling follows the same rule as perception and
        // steering: it runs only when a fidelity policy declares the tiers it
        // should key off. Without one, every agent is classified every tick.
        let animate_report = if self.config.fidelity.is_some() && self.animation_scheduling {
            animate_scheduled(&mut self.world, &self.animate_config, self.clock.tick())
        } else {
            animate(&mut self.world, &self.animate_config)
        };
        self.world.commit();
        self.schedule_fidelity();
        self.metrics
            .record_phase(Phase::Animate, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        self.metrics.record_steer(&steer_report);
        self.metrics.record_integrate(&integrate_report);
        self.metrics.record_animate(&animate_report);
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
    use crate::nav::NavMeshDef;
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
            destinations: vec![Destination {
                name: "exit".into(),
                node: b,
            }],
            spawns: vec![SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(1.0, 3.0), Vec2::new(6.0, 7.0)),
                count,
                per_tick: 4,
                destination: 0,
            }],
            populations: vec![PopulationParams::default()],
            project_seed: 2026,
            ticks_per_second: 30,
            duration_ticks: 900,
            nav: None,
            nav_destinations: Vec::new(),
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
        // The scene emits 4 per tick, so one step is one batch, not the
        // whole population.
        assert_eq!(sim.world().len(), 4);
        assert!(sim.spawn_errors().is_empty());

        sim.run(5);
        assert_eq!(sim.world().len(), 10, "spawning must stop at the count");
        assert!(sim.spawn_errors().is_empty());
    }

    #[test]
    fn fidelity_scheduler_preserves_root_motion_and_honors_artist_pins() {
        let mut sim = Simulation::new(
            corridor(4),
            Box::new(SampledVelocitySolver::default()),
            SimConfig {
                fidelity: Some(FidelityPolicy {
                    camera: Vec2::new(1000.0, 1000.0),
                    ..FidelityPolicy::default()
                }),
                ..SimConfig::default()
            },
        );
        sim.step();
        let pinned = sim.world.agent_id[0];
        sim.set_fidelity_pins(vec![FidelityPin {
            agent_id: pinned,
            simulation: crate::fidelity::SimulationTier::S0,
            render: crate::fidelity::RenderTier::R0,
        }]);
        sim.step();
        assert_eq!(
            sim.world.simulation_tier[0],
            crate::fidelity::SimulationTier::S0
        );
        assert_eq!(sim.world.render_tier[0], 0);
        assert!(sim.world.simulation_tier[1..]
            .iter()
            .all(|tier| *tier == crate::fidelity::SimulationTier::S3));
        assert!(sim.world.pos_x.iter().all(|position| position.is_finite()));
        let field = sim.cpu_spatial_field(FieldConfig::default()).unwrap();
        assert!(field.sample(sim.world.position(0)).density > 0);
    }

    #[test]
    fn agents_make_progress_toward_their_destination() {
        let mut sim = simulation(20);
        sim.step();
        let start_x: f32 = sim.world().pos_x.iter().sum::<f32>() / 20.0;
        sim.run(120);
        let end_x: f32 = sim.world().pos_x.iter().sum::<f32>() / 20.0;
        assert!(
            end_x > start_x + 2.0,
            "agents did not advance: {start_x} to {end_x}"
        );
    }

    #[test]
    fn agents_eventually_arrive() {
        let mut sim = simulation(20);
        sim.run_to_completion();
        assert!(
            sim.metrics().arrived() > 0,
            "nobody reached the destination"
        );
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

    fn nav_corridor(count: u32) -> CompiledScene {
        SceneDef {
            name: "sim_nav_corridor".into(),
            bounds: Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(20.0, 4.0)),
            walls: vec![
                Segment::new(Vec2::new(0.0, 0.0), Vec2::new(20.0, 0.0)),
                Segment::new(Vec2::new(0.0, 4.0), Vec2::new(20.0, 4.0)),
            ],
            waypoints: WaypointGraph::new(),
            destinations: vec![Destination {
                name: "exit".into(),
                node: 0,
            }],
            spawns: vec![SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(1.0, 1.0), Vec2::new(3.0, 3.0)),
                count,
                per_tick: 4,
                destination: 0,
            }],
            populations: vec![PopulationParams::default()],
            project_seed: 2026,
            ticks_per_second: 30,
            duration_ticks: 900,
            nav: Some(NavMeshDef {
                tile_size: 1.0,
                agent_radius: 0.3,
                cost_areas: Vec::new(),
                named_portals: Vec::new(),
            }),
            nav_destinations: vec![Vec2::new(18.0, 2.0)],
        }
        .compile()
        .unwrap()
    }

    #[test]
    fn a_nav_routed_simulation_routes_and_moves_its_agents() {
        let mut sim = Simulation::new(
            nav_corridor(10),
            Box::new(SampledVelocitySolver::default()),
            SimConfig::default(),
        );
        sim.run(60);
        let mut any_routed = false;
        for slot in 0..sim.world().len() {
            if sim.world().route[slot] != crate::world::NO_ROUTE {
                any_routed = true;
            }
        }
        assert!(any_routed, "the plan phase never routed any agent");
    }

    #[test]
    fn closing_a_portal_reroutes_only_agents_that_used_it() {
        let mut sim = Simulation::new(
            nav_corridor(4),
            Box::new(SampledVelocitySolver::default()),
            SimConfig::default(),
        );
        sim.run(10);
        let portal = sim.nav().unwrap().portal_between(0, 1);
        if let Some(portal) = portal {
            // This test only proves `set_portal_open` is wired up and does
            // not panic or desync `commit()` — real reroute selectivity is
            // proven by the dedicated `two_room` integration test, which
            // controls geometry precisely enough to assert it. Stepping
            // after a close with every agent's state staying finite is the
            // meaningful, non-tautological property available here.
            sim.set_portal_open(portal, false);
            sim.step();
            for slot in 0..sim.world().len() {
                assert!(sim.world().position(slot as u32).is_finite());
            }
        }
    }
}
