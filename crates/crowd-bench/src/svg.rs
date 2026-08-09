//! Dependency-free trajectory visualisation.
//!
//! Metrics cannot tell you a crowd looks robotic. Contract section 16 names
//! "avoidance looks robotic or deadlocks" as a top risk, and this is the only
//! way to see it before the Blender bridge exists.

use std::fmt::Write;

use crowd_core::geometry::Segment;
use crowd_core::nav::NavDebugSnapshot;
use crowd_core::sim::Simulation;
use crowd_core::units::{Aabb, Vec2};

/// Pixels per meter in the emitted SVG.
const SCALE: f32 = 20.0;
const MARGIN: f32 = 20.0;

pub struct TrajectoryRecorder {
    sample_interval: u64,
    max_agents: usize,
    ticks_seen: u64,
    sample_count: usize,
    /// One polyline per tracked agent, in stable slot order.
    tracks: Vec<Vec<Vec2>>,
}

impl TrajectoryRecorder {
    pub fn new(sample_interval: u64, max_agents: usize) -> Self {
        Self {
            sample_interval: sample_interval.max(1),
            max_agents,
            ticks_seen: 0,
            sample_count: 0,
            tracks: Vec::new(),
        }
    }

    // Not called from the CLI; part of the module's required interface and
    // exercised directly by tests.
    #[allow(dead_code)]
    pub fn sample_count(&self) -> usize {
        self.sample_count
    }

    #[allow(dead_code)]
    pub fn tracked_agents(&self) -> usize {
        self.tracks.len()
    }

    /// Sample the world if this tick falls on the interval.
    ///
    /// Only the first `max_agents` slots are tracked. Slots are assigned in
    /// spawn order, so the tracked set is stable across runs rather than an
    /// arbitrary sample that changes shape between renders.
    pub fn record(&mut self, sim: &Simulation) {
        self.ticks_seen += 1;
        if !self.ticks_seen.is_multiple_of(self.sample_interval) {
            return;
        }
        self.sample_count += 1;

        let world = sim.world();
        let tracked = world.len().min(self.max_agents);
        if self.tracks.len() < tracked {
            self.tracks.resize(tracked, Vec::new());
        }
        for slot in 0..tracked {
            let p = world.position(slot as u32);
            if p.is_finite() {
                self.tracks[slot].push(p);
            }
        }
    }

    pub fn write_svg(&self, scene_name: &str, bounds: Aabb, walls: &[Segment]) -> String {
        let size = bounds.size();
        let width = size.x * SCALE + MARGIN * 2.0;
        let height = size.y * SCALE + MARGIN * 2.0;

        // SVG's Y axis points down; the simulation's points up. Flipping here
        // keeps the rendered image matching the scene as authored.
        let project = |p: Vec2| -> (f32, f32) {
            (
                (p.x - bounds.min.x) * SCALE + MARGIN,
                height - ((p.y - bounds.min.y) * SCALE + MARGIN),
            )
        };

        let mut out = String::new();
        let _ = write!(
            out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}">"#
        );
        let _ = write!(out, r##"<rect width="100%" height="100%" fill="#111"/>"##);
        let _ = write!(
            out,
            r##"<text x="{MARGIN:.0}" y="{:.0}" fill="#eee" font-family="monospace" font-size="14">{scene_name}</text>"##,
            MARGIN - 4.0
        );

        for wall in walls {
            let (x1, y1) = project(wall.a);
            let (x2, y2) = project(wall.b);
            if [x1, y1, x2, y2].iter().all(|v| v.is_finite()) {
                let _ = write!(
                    out,
                    r##"<line x1="{x1:.1}" y1="{y1:.1}" x2="{x2:.1}" y2="{y2:.1}" stroke="#888" stroke-width="2"/>"##
                );
            }
        }

        for (index, track) in self.tracks.iter().enumerate() {
            if track.len() < 2 {
                continue;
            }
            // A fixed hue rotation keeps neighboring agents distinguishable
            // without needing a palette dependency.
            let hue = (index * 47) % 360;
            let mut points = String::new();
            for p in track {
                let (x, y) = project(*p);
                if x.is_finite() && y.is_finite() {
                    let _ = write!(points, "{x:.1},{y:.1} ");
                }
            }
            let _ = write!(
                out,
                r#"<polyline points="{}" fill="none" stroke="hsl({hue},70%,60%)" stroke-width="1.2" opacity="0.75"/>"#,
                points.trim_end()
            );
        }

        out.push_str("</svg>\n");
        out
    }

