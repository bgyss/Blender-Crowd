//! The six benchmark scenes.
//!
//! Chosen to cover the failure modes contract section 6.2 names: lane
//! formation, perpendicular conflict, doorway congestion, dense convergence,
//! and the antipodal swap that is the cheapest known exposure of oscillation
//! and deadlock.

use crate::geometry::Segment;
use crate::route::WaypointGraph;
use crate::scene::{Destination, PopulationParams, SceneDef, SpawnRegion};
use crate::units::{Aabb, Vec2};

pub const SCENE_NAMES: [&str; 6] = [
    "bidirectional_corridor",
    "crossing",
    "bottleneck",
    "dense_flow",
    "circle",
    "l_corridor",
];

pub fn build(name: &str, agents: u32, seed: u64) -> Option<SceneDef> {
    let scale = population_scale(agents);
    let scene = match name {
        "bidirectional_corridor" => {
            scale_to_population(bidirectional_corridor(agents, seed), scale)
        }
        "crossing" => scale_to_population(crossing(agents, seed), scale),
        "circle" => scale_to_population(circle(agents, seed), scale),
        "l_corridor" => scale_to_population(l_corridor(agents, seed), scale),
        // These two build themselves at scale. Their constriction is a fixed
        // physical feature like the agents themselves, so it must not grow
        // with the population — see `population_scale`.
        "bottleneck" => bottleneck(agents, seed, scale),
        "dense_flow" => dense_flow(agents, seed, scale),
        _ => return None,
    };
    Some(scene)
}

/// Population these scenes are dimensioned for at scale 1.
const REFERENCE_POPULATION: f32 = 100.0;

/// Linear scale factor for a given population.
///
/// Area scales with population, so lengths scale with its square root.
///
/// Note what must NOT be scaled: agent radii, and any constriction whose whole
/// purpose is to be narrow relative to an agent. Scaling a 1.6 m doorway to
/// 5 m at 1,000 agents leaves the bottleneck scene without a bottleneck, and
/// its measurements no longer describe the same experiment as at 100 agents.
pub fn population_scale(agents: u32) -> f32 {
    (agents as f32 / REFERENCE_POPULATION).sqrt().max(1.0)
}

/// Grow scene geometry with the population so agent density stays constant.
///
/// Without this the benchmark measures the wrong thing. The scenes are sized
/// for a few hundred agents; running 1,000 through the same geometry puts the
/// corridor at 3.1 agents/m² — past the jamming threshold and heading for
/// crush density — so completion collapses and the report describes how
/// over-subscribed the scene was rather than how well the solver steers.
///
/// Area scales with population, so lengths scale with its square root. Agent
/// radii deliberately do not scale: the agents are the fixed physical thing,
/// and the world around them is what grows.
fn scale_to_population(mut scene: SceneDef, scale: f32) -> SceneDef {
    if scale <= 1.0 {
        return scene;
    }

    let point = |p: Vec2| p * scale;
    let area = |b: Aabb| Aabb::new(point(b.min), point(b.max));

    scene.bounds = area(scene.bounds);
    for wall in &mut scene.walls {
        *wall = Segment::new(point(wall.a), point(wall.b));
    }
    scene.waypoints = scene.waypoints.scaled(scale);
    for spawn in &mut scene.spawns {
        spawn.area = area(spawn.area);
        // Fill the larger scene in comparable time, or the run ends before
        // the population has finished entering.
        spawn.per_tick = ((spawn.per_tick as f32 * scale).round() as u32).max(1);
    }
    // Longer scene, proportionally longer to walk across.
    scene.duration_ticks = (scene.duration_ticks as f32 * scale).round() as u64;
    scene
}

/// The line crossings are counted against, for scenes where throughput is the
/// interesting measure.
/// Takes the agent count because scene geometry scales with population; a
/// gate at fixed coordinates would end up in the wrong place, or outside the
/// scene entirely.
pub fn throughput_gate(name: &str, agents: u32) -> Option<Segment> {
    let scale = population_scale(agents);
    // The gate spans the constriction, and constrictions are held fixed while
    // the room around them grows — so the gate's *position* scales but its
    // *extent* does not.
    match name {
        "bottleneck" => {
            let x = 20.0 * scale;
            let mid = 10.0 * scale;
            Some(Segment::new(
                Vec2::new(x, mid - 1.2),
                Vec2::new(x, mid + 1.2),
            ))
        }
        "dense_flow" => {
            let x = 28.0 * scale;
            let mid = 15.0 * scale;
            Some(Segment::new(
                Vec2::new(x, mid - 3.5),
                Vec2::new(x, mid + 3.5),
            ))
        }
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
        // Enclosed, like every other scene. Both destinations sit inside the
        // corridor, so agents never need to leave through an end -- and
        // without end caps, crowd pressure pushed agents up to 8 m past the
        // west destination and out of the scene entirely.
        walls: box_walls(bounds),
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
        nav: None,
        nav_destinations: Vec::new(),
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
        nav: None,
        nav_destinations: Vec::new(),
    }
}

