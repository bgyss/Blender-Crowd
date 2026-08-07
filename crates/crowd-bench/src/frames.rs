//! Dependency-free per-tick frame rasterisation.
//!
//! `svg.rs` draws whole trajectories: it answers "where did the crowd go".
//! This module answers "what does the crowd look like while it moves", which
//! is the only way to see the robotic-motion and congestion risks named in
//! contract section 16 before the Blender bridge exists.
//!
//! Frames are written as binary PPM (P6) because it needs no encoder: every
//! image tool in the pipeline reads it, and adding a PNG or GIF crate to get a
//! visualisation aid would cost more than it saves.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use crowd_core::geometry::Segment;
use crowd_core::sim::Simulation;
use crowd_core::units::{Aabb, Vec2};

/// Largest emitted frame edge, in pixels. Frames are for a README animation,
/// not for analysis, so a scene that would rasterise huge is scaled down
/// rather than allowed to produce a multi-megabyte GIF.
const MAX_EDGE_PIXELS: f32 = 960.0;
/// Upper bound on pixels per meter, so a small scene is not blown up past the
/// point where agent discs overlap into a single blob.
const MAX_SCALE: f32 = 24.0;
const MARGIN_PIXELS: f32 = 12.0;

/// Default ticks between emitted frames. At the scenes' 30 Hz tick, 9 ticks
/// samples a third of a simulated second, so a 240-frame capture spans 72 s of
/// simulation -- long enough for congestion to build, which is the behaviour
/// worth watching. Override with `--frame-interval`.
pub const DEFAULT_FRAME_INTERVAL_TICKS: u64 = 9;
/// Frame cap. Scenes run for thousands of ticks and each frame is an
/// uncompressed PPM, so an uncapped capture would fill a disk to make an
/// animation nobody watches to the end.
const MAX_FRAMES: usize = 240;

const BACKGROUND: [u8; 3] = [17, 17, 17];
const WALL: [u8; 3] = [136, 136, 136];
/// Arrived agents are drawn dim rather than hidden: a vanishing agent reads as
/// a bug, a fading one reads as a destination reached.
const ARRIVED: [u8; 3] = [70, 70, 70];

/// One rasterised frame, RGB8, row-major from the top-left.
pub struct Frame {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl Frame {
    // Dimensions and pixel reads are part of the module's required interface
    // and are exercised by tests; the CLI path only needs `write_ppm`.
    #[allow(dead_code)]
    pub fn width(&self) -> usize {
        self.width
    }

    #[allow(dead_code)]
    pub fn height(&self) -> usize {
        self.height
    }

    #[allow(dead_code)]
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.width + x) * 3;
        [self.pixels[i], self.pixels[i + 1], self.pixels[i + 2]]
    }

    pub fn write_ppm(&self, path: &Path) -> Result<(), String> {
        let file = std::fs::File::create(path)
            .map_err(|e| format!("cannot create {}: {e}", path.display()))?;
        let mut out = std::io::BufWriter::new(file);
        write!(out, "P6\n{} {}\n255\n", self.width, self.height)
            .and_then(|()| out.write_all(&self.pixels))
            .and_then(|()| out.flush())
            .map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    fn set(&mut self, x: i32, y: i32, color: [u8; 3]) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let i = (y as usize * self.width + x as usize) * 3;
        self.pixels[i..i + 3].copy_from_slice(&color);
    }

    fn disc(&mut self, cx: f32, cy: f32, radius: f32, color: [u8; 3]) {
        // At least one pixel: an agent that rounds away to nothing would make
        // a dense scene look emptier than it is.
        let r = radius.max(1.0);
        let r2 = r * r;
        let x0 = (cx - r).floor() as i32;
        let x1 = (cx + r).ceil() as i32;
        let y0 = (cy - r).floor() as i32;
        let y1 = (cy + r).ceil() as i32;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                if dx * dx + dy * dy <= r2 {
                    self.set(x, y, color);
                }
            }
        }
    }

    fn line(&mut self, from: (f32, f32), to: (f32, f32), color: [u8; 3]) {
        // Integer DDA. Bresenham would avoid the divide, but this runs a few
        // dozen times per frame and reads more obviously.
        let dx = to.0 - from.0;
        let dy = to.1 - from.1;
        let steps = dx.abs().max(dy.abs()).ceil().max(1.0);
        for step in 0..=(steps as i32) {
            let t = step as f32 / steps;
            self.set(
                (from.0 + dx * t).round() as i32,
                (from.1 + dy * t).round() as i32,
                color,
            );
        }
    }
}

/// Projects world meters to frame pixels and rasterises agents and walls.
///
/// Built once per run so every frame in a sequence shares one projection: a
/// per-frame fit would make the camera drift as the crowd spreads.
pub struct FrameRenderer {
    bounds: Aabb,
    scale: f32,
    width: usize,
    height: usize,
}