    pub fn write_svg_with_nav(
        &self,
        scene_name: &str,
        bounds: Aabb,
        walls: &[Segment],
        nav: Option<&NavDebugSnapshot>,
    ) -> String {
        let size = bounds.size();
        let width = size.x * SCALE + MARGIN * 2.0;
        let height = size.y * SCALE + MARGIN * 2.0;
        let project = |p: Vec2| -> (f32, f32) {
            (
                (p.x - bounds.min.x) * SCALE + MARGIN,
                height - ((p.y - bounds.min.y) * SCALE + MARGIN),
            )
        };

        let mut out = String::new();
        let _ = write!(
            out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}">"#
        );
        let _ = write!(out, r##"<rect width="100%" height="100%" fill="#111"/>"##);

        if let Some(nav) = nav {
            for tile in 0..(nav.cols * nav.rows) {
                if !nav.walkable[tile as usize] {
                    continue;
                }
                let col = tile % nav.cols;
                let row = tile / nav.cols;
                let center = Vec2::new(
                    nav.origin.x + (col as f32 + 0.5) * nav.tile_size,
                    nav.origin.y + (row as f32 + 0.5) * nav.tile_size,
                );
                let (x, y) = project(center);
                let cost = nav.cost[tile as usize];
                let fill = if cost > 1.01 { "#553311" } else { "#1a2a1a" };
                let half = nav.tile_size * SCALE * 0.5;
                let _ = write!(
                    out,
                    r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{fill}"/>"##,
                    x - half,
                    y - half,
                    half * 2.0,
                    half * 2.0
                );
            }
            for (portal, midpoint) in &nav.portals {
                let (x, y) = project(*midpoint);
                let color = if portal.open { "#4caf50" } else { "#f44336" };
                let _ = write!(
                    out,
                    r##"<circle cx="{x:.1}" cy="{y:.1}" r="3" fill="{color}"/>"##
                );
            }
            for (_, corridor) in &nav.corridors {
                let mut points = String::new();
                for p in corridor {
                    let (x, y) = project(*p);
                    let _ = write!(points, "{x:.1},{y:.1} ");
                }
                let _ = write!(
                    out,
                    r##"<polyline points="{}" fill="none" stroke="#00bcd4" stroke-width="1" opacity="0.6"/>"##,
                    points.trim_end()
                );
            }
        }

        for wall in walls {
            let (x1, y1) = project(wall.a);
            let (x2, y2) = project(wall.b);
            if [x1, y1, x2, y2].iter().all(|v| v.is_finite()) {
                let _ = write!(
                    out,
                    r##"<line x1="{x1:.1}" y1="{y1:.1}" x2="{x2:.1}" y2="{y2:.1}" stroke="#888" stroke-width="2"/>"##
                );
            }
        }

        let _ = write!(
            out,
            r##"<text x="{MARGIN:.0}" y="{:.0}" fill="#eee" font-family="monospace" font-size="14">{scene_name}</text>"##,
            MARGIN - 4.0
        );

        out.push_str("</svg>\n");
        out
    }
}

#[cfg(test)]
mod nav_svg_tests {
    use super::*;
    use crowd_core::nav::{NavDebugSnapshot, NavMeshDef};
    use crowd_core::route::RouteArena;
    use crowd_core::units::{Aabb, Vec2};
    use crowd_core::world::World;

