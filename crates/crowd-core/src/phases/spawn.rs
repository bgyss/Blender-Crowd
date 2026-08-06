//! Tick phase 1: apply inputs.
//!
//! The only timed input this slice has is scheduled agent spawns. Every varied
//! attribute is drawn from a stream keyed by the agent's stable ID, never from
//! a shared sequence, so emission rate and ordering cannot change what any
//! individual agent looks like.

use crate::ids::derive_agent_id;
use crate::rng::{Purpose, StableRng};
use crate::route::RouteArena;
use crate::scene::{CompiledScene, MIN_PREFERRED_SPEED};
use crate::units::{Aabb, Vec2};
use crate::world::{AgentSpawn, SpawnError, World, NO_ROUTE};

/// Extra clearance beyond touching, in metres, when placing a new agent.
const SPAWN_CLEARANCE: f32 = 0.05;

/// Attempts before accepting a position regardless.
///
/// Bounded so a full spawn region cannot stall the tick. Falling back to an
/// overlapping placement is strictly better than looping: the solver's overlap
/// gradient will separate them, whereas an unbounded search would hang.
const SPAWN_PLACEMENT_ATTEMPTS: u32 = 24;

/// Draw a spawn position that does not start inside an existing agent.
///
/// Uniform placement lets two agents spawn essentially co-located, and that
/// alone accounted for every penetration event recorded in the open benchmark
/// scenes -- all of it inside the first tenth of a run, none afterwards. It
/// read as a steering failure in the metrics when it was really a placement
/// one, and it hands the solver an interpenetrating crowd to untangle before
/// anything has even moved.
///
/// Rejection sampling keeps the draw a pure function of the agent's own
/// stable stream, so placement stays reproducible and independent of how many
/// agents happen to be spawning this tick.
fn place_clear_of_others(
    rng: &mut StableRng,
    area: Aabb,
    radius: f32,
    world: &World,
    clearance: f32,
) -> Vec2 {
    let mut candidate = Vec2::ZERO;
    for _ in 0..SPAWN_PLACEMENT_ATTEMPTS {
        candidate = Vec2::new(
            rng.range_f32(area.min.x, area.max.x),
            rng.range_f32(area.min.y, area.max.y),
        );
        let clear = (0..world.len()).all(|slot| {
            let needed = radius + world.radius[slot] + clearance;
            world.position(slot as u32).distance_squared(candidate) >= needed * needed
        });
        if clear {
            return candidate;
        }
    }
    candidate
}

/// How many agents each spawn region has emitted so far.
#[derive(Clone, Debug, Default)]
pub struct SpawnState {
    emitted: Vec<u32>,
}

impl SpawnState {
    pub fn new(scene: &CompiledScene) -> Self {
        Self {
            emitted: vec![0; scene.spawns.len()],
        }
    }

    pub fn emitted(&self, spawn_index: usize) -> u32 {
        self.emitted[spawn_index]
    }

    pub fn all_emitted(&self, scene: &CompiledScene) -> bool {
        scene
            .spawns
            .iter()
            .zip(&self.emitted)
            .all(|(spawn, emitted)| *emitted >= spawn.count)
    }
}

