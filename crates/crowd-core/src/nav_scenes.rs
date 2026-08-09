//! The dedicated tiled-navmesh prototype scene: two rooms joined by two
//! doorways, built specifically to host M0 acceptance criterion 3 (a
//! 1,000-agent reroute after a portal change).
//!
//! Deliberately not part of `crowd_core::scenes::SCENE_NAMES` — it is a
//! navigation-architecture proof, not an avoidance-solver benchmark scene,
//! and folding it into `scenes::build` would pull it into the default
//! (no `--scene`) `crowd-bench sweep|baseline|check|compare` runs, which
//! need a checked-in baseline it does not have.

use crate::geometry::Segment;
use crate::nav::NavMeshDef;
use crate::scene::{Destination, PopulationParams, SceneDef, SpawnRegion};
use crate::units::{Aabb, Vec2};

pub const NORTH_DOOR: &str = "north_door";
pub const SOUTH_DOOR: &str = "south_door";

/// Room A: x in [0, 20]. Room B: x in [20, 40]. Both rooms span y in [0, 20].
/// Two doorways in the dividing wall at x=20: one centered at y=6 (south),
/// one at y=14 (north), each 1.6 m wide — wide enough to stay walkable after
/// the default 0.3 m agent-radius inflation this scene uses.
pub fn two_room(agents: u32, seed: u64) -> SceneDef {
    const DOOR_HALF_WIDTH: f32 = 0.8;
    let bounds = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(40.0, 20.0));
    let divider_x = 20.0;
    let south_y = 6.0;
    let north_y = 14.0;

    let mut walls = vec![
        Segment::new(Vec2::new(0.0, 0.0), Vec2::new(40.0, 0.0)),
        Segment::new(Vec2::new(40.0, 0.0), Vec2::new(40.0, 20.0)),
        Segment::new(Vec2::new(40.0, 20.0), Vec2::new(0.0, 20.0)),
        Segment::new(Vec2::new(0.0, 20.0), Vec2::new(0.0, 0.0)),
    ];
    // The dividing wall, with two doorway gaps.
    walls.push(Segment::new(
        Vec2::new(divider_x, 0.0),
        Vec2::new(divider_x, south_y - DOOR_HALF_WIDTH),
    ));
    walls.push(Segment::new(
        Vec2::new(divider_x, south_y + DOOR_HALF_WIDTH),
        Vec2::new(divider_x, north_y - DOOR_HALF_WIDTH),
    ));
    walls.push(Segment::new(
        Vec2::new(divider_x, north_y + DOOR_HALF_WIDTH),
        Vec2::new(divider_x, 20.0),
    ));

    SceneDef {
        name: "two_room".into(),
        bounds,
        walls,
        waypoints: crate::route::WaypointGraph::new(),
        destinations: vec![Destination {
            name: "room_b".into(),
            node: 0, // unused: nav_destinations carries the real point
        }],
        spawns: vec![SpawnRegion {
            id: 0,
            population_id: 0,
            area: Aabb::new(Vec2::new(1.0, 1.0), Vec2::new(18.0, 19.0)),
            count: agents,
            per_tick: 8,
            destination: 0,
        }],
        populations: vec![PopulationParams::default()],
        project_seed: seed,
        ticks_per_second: 30,
        duration_ticks: 3600,
        nav: Some(NavMeshDef {
            tile_size: 0.5,
            agent_radius: 0.3,
            cost_areas: Vec::new(),
            named_portals: vec![
                (SOUTH_DOOR.to_string(), Vec2::new(divider_x, south_y)),
                (NORTH_DOOR.to_string(), Vec2::new(divider_x, north_y)),
            ],
        }),
        nav_destinations: vec![Vec2::new(38.0, 10.0)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avoidance::SampledVelocitySolver;
    use crate::sim::{SimConfig, Simulation};

    #[test]
    fn two_room_compiles_without_diagnostics() {
        assert!(two_room(100, 42).compile().is_ok());
    }

    #[test]
    fn two_room_spawns_the_requested_agent_count() {
        let compiled = two_room(200, 42).compile().unwrap();
        assert_eq!(compiled.total_agents(), 200);
    }

    #[test]
    fn both_named_doors_resolve_to_distinct_portals() {
        let compiled = two_room(50, 42).compile().unwrap();
        let nav = compiled.nav.as_ref().unwrap();
        let south = nav.portal_named(SOUTH_DOOR);
        let north = nav.portal_named(NORTH_DOOR);
        assert!(south.is_some());
        assert!(north.is_some());
        assert_ne!(south, north);
    }

    #[test]
    fn two_room_runs_without_producing_nonfinite_state() {
        let compiled = two_room(50, 42).compile().unwrap();
        let mut sim = Simulation::new(
            compiled,
            Box::new(SampledVelocitySolver::default()),
            SimConfig::default(),
        );
        sim.run(300);
        for slot in 0..sim.world().len() {
            assert!(sim.world().position(slot as u32).is_finite());
        }
    }
}
