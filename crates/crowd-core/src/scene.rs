//! Scene authoring input and its compiled, validated form.
//!
//! Compilation is where contract section 10.3's error model lives: every
//! diagnostic names the offending entity and every independent fault is
//! reported in one pass, so a user is not forced to fix problems one at a
//! time.

use crate::geometry::Segment;
use crate::grid::SegmentIndex;
use crate::ids::{hash_combine, hash_str, mix64};
use crate::nav::{NavMeshDef, TileGraph};
use crate::route::WaypointGraph;
use crate::units::{Aabb, Vec2};

/// A named goal region, anchored to a waypoint node.
#[derive(Clone, Debug)]
pub struct Destination {
    pub name: String,
    pub node: u32,
}

/// Where and how fast agents enter the scene.
#[derive(Clone, Copy, Debug)]
pub struct SpawnRegion {
    pub id: u16,
    pub population_id: u16,
    pub area: Aabb,
    /// Total agents this region will ever emit.
    pub count: u32,
    /// Agents emitted per tick until `count` is exhausted.
    pub per_tick: u32,
    /// Index into `SceneDef::destinations`.
    pub destination: u16,
}

/// Floor on an agent's preferred speed, in metres per second.
///
/// The spawn phase clamps sampled speeds to this floor, and compilation
/// rejects any population whose mean sits below it. Both use this one constant
/// deliberately: if the clamp's floor could exceed its ceiling, `f32::clamp`
/// panics — in release as well as debug — and the tick loop is required to be
/// infallible.
pub const MIN_PREFERRED_SPEED: f32 = 0.4;

/// Distributions an agent's varied attributes are drawn from.
#[derive(Clone, Copy, Debug)]
pub struct PopulationParams {
    pub radius_min: f32,
    pub radius_max: f32,
    pub speed_mean: f32,
    pub speed_stddev: f32,
    /// Maximum speed as a multiple of the agent's preferred speed.
    pub max_speed_factor: f32,
}

impl Default for PopulationParams {
    /// Pedestrian defaults from contract section 4.2.
    fn default() -> Self {
        Self {
            radius_min: 0.24,
            radius_max: 0.38,
            speed_mean: 1.35,
            speed_stddev: 0.18,
            max_speed_factor: 1.5,
        }
    }
}

/// Authoring input for one benchmark scene.
#[derive(Clone, Debug)]
pub struct SceneDef {
    pub name: String,
    pub bounds: Aabb,
    pub walls: Vec<Segment>,
    pub waypoints: WaypointGraph,
    pub destinations: Vec<Destination>,
    pub spawns: Vec<SpawnRegion>,
    pub populations: Vec<PopulationParams>,
    pub project_seed: u64,
    pub ticks_per_second: u32,
    pub duration_ticks: u64,
    /// `None` for waypoint-routed scenes (every scene today). `Some` opts the
    /// scene into the tiled navmesh instead — `waypoints` is then unused and
    /// may be left as `WaypointGraph::new()`.
    pub nav: Option<NavMeshDef>,
    /// Parallel to `destinations`; meaningful only when `nav.is_some()`.
    pub nav_destinations: Vec<Vec2>,
}

/// A bake-blocking authoring fault. Each names the entity at fault.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneError {
    EmptyWaypointGraph,
    DisconnectedWaypointGraph,
    NoDestinations,
    NoSpawns,
    SpawnOutsideBounds {
        spawn: u16,
    },
    UnknownDestination {
        spawn: u16,
        destination: u16,
    },
    UnknownPopulation {
        spawn: u16,
        population: u16,
    },
    DestinationNodeMissing {
        destination: u16,
        node: u32,
    },
    UnreachableDestination {
        spawn: u16,
        destination: u16,
    },
    InvalidTickRate {
        ticks_per_second: u32,
    },
    InvalidPopulation {
        population: u16,
        field: &'static str,
    },
    EmptyNavMesh,
    UnwalkableDestination {
        destination: u16,
    },
    UnknownNamedPortal {
        name: String,
    },
    /// A* heuristic admissibility depends on every cost multiplier being
    /// `>= 1.0` (see `nav::pathfind::find_path`'s doc comment); this rejects
    /// the scene before an inadmissible heuristic can silently break search
    /// optimality.
    InvalidCostArea {
        area: usize,
    },
}