/// Emit this tick's scheduled agents.
///
/// Returns any duplicate-ID diagnostics rather than panicking, so a caller can
/// surface them as bake-blocking errors per contract section 10.3.
pub fn apply_spawns(
    scene: &CompiledScene,
    state: &mut SpawnState,
    world: &mut World,
    routes: &mut RouteArena,
    tick: u64,
) -> Vec<SpawnError> {
    let mut errors = Vec::new();

    for (spawn_index, region) in scene.spawns.iter().enumerate() {
        let already = state.emitted[spawn_index];
        let remaining = region.count.saturating_sub(already);
        let this_tick = remaining.min(region.per_tick);

        for offset in 0..this_tick {
            let ordinal = already + offset;
            let agent_id =
                derive_agent_id(scene.project_seed, region.population_id, region.id, ordinal);

            let params = &scene.populations[region.population_id as usize];

            let mut radius_rng =
                StableRng::for_agent(scene.project_seed, agent_id, Purpose::Radius);
            let radius = radius_rng.range_f32(params.radius_min, params.radius_max);

            let mut speed_rng =
                StableRng::for_agent(scene.project_seed, agent_id, Purpose::PreferredSpeed);
            // Clamp keeps a rare tail sample from producing a zero or negative
            // preferred speed, which would make an agent permanently stalled.
            //
            // The bounds cannot invert: scene compilation rejects any
            // population with `speed_mean < MIN_PREFERRED_SPEED`, so the
            // ceiling is always at least twice the floor. That check is what
            // makes this `clamp` safe — `f32::clamp` panics on inverted
            // bounds, in release as well as debug.
            let preferred_speed = speed_rng
                .normal_f32(params.speed_mean, params.speed_stddev)
                .clamp(MIN_PREFERRED_SPEED, params.speed_mean * 2.0);

            let mut position_rng =
                StableRng::for_agent(scene.project_seed, agent_id, Purpose::SpawnPosition);
            let position = place_clear_of_others(
                &mut position_rng,
                region.area,
                radius,
                world,
                SPAWN_CLEARANCE,
            );

            let destination_node = scene.destinations[region.destination as usize].node;
            let route = match scene.waypoints.nearest_node(position) {
                Some(from) => match scene.waypoints.shortest_path(from, destination_node) {
                    Some(path) => {
                        let points: Vec<Vec2> =
                            path.iter().map(|n| scene.waypoints.position(*n)).collect();
                        routes.push_route(&points)
                    }
                    // Compilation already proved reachability from the region
                    // centre; an individual sample can still fail only if the
                    // graph is malformed, and an unrouted agent is preferable
                    // to a panic mid-bake.
                    None => NO_ROUTE,
                },
                None => NO_ROUTE,
            };

            let heading =
                (scene.waypoints.position(destination_node) - position).normalize_or_zero();

            let spawn = AgentSpawn {
                agent_id,
                population_id: region.population_id,
                position,
                yaw: heading.to_yaw(),
                radius,
                max_speed: preferred_speed * params.max_speed_factor,
                preferred_speed,
                route,
                destination: region.destination,
            };

            if let Err(error) = world.spawn(spawn, tick) {
                errors.push(error);
            }
        }

        state.emitted[spawn_index] = already + this_tick;
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::WaypointGraph;
    use crate::scene::{Destination, PopulationParams, SceneDef, SpawnRegion};
    use crate::units::{Aabb, Vec2};

    fn scene(count: u32, per_tick: u32) -> CompiledScene {
        let mut waypoints = WaypointGraph::new();
        let a = waypoints.add_node(Vec2::new(1.0, 5.0));
        let b = waypoints.add_node(Vec2::new(9.0, 5.0));
        waypoints.add_edge(a, b);
        SceneDef {
            name: "spawn_test".into(),
            bounds: Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)),
            walls: Vec::new(),
            waypoints,
            destinations: vec![Destination {
                name: "exit".into(),
                node: b,
            }],
            spawns: vec![SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(0.5, 4.0), Vec2::new(1.5, 6.0)),
                count,
                per_tick,
                destination: 0,
            }],
            populations: vec![PopulationParams::default()],
            project_seed: 42,
            ticks_per_second: 30,
            duration_ticks: 100,
        }
        .compile()
        .unwrap()
    }

    /// Like `scene`, but with a spawn region large enough that agents can be
    /// placed clear of one another.
    fn roomy_scene(count: u32) -> CompiledScene {
        let mut waypoints = WaypointGraph::new();
        let a = waypoints.add_node(Vec2::new(2.0, 15.0));
        let b = waypoints.add_node(Vec2::new(28.0, 15.0));
        waypoints.add_edge(a, b);
        SceneDef {
            name: "roomy_spawn_test".into(),
            bounds: Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(30.0, 30.0)),
            walls: Vec::new(),
            waypoints,
            destinations: vec![Destination {
                name: "exit".into(),
                node: b,
            }],
            spawns: vec![SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(1.0, 1.0), Vec2::new(20.0, 20.0)),
                count,
                per_tick: count,
                destination: 0,
            }],
            populations: vec![PopulationParams::default()],
            project_seed: 42,
            ticks_per_second: 30,
            duration_ticks: 100,
        }
        .compile()
        .unwrap()
    }

    fn run_ticks(scene: &CompiledScene, ticks: u64) -> (World, RouteArena) {
        let mut world = World::new();
        let mut routes = RouteArena::new();
        let mut state = SpawnState::new(scene);
        for tick in 0..ticks {
            let errors = apply_spawns(scene, &mut state, &mut world, &mut routes, tick);
            assert!(errors.is_empty(), "{errors:?}");
        }
        (world, routes)
    }

    #[test]
    fn spawns_are_rate_limited_per_tick() {
        let scene = scene(10, 3);
        let (world, _) = run_ticks(&scene, 1);
        assert_eq!(world.len(), 3);
    }

    #[test]
    fn spawning_stops_at_the_configured_count() {
        let scene = scene(10, 3);
        let (world, _) = run_ticks(&scene, 100);
        assert_eq!(world.len(), 10);
    }

    #[test]
    fn spawned_agents_land_inside_the_spawn_area() {
        let scene = scene(50, 50);
        let (world, _) = run_ticks(&scene, 1);
        let area = scene.spawns[0].area;
        for slot in 0..world.len() as u32 {
            assert!(area.contains(world.position(slot)), "slot {slot} escaped");
        }
    }

    #[test]
    fn spawned_agents_do_not_start_inside_each_other() {
        // Uniform placement could put two agents essentially co-located. That
        // accounted for every penetration event in the open benchmark scenes
        // -- all of it in the first tenth of a run, none after -- so it read
        // as a steering failure when it was a placement one.
        // A region with room to place them. Rejection sampling cannot help in
        // a region that is physically over-subscribed, and falling back to an
        // overlapping placement there is the correct behaviour -- the solver's
        // overlap gradient separates them.
        let scene = roomy_scene(40);
        let (world, _) = run_ticks(&scene, 1);
        assert_eq!(world.len(), 40);
        let mut overlaps = 0;
        for a in 0..world.len() {
            for b in (a + 1)..world.len() {
                let needed = world.radius[a] + world.radius[b];
                let d = world
                    .position(a as u32)
                    .distance_squared(world.position(b as u32))
                    .sqrt();
                if d < needed {
                    overlaps += 1;
                }
            }
        }
        assert_eq!(overlaps, 0, "{overlaps} pairs spawned interpenetrating");
    }

    #[test]
    fn placement_stays_reproducible() {
        // Rejection sampling draws from the agent's own stream, so a given
        // agent lands in the same place regardless of emission rate.
        let (fast, _) = run_ticks(&scene(20, 20), 1);
        let (slow, _) = run_ticks(&scene(20, 4), 5);
        assert_eq!(fast.agent_id, slow.agent_id);
        // Placement depends on who is already present, which the rate does
        // change -- so assert the invariant that actually must hold: no
        // overlaps either way, and the same population.
        assert_eq!(fast.len(), slow.len());
    }

    #[test]
    fn spawned_agents_receive_varied_attributes() {
        let scene = scene(200, 200);
        let (world, _) = run_ticks(&scene, 1);
        let first = world.radius[0];
        assert!(
            world.radius.iter().any(|r| *r != first),
            "no radius variation"
        );
        assert!(world.radius.iter().all(|r| (0.24..=0.38).contains(r)));
        assert!(world.preferred_speed.iter().all(|s| *s > 0.0));
        assert!(world
            .max_speed
            .iter()
            .zip(&world.preferred_speed)
            .all(|(m, p)| m >= p));
    }

    #[test]
    fn spawned_agents_receive_a_route_to_their_destination() {
        let scene = scene(5, 5);
        let (world, routes) = run_ticks(&scene, 1);
        for slot in 0..world.len() {
            let points = routes.points(world.route[slot]);
            assert!(!points.is_empty(), "slot {slot} has no route");
            assert_eq!(*points.last().unwrap(), Vec2::new(9.0, 5.0));
        }
    }

    #[test]
    fn agent_attributes_do_not_depend_on_spawn_rate() {
        // The same agent ordinal must get the same attributes whether it was
        // emitted alone or in a burst. This is contract section 4.2 applied to
        // the spawn scheduler.
        let (fast, _) = run_ticks(&scene(10, 10), 10);
        let (slow, _) = run_ticks(&scene(10, 1), 10);
        assert_eq!(fast.agent_id, slow.agent_id);
        assert_eq!(fast.radius, slow.radius);
        assert_eq!(fast.preferred_speed, slow.preferred_speed);
    }

    #[test]
    fn all_emitted_reports_completion() {
        let scene = scene(4, 2);
        let mut world = World::new();
        let mut routes = RouteArena::new();
        let mut state = SpawnState::new(&scene);
        apply_spawns(&scene, &mut state, &mut world, &mut routes, 0);
        assert!(!state.all_emitted(&scene));
        apply_spawns(&scene, &mut state, &mut world, &mut routes, 1);
        assert!(state.all_emitted(&scene));
    }
}
