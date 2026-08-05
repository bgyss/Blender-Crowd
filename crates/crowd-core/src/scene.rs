//! Scene authoring input and its compiled, validated form.
//!
//! Compilation is where contract section 10.3's error model lives: every
//! diagnostic names the offending entity and every independent fault is
//! reported in one pass, so a user is not forced to fix problems one at a
//! time.

use crate::geometry::Segment;
use crate::grid::SegmentIndex;
use crate::ids::{hash_combine, hash_str, mix64};
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
}

/// A bake-blocking authoring fault. Each names the entity at fault.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneError {
    EmptyWaypointGraph,
    DisconnectedWaypointGraph,
    NoDestinations,
    NoSpawns,
    SpawnOutsideBounds { spawn: u16 },
    UnknownDestination { spawn: u16, destination: u16 },
    UnknownPopulation { spawn: u16, population: u16 },
    DestinationNodeMissing { destination: u16, node: u32 },
    UnreachableDestination { spawn: u16, destination: u16 },
    InvalidTickRate { ticks_per_second: u32 },
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
        if self.waypoints.node_count() == 0 {
            errors.push(SceneError::EmptyWaypointGraph);
        } else if !self.waypoints.is_connected() {
            errors.push(SceneError::DisconnectedWaypointGraph);
        }
        if self.destinations.is_empty() {
            errors.push(SceneError::NoDestinations);
        }
        if self.spawns.is_empty() {
            errors.push(SceneError::NoSpawns);
        }

        for (index, destination) in self.destinations.iter().enumerate() {
            if destination.node >= self.waypoints.node_count() {
                errors.push(SceneError::DestinationNodeMissing {
                    destination: index as u16,
                    node: destination.node,
                });
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
            // Reachability is only meaningful once the graph itself is sound.
            if self.waypoints.node_count() > 0 && destination.node < self.waypoints.node_count() {
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
        }
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
