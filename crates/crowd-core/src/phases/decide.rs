//! Tick phases 4 and 5: decide and plan.
//!
//! Contract section 6 separates these, but with a fixed rule and routes
//! resolved at spawn there is nothing to separate. The behavior graph
//! (contract section 4.4) will split them apart; until then, collapsing them
//! avoids a phase that does nothing.
//!
//! Writes each agent's *preferred* velocity. The steer phase applies avoidance
//! on top, so the two concerns stay independently testable.

use crate::route::{next_target, RouteArena};
use crate::units::Vec2;
use crate::world::World;

#[derive(Clone, Copy, Debug)]
pub struct DecideConfig {
    /// How close counts as having reached a waypoint.
    pub arrive_radius: f32,
}

impl Default for DecideConfig {
    fn default() -> Self {
        Self { arrive_radius: 0.6 }
    }
}

pub fn decide(world: &mut World, routes: &RouteArena, config: &DecideConfig) {
    for slot in 0..world.len() {
        if world.arrived[slot] {
            world.des_vel_x[slot] = 0.0;
            world.des_vel_y[slot] = 0.0;
            continue;
        }

        let position = Vec2::new(world.pos_x[slot], world.pos_y[slot]);
        let points = routes.points(world.route[slot]);
        let mut index = world.route_index[slot];
        let target = next_target(points, &mut index, position, config.arrive_radius);
        world.route_index[slot] = index;

        match target {
            Some(target) => {
                let direction = (target - position).normalize_or_zero();
                let preferred = direction * world.preferred_speed[slot];
                world.des_vel_x[slot] = preferred.x;
                world.des_vel_y[slot] = preferred.y;
            }
            None => {
                // Route exhausted, or the agent never had one. Stopping rather
                // than drifting keeps an unrouted agent from wandering into
                // other agents and corrupting the metrics.
                world.arrived[slot] = true;
                world.des_vel_x[slot] = 0.0;
                world.des_vel_y[slot] = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::AgentId;
    use crate::units::Vec2;
    use crate::world::{AgentSpawn, World, NO_ROUTE};

    fn world_with_route(position: Vec2, points: &[Vec2]) -> (World, RouteArena) {
        let mut routes = RouteArena::new();
        let route = if points.is_empty() {
            NO_ROUTE
        } else {
            routes.push_route(points)
        };
        let mut world = World::new();
        world
            .spawn(
                AgentSpawn {
                    agent_id: AgentId(1),
                    population_id: 0,
                    position,
                    yaw: 0.0,
                    radius: 0.3,
                    max_speed: 2.0,
                    preferred_speed: 1.5,
                    route,
                    destination: 0,
                },
                0,
            )
            .unwrap();
        (world, routes)
    }

    #[test]
    fn preferred_velocity_points_at_the_next_waypoint() {
        let (mut world, routes) = world_with_route(Vec2::ZERO, &[Vec2::new(10.0, 0.0)]);
        decide(&mut world, &routes, &DecideConfig::default());
        assert!((world.des_vel_x[0] - 1.5).abs() < 1e-5);
        assert!(world.des_vel_y[0].abs() < 1e-5);
    }

    #[test]
    fn preferred_velocity_magnitude_is_the_preferred_speed() {
        let (mut world, routes) = world_with_route(Vec2::ZERO, &[Vec2::new(3.0, 4.0)]);
        decide(&mut world, &routes, &DecideConfig::default());
        let speed = world.desired_velocity(0).length();
        assert!((speed - 1.5).abs() < 1e-5, "speed was {speed}");
    }

    #[test]
    fn reaching_the_final_waypoint_marks_arrival_and_stops_the_agent() {
        let (mut world, routes) = world_with_route(Vec2::new(10.0, 0.0), &[Vec2::new(10.0, 0.0)]);
        decide(&mut world, &routes, &DecideConfig::default());
        assert!(world.arrived[0]);
        assert_eq!(world.desired_velocity(0), Vec2::ZERO);
    }

    #[test]
    fn passing_a_waypoint_advances_the_route_index() {
        let (mut world, routes) = world_with_route(Vec2::ZERO, &[Vec2::ZERO, Vec2::new(10.0, 0.0)]);
        decide(&mut world, &routes, &DecideConfig::default());
        assert_eq!(world.route_index[0], 1);
        assert!(!world.arrived[0]);
    }

    #[test]
    fn an_agent_with_no_route_stops_rather_than_drifting() {
        let (mut world, routes) = world_with_route(Vec2::ZERO, &[]);
        decide(&mut world, &routes, &DecideConfig::default());
        assert_eq!(world.desired_velocity(0), Vec2::ZERO);
        assert!(world.arrived[0]);
    }

    #[test]
    fn an_arrived_agent_stays_arrived() {
        let (mut world, routes) = world_with_route(Vec2::new(10.0, 0.0), &[Vec2::new(10.0, 0.0)]);
        decide(&mut world, &routes, &DecideConfig::default());
        let index_after_arrival = world.route_index[0];
        decide(&mut world, &routes, &DecideConfig::default());
        assert!(world.arrived[0]);
        assert_eq!(world.route_index[0], index_after_arrival);
    }
}