/// One room emptying into another through a 1.6 m doorway.
fn bottleneck(agents: u32, seed: u64, scale: f32) -> SceneDef {
    /// Half-width of the doorway, in metres. Fixed: a doorway that widened
    /// with the population would stop being a bottleneck, and `bottleneck` at
    /// 1,000 agents would no longer be the same experiment as at 100.
    const DOOR_HALF_WIDTH: f32 = 0.8;

    let bounds = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(40.0 * scale, 20.0 * scale));
    let mid_y = 10.0 * scale;
    let divider_x = 20.0 * scale;

    let mut waypoints = WaypointGraph::new();
    let start = waypoints.add_node(Vec2::new(8.0 * scale, mid_y));
    let approach = waypoints.add_node(Vec2::new(divider_x - 1.5, mid_y));
    let through = waypoints.add_node(Vec2::new(divider_x + 1.5, mid_y));
    let exit = waypoints.add_node(Vec2::new(34.0 * scale, mid_y));
    waypoints.add_edge(start, approach);
    waypoints.add_edge(approach, through);
    waypoints.add_edge(through, exit);

    let mut walls = box_walls(bounds);
    walls.push(Segment::new(
        Vec2::new(divider_x, 0.0),
        Vec2::new(divider_x, mid_y - DOOR_HALF_WIDTH),
    ));
    walls.push(Segment::new(
        Vec2::new(divider_x, mid_y + DOOR_HALF_WIDTH),
        Vec2::new(divider_x, 20.0 * scale),
    ));

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
            area: Aabb::new(
                Vec2::new(1.0 * scale, 2.0 * scale),
                Vec2::new(14.0 * scale, 18.0 * scale),
            ),
            count: agents,
            per_tick: ((8.0 * scale).round() as u32).max(1),
            destination: 0,
        }],
        populations: vec![PopulationParams::default()],
        project_seed: seed,
        ticks_per_second: 30,
        duration_ticks: (3600.0 * scale).round() as u64,
        nav: None,
        nav_destinations: Vec::new(),
    }
}

/// A dense blob converging on a single exit corridor.
fn dense_flow(agents: u32, seed: u64, scale: f32) -> SceneDef {
    /// Half-width of the funnel mouth, in metres. Fixed, for the same reason
    /// as the bottleneck's doorway.
    const MOUTH_HALF_WIDTH: f32 = 3.0;

    let bounds = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(40.0 * scale, 30.0 * scale));
    let mid_y = 15.0 * scale;
    let throat_x = 28.0 * scale;

    let mut waypoints = WaypointGraph::new();
    let start = waypoints.add_node(Vec2::new(12.0 * scale, mid_y));
    let mouth = waypoints.add_node(Vec2::new(26.0 * scale, mid_y));
    let exit = waypoints.add_node(Vec2::new(38.0 * scale, mid_y));
    waypoints.add_edge(start, mouth);
    waypoints.add_edge(mouth, exit);

    let mut walls = box_walls(bounds);
    let low = mid_y - MOUTH_HALF_WIDTH;
    let high = mid_y + MOUTH_HALF_WIDTH;
    walls.push(Segment::new(
        Vec2::new(24.0 * scale, 0.0),
        Vec2::new(throat_x, low),
    ));
    walls.push(Segment::new(
        Vec2::new(24.0 * scale, 30.0 * scale),
        Vec2::new(throat_x, high),
    ));
    walls.push(Segment::new(
        Vec2::new(throat_x, low),
        Vec2::new(40.0 * scale, low),
    ));
    walls.push(Segment::new(
        Vec2::new(throat_x, high),
        Vec2::new(40.0 * scale, high),
    ));

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
            area: Aabb::new(
                Vec2::new(1.0 * scale, 2.0 * scale),
                Vec2::new(18.0 * scale, 28.0 * scale),
            ),
            count: agents,
            per_tick: ((12.0 * scale).round() as u32).max(1),
            destination: 0,
        }],
        populations: vec![PopulationParams::default()],
        project_seed: seed,
        ticks_per_second: 30,
        duration_ticks: (3600.0 * scale).round() as u64,
        nav: None,
        nav_destinations: Vec::new(),
    }
}

