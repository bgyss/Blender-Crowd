//! The five benchmark scenes.
//!
//! Chosen to cover the failure modes contract section 6.2 names: lane
//! formation, perpendicular conflict, doorway congestion, dense convergence,
//! and the antipodal swap that is the cheapest known exposure of oscillation
//! and deadlock.

use crate::geometry::Segment;
use crate::route::WaypointGraph;
use crate::scene::{Destination, PopulationParams, SceneDef, SpawnRegion};
use crate::units::{Aabb, Vec2};

pub const SCENE_NAMES: [&str; 5] = [
    "bidirectional_corridor",
    "crossing",
    "bottleneck",
    "dense_flow",
    "circle",
];

pub fn build(name: &str, agents: u32, seed: u64) -> Option<SceneDef> {
    match name {
        "bidirectional_corridor" => Some(bidirectional_corridor(agents, seed)),
        "crossing" => Some(crossing(agents, seed)),
        "bottleneck" => Some(bottleneck(agents, seed)),
        "dense_flow" => Some(dense_flow(agents, seed)),
        "circle" => Some(circle(agents, seed)),
        _ => None,
    }
}

/// The line crossings are counted against, for scenes where throughput is the
/// interesting measure.
pub fn throughput_gate(name: &str) -> Option<Segment> {
    match name {
        "bottleneck" => Some(Segment::new(Vec2::new(20.0, 8.0), Vec2::new(20.0, 12.0))),
        "dense_flow" => Some(Segment::new(Vec2::new(28.0, 12.0), Vec2::new(28.0, 18.0))),
        _ => None,
    }
}

/// Split `total` across `parts` so the counts sum to exactly `total`.
///
/// The naive `total / parts` silently drops the remainder, which would make a
/// requested 1,000-agent benchmark quietly run 992 agents.
fn split_count(total: u32, parts: u32, index: u32) -> u32 {
    let base = total / parts;
    let remainder = total % parts;
    base + u32::from(index < remainder)
}

fn box_walls(bounds: Aabb) -> Vec<Segment> {
    let Aabb { min, max } = bounds;
    vec![
        Segment::new(min, Vec2::new(max.x, min.y)),
        Segment::new(Vec2::new(max.x, min.y), max),
        Segment::new(max, Vec2::new(min.x, max.y)),
        Segment::new(Vec2::new(min.x, max.y), min),
    ]
}

/// Two opposing flows down one corridor. Lane formation is the thing to watch.
fn bidirectional_corridor(agents: u32, seed: u64) -> SceneDef {
    let bounds = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(40.0, 8.0));
    let mut waypoints = WaypointGraph::new();
    let west = waypoints.add_node(Vec2::new(2.0, 4.0));
    let east = waypoints.add_node(Vec2::new(38.0, 4.0));
    waypoints.add_edge(west, east);

    SceneDef {
        name: "bidirectional_corridor".into(),
        bounds,
        walls: vec![
            Segment::new(Vec2::new(0.0, 0.0), Vec2::new(40.0, 0.0)),
            Segment::new(Vec2::new(0.0, 8.0), Vec2::new(40.0, 8.0)),
        ],
        waypoints,
        destinations: vec![
            Destination {
                name: "west".into(),
                node: west,
            },
            Destination {
                name: "east".into(),
                node: east,
            },
        ],
        spawns: vec![
            SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(0.5, 0.5), Vec2::new(6.0, 7.5)),
                count: split_count(agents, 2, 0),
                per_tick: 4,
                destination: 1,
            },
            SpawnRegion {
                id: 1,
                population_id: 0,
                area: Aabb::new(Vec2::new(34.0, 0.5), Vec2::new(39.5, 7.5)),
                count: split_count(agents, 2, 1),
                per_tick: 4,
                destination: 0,
            },
        ],
        populations: vec![PopulationParams::default()],
        project_seed: seed,
        ticks_per_second: 30,
        duration_ticks: 1800,
    }
}

