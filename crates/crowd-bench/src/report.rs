//! Benchmark execution and the report schema.
//!
//! Reports record the environment contract section 8.3 requires, so a metrics
//! number can never be quoted without the machine it came from.

use std::path::PathBuf;
use std::time::Instant;

use crowd_core::avoidance::{
    AnticipatorySolver, AvoidanceSolver, OrcaSolver, SampledVelocitySolver,
};
use crowd_core::fidelity::S2_UPDATE_INTERVAL_TICKS;
use crowd_core::metrics::{MetricsConfig, MetricsSummary};
use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};
use crowd_core::{FidelityPolicy, RenderTier, SimulationTier};
use serde::{Deserialize, Serialize};

use crate::alloc;
use crate::frames::FrameWriter;
use crate::svg::TrajectoryRecorder;

/// Bumped whenever the report schema changes incompatibly.
pub const REPORT_SCHEMA_VERSION: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolverKind {
    SampledVelocity,
    Orca,
    Anticipatory,
}

impl SolverKind {
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "sampled_velocity" => Ok(SolverKind::SampledVelocity),
            "orca" => Ok(SolverKind::Orca),
            "anticipatory" => Ok(SolverKind::Anticipatory),
            other => Err(format!(
                "unknown solver: {other}; known solvers: sampled_velocity, orca, anticipatory"
            )),
        }
    }

    fn build(self) -> Box<dyn AvoidanceSolver> {
        match self {
            SolverKind::SampledVelocity => Box::new(SampledVelocitySolver::default()),
            SolverKind::Orca => Box::new(OrcaSolver::default()),
            SolverKind::Anticipatory => Box::new(AnticipatorySolver::default()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RunOptions {
    pub scene: String,
    pub agents: u32,
    pub seed: u64,
    pub svg: bool,
    /// Write one PPM per sampled tick, for assembly into an animation.
    pub frames: bool,
    /// Ticks between emitted frames. Ignored unless `frames` is set.
    pub frame_interval: u64,
    pub out_dir: PathBuf,
    pub solver: SolverKind,
    /// Emit a trace v0 file (`crowd-trace`) instead of stepping via
    /// `run_to_completion`. Like `svg`/`frames`, this samples every tick, so
    /// a traced run's `ticks_per_second` is not a performance measurement.
    pub trace: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Environment {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub ram_bytes: u64,
    pub rustc_version: String,
    pub build_profile: String,
    /// RFC 3339, e.g. "2026-08-06T00:00:00Z". Best-effort: this project has
    /// no date/time dependency, so it is read from the system clock via
    /// `std::time::SystemTime` and formatted by hand rather than pulling one
    /// in for a single field.
    pub captured_at: String,
}

impl Environment {
    pub fn capture() -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            cpu: detect_cpu(),
            ram_bytes: detect_ram_bytes(),
            rustc_version: env!("CROWD_RUSTC_VERSION").to_string(),
            build_profile: if cfg!(debug_assertions) {
                "debug".to_string()
            } else {
                "release".to_string()
            },
            captured_at: format_utc_now(),
        }
    }
}

fn format_utc_now() -> String {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let days_since_epoch = duration.as_secs() / 86_400;
    let seconds_today = duration.as_secs() % 86_400;
    let (year, month, day) = civil_from_days(days_since_epoch as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds_today / 3600,
        (seconds_today % 3600) / 60,
        seconds_today % 60
    )
}

/// Howard Hinnant's `civil_from_days`: days since the Unix epoch to a
/// proleptic Gregorian (year, month, day). Small, dependency-free, and exact
/// -- the usual reason to reach for a date-time crate (leap years, month
/// lengths) is handled by this one closed-form conversion.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Best-effort CPU name. Unknown is acceptable; a wrong value is not.
///
/// `Command` is imported inside each `cfg` block rather than at module scope so
/// a platform with no detection path does not trip the unused-import lint,
/// which `clippy -D warnings` treats as an error.
fn detect_cpu() -> String {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if let Ok(out) = Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
        {
            if let Ok(text) = String::from_utf8(out.stdout) {
                let text = text.trim();
                if !text.is_empty() {
                    return text.to_string();
                }
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(text) = std::fs::read_to_string("/proc/cpuinfo") {
            for line in text.lines() {
                if let Some(value) = line.strip_prefix("model name") {
                    if let Some(value) = value.split(':').nth(1) {
                        return value.trim().to_string();
                    }
                }
            }
        }
    }
    "unknown".to_string()
}

fn detect_ram_bytes() -> u64 {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if let Ok(out) = Command::new("sysctl").args(["-n", "hw.memsize"]).output() {
            if let Ok(text) = String::from_utf8(out.stdout) {
                if let Ok(bytes) = text.trim().parse::<u64>() {
                    return bytes;
                }
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(text) = std::fs::read_to_string("/proc/meminfo") {
            for line in text.lines() {
                if let Some(value) = line.strip_prefix("MemTotal:") {
                    if let Some(kb) = value.split_whitespace().next() {
                        if let Ok(kb) = kb.parse::<u64>() {
                            return kb * 1024;
                        }
                    }
                }
            }
        }
    }
    0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub schema_version: u32,
    pub scene: String,
    pub solver: String,
    pub requested_agents: u32,
    pub seed: u64,
    pub ticks_per_second: u32,
    pub duration_ticks: u64,
    pub scene_hash: u64,
    pub final_state_hash: u64,
    /// Declared M5 fidelity mix, never inferred from an arbitrary camera.
    pub fidelity_profile: Option<FidelityProfileReport>,
    pub environment: Environment,
    pub metrics: MetricsSummary,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FidelityProfileReport {
    pub mode: String,
    pub s1_agents: u32,
    pub s2_agents: u32,
    pub r1_agents: u32,
    pub r2_agents: u32,
    pub s2_perception_interval_ticks: u32,
    pub s2_steering_interval_ticks: u32,
    pub s3_individual_perception: bool,
}

/// Build, run, and measure one scene.
pub fn run_scene(options: &RunOptions) -> Result<Report, String> {
    let scene_def = scenes::build(&options.scene, options.agents, options.seed)
        .ok_or_else(|| format!("unknown scene: {}", options.scene))?;
    let scene = scene_def
        .compile()
        .map_err(|errors| format!("{} failed to compile: {errors:?}", options.scene))?;

    let scene_hash = scene.scene_hash();
    let ticks_per_second = scene.ticks_per_second;
    let duration_ticks = scene.duration_ticks;

    let fidelity = if options.scene == "m5_city_flow" {
        Some(FidelityPolicy::m5_10k_profile())
    } else {
        None
    };
    let config = SimConfig {
        metrics: MetricsConfig {
            throughput_gate: scenes::throughput_gate(&options.scene, options.agents),
            ..MetricsConfig::default()
        },
        fidelity,
        ..SimConfig::default()
    };

    let mut sim = Simulation::new(scene, options.solver.build(), config);

    // PEAK/LIVE are process-wide (see alloc::measurement_lock), so exclude
    // concurrent measurement windows (parallel tests, or another run_scene on
    // another thread) from corrupting this run's peak reading.
    let _measurement_guard = alloc::measurement_lock().lock().unwrap();
    // Reset after construction so scene-build allocations are excluded.
    alloc::reset_peak();
    let started = Instant::now();
    let mut recorder = if options.svg {
        Some(TrajectoryRecorder::new(5, 400))
    } else {
        None
    };
    let frame_writer = if options.frames {
        let dir = options
            .out_dir
            .join(format!("frames-{}-{}", options.scene, options.agents));
        Some(FrameWriter::new(
            dir,
            sim.scene().bounds,
            options.frame_interval,
        )?)
    } else {
        None
    };
    // Recording costs a per-tick sample and, for frames, a rasterise and a
    // file write. The un-recorded path stays a tight loop so the timing metric
    // is not skewed by visualisation; a recorded run's `ticks_per_second` is
    // not comparable to a baseline and must not be quoted as a result.
    if recorder.is_some() || frame_writer.is_some() {
        let mut frame_writer = frame_writer;
        while sim.clock().tick() < duration_ticks {
            sim.step();
            if let Some(recorder) = recorder.as_mut() {
                recorder.record(&sim);
            }
            if let Some(writer) = frame_writer.as_mut() {
                writer.record(&sim)?;
            }
        }
        if let Some(writer) = frame_writer.as_ref() {
            println!(
                "{}: wrote {} frames ({}x{}) to {}",
                options.scene,
                writer.written(),
                writer.dimensions().0,
                writer.dimensions().1,
                writer.dir().display()
            );
        }
    } else if options.trace {
        // Replaces `run_to_completion` rather than following it: once the
        // scene's duration has elapsed there are no ticks left to record,
        // so a trace written afterwards would be empty. `write_trace` does
        // its own stepping, one tick per record batch, so this is the only
        // place `sim` is advanced when tracing is on.
        let path = options
            .out_dir
            .join(format!("{}-{}.crowdtrace", options.scene, options.agents));
        let remaining = duration_ticks.saturating_sub(sim.clock().tick());
        let ticks = crowd_bench::trace_out::write_trace(&mut sim, &path, remaining)
            .map_err(|e| format!("writing trace: {e}"))?;
        println!("trace: {} ({ticks} ticks)", path.display());
    } else {
        sim.run_to_completion();
    }
    let wall_time_seconds = started.elapsed().as_secs_f64();
    let peak_allocated_bytes = alloc::peak_bytes() as u64;

    if let Some(recorder) = recorder.as_ref() {
        std::fs::create_dir_all(&options.out_dir)
            .map_err(|e| format!("cannot create {}: {e}", options.out_dir.display()))?;
        let svg = recorder.write_svg(&options.scene, sim.scene().bounds, sim.walls());
        let path = options.out_dir.join(format!("{}.svg", options.scene));
        std::fs::write(&path, svg).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }

    let metrics = sim.metrics().summarize(
        sim.world(),
        sim.scene(),
        wall_time_seconds,
        peak_allocated_bytes,
    );

    let fidelity_profile = if options.scene == "m5_city_flow" {
        // Count the committed assignments rather than re-deriving the target
        // share from `options.agents`. The report is evidence of what ran.
        let s1_agents = sim
            .world()
            .simulation_tier
            .iter()
            .filter(|tier| **tier == SimulationTier::S1)
            .count() as u32;
        let s2_agents = sim
            .world()
            .simulation_tier
            .iter()
            .filter(|tier| **tier == SimulationTier::S2)
            .count() as u32;
        let r1_agents = sim
            .world()
            .render_fidelity_tier
            .iter()
            .filter(|tier| **tier == RenderTier::R1)
            .count() as u32;
        let r2_agents = sim
            .world()
            .render_fidelity_tier
            .iter()
            .filter(|tier| **tier == RenderTier::R2)
            .count() as u32;
        Some(FidelityProfileReport {
            mode: "stable_agent_id_hash_target_10_percent_s1_90_percent_s2".to_owned(),
            s1_agents,
            s2_agents,
            r1_agents,
            r2_agents,
            s2_perception_interval_ticks: S2_UPDATE_INTERVAL_TICKS as u32,
            s2_steering_interval_ticks: S2_UPDATE_INTERVAL_TICKS as u32,
            s3_individual_perception: false,
        })
    } else {
        None
    };
    Ok(Report {
        schema_version: REPORT_SCHEMA_VERSION,
        scene: options.scene.clone(),
        solver: sim.solver_name().to_string(),
        requested_agents: options.agents,
        seed: options.seed,
        ticks_per_second,
        duration_ticks,
        scene_hash,
        final_state_hash: sim.state_hash(),
        fidelity_profile,
        environment: Environment::capture(),
        metrics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(scene: &str, agents: u32) -> RunOptions {
        RunOptions {
            scene: scene.to_string(),
            agents,
            seed: 2026,
            svg: false,
            frames: false,
            frame_interval: crate::frames::DEFAULT_FRAME_INTERVAL_TICKS,
            out_dir: std::env::temp_dir().join("crowd_bench_test"),
            solver: SolverKind::SampledVelocity,
            trace: false,
        }
    }

    #[test]
    fn running_a_known_scene_produces_a_report() {
        let report = run_scene(&options("bidirectional_corridor", 50)).unwrap();
        assert_eq!(report.scene, "bidirectional_corridor");
        assert_eq!(report.requested_agents, 50);
        assert_eq!(report.solver, "sampled_velocity");
        assert!(report.metrics.ticks > 0);
    }

    #[test]
    fn running_an_unknown_scene_reports_an_error() {
        let error = run_scene(&options("nope", 10)).unwrap_err();
        assert!(error.contains("nope"), "unhelpful error: {error}");
    }

    #[test]
    fn the_report_round_trips_through_json() {
        let report = run_scene(&options("crossing", 30)).unwrap();
        let json = serde_json::to_string_pretty(&report).unwrap();
        let parsed: Report = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, report);
    }

    #[test]
    fn identical_runs_produce_identical_quality_metrics() {
        // Timing fields legitimately vary; quality fields must not, because
        // the simulation is deterministic.
        let a = run_scene(&options("bottleneck", 40)).unwrap();
        let b = run_scene(&options("bottleneck", 40)).unwrap();
        assert_eq!(a.final_state_hash, b.final_state_hash);
        assert_eq!(
            a.metrics.penetration_pair_ticks,
            b.metrics.penetration_pair_ticks
        );
        assert_eq!(a.metrics.agents_arrived, b.metrics.agents_arrived);
        assert_eq!(a.metrics.heading_reversals, b.metrics.heading_reversals);
    }

    #[test]
    fn the_environment_is_captured() {
        let environment = Environment::capture();
        assert!(!environment.os.is_empty());
        assert!(!environment.arch.is_empty());
        assert!(!environment.rustc_version.is_empty());
    }

    #[test]
    fn the_report_records_the_scene_hash() {
        let report = run_scene(&options("circle", 32)).unwrap();
        assert_ne!(report.scene_hash, 0);
    }

    #[test]
    fn m5_city_flow_records_its_declared_background_mix() {
        let report = run_scene(&options("m5_city_flow", 100)).unwrap();
        let profile = report.fidelity_profile.expect("M5 profile was omitted");
        assert_eq!(
            profile.mode,
            "stable_agent_id_hash_target_10_percent_s1_90_percent_s2"
        );
        assert_eq!(profile.s1_agents + profile.s2_agents, 100);
        assert_eq!(profile.r1_agents, profile.s1_agents);
        assert_eq!(profile.r2_agents, profile.s2_agents);
        assert!(
            profile.s1_agents > 0,
            "stable profile assigned no S1 agents"
        );
        assert!(profile.s2_agents > profile.s1_agents);
    }

    #[test]
    fn civil_from_days_matches_a_known_date() {
        // 2026-08-06 is 20,671 days after the Unix epoch (1970-01-01),
        // cross-checked against Python's `datetime` stdlib:
        // `date(1970,1,1) + timedelta(days=20671) == date(2026,8,6)`.
        // (The brief's original constant, 20_306, was independently
        // verified to correspond to 2025-08-06, not 2026-08-06.)
        assert_eq!(civil_from_days(20_671), (2026, 8, 6));
    }

    #[test]
    fn captured_at_is_well_formed_rfc3339() {
        let environment = Environment::capture();
        assert_eq!(
            environment.captured_at.len(),
            20,
            "{}",
            environment.captured_at
        );
        assert!(environment.captured_at.ends_with('Z'));
    }
}
