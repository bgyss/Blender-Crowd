//! Tick phase 7: integrate.
//!
//! The sole writer of position and orientation. Applies acceleration and
//! turn-rate limits, advances state, and resolves residual wall penetration.
//!
//! Non-finite state is neutralised and counted rather than propagated: contract
//! section 10 makes the tick loop infallible, and silently carrying a NaN
//! through a bake is the worst available outcome.

use crate::geometry::Segment;
use crate::scene::CompiledScene;
use crate::units::{wrap_angle, Vec2};
use crate::world::World;

#[derive(Clone, Copy, Debug)]
pub struct IntegrateConfig {
    /// Metres per second squared. Pedestrians accelerate modestly; a high cap
    /// lets the solver produce visible velocity discontinuities.
    pub max_acceleration: f32,
    /// Radians per second.
    pub max_turn_rate: f32,
    pub wall_query_radius: f32,
}

impl Default for IntegrateConfig {
    fn default() -> Self {
        Self {
            max_acceleration: 4.0,
            max_turn_rate: 6.0,
            wall_query_radius: 2.0,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct IntegrateScratch {
    wall_indices: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IntegrateReport {
    /// Agents pushed out of a wall this tick. A persistently non-zero count
    /// means avoidance is failing upstream, not that the fix-up is working.
    pub wall_corrections: u32,
    pub nonfinite_corrections: u32,
}

pub fn integrate(
    world: &mut World,
    scene: &CompiledScene,
    config: &IntegrateConfig,
    dt: f32,
    scratch: &mut IntegrateScratch,
) -> IntegrateReport {
    let mut report = IntegrateReport::default();
    let max_delta_v = config.max_acceleration * dt;
    let max_delta_yaw = config.max_turn_rate * dt;

    for slot in 0..world.len() {
        let position = Vec2::new(world.pos_x[slot], world.pos_y[slot]);
        let velocity = Vec2::new(world.vel_x[slot], world.vel_y[slot]);
        let mut desired = Vec2::new(world.des_vel_x[slot], world.des_vel_y[slot]);

        if !desired.is_finite() {
            desired = Vec2::ZERO;
            report.nonfinite_corrections += 1;
        }

        // Acceleration limit, then speed limit. Order matters: clamping speed
        // first would let a large lateral correction slip through.
        let delta = (desired - velocity).clamp_length(max_delta_v);
        let new_velocity = (velocity + delta).clamp_length(world.max_speed[slot]);
        let mut new_position = position + new_velocity * dt;

        // Resolve residual penetration. Avoidance should have prevented this;
        // when it did not, pushing out along the wall normal is preferable to
        // letting an agent walk through geometry.
        scene.wall_index.query(
            new_position,
            config.wall_query_radius,
            &mut scratch.wall_indices,
        );
        let mut corrected = false;
        for &index in &scratch.wall_indices {
            let wall: Segment = scene.walls[index as usize];
            let closest = wall.closest_point(new_position);
            let offset = new_position - closest;
            let distance = offset.length();
            let radius = world.radius[slot];
            if distance < radius {
                let normal = if distance > f32::MIN_POSITIVE {
                    offset * (1.0 / distance)
                } else {
                    // Exactly on the wall: push along its normal, chosen by a
                    // fixed rule so the correction stays deterministic.
                    (wall.b - wall.a).normalize_or_zero().perp()
                };
                new_position = closest + normal * radius;
                corrected = true;
            }
        }
        if corrected {
            report.wall_corrections += 1;
        }

        let mut new_yaw = world.yaw[slot];
        if new_velocity.length_squared() > 1e-6 {
            let target_yaw = new_velocity.to_yaw();
            let delta_yaw = wrap_angle(target_yaw - new_yaw).clamp(-max_delta_yaw, max_delta_yaw);
            new_yaw = wrap_angle(new_yaw + delta_yaw);
        }

        if !new_position.is_finite() || !new_velocity.is_finite() || !new_yaw.is_finite() {
            report.nonfinite_corrections += 1;
            world.next_pos_x[slot] = position.x;
            world.next_pos_y[slot] = position.y;
            world.next_vel_x[slot] = 0.0;
            world.next_vel_y[slot] = 0.0;
            world.next_yaw[slot] = world.yaw[slot];
            continue;
        }

        world.next_pos_x[slot] = new_position.x;
        world.next_pos_y[slot] = new_position.y;
        world.next_vel_x[slot] = new_velocity.x;
        world.next_vel_y[slot] = new_velocity.y;
        world.next_yaw[slot] = new_yaw;
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::AgentId;
    use crate::route::WaypointGraph;
    use crate::scene::{Destination, PopulationParams, SceneDef, SpawnRegion};
    use crate::units::{Aabb, Vec2};
    use crate::world::{AgentSpawn, World, NO_ROUTE};

    fn scene_with(walls: Vec<Segment>) -> CompiledScene {
        let mut waypoints = WaypointGraph::new();
        let a = waypoints.add_node(Vec2::new(1.0, 5.0));
        let b = waypoints.add_node(Vec2::new(9.0, 5.0));
        waypoints.add_edge(a, b);
        SceneDef {
            name: "integrate_test".into(),
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

    fn world_with(position: Vec2, velocity: Vec2, desired: Vec2, yaw: f32) -> World {
        let mut world = World::new();
        world
            .spawn(
                AgentSpawn {
                    agent_id: AgentId(1),
                    population_id: 0,
                    position,
                    yaw,
                    radius: 0.3,
                    max_speed: 2.0,
                    preferred_speed: 1.35,
                    route: NO_ROUTE,
                    destination: 0,
                },
                0,
            )
            .unwrap();
        world.vel_x[0] = velocity.x;
        world.vel_y[0] = velocity.y;
        world.des_vel_x[0] = desired.x;
        world.des_vel_y[0] = desired.y;
        world
    }

    fn run(world: &mut World, scene: &CompiledScene) -> IntegrateReport {
        let mut scratch = IntegrateScratch::default();
        let report = integrate(
            world,
            scene,
            &IntegrateConfig::default(),
            1.0 / 30.0,
            &mut scratch,
        );
        world.commit();
        report
    }

    #[test]
    fn an_agent_moves_along_its_desired_velocity() {
        let scene = scene_with(Vec::new());
        let mut world = world_with(Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(1.0, 0.0), 0.0);
        run(&mut world, &scene);
        assert!((world.position(0).x - 1.0 / 30.0).abs() < 1e-5);
        assert!(world.position(0).y.abs() < 1e-6);
    }

    #[test]
    fn acceleration_is_limited() {
        // Desired velocity jumps from rest to 2 m/s in one 1/30 s tick. With
        // max_acceleration 4.0, only 4/30 m/s of change is allowed.
        let scene = scene_with(Vec::new());
        let mut world = world_with(Vec2::ZERO, Vec2::ZERO, Vec2::new(2.0, 0.0), 0.0);
        run(&mut world, &scene);
        let speed = world.velocity(0).length();
        assert!((speed - 4.0 / 30.0).abs() < 1e-4, "speed was {speed}");
    }

    #[test]
    fn speed_never_exceeds_max_speed() {
        let scene = scene_with(Vec::new());
        let mut world = world_with(Vec2::ZERO, Vec2::new(1.9, 0.0), Vec2::new(50.0, 0.0), 0.0);
        for _ in 0..100 {
            run(&mut world, &scene);
        }
        assert!(world.velocity(0).length() <= 2.0 + 1e-4);
    }

    #[test]
    fn turn_rate_is_limited() {
        // A 180-degree reversal cannot complete in one tick at 6 rad/s.
        let scene = scene_with(Vec::new());
        let mut world = world_with(Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(-1.0, 0.0), 0.0);
        run(&mut world, &scene);
        assert!(
            world.yaw[0].abs() <= 6.0 / 30.0 + 1e-4,
            "yaw was {}",
            world.yaw[0]
        );
    }

    #[test]
    fn yaw_is_unchanged_when_an_agent_is_stationary() {
        let scene = scene_with(Vec::new());
        let mut world = world_with(Vec2::ZERO, Vec2::ZERO, Vec2::ZERO, 1.234);
        run(&mut world, &scene);
        assert!((world.yaw[0] - 1.234).abs() < 1e-6);
    }

    #[test]
    fn an_agent_is_pushed_out_of_a_wall_it_penetrated() {
        let walls = vec![Segment::new(Vec2::new(1.0, -5.0), Vec2::new(1.0, 5.0))];
        let scene = scene_with(walls);
        // Start 0.1m from the wall with an agent radius of 0.3: penetrated.
        let mut world = world_with(Vec2::new(0.9, 0.0), Vec2::ZERO, Vec2::ZERO, 0.0);
        let report = run(&mut world, &scene);
        assert_eq!(report.wall_corrections, 1);
        assert!(
            world.position(0).x <= 0.7 + 1e-4,
            "agent was not pushed clear: {:?}",
            world.position(0)
        );
    }

    #[test]
    fn an_agent_clear_of_walls_is_not_corrected() {
        let walls = vec![Segment::new(Vec2::new(5.0, -5.0), Vec2::new(5.0, 5.0))];
        let scene = scene_with(walls);
        let mut world = world_with(Vec2::ZERO, Vec2::ZERO, Vec2::ZERO, 0.0);
        assert_eq!(run(&mut world, &scene).wall_corrections, 0);
    }

    #[test]
    fn a_nonfinite_desired_velocity_is_neutralised() {
        let scene = scene_with(Vec::new());
        let mut world = world_with(Vec2::ZERO, Vec2::ZERO, Vec2::ZERO, 0.0);
        world.des_vel_x[0] = f32::NAN;
        let report = integrate(
            &mut world,
            &scene,
            &IntegrateConfig::default(),
            1.0 / 30.0,
            &mut IntegrateScratch::default(),
        );
        world.commit();
        assert_eq!(report.nonfinite_corrections, 1);
        assert!(world.position(0).is_finite());
        assert!(world.velocity(0).is_finite());
    }

    #[test]
    fn integration_is_the_only_writer_of_position() {
        let scene = scene_with(Vec::new());
        let mut world = world_with(Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(1.0, 0.0), 0.0);
        let before = world.position(0);
        let mut scratch = IntegrateScratch::default();
        integrate(
            &mut world,
            &scene,
            &IntegrateConfig::default(),
            1.0 / 30.0,
            &mut scratch,
        );
        // Until commit, current state is untouched.
        assert_eq!(world.position(0), before);
        world.commit();
        assert_ne!(world.position(0), before);
    }
}