/// Two perpendicular flows through a shared plaza.
fn crossing(agents: u32, seed: u64) -> SceneDef {
    let bounds = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(40.0, 40.0));
    let mut waypoints = WaypointGraph::new();
    let centre = waypoints.add_node(Vec2::new(20.0, 20.0));
    let west = waypoints.add_node(Vec2::new(2.0, 20.0));
    let east = waypoints.add_node(Vec2::new(38.0, 20.0));
    let south = waypoints.add_node(Vec2::new(20.0, 2.0));
    let north = waypoints.add_node(Vec2::new(20.0, 38.0));
    for node in [west, east, south, north] {
        waypoints.add_edge(centre, node);
    }

    SceneDef {
        name: "crossing".into(),
        bounds,
        walls: box_walls(bounds),
        waypoints,
        destinations: vec![
            Destination {
                name: "east".into(),
                node: east,
            },
            Destination {
                name: "north".into(),
                node: north,
            },
        ],
        spawns: vec![
            SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(0.5, 15.0), Vec2::new(6.0, 25.0)),
                count: split_count(agents, 2, 0),
                per_tick: 4,
                destination: 0,
            },
            SpawnRegion {
                id: 1,
                population_id: 0,
                area: Aabb::new(Vec2::new(15.0, 0.5), Vec2::new(25.0, 6.0)),
                count: split_count(agents, 2, 1),
                per_tick: 4,
                destination: 1,
            },
        ],
        populations: vec![PopulationParams::default()],
        project_seed: seed,
        ticks_per_second: 30,
        duration_ticks: 1800,
    }
}

/// One room emptying into another through a 1.6 m doorway.
fn bottleneck(agents: u32, seed: u64) -> SceneDef {
    let bounds = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(40.0, 20.0));
    let mut waypoints = WaypointGraph::new();
    let start = waypoints.add_node(Vec2::new(8.0, 10.0));
    let approach = waypoints.add_node(Vec2::new(18.5, 10.0));
    let through = waypoints.add_node(Vec2::new(21.5, 10.0));
    let exit = waypoints.add_node(Vec2::new(34.0, 10.0));
    waypoints.add_edge(start, approach);
    waypoints.add_edge(approach, through);
    waypoints.add_edge(through, exit);

    let mut walls = box_walls(bounds);
    // The divider, with a gap from y = 9.2 to y = 10.8.
    walls.push(Segment::new(Vec2::new(20.0, 0.0), Vec2::new(20.0, 9.2)));
    walls.push(Segment::new(Vec2::new(20.0, 10.8), Vec2::new(20.0, 20.0)));

    SceneDef {
        name: "bottleneck".into(),
        bounds,
        walls,
        waypoints,
        destinations: vec![Destination {
            name: "exit".into(),
            node: exit,
        }],
        spawns: vec![SpawnRegion {
            id: 0,
            population_id: 0,
            area: Aabb::new(Vec2::new(1.0, 2.0), Vec2::new(14.0, 18.0)),
            count: agents,
            per_tick: 8,
            destination: 0,
        }],
        populations: vec![PopulationParams::default()],
        project_seed: seed,
        ticks_per_second: 30,
        duration_ticks: 3600,
    }
}

/// A dense blob converging on a single exit corridor.
fn dense_flow(agents: u32, seed: u64) -> SceneDef {
    let bounds = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(40.0, 30.0));
    let mut waypoints = WaypointGraph::new();
    let start = waypoints.add_node(Vec2::new(12.0, 15.0));
    let mouth = waypoints.add_node(Vec2::new(26.0, 15.0));
    let exit = waypoints.add_node(Vec2::new(38.0, 15.0));
    waypoints.add_edge(start, mouth);
    waypoints.add_edge(mouth, exit);

    let mut walls = box_walls(bounds);
    // A funnel narrowing to a 6 m mouth.
    walls.push(Segment::new(Vec2::new(24.0, 0.0), Vec2::new(28.0, 12.0)));
    walls.push(Segment::new(Vec2::new(24.0, 30.0), Vec2::new(28.0, 18.0)));
    walls.push(Segment::new(Vec2::new(28.0, 12.0), Vec2::new(40.0, 12.0)));
    walls.push(Segment::new(Vec2::new(28.0, 18.0), Vec2::new(40.0, 18.0)));

    SceneDef {
        name: "dense_flow".into(),
        bounds,
        walls,
        waypoints,
        destinations: vec![Destination {
            name: "exit".into(),
            node: exit,
        }],
        spawns: vec![SpawnRegion {
            id: 0,
            population_id: 0,
            area: Aabb::new(Vec2::new(1.0, 2.0), Vec2::new(18.0, 28.0)),
            count: agents,
            per_tick: 12,
            destination: 0,
        }],
        populations: vec![PopulationParams::default()],
        project_seed: seed,
        ticks_per_second: 30,
        duration_ticks: 3600,
    }
}

