//! Tick phase 1: apply inputs.
//!
//! The only timed input this slice has is scheduled agent spawns. Every varied
//! attribute is drawn from a stream keyed by the agent's stable ID, never from
//! a shared sequence, so emission rate and ordering cannot change what any
//! individual agent looks like.

use crate::ids::derive_agent_id;
use crate::rng::{Purpose, StableRng};
use crate::route::RouteArena;
use crate::scene::CompiledScene;
use crate::units::Vec2;
use crate::world::{AgentSpawn, SpawnError, World, NO_ROUTE};

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
            let preferred_speed = speed_rng
                .normal_f32(params.speed_mean, params.speed_stddev)
                .clamp(0.4, params.speed_mean * 2.0);

            let mut position_rng =
                StableRng::for_agent(scene.project_seed, agent_id, Purpose::SpawnPosition);
            let position = Vec2::new(
                position_rng.range_f32(region.area.min.x, region.area.max.x),
                position_rng.range_f32(region.area.min.y, region.area.max.y),
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