/// A corridor with a right-angle turn around a solid block.
///
/// Every other scene routes its population along a straight line, so the
/// routing layer's corner handling — leg consumption, and steering that
/// converges onto a corridor without cutting across its inside edge — is
/// exercised only by unit tests. Here a crowd has to actually round a corner
/// that a wall makes it impossible to cut.
fn l_corridor(agents: u32, seed: u64) -> SceneDef {
    let bounds = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(40.0, 40.0));

    let mut waypoints = WaypointGraph::new();
    let start = waypoints.add_node(Vec2::new(4.0, 4.0));
    let corner = waypoints.add_node(Vec2::new(4.0, 34.0));
    let exit = waypoints.add_node(Vec2::new(36.0, 34.0));
    waypoints.add_edge(start, corner);
    waypoints.add_edge(corner, exit);

    let mut walls = box_walls(bounds);
    // The inside block. Its corner at (10, 28) is what the route has to go
    // around: an agent that cuts the turn walks straight into it.
    walls.push(Segment::new(Vec2::new(10.0, 0.0), Vec2::new(10.0, 28.0)));
    walls.push(Segment::new(Vec2::new(10.0, 28.0), Vec2::new(40.0, 28.0)));

    SceneDef {
        name: "l_corridor".into(),
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
            area: Aabb::new(Vec2::new(0.5, 0.5), Vec2::new(9.5, 10.0)),
            count: agents,
            per_tick: 4,
            destination: 0,
        }],
        populations: vec![PopulationParams::default()],
        project_seed: seed,
        ticks_per_second: 30,
        duration_ticks: 2400,
        nav: None,
        nav_destinations: Vec::new(),
    }
}

/// Agents on a ring walking to the antipodal point.
///
/// Everyone converges on the centre simultaneously with perfect symmetry,
/// which is why this is the standard oscillation and deadlock test.
// Authored as literal f32 points rather than evaluated with sin/cos.
// LLVM can reassociate the angle arithmetic in optimized builds, producing a
// one-bit coordinate difference and therefore a different scene identity for
// the same source input in debug and release profiles.
// The bits are the previously measured release geometry, so making the
// coordinates deterministic does not redefine the accepted benchmark.
const CIRCLE_POINTS: [Vec2; 16] = [
    Vec2::new(f32::from_bits(0x4170_0000), f32::from_bits(0x0000_0000)),
    Vec2::new(f32::from_bits(0x415d_bb28), f32::from_bits(0x40b7_b025)),
    Vec2::new(f32::from_bits(0x4129_b4a4), f32::from_bits(0x4129_b4a4)),
    Vec2::new(f32::from_bits(0x40b7_b024), f32::from_bits(0x415d_bb28)),
    Vec2::new(f32::from_bits(0xb530_015b), f32::from_bits(0x4170_0000)),
    Vec2::new(f32::from_bits(0xc0b7_b026), f32::from_bits(0x415d_bb28)),
    Vec2::new(f32::from_bits(0xc129_b4a4), f32::from_bits(0x4129_b4a4)),
    Vec2::new(f32::from_bits(0xc15d_bb2a), f32::from_bits(0x40b7_b01f)),
    Vec2::new(f32::from_bits(0xc170_0000), f32::from_bits(0xb5b0_015b)),
    Vec2::new(f32::from_bits(0xc15d_bb28), f32::from_bits(0xc0b7_b024)),
    Vec2::new(f32::from_bits(0xc129_b4a2), f32::from_bits(0xc129_b4a6)),
    Vec2::new(f32::from_bits(0xc0b7_b01a), f32::from_bits(0xc15d_bb2b)),
    Vec2::new(f32::from_bits(0x3440_104b), f32::from_bits(0xc170_0000)),
    Vec2::new(f32::from_bits(0x40b7_b029), f32::from_bits(0xc15d_bb27)),
    Vec2::new(f32::from_bits(0x4129_b4a8), f32::from_bits(0xc129_b4a0)),
    Vec2::new(f32::from_bits(0x415d_bb29), f32::from_bits(0xc0b7_b024)),
];