/// A validated scene, ready to simulate.
#[derive(Clone, Debug)]
pub struct CompiledScene {
    pub name: String,
    pub bounds: Aabb,
    pub walls: Vec<Segment>,
    pub wall_index: SegmentIndex,
    pub waypoints: WaypointGraph,
    pub destinations: Vec<Destination>,
    pub spawns: Vec<SpawnRegion>,
    pub populations: Vec<PopulationParams>,
    pub project_seed: u64,
    pub ticks_per_second: u32,
    pub duration_ticks: u64,
    pub nav: Option<TileGraph>,
    pub nav_destinations: Vec<Vec2>,
    scene_hash: u64,
}

/// Wall index cell size. Chosen as a few agent diameters: small enough that a
/// query touches few cells, large enough that long walls do not fan out.
const WALL_CELL_SIZE: f32 = 2.0;

impl SceneDef {
    /// Validate and compile, reporting every independent fault at once.
    pub fn compile(self) -> Result<CompiledScene, Vec<SceneError>> {
        let mut errors = Vec::new();

        if self.ticks_per_second == 0 {
            errors.push(SceneError::InvalidTickRate {
                ticks_per_second: self.ticks_per_second,
            });
        }
        if self.nav.is_none() {
            if self.waypoints.node_count() == 0 {
                errors.push(SceneError::EmptyWaypointGraph);
            } else if !self.waypoints.is_connected() {
                errors.push(SceneError::DisconnectedWaypointGraph);
            }
        }
        if self.destinations.is_empty() {
            errors.push(SceneError::NoDestinations);
        }
        if self.spawns.is_empty() {
            errors.push(SceneError::NoSpawns);
        }

        // Populations are validated here rather than defended against in the
        // spawn phase. A nonsensical distribution is an authoring fault, and
        // catching it at the boundary keeps the tick loop free of guards.
        for (index, population) in self.populations.iter().enumerate() {
            let index = index as u16;
            let mut reject = |field: &'static str| {
                errors.push(SceneError::InvalidPopulation {
                    population: index,
                    field,
                })
            };
            // NaN is tested explicitly rather than relying on a negated
            // comparison. Both forms reject it, but `is_nan()` says so out
            // loud, and clippy rejects the negated form under `-D warnings`.
            if population.radius_min.is_nan() || population.radius_min <= 0.0 {
                reject("radius_min");
            }
            if population.radius_max.is_nan() || population.radius_max < population.radius_min {
                reject("radius_max");
            }
            // Below this floor the spawn clamp's minimum would exceed its
            // maximum, and `f32::clamp` panics on inverted bounds.
            if population.speed_mean.is_nan() || population.speed_mean < MIN_PREFERRED_SPEED {
                reject("speed_mean");
            }
            if population.speed_stddev.is_nan() || population.speed_stddev < 0.0 {
                reject("speed_stddev");
            }
            if population.max_speed_factor.is_nan() || population.max_speed_factor < 1.0 {
                reject("max_speed_factor");
            }
        }

        if self.nav.is_none() {
            for (index, destination) in self.destinations.iter().enumerate() {
                if destination.node >= self.waypoints.node_count() {
                    errors.push(SceneError::DestinationNodeMissing {
                        destination: index as u16,
                        node: destination.node,
                    });
                }
            }
        }

        if let Some(nav_def) = &self.nav {
            // Admissibility of the A* heuristic (Euclidean distance to goal)
            // requires every edge's cost multiplier to be >= 1.0; a multiplier
            // below that would let the heuristic overestimate the true cost
            // and silently break search optimality.
            for (index, (_, multiplier)) in nav_def.cost_areas.iter().enumerate() {
                if multiplier.is_nan() || *multiplier < 1.0 {
                    errors.push(SceneError::InvalidCostArea { area: index });
                }
            }
        }

        let nav_graph = self
            .nav
            .as_ref()
            .map(|def| def.build_graph(self.bounds, &self.walls));
        if let Some(graph) = &nav_graph {
            if graph.grid().tile_count() == 0
                || (0..graph.grid().tile_count()).all(|t| !graph.grid().is_walkable(t))
            {
                errors.push(SceneError::EmptyNavMesh);
            }
            for (index, point) in self.nav_destinations.iter().enumerate() {
                if graph.grid().nearest_walkable_tile(*point).is_none() {
                    errors.push(SceneError::UnwalkableDestination {
                        destination: index as u16,
                    });
                }
            }
            if let Some(nav_def) = &self.nav {
                for (name, _, _) in &nav_def.named_portals {
                    if graph.portals_named(name).is_empty() {
                        errors.push(SceneError::UnknownNamedPortal { name: name.clone() });
                    }
                }
            }
        }