impl FrameRenderer {
    pub fn new(bounds: Aabb) -> Self {
        let size = bounds.size();
        let span_x = size.x.max(1.0);
        let span_y = size.y.max(1.0);
        let usable = MAX_EDGE_PIXELS - MARGIN_PIXELS * 2.0;
        let scale = (usable / span_x).min(usable / span_y).min(MAX_SCALE);
        // Even dimensions: several video encoders reject odd ones, and the
        // frames exist to be fed to exactly such a tool.
        let width = to_even(span_x * scale + MARGIN_PIXELS * 2.0);
        let height = to_even(span_y * scale + MARGIN_PIXELS * 2.0);
        Self {
            bounds,
            scale,
            width,
            height,
        }
    }

    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// World point to pixel. The simulation's Y axis points up and an image's
    /// points down, so Y is flipped here to keep frames matching the scene as
    /// authored -- and matching the SVG output.
    fn project(&self, p: Vec2) -> (f32, f32) {
        (
            (p.x - self.bounds.min.x) * self.scale + MARGIN_PIXELS,
            self.height as f32 - ((p.y - self.bounds.min.y) * self.scale + MARGIN_PIXELS),
        )
    }

    pub fn render(&self, sim: &Simulation) -> Frame {
        let mut frame = Frame {
            width: self.width,
            height: self.height,
            pixels: BACKGROUND
                .iter()
                .copied()
                .cycle()
                .take(self.width * self.height * 3)
                .collect(),
        };

        self.draw_walls(&mut frame, sim.walls());

        let world = sim.world();
        for slot in 0..world.len() {
            let position = world.position(slot as u32);
            if !position.is_finite() {
                continue;
            }
            let (x, y) = self.project(position);
            if !x.is_finite() || !y.is_finite() {
                continue;
            }
            let color = if world.arrived[slot] {
                ARRIVED
            } else {
                stream_color(world.destination[slot])
            };
            frame.disc(x, y, world.radius[slot] * self.scale, color);
        }

        frame
    }

    fn draw_walls(&self, frame: &mut Frame, walls: &[Segment]) {
        for wall in walls {
            let a = self.project(wall.a);
            let b = self.project(wall.b);
            if [a.0, a.1, b.0, b.1].iter().all(|v| v.is_finite()) {
                frame.line(a, b, WALL);
            }
        }
    }
}

/// Samples a running simulation and writes numbered PPM frames to a directory.
///
/// The directory is emptied of stale `frame-*.ppm` on construction: a shorter
/// second run would otherwise leave a longer first run's tail frames behind,
/// and the assembled animation would end with footage from a different run.
pub struct FrameWriter {
    dir: PathBuf,
    renderer: FrameRenderer,
    interval_ticks: u64,
    ticks_seen: u64,
    written: usize,
}