    #[test]
    fn nav_debug_svg_renders_tiles_and_portals() {
        let graph = NavMeshDef {
            tile_size: 1.0,
            agent_radius: 0.3,
            cost_areas: Vec::new(),
            named_portals: Vec::new(),
        }
        .build_graph(Aabb::new(Vec2::ZERO, Vec2::new(3.0, 3.0)), &[]);
        let world = World::new();
        let routes = RouteArena::new();
        let snapshot = NavDebugSnapshot::capture(&graph, &world, &routes, 100);
        let recorder = TrajectoryRecorder::new(5, 100);
        let svg = recorder.write_svg_with_nav(
            "two_room",
            Aabb::new(Vec2::ZERO, Vec2::new(3.0, 3.0)),
            &[],
            Some(&snapshot),
        );
        assert!(svg.contains("<rect"));
        assert!(svg.contains("<circle"));
    }

    #[test]
    fn nav_debug_svg_without_a_snapshot_still_renders() {
        let recorder = TrajectoryRecorder::new(5, 100);
        let svg = recorder.write_svg_with_nav(
            "empty",
            Aabb::new(Vec2::ZERO, Vec2::new(3.0, 3.0)),
            &[],
            None,
        );
        assert!(svg.starts_with("<svg"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crowd_core::avoidance::SampledVelocitySolver;
    use crowd_core::scenes;
    use crowd_core::sim::{SimConfig, Simulation};

    fn simulation() -> Simulation {
        let scene = scenes::build("bidirectional_corridor", 20, 1)
            .unwrap()
            .compile()
            .unwrap();
        Simulation::new(
            scene,
            Box::new(SampledVelocitySolver::default()),
            SimConfig::default(),
        )
    }

    #[test]
    fn an_empty_recorder_still_writes_valid_svg() {
        let sim = simulation();
        let recorder = TrajectoryRecorder::new(5, 100);
        let svg = recorder.write_svg("empty", sim.scene().bounds, sim.walls());
        assert!(svg.starts_with("<svg"));
        assert!(svg.trim_end().ends_with("</svg>"));
    }

    #[test]
    fn recorded_trajectories_appear_as_polylines() {
        let mut sim = simulation();
        let mut recorder = TrajectoryRecorder::new(1, 100);
        for _ in 0..30 {
            sim.step();
            recorder.record(&sim);
        }
        let svg = recorder.write_svg("corridor", sim.scene().bounds, sim.walls());
        assert!(svg.contains("<polyline"), "no trajectories drawn");
    }

    #[test]
    fn walls_are_drawn() {
        let sim = simulation();
        let recorder = TrajectoryRecorder::new(5, 100);
        let svg = recorder.write_svg("corridor", sim.scene().bounds, sim.walls());
        assert!(svg.contains("<line"), "no walls drawn");
    }

    #[test]
    fn the_sample_interval_is_respected() {
        let mut sim = simulation();
        let mut recorder = TrajectoryRecorder::new(10, 100);
        for _ in 0..30 {
            sim.step();
            recorder.record(&sim);
        }
        assert_eq!(recorder.sample_count(), 3);
    }

    #[test]
    fn the_agent_cap_is_respected() {
        let mut sim = simulation();
        let mut recorder = TrajectoryRecorder::new(1, 5);
        for _ in 0..20 {
            sim.step();
            recorder.record(&sim);
        }
        assert!(recorder.tracked_agents() <= 5);
    }

    #[test]
    fn output_contains_no_non_finite_coordinates() {
        let mut sim = simulation();
        let mut recorder = TrajectoryRecorder::new(2, 50);
        for _ in 0..40 {
            sim.step();
            recorder.record(&sim);
        }
        let svg = recorder.write_svg("corridor", sim.scene().bounds, sim.walls());
        assert!(!svg.contains("NaN"), "NaN leaked into the SVG");
        assert!(!svg.contains("inf"), "infinity leaked into the SVG");
    }
}