        for spawn in &self.spawns {
            if !self.bounds.contains(spawn.area.min) || !self.bounds.contains(spawn.area.max) {
                errors.push(SceneError::SpawnOutsideBounds { spawn: spawn.id });
            }
            if spawn.population_id as usize >= self.populations.len() {
                errors.push(SceneError::UnknownPopulation {
                    spawn: spawn.id,
                    population: spawn.population_id,
                });
            }
            let Some(destination) = self.destinations.get(spawn.destination as usize) else {
                errors.push(SceneError::UnknownDestination {
                    spawn: spawn.id,
                    destination: spawn.destination,
                });
                continue;
            };
            // A nav-routed scene's real destination points live in the
            // parallel `nav_destinations` vec, not in `destinations` (which
            // only carries display names for it). A short `nav_destinations`
            // would otherwise make `spawn.rs` silently fall through to the
            // (unused, for a nav scene) waypoint branch and `plan()` silently
            // skip the agent forever with no diagnostic.
            if nav_graph.is_some()
                && self
                    .nav_destinations
                    .get(spawn.destination as usize)
                    .is_none()
            {
                errors.push(SceneError::UnknownDestination {
                    spawn: spawn.id,
                    destination: spawn.destination,
                });
                continue;
            }
            // Reachability is only meaningful once routing itself is sound.
            if let Some(graph) = &nav_graph {
                if let Some(dest_point) = self.nav_destinations.get(spawn.destination as usize) {
                    let from = graph.grid().nearest_walkable_tile(spawn.area.center());
                    let to = graph.grid().nearest_walkable_tile(*dest_point);
                    let reachable = match (from, to) {
                        (Some(from), Some(to)) => {
                            crate::nav::find_path(graph, from, to).0.is_some()
                        }
                        _ => false,
                    };
                    if !reachable {
                        errors.push(SceneError::UnreachableDestination {
                            spawn: spawn.id,
                            destination: spawn.destination,
                        });
                    }
                }
            } else if self.waypoints.node_count() > 0
                && destination.node < self.waypoints.node_count()
            {
                let from = self
                    .waypoints
                    .nearest_node(spawn.area.center())
                    .expect("non-empty graph has a nearest node");
                if self
                    .waypoints
                    .shortest_path(from, destination.node)
                    .is_none()
                {
                    errors.push(SceneError::UnreachableDestination {
                        spawn: spawn.id,
                        destination: spawn.destination,
                    });
                }
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        let scene_hash = compute_scene_hash(&self);
        let wall_index = SegmentIndex::build(
            self.bounds.expanded(WALL_CELL_SIZE),
            WALL_CELL_SIZE,
            &self.walls,
        );

        Ok(CompiledScene {
            name: self.name,
            bounds: self.bounds,
            walls: self.walls,
            wall_index,
            waypoints: self.waypoints,
            destinations: self.destinations,
            spawns: self.spawns,
            populations: self.populations,
            project_seed: self.project_seed,
            ticks_per_second: self.ticks_per_second,
            duration_ticks: self.duration_ticks,
            nav: nav_graph,
            nav_destinations: self.nav_destinations,
            scene_hash,
        })
    }
}

/// Content hash over everything that affects simulation output.
///
/// Reports carry this so a metrics comparison against a baseline generated
/// from a different scene is detectable rather than silently misleading.
fn compute_scene_hash(scene: &SceneDef) -> u64 {
    let mut h = hash_str(&scene.name);
    h = hash_combine(h, scene.project_seed);
    h = hash_combine(h, scene.ticks_per_second as u64);
    h = hash_combine(h, scene.duration_ticks);
    for value in [
        scene.bounds.min.x,
        scene.bounds.min.y,
        scene.bounds.max.x,
        scene.bounds.max.y,
    ] {
        h = hash_combine(h, value.to_bits() as u64);
    }
    // Each collection folds in its length first. Without it, a differently
    // shaped sequence of the same values could in principle mix to the same
    // state — cheap to close, and this hash gates baseline compatibility.
    h = hash_combine(h, scene.walls.len() as u64);
    for wall in &scene.walls {
        for value in [wall.a.x, wall.a.y, wall.b.x, wall.b.y] {
            h = hash_combine(h, value.to_bits() as u64);
        }
    }
    h = hash_combine(h, scene.waypoints.node_count() as u64);
    for node in 0..scene.waypoints.node_count() {
        let p = scene.waypoints.position(node);
        h = hash_combine(h, p.x.to_bits() as u64);
        h = hash_combine(h, p.y.to_bits() as u64);
        // Topology, not just geometry. Routing follows the edges, so a rewired
        // graph with unmoved nodes simulates differently and must not share a
        // hash with the original.
        let neighbors = scene.waypoints.neighbors(node);
        h = hash_combine(h, neighbors.len() as u64);
        for neighbor in neighbors {
            h = hash_combine(h, *neighbor as u64);
        }
    }
    h = hash_combine(h, scene.destinations.len() as u64);
    for destination in &scene.destinations {
        h = hash_combine(h, hash_str(&destination.name));
        h = hash_combine(h, destination.node as u64);
    }
    h = hash_combine(h, scene.spawns.len() as u64);
    for spawn in &scene.spawns {
        h = hash_combine(h, spawn.id as u64);
        h = hash_combine(h, spawn.population_id as u64);
        h = hash_combine(h, spawn.count as u64);
        h = hash_combine(h, spawn.per_tick as u64);
        h = hash_combine(h, spawn.destination as u64);
        for value in [
            spawn.area.min.x,
            spawn.area.min.y,
            spawn.area.max.x,
            spawn.area.max.y,
        ] {
            h = hash_combine(h, value.to_bits() as u64);
        }
    }
    for population in &scene.populations {
        for value in [
            population.radius_min,
            population.radius_max,
            population.speed_mean,
            population.speed_stddev,
            population.max_speed_factor,
        ] {
            h = hash_combine(h, value.to_bits() as u64);
        }
    }
    // `nav` opts a scene into a whole different routing path (tiled navmesh
    // instead of the waypoint graph already folded in above), so its content
    // must gate baseline compatibility exactly like everything else here. A
    // presence marker distinguishes `None` from `Some` of an
    // otherwise-all-zero `NavMeshDef`.
    match &scene.nav {
        Some(nav) => {
            h = hash_combine(h, 1u64);
            h = hash_combine(h, nav.tile_size.to_bits() as u64);
            h = hash_combine(h, nav.agent_radius.to_bits() as u64);
            h = hash_combine(h, nav.cost_areas.len() as u64);
            for (area, multiplier) in &nav.cost_areas {
                for value in [area.min.x, area.min.y, area.max.x, area.max.y, *multiplier] {
                    h = hash_combine(h, value.to_bits() as u64);
                }
            }
            h = hash_combine(h, nav.named_portals.len() as u64);
            for (name, point, axis) in &nav.named_portals {
                h = hash_combine(h, hash_str(name));
                h = hash_combine(h, point.x.to_bits() as u64);
                h = hash_combine(h, point.y.to_bits() as u64);
                h = hash_combine(
                    h,
                    match axis {
                        crate::nav::CrossingAxis::EastWest => 0u64,
                        crate::nav::CrossingAxis::NorthSouth => 1u64,
                    },
                );
            }
        }
        None => h = hash_combine(h, 0u64),
    }
    h = hash_combine(h, scene.nav_destinations.len() as u64);
    for point in &scene.nav_destinations {
        h = hash_combine(h, point.x.to_bits() as u64);
        h = hash_combine(h, point.y.to_bits() as u64);
    }
    mix64(h)
}

impl CompiledScene {
    pub fn scene_hash(&self) -> u64 {
        self.scene_hash
    }