/// Agents on a ring walking to the antipodal point.
///
/// Everyone converges on the centre simultaneously with perfect symmetry,
/// which is why this is the standard oscillation and deadlock test.
fn circle(agents: u32, seed: u64) -> SceneDef {
    const SECTORS: u32 = 16;
    const RADIUS: f32 = 15.0;
    let bounds = Aabb::new(Vec2::new(-20.0, -20.0), Vec2::new(20.0, 20.0));

    let mut waypoints = WaypointGraph::new();
    let centre = waypoints.add_node(Vec2::ZERO);
    let mut perimeter = Vec::new();
    for sector in 0..SECTORS {
        let angle = std::f32::consts::TAU * sector as f32 / SECTORS as f32;
        let node = waypoints.add_node(Vec2::from_yaw(angle) * RADIUS);
        waypoints.add_edge(centre, node);
        perimeter.push(node);
    }

    let destinations: Vec<Destination> = (0..SECTORS)
        .map(|sector| Destination {
            name: format!("sector_{sector}"),
            node: perimeter[sector as usize],
        })
        .collect();

    let spawns: Vec<SpawnRegion> = (0..SECTORS)
        .map(|sector| {
            let angle = std::f32::consts::TAU * sector as f32 / SECTORS as f32;
            let centre_point = Vec2::from_yaw(angle) * RADIUS;
            SpawnRegion {
                id: sector as u16,
                population_id: 0,
                area: Aabb::new(
                    Vec2::new(centre_point.x - 1.5, centre_point.y - 1.5),
                    Vec2::new(centre_point.x + 1.5, centre_point.y + 1.5),
                ),
                count: split_count(agents, SECTORS, sector),
                per_tick: 4,
                // The antipodal sector.
                destination: ((sector + SECTORS / 2) % SECTORS) as u16,
            }
        })
        .collect();

    SceneDef {
        name: "circle".into(),
        bounds,
        walls: Vec::new(),
        waypoints,
        destinations,
        spawns,
        populations: vec![PopulationParams::default()],
        project_seed: seed,
        ticks_per_second: 30,
        duration_ticks: 1800,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avoidance::SampledVelocitySolver;
    use crate::sim::{SimConfig, Simulation};

    #[test]
    fn every_named_scene_builds() {
        for name in SCENE_NAMES {
            assert!(build(name, 100, 42).is_some(), "{name} did not build");
        }
    }

    #[test]
    fn an_unknown_scene_name_returns_none() {
        assert!(build("no_such_scene", 100, 42).is_none());
    }

    #[test]
    fn every_named_scene_compiles_without_diagnostics() {
        for name in SCENE_NAMES {
            let scene = build(name, 100, 42).unwrap();
            if let Err(errors) = scene.compile() {
                panic!("{name} failed to compile: {errors:?}");
            }
        }
    }

    #[test]
    fn every_scene_spawns_the_requested_agent_count() {
        for name in SCENE_NAMES {
            let compiled = build(name, 200, 42).unwrap().compile().unwrap();
            assert_eq!(compiled.total_agents(), 200, "{name} miscounted");
        }
    }

    #[test]
    fn every_scene_runs_without_producing_nonfinite_state() {
        for name in SCENE_NAMES {
            let compiled = build(name, 100, 42).unwrap().compile().unwrap();
            let mut sim = Simulation::new(
                compiled,
                Box::new(SampledVelocitySolver::default()),
                SimConfig::default(),
            );
            sim.run(200);
            for slot in 0..sim.world().len() {
                assert!(
                    sim.world().position(slot as u32).is_finite(),
                    "{name} slot {slot} went non-finite"
                );
            }
        }
    }

    #[test]
    fn agents_reach_destinations_in_every_scene() {
        for name in SCENE_NAMES {
            let compiled = build(name, 100, 42).unwrap().compile().unwrap();
            let mut sim = Simulation::new(
                compiled,
                Box::new(SampledVelocitySolver::default()),
                SimConfig::default(),
            );
            sim.run_to_completion();
            assert!(
                sim.metrics().arrived() > 0,
                "{name}: nobody reached a destination"
            );
        }
    }

    #[test]
    fn the_bottleneck_scene_has_a_throughput_gate() {
        assert!(throughput_gate("bottleneck").is_some());
        assert!(throughput_gate("circle").is_none());
    }

    #[test]
    fn scene_agent_counts_split_evenly_across_spawn_regions() {
        // A count that does not divide evenly must still total exactly.
        let compiled = build("crossing", 101, 42).unwrap().compile().unwrap();
        assert_eq!(compiled.total_agents(), 101);
    }
}