fn circle(agents: u32, seed: u64) -> SceneDef {
    const SECTORS: u32 = CIRCLE_POINTS.len() as u32;
    let bounds = Aabb::new(Vec2::new(-20.0, -20.0), Vec2::new(20.0, 20.0));

    let mut waypoints = WaypointGraph::new();
    let centre = waypoints.add_node(Vec2::ZERO);
    let mut perimeter = Vec::new();
    for point in CIRCLE_POINTS {
        let node = waypoints.add_node(point);
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
            let centre_point = CIRCLE_POINTS[sector as usize];
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
        nav: None,
        nav_destinations: Vec::new(),
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
    fn circle_geometry_uses_profile_independent_authored_directions() {
        let expected = [
            [0x4170_0000, 0x0000_0000],
            [0x415d_bb28, 0x40b7_b025],
            [0x4129_b4a4, 0x4129_b4a4],
            [0x40b7_b024, 0x415d_bb28],
            [0xb530_015b, 0x4170_0000],
            [0xc0b7_b026, 0x415d_bb28],
            [0xc129_b4a4, 0x4129_b4a4],
            [0xc15d_bb2a, 0x40b7_b01f],
            [0xc170_0000, 0xb5b0_015b],
            [0xc15d_bb28, 0xc0b7_b024],
            [0xc129_b4a2, 0xc129_b4a6],
            [0xc0b7_b01a, 0xc15d_bb2b],
            [0x3440_104b, 0xc170_0000],
            [0x40b7_b029, 0xc15d_bb27],
            [0x4129_b4a8, 0xc129_b4a0],
            [0x415d_bb29, 0xc0b7_b024],
        ];
        let scene = circle(16, 2026);

        for (sector, expected_bits) in expected.into_iter().enumerate() {
            let actual = scene.waypoints.position(sector as u32 + 1);
            assert_eq!(
                [actual.x.to_bits(), actual.y.to_bits()],
                expected_bits,
                "sector {sector} changed with floating-point evaluation strategy"
            );
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
    fn constrictions_do_not_widen_with_the_population() {
        // The doorway and the funnel mouth are fixed physical features, like
        // the agents. If they scaled with the room, `bottleneck` at 1,000
        // agents would have a 5 m "doorway" and would no longer be measuring
        // the same thing as at 100.
        for (scene, expected_gap) in [("bottleneck", 1.6f32), ("dense_flow", 6.0f32)] {
            let small = gap_width(scene, 100);
            let large = gap_width(scene, 1000);
            assert!(
                (small - expected_gap).abs() < 0.05,
                "{scene} gap at 100 agents was {small}, expected {expected_gap}"
            );
            assert!(
                (large - expected_gap).abs() < 0.05,
                "{scene} gap widened to {large} at 1000 agents"
            );
        }
    }

    /// Width of the narrowest vertical gap between wall endpoints, which for
    /// these two scenes is the constriction.
    fn gap_width(scene: &str, agents: u32) -> f32 {
        let def = build(scene, agents, 1).unwrap();
        let bounds = def.bounds;
        // Sample a vertical line through the constriction and find the widest
        // clear span, which is the gap agents must pass through.
        let x = match scene {
            "bottleneck" => 20.0 * population_scale(agents),
            _ => 28.0 * population_scale(agents),
        };
        let mut blocked: Vec<(f32, f32)> = Vec::new();
        for wall in &def.walls {
            let (lo, hi) = (wall.a.x.min(wall.b.x), wall.a.x.max(wall.b.x));
            if lo - 1e-3 <= x && x <= hi + 1e-3 {
                blocked.push((wall.a.y.min(wall.b.y), wall.a.y.max(wall.b.y)));
            }
        }
        blocked.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut widest = 0.0f32;
        let mut cursor = bounds.min.y;
        for (lo, hi) in blocked {
            if lo > cursor {
                widest = widest.max(lo - cursor);
            }
            cursor = cursor.max(hi);
        }
        widest
    }

    #[test]
    fn agents_actually_cross_the_throughput_gate() {
        // The gate read zero crossings while 37% of the population walked
        // through the doorway, because the direction test was inverted and
        // nothing checked the metric against reality.
        for scene in ["bottleneck", "dense_flow"] {
            let compiled = build(scene, 100, 42).unwrap().compile().unwrap();
            let config = SimConfig {
                metrics: crate::metrics::MetricsConfig {
                    throughput_gate: throughput_gate(scene, 100),
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut sim =
                Simulation::new(compiled, Box::new(SampledVelocitySolver::default()), config);
            sim.run_to_completion();
            let crossings = sim.metrics().gate_crossings();
            let arrived = sim.metrics().arrived();
            assert!(
                crossings > 0,
                "{scene}: {arrived} agents arrived but the gate counted none"
            );
            // Everyone who arrived had to pass through the constriction, and
            // only forward crossings count, so the two should be comparable.
            assert!(
                crossings >= arrived / 2,
                "{scene}: {crossings} crossings against {arrived} arrivals"
            );
        }
    }

    #[test]
    fn the_bottleneck_scene_has_a_throughput_gate() {
        assert!(throughput_gate("bottleneck", 100).is_some());
        assert!(throughput_gate("circle", 100).is_none());

        // The gate must track the scene as it grows with the population,
        // or it ends up in the wrong place — or outside the scene entirely.
        let small = throughput_gate("bottleneck", 100).unwrap();
        let large = throughput_gate("bottleneck", 1000).unwrap();
        assert!(large.a.x > small.a.x, "gate did not scale with the scene");
    }

    #[test]
    fn scene_agent_counts_split_evenly_across_spawn_regions() {
        // A count that does not divide evenly must still total exactly.
        let compiled = build("crossing", 101, 42).unwrap().compile().unwrap();
        assert_eq!(compiled.total_agents(), 101);
    }
}