impl FrameWriter {
    pub fn new(dir: PathBuf, bounds: Aabb, interval_ticks: u64) -> Result<Self, String> {
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        for entry in
            std::fs::read_dir(&dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?
        {
            let path = entry
                .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
                .path();
            let is_frame = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("frame-") && name.ends_with(".ppm"));
            if is_frame {
                std::fs::remove_file(&path)
                    .map_err(|e| format!("cannot remove {}: {e}", path.display()))?;
            }
        }
        Ok(Self {
            dir,
            renderer: FrameRenderer::new(bounds),
            interval_ticks: interval_ticks.max(1),
            ticks_seen: 0,
            written: 0,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn dimensions(&self) -> (usize, usize) {
        self.renderer.dimensions()
    }

    pub fn written(&self) -> usize {
        self.written
    }

    /// Render and write this tick if it falls on the interval and the cap has
    /// not been reached.
    pub fn record(&mut self, sim: &Simulation) -> Result<(), String> {
        self.ticks_seen += 1;
        if self.written >= MAX_FRAMES || !self.ticks_seen.is_multiple_of(self.interval_ticks) {
            return Ok(());
        }
        let path = self.dir.join(format!("frame-{:05}.ppm", self.written));
        self.renderer.render(sim).write_ppm(&path)?;
        self.written += 1;
        Ok(())
    }
}

fn to_even(value: f32) -> usize {
    let rounded = value.ceil().max(2.0) as usize;
    rounded + rounded % 2
}

/// Colour by destination, so opposing streams are visually separable.
///
/// Destination rather than population because the scenes that are worth
/// animating -- `crossing`, `bidirectional_corridor` -- put both streams in
/// one population and distinguish them only by where they are headed.
/// Colouring by population would draw those scenes in a single colour.
///
/// A fixed palette keeps this dependency-free and stable across runs; a
/// random one would make two renders of the same scene incomparable.
fn stream_color(destination: u16) -> [u8; 3] {
    const PALETTE: [[u8; 3]; 6] = [
        [102, 179, 255], // blue
        [255, 143, 102], // orange
        [140, 224, 140], // green
        [232, 140, 224], // magenta
        [240, 216, 120], // yellow
        [150, 160, 255], // periwinkle
    ];
    PALETTE[destination as usize % PALETTE.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crowd_core::avoidance::SampledVelocitySolver;
    use crowd_core::scenes;
    use crowd_core::sim::{SimConfig, Simulation};

    fn simulation(scene_name: &str) -> Simulation {
        let scene = scenes::build(scene_name, 40, 1).unwrap().compile().unwrap();
        Simulation::new(
            scene,
            Box::new(SampledVelocitySolver::default()),
            SimConfig::default(),
        )
    }

    fn renderer(sim: &Simulation) -> FrameRenderer {
        FrameRenderer::new(sim.scene().bounds)
    }

    #[test]
    fn frame_dimensions_are_even_and_bounded() {
        for scene in scenes::SCENE_NAMES {
            let sim = simulation(scene);
            let (width, height) = renderer(&sim).dimensions();
            assert_eq!(width % 2, 0, "{scene} width {width} is odd");
            assert_eq!(height % 2, 0, "{scene} height {height} is odd");
            assert!(
                width as f32 <= MAX_EDGE_PIXELS && height as f32 <= MAX_EDGE_PIXELS,
                "{scene} frame {width}x{height} exceeds the edge cap"
            );
        }
    }

    #[test]
    fn every_scene_renders_agents_against_the_background() {
        for scene in scenes::SCENE_NAMES {
            let mut sim = simulation(scene);
            sim.run(30);
            let frame = renderer(&sim).render(&sim);
            let non_background = frame
                .pixels
                .chunks_exact(3)
                .filter(|p| p != &BACKGROUND)
                .count();
            assert!(non_background > 0, "{scene} rendered an empty frame");
        }
    }

    #[test]
    fn a_frame_at_rest_is_identical_when_rendered_twice() {
        let mut sim = simulation("crossing");
        sim.run(20);
        let renderer = renderer(&sim);
        assert_eq!(renderer.render(&sim).pixels, renderer.render(&sim).pixels);
    }

    #[test]
    fn the_projection_flips_the_y_axis() {
        let sim = simulation("crossing");
        let renderer = renderer(&sim);
        let bounds = sim.scene().bounds;
        let low = renderer.project(Vec2::new(bounds.min.x, bounds.min.y));
        let high = renderer.project(Vec2::new(bounds.min.x, bounds.max.y));
        assert!(
            high.1 < low.1,
            "world +Y should map to a smaller pixel row, got {} then {}",
            low.1,
            high.1
        );
    }

    #[test]
    fn drawing_outside_the_frame_does_not_panic() {
        let sim = simulation("crossing");
        let renderer = renderer(&sim);
        let mut frame = renderer.render(&sim);
        frame.disc(-50.0, -50.0, 8.0, WALL);
        frame.disc(1.0e6, 1.0e6, 8.0, WALL);
        frame.line((-100.0, -100.0), (1.0e6, 1.0e6), WALL);
    }

    #[test]
    fn a_ppm_round_trips_its_header_and_payload_size() {
        let dir = std::env::temp_dir().join(format!("crowd-frames-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("frame.ppm");

        let mut sim = simulation("bottleneck");
        sim.run(10);
        let frame = renderer(&sim).render(&sim);
        frame.write_ppm(&path).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let header = format!("P6\n{} {}\n255\n", frame.width(), frame.height());
        assert!(bytes.starts_with(header.as_bytes()));
        assert_eq!(
            bytes.len(),
            header.len() + frame.width() * frame.height() * 3
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_writer_honours_its_interval_and_writes_one_file_per_frame() {
        let dir = std::env::temp_dir().join(format!("crowd-writer-{}", std::process::id()));
        let mut sim = simulation("crossing");
        let mut writer = FrameWriter::new(dir.clone(), sim.scene().bounds, 5).unwrap();
        for _ in 0..37 {
            sim.step();
            writer.record(&sim).unwrap();
        }
        assert_eq!(writer.written(), 7);
        let files = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(files, 7);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn stale_frames_from_a_longer_previous_run_are_removed() {
        let dir = std::env::temp_dir().join(format!("crowd-stale-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("frame-09999.ppm"), b"stale").unwrap();
        std::fs::write(dir.join("notes.txt"), b"keep me").unwrap();

        let sim = simulation("crossing");
        FrameWriter::new(dir.clone(), sim.scene().bounds, 1).unwrap();

        assert!(
            !dir.join("frame-09999.ppm").exists(),
            "stale frame survived"
        );
        assert!(dir.join("notes.txt").exists(), "unrelated file was deleted");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn streams_to_different_destinations_are_drawn_in_distinct_colors() {
        assert_ne!(stream_color(0), stream_color(1));
    }

    #[test]
    fn a_two_destination_scene_renders_in_two_agent_colors() {
        let mut sim = simulation("crossing");
        sim.run(60);
        let frame = FrameRenderer::new(sim.scene().bounds).render(&sim);
        let seen: std::collections::BTreeSet<[u8; 3]> = frame
            .pixels
            .chunks_exact(3)
            .map(|p| [p[0], p[1], p[2]])
            .filter(|p| *p == stream_color(0) || *p == stream_color(1))
            .collect();
        assert_eq!(seen.len(), 2, "both crossing streams should be visible");
    }
}
