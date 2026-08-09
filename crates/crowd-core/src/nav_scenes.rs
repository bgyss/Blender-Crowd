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
use crate::nav::{CrossingAxis, NavMeshDef};
use crate::scene::{Destination, PopulationParams, SceneDef, SpawnRegion};
use crate::units::{Aabb, Vec2};

pub const NORTH_DOOR: &str = "north_door";
pub const SOUTH_DOOR: &str = "south_door";

/// The x coordinate of the wall separating room A (x < DIVIDER_X) from room B
/// (x > DIVIDER_X). Exposed so callers such as tests can tell, from an
/// agent's position alone, whether it still needs to cross a doorway to reach
/// room B or has already crossed the divider before a door closed.
pub const DIVIDER_X: f32 = 20.0;

/// Room A: x in [0, 20]. Room B: x in [20, 40]. Both rooms span y in [0, 20].
/// Two doorways in the dividing wall at x=20: one centered at y=6 (south),
/// one at y=14 (north), each 1.6 m wide — wide enough to stay walkable after
/// the default 0.3 m agent-radius inflation this scene uses.
pub fn two_room(agents: u32, seed: u64) -> SceneDef {
    const DOOR_HALF_WIDTH: f32 = 0.8;
    const TILE_SIZE: f32 = 0.5;
    // A doorway 1.6 m wide, tiled at 0.5 m and inflated by the scene's 0.3 m
    // agent radius, crosses the divider through more than one adjacent tile
    // row — so it has more than one portal. `portals_crossing` walks the
    // connected run of crossing portals outward from each door's named
    // point, so no capture radius is needed here: the walk stops on its own
    // at the solid wall bounding each doorway, and never spills into the
    // other doorway 8 m away.
    let bounds = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(40.0, 20.0));
    let divider_x = DIVIDER_X;
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
            tile_size: TILE_SIZE,
            agent_radius: 0.3,
            cost_areas: Vec::new(),
            named_portals: vec![
                (
                    SOUTH_DOOR.to_string(),
                    Vec2::new(divider_x, south_y),
                    // Both doorways sit in the vertical divider wall at
                    // x=DIVIDER_X, so only east-west portals (column-adjacent
                    // tiles) can cross them.
                    CrossingAxis::EastWest,
                ),
                (
                    NORTH_DOOR.to_string(),
                    Vec2::new(divider_x, north_y),
                    CrossingAxis::EastWest,
                ),
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
    fn both_named_doors_resolve_to_distinct_portal_sets() {
        let compiled = two_room(50, 42).compile().unwrap();
        let nav = compiled.nav.as_ref().unwrap();
        let south = nav.portals_named(SOUTH_DOOR);
        let north = nav.portals_named(NORTH_DOOR);
        assert!(!south.is_empty());
        assert!(!north.is_empty());
        assert!(
            south.iter().all(|s| !north.contains(s)),
            "south and north doors must not share a portal: south={south:?} north={north:?}"
        );
    }

    #[test]
    fn south_door_names_every_portal_that_crosses_its_doorway() {
        // The load-bearing regression for the whole-branch review's critical
        // finding: a 1.6 m doorway tiled at 0.5 m, inflated by the 0.3 m
        // agent radius, spans more than one tile row and therefore more than
        // one crossing portal. Naming only one of them left the doorway only
        // half-closed. This scene's south doorway must resolve to at least
        // two portals.
        let compiled = two_room(50, 42).compile().unwrap();
        let nav = compiled.nav.as_ref().unwrap();
        let south = nav.portals_named(SOUTH_DOOR);
        assert!(
            south.len() >= 2,
            "south doorway resolved to only {} portal(s); closing it would leave the \
             doorway's other crossing point open: {south:?}",
            south.len()
        );
    }

    #[test]
    fn named_doors_resolve_to_exactly_the_portals_that_cross_the_divider() {
        // `portals_crossing` walks the connected run of straddling portals
        // outward from each door's named point rather than filtering a
        // fixed-radius proximity set, so it cannot under- or over-capture:
        // it always stops exactly at the solid wall bounding a doorway on
        // each side. This test pins down the resulting exact set for this
        // scene's geometry — not merely "at least one" or "more than the
        // radius-based candidate count minus false positives," but the
        // precise portal set, cross-checked here against a direct
        // straddle/axis scan of every returned portal.
        //
        // Why exactly 2: DOOR_HALF_WIDTH is 0.8 and TILE_SIZE is 0.5, so the
        // doorway spans y in roughly [5.2, 6.8] (south) — 1.6 m — which
        // after 0.3 m agent-radius inflation still clears at most two 0.5 m
        // tile rows worth of walkable center points immediately adjacent to
        // the divider on each side, giving exactly two east-west portal
        // crossings per doorway.
        let compiled = two_room(50, 42).compile().unwrap();
        let nav = compiled.nav.as_ref().unwrap();
        let grid = nav.grid();

        for door in [SOUTH_DOOR, NORTH_DOOR] {
            let ids = nav.portals_named(door);
            assert_eq!(
                ids.len(),
                2,
                "{door} resolved to {} portal(s), expected exactly 2: {ids:?}",
                ids.len()
            );
            for &id in ids {
                let portal = nav.portal(id);
                let (col_a, _row_a) = grid.col_row(portal.tile_a);
                let (col_b, _row_b) = grid.col_row(portal.tile_b);
                let a_center = grid.tile_center(portal.tile_a);
                let b_center = grid.tile_center(portal.tile_b);
                assert_ne!(
                    col_a, col_b,
                    "{door} portal {id:?} is not east-west: {portal:?}"
                );
                assert!(
                    (a_center.x < DIVIDER_X) != (b_center.x < DIVIDER_X),
                    "{door} portal {id:?} does not straddle the divider at x={DIVIDER_X}: \
                     tile_a center={a_center:?}, tile_b center={b_center:?}"
                );
            }
        }
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
