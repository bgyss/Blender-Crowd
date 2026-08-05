//! Benchmark execution and the report schema.
//!
//! Reports record the environment contract section 8.3 requires, so a metrics
//! number can never be quoted without the machine it came from.
//!
//! `run_scene`, `Report`, and the `svg`/`out_dir` fields of `RunOptions` are
//! only exercised by tests until Task 23 wires the real CLI into `main.rs`;
//! until then the plain (non-test) binary target sees them as unused. Allowed
//! rather than removed since they are the module's required interface.
#![allow(dead_code)]

use std::path::PathBuf;
use std::time::Instant;

use crowd_core::avoidance::SampledVelocitySolver;
use crowd_core::metrics::{MetricsConfig, MetricsSummary};
use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};
use serde::{Deserialize, Serialize};

use crate::alloc;

/// Bumped whenever the report schema changes incompatibly.
pub const REPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub struct RunOptions {
    pub scene: String,
    pub agents: u32,
    pub seed: u64,
    pub svg: bool,
    pub out_dir: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Environment {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub ram_bytes: u64,
    pub rustc_version: String,
    pub build_profile: String,
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
        }
    }
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
    pub environment: Environment,
    pub metrics: MetricsSummary,
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

    let config = SimConfig {
        metrics: MetricsConfig {
            throughput_gate: scenes::throughput_gate(&options.scene),
            ..MetricsConfig::default()
        },
        ..SimConfig::default()
    };

    let mut sim = Simulation::new(scene, Box::new(SampledVelocitySolver::default()), config);

    // Reset after construction so scene-build allocations are excluded.
    alloc::reset_peak();
    let started = Instant::now();
    sim.run_to_completion();
    let wall_time_seconds = started.elapsed().as_secs_f64();
    let peak_allocated_bytes = alloc::peak_bytes() as u64;

    let metrics = sim.metrics().summarize(
        sim.world(),
        sim.scene(),
        wall_time_seconds,
        peak_allocated_bytes,
    );

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
            out_dir: std::env::temp_dir().join("crowd_bench_test"),
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
        assert_eq!(a.metrics.penetration_events, b.metrics.penetration_events);
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
}