    pub fn total_agents(&self) -> u32 {
        self.spawns.iter().map(|s| s.count).sum()
    }

    /// `None` for an unknown destination index.
    ///
    /// Every internally-produced index is validated at compile time, but a
    /// caller iterating the wrong bound would otherwise panic mid-bake — and
    /// the error model says diagnose, never crash.
    pub fn destination_position(&self, destination: u16) -> Option<Vec2> {
        self.destinations
            .get(destination as usize)
            .map(|d| self.waypoints.position(d.node))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Vec2;

    /// A corridor with two nodes, one spawn at the left, one exit at the right.
    fn valid_scene() -> SceneDef {
        let mut waypoints = WaypointGraph::new();
        let a = waypoints.add_node(Vec2::new(1.0, 5.0));
        let b = waypoints.add_node(Vec2::new(9.0, 5.0));
        waypoints.add_edge(a, b);

        SceneDef {
            name: "corridor".into(),
            bounds: Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)),
            walls: vec![
                Segment::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)),
                Segment::new(Vec2::new(0.0, 10.0), Vec2::new(10.0, 10.0)),
            ],
            waypoints,
            destinations: vec![Destination {
                name: "exit".into(),
                node: b,
            }],
            spawns: vec![SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(0.5, 4.0), Vec2::new(1.5, 6.0)),
                count: 10,
                per_tick: 2,
                destination: 0,
            }],
            populations: vec![PopulationParams::default()],
            project_seed: 42,
            ticks_per_second: 30,
            duration_ticks: 300,
            nav: None,
            nav_destinations: Vec::new(),
        }
    }

    use crate::nav::NavMeshDef;

    fn nav_scene() -> SceneDef {
        SceneDef {
            name: "nav_corridor".into(),
            bounds: Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 3.0)),
            walls: vec![
                Segment::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)),
                Segment::new(Vec2::new(0.0, 3.0), Vec2::new(10.0, 3.0)),
            ],
            waypoints: WaypointGraph::new(),
            destinations: vec![Destination {
                name: "exit".into(),
                node: 0,
            }],
            spawns: vec![SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(0.5, 1.0), Vec2::new(1.5, 2.0)),
                count: 10,
                per_tick: 2,
                destination: 0,
            }],
            populations: vec![PopulationParams::default()],
            project_seed: 42,
            ticks_per_second: 30,
            duration_ticks: 300,
            nav: Some(NavMeshDef {
                tile_size: 1.0,
                agent_radius: 0.3,
                cost_areas: Vec::new(),
                named_portals: Vec::new(),
            }),
            nav_destinations: vec![Vec2::new(9.0, 1.5)],
        }
    }

    #[test]
    fn a_nav_routed_scene_compiles_without_a_waypoint_graph() {
        assert!(nav_scene().compile().is_ok());
    }

    #[test]
    fn an_unwalkable_nav_destination_is_rejected() {
        let mut scene = nav_scene();
        // Outside the corridor walls entirely.
        scene.nav_destinations[0] = Vec2::new(9.0, 50.0);
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::UnwalkableDestination { destination: 0 }));
    }

    #[test]
    fn an_unreachable_nav_destination_is_rejected() {
        let mut scene = nav_scene();
        // A wall straight across the corridor, with no doorway. Runs through
        // the centers of the tile column at x=5.5 (tile_size is 1.0, origin
        // 0.0) so every tile in that column is guaranteed blocked regardless
        // of how rasterization treats a wall sitting exactly on a tile edge.
        scene
            .walls
            .push(Segment::new(Vec2::new(5.5, 0.0), Vec2::new(5.5, 3.0)));
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::UnreachableDestination {
            spawn: 0,
            destination: 0
        }));
    }

    #[test]
    fn a_named_door_with_no_crossing_portal_is_rejected() {
        let mut scene = nav_scene();
        scene.nav.as_mut().unwrap().named_portals = vec![(
            "nowhere".to_string(),
            // Far outside the 10x3 corridor entirely, so no portal ever
            // straddles this point.
            Vec2::new(500.0, 500.0),
            crate::nav::CrossingAxis::EastWest,
        )];
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::UnknownNamedPortal {
            name: "nowhere".to_string()
        }));
    }

    #[test]
    fn a_cost_multiplier_below_one_is_rejected() {
        let mut scene = nav_scene();
        scene.nav.as_mut().unwrap().cost_areas = vec![(
            Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(2.0, 3.0)),
            0.5, // < 1.0 breaks A* heuristic admissibility.
        )];
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::InvalidCostArea { area: 0 }));
    }

    #[test]
    fn a_spawn_destination_index_missing_from_nav_destinations_is_rejected() {
        let mut scene = nav_scene();
        // `destinations` still has one entry, but `nav_destinations` (the
        // vec that actually carries the routed point) is emptied out from
        // under it.
        scene.nav_destinations.clear();
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::UnknownDestination {
            spawn: 0,
            destination: 0
        }));
    }

    #[test]
    fn scene_hash_changes_when_nav_tile_size_changes() {
        let a = nav_scene().compile().unwrap();
        let mut scene = nav_scene();
        scene.nav.as_mut().unwrap().tile_size = 0.5;
        let b = scene.compile().unwrap();
        assert_ne!(a.scene_hash(), b.scene_hash());
    }

    #[test]
    fn scene_hash_changes_when_nav_destinations_change() {
        let a = nav_scene().compile().unwrap();
        let mut scene = nav_scene();
        scene.nav_destinations[0] = Vec2::new(8.0, 1.5);
        let b = scene.compile().unwrap();
        assert_ne!(a.scene_hash(), b.scene_hash());
    }

    #[test]
    fn scene_hash_differs_between_nav_routed_and_waypoint_routed_scenes() {
        // `nav: Some(...)` vs `nav: None` must not collide even when every
        // other hashed field happens to match.
        let waypoint = valid_scene().compile().unwrap();
        let mut nav = valid_scene();
        nav.nav = Some(NavMeshDef {
            tile_size: 1.0,
            agent_radius: 0.3,
            cost_areas: Vec::new(),
            named_portals: Vec::new(),
        });
        // A nav-routed scene needs its own destination point; `valid_scene`'s
        // `nav_destinations` is empty since it is authored as waypoint-only.
        nav.nav_destinations = vec![Vec2::new(9.0, 5.0)];
        let nav = nav.compile().unwrap();
        assert_ne!(waypoint.scene_hash(), nav.scene_hash());
    }

    #[test]
    fn a_nav_scene_does_not_require_a_waypoint_graph() {
        // The waypoint-only checks must not fire for a nav-routed scene, even
        // though its `waypoints` field is the empty default.
        let scene = nav_scene();
        assert!(scene.waypoints.node_count() == 0);
        assert!(scene.compile().is_ok());
    }

    #[test]
    fn a_waypoint_scene_is_unaffected_by_the_nav_field_existing() {
        // `valid_scene()` (the pre-existing helper) has `nav: None` and must
        // compile exactly as before.
        assert!(valid_scene().compile().is_ok());
    }

    #[test]
    fn a_valid_scene_compiles() {
        assert!(valid_scene().compile().is_ok());
    }

    #[test]
    fn compiled_scene_reports_total_agent_count() {
        let compiled = valid_scene().compile().unwrap();
        assert_eq!(compiled.total_agents(), 10);
    }

    #[test]
    fn spawn_outside_bounds_is_rejected() {
        let mut scene = valid_scene();
        scene.spawns[0].area = Aabb::new(Vec2::new(-50.0, -50.0), Vec2::new(-40.0, -40.0));
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::SpawnOutsideBounds { spawn: 0 }));
    }

    #[test]
    fn unknown_destination_reference_is_rejected() {
        let mut scene = valid_scene();
        scene.spawns[0].destination = 7;
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::UnknownDestination {
            spawn: 0,
            destination: 7
        }));
    }

    #[test]
    fn disconnected_waypoint_graph_is_rejected() {
        let mut scene = valid_scene();
        scene.waypoints.add_node(Vec2::new(5.0, 9.0));
        let errors = scene.compile().unwrap_err();
        assert!(matches!(
            errors.as_slice(),
            [SceneError::DisconnectedWaypointGraph]
        ));
    }

    #[test]
    fn empty_waypoint_graph_is_rejected() {
        let mut scene = valid_scene();
        scene.waypoints = WaypointGraph::new();
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::EmptyWaypointGraph));
    }

    #[test]
    fn destination_node_outside_the_graph_is_rejected() {
        let mut scene = valid_scene();
        scene.destinations[0].node = 99;
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::DestinationNodeMissing {
            destination: 0,
            node: 99
        }));
    }

    #[test]
    fn missing_population_reference_is_rejected() {
        let mut scene = valid_scene();
        scene.spawns[0].population_id = 5;
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::UnknownPopulation {
            spawn: 0,
            population: 5
        }));
    }

    #[test]
    fn all_independent_errors_are_reported_together() {
        // A user fixing one problem at a time is a bad experience; the
        // compiler reports every independent fault in one pass.
        let mut scene = valid_scene();
        scene.spawns[0].destination = 7;
        scene.spawns[0].population_id = 5;
        let errors = scene.compile().unwrap_err();
        assert!(errors.len() >= 2, "got {errors:?}");
    }

    #[test]
    fn scene_hash_is_stable_for_identical_input() {
        let a = valid_scene().compile().unwrap();
        let b = valid_scene().compile().unwrap();
        assert_eq!(a.scene_hash(), b.scene_hash());
    }

    #[test]
    fn scene_hash_changes_when_geometry_changes() {
        let a = valid_scene().compile().unwrap();
        let mut scene = valid_scene();
        scene
            .walls
            .push(Segment::new(Vec2::new(5.0, 0.0), Vec2::new(5.0, 4.0)));
        let b = scene.compile().unwrap();
        assert_ne!(a.scene_hash(), b.scene_hash());
    }

    #[test]
    fn scene_hash_changes_when_the_seed_changes() {
        let a = valid_scene().compile().unwrap();
        let mut scene = valid_scene();
        scene.project_seed = 43;
        let b = scene.compile().unwrap();
        assert_ne!(a.scene_hash(), b.scene_hash());
    }

    #[test]
    fn scene_hash_changes_when_graph_topology_changes() {
        // Same node positions, different edges. Routing follows the edges, so
        // these simulate differently and must not share a hash.
        let a = valid_scene().compile().unwrap();
        let mut scene = valid_scene();
        let detour = scene.waypoints.add_node(Vec2::new(5.0, 8.0));
        scene.waypoints.add_edge(0, detour);
        scene.waypoints.add_edge(detour, 1);
        let with_detour = scene.compile().unwrap();

        let mut positions_only = valid_scene();
        positions_only.waypoints.add_node(Vec2::new(5.0, 8.0));
        positions_only.waypoints.add_edge(0, 2);
        let without_detour = positions_only.compile().unwrap();

        assert_ne!(a.scene_hash(), with_detour.scene_hash());
        assert_ne!(with_detour.scene_hash(), without_detour.scene_hash());
    }

    #[test]
    fn an_unreachable_destination_is_rejected() {
        // Two disjoint components: the spawn sits in one, the destination in
        // the other. Both the connectivity fault and the reachability fault
        // are real and both must be reported.
        let mut scene = valid_scene();
        let island = scene.waypoints.add_node(Vec2::new(9.0, 9.0));
        scene.destinations[0].node = island;
        let errors = scene.compile().unwrap_err();
        assert!(
            errors.contains(&SceneError::UnreachableDestination {
                spawn: 0,
                destination: 0
            }),
            "got {errors:?}"
        );
    }

    #[test]
    fn a_population_too_slow_to_clamp_safely_is_rejected() {
        // Below MIN_PREFERRED_SPEED the spawn phase's clamp would have a floor
        // above its ceiling, and `f32::clamp` panics on inverted bounds — in
        // release as well as debug. Compilation must reject it first.
        let mut scene = valid_scene();
        scene.populations[0].speed_mean = 0.1;
        let errors = scene.compile().unwrap_err();
        assert!(
            errors.contains(&SceneError::InvalidPopulation {
                population: 0,
                field: "speed_mean"
            }),
            "got {errors:?}"
        );
    }

    #[test]
    fn a_population_with_impossible_radii_is_rejected() {
        let mut scene = valid_scene();
        scene.populations[0].radius_min = 0.5;
        scene.populations[0].radius_max = 0.2;
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::InvalidPopulation {
            population: 0,
            field: "radius_max"
        }));
    }

    #[test]
    fn a_non_positive_radius_is_rejected() {
        let mut scene = valid_scene();
        scene.populations[0].radius_min = 0.0;
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::InvalidPopulation {
            population: 0,
            field: "radius_min"
        }));
    }

    #[test]
    fn a_max_speed_below_preferred_speed_is_rejected() {
        let mut scene = valid_scene();
        scene.populations[0].max_speed_factor = 0.8;
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::InvalidPopulation {
            population: 0,
            field: "max_speed_factor"
        }));
    }

    #[test]
    fn nan_population_values_are_rejected() {
        // NaN compares false against every bound, so a plain `x < min` check
        // would wave it through. The validator tests `is_nan()` explicitly.
        let mut scene = valid_scene();
        scene.populations[0].speed_mean = f32::NAN;
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::InvalidPopulation {
            population: 0,
            field: "speed_mean"
        }));
    }

    #[test]
    fn a_zero_tick_rate_is_rejected() {
        let mut scene = valid_scene();
        scene.ticks_per_second = 0;
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::InvalidTickRate {
            ticks_per_second: 0
        }));
    }

    #[test]
    fn destination_position_is_none_when_out_of_range() {
        let compiled = valid_scene().compile().unwrap();
        assert_eq!(compiled.destination_position(0), Some(Vec2::new(9.0, 5.0)));
        assert_eq!(compiled.destination_position(7), None);
    }
}
