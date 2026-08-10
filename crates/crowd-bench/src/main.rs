//! Benchmark runner.
//!
//! Argument parsing is hand-rolled to keep the dependency set at serde alone;
//! the surface is small enough that a parser crate would cost more than it
//! saves.

mod alloc;
mod baseline;
mod frames;
mod nav_bench;
mod report;
mod svg;

use std::path::PathBuf;
use std::process::ExitCode;

use crowd_bench::cache_bench::{run_experiment, write_report, ExperimentOptions};
use crowd_core::scenes;

use crate::report::{run_scene, RunOptions};

#[global_allocator]
static ALLOCATOR: alloc::CountingAllocator = alloc::CountingAllocator;

const DEFAULT_AGENTS: u32 = 1000;
const DEFAULT_SEED: u64 = 2026;
const BASELINE_DIR: &str = "benchmarks/baselines";
const REPORT_DIR: &str = "benchmarks/reports";

fn usage() -> &'static str {
    "usage:
  crowd-bench run [--scene NAME] [--agents N] [--seed N] [--svg] [--frames] [--frame-interval N] [--out DIR] [--solver NAME] [--trace]
  crowd-bench sweep [--scene NAME] [--seed N]
  crowd-bench baseline [--scene NAME] [--agents N] [--seed N] [--solver NAME]
  crowd-bench check [--agents N] [--seed N] [--solver NAME]
  crowd-bench compare [--scene NAME] [--out DIR]
  crowd-bench nav-reroute [--agents N] [--seed N] [--out DIR] [--svg]
  crowd-bench cache-experiment [--agents N] [--seed N] [--out DIR]

Omitting --scene runs every scene."
}

struct Args {
    scene: Option<String>,
    agents: u32,
    seed: u64,
    svg: bool,
    frames: bool,
    frame_interval: u64,
    out: PathBuf,
    solver: crate::report::SolverKind,
    trace: bool,
}

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let mut args = Args {
        scene: None,
        agents: DEFAULT_AGENTS,
        seed: DEFAULT_SEED,
        svg: false,
        frames: false,
        frame_interval: frames::DEFAULT_FRAME_INTERVAL_TICKS,
        out: PathBuf::from(REPORT_DIR),
        solver: crate::report::SolverKind::SampledVelocity,
        trace: false,
    };
    let mut index = 0;
    while index < raw.len() {
        match raw[index].as_str() {
            "--scene" => {
                index += 1;
                args.scene = Some(raw.get(index).ok_or("--scene needs a value")?.clone());
            }
            "--agents" => {
                index += 1;
                args.agents = raw
                    .get(index)
                    .ok_or("--agents needs a value")?
                    .parse()
                    .map_err(|_| "--agents must be a number")?;
            }
            "--seed" => {
                index += 1;
                args.seed = raw
                    .get(index)
                    .ok_or("--seed needs a value")?
                    .parse()
                    .map_err(|_| "--seed must be a number")?;
            }
            "--out" => {
                index += 1;
                args.out = PathBuf::from(raw.get(index).ok_or("--out needs a value")?);
            }
            "--svg" => args.svg = true,
            "--frames" => args.frames = true,
            "--trace" => args.trace = true,
            "--frame-interval" => {
                index += 1;
                args.frame_interval = raw
                    .get(index)
                    .ok_or("--frame-interval needs a value")?
                    .parse()
                    .map_err(|_| "--frame-interval must be a number")?;
            }
            "--solver" => {
                index += 1;
                let name = raw.get(index).ok_or("--solver needs a value")?;
                args.solver = crate::report::SolverKind::parse(name)?;
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        index += 1;
    }
    // A trace samples every tick via its own stepping loop in `write_trace`
    // (see report.rs), which is mutually exclusive with the svg/frames
    // sampling loop. Silently picking one (as the branch order in report.rs
    // otherwise would) means `--trace --svg` writes no trace file and says
    // nothing, so the user asked for a trace and never finds out they didn't
    // get one. Reject the combination up front instead.
    if args.trace && (args.svg || args.frames) {
        return Err("--trace cannot be combined with --svg or --frames".to_string());
    }
    Ok(args)
}

fn scenes_to_run(args: &Args) -> Result<Vec<String>, String> {
    match &args.scene {
        Some(name) => {
            if !scenes::SCENE_NAMES.contains(&name.as_str()) {
                return Err(format!(
                    "unknown scene {name}; known scenes: {}",
                    scenes::SCENE_NAMES.join(", ")
                ));
            }
            Ok(vec![name.clone()])
        }
        None => Ok(scenes::SCENE_NAMES.iter().map(|s| s.to_string()).collect()),
    }
}

fn options_for(scene: &str, args: &Args) -> RunOptions {
    RunOptions {
        scene: scene.to_string(),
        agents: args.agents,
        seed: args.seed,
        svg: args.svg,
        frames: args.frames,
        frame_interval: args.frame_interval,
        out_dir: args.out.clone(),
        solver: args.solver,
        trace: args.trace,
    }
}

fn command_run(args: &Args) -> Result<(), String> {
    std::fs::create_dir_all(&args.out).map_err(|e| e.to_string())?;
    for scene in scenes_to_run(args)? {
        let report = run_scene(&options_for(&scene, args))?;
        let path = args.out.join(format!("{scene}-{}.json", args.agents));
        let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
        println!(
            "{scene}: {} arrived / {} spawned, {} penetrations, {:.2}s wall, {:.0} ticks/s -> {}",
            report.metrics.agents_arrived,
            report.metrics.agents_spawned,
            report.metrics.penetration_pair_ticks,
            report.metrics.wall_time_seconds,
            report.metrics.ticks_per_second_achieved,
            path.display()
        );
    }
    Ok(())
}

fn command_sweep(args: &Args) -> Result<(), String> {
    for scene in scenes_to_run(args)? {
        for agents in [100u32, 500, 1000, 2000] {
            // Never record SVGs, frames, or a trace during a sweep: the
            // per-tick sampling would skew the very timing numbers the
            // sweep exists to measure.
            let sweep_args = Args {
                scene: Some(scene.clone()),
                agents,
                seed: args.seed,
                svg: false,
                frames: false,
                frame_interval: args.frame_interval,
                out: args.out.clone(),
                solver: args.solver,
                trace: false,
            };
            let report = run_scene(&options_for(&scene, &sweep_args))?;
            println!(
                "{scene},{agents},{:.4},{:.1},{}",
                report.metrics.wall_time_seconds,
                report.metrics.ticks_per_second_achieved,
                report.metrics.peak_allocated_bytes
            );
        }
    }
    Ok(())
}

fn command_baseline(args: &Args) -> Result<(), String> {
    std::fs::create_dir_all(BASELINE_DIR).map_err(|e| e.to_string())?;
    for scene in scenes_to_run(args)? {
        let report = run_scene(&options_for(&scene, args))?;
        let baseline = baseline::from_report(&report);
        let path = PathBuf::from(BASELINE_DIR).join(format!("{scene}.json"));
        let json = serde_json::to_string_pretty(&baseline).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
        println!("wrote {}", path.display());
    }
    Ok(())
}

fn command_check(args: &Args) -> Result<bool, String> {
    let mut all_passed = true;
    for scene in scenes_to_run(args)? {
        let path = PathBuf::from(BASELINE_DIR).join(format!("{scene}.json"));
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let stored: baseline::Baseline =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;

        // Replay with the solver the caller actually asked for via --solver
        // (defaulting like `run`/`baseline` to sampled_velocity), NOT the
        // stored baseline's own solver field. Using the stored solver here
        // would make `report.solver` equal `stored.solver` by construction,
        // which makes `comparison.solver_mismatch` below unreachable outside
        // its unit test. Comparing against what was actually requested is
        // what lets a `--solver` typo (or an intentional cross-solver check)
        // be caught instead of silently passing.
        let report = run_scene(&options_for(
            &scene,
            &Args {
                scene: Some(scene.clone()),
                agents: stored.agents,
                seed: stored.seed,
                svg: false,
                frames: false,
                frame_interval: args.frame_interval,
                out: args.out.clone(),
                solver: args.solver,
                trace: false,
            },
        ))?;

        let comparison = baseline::compare(&stored, &report);
        if comparison.passed {
            println!("{scene}: OK");
        } else {
            all_passed = false;
            println!("{scene}: DRIFT");
            if let Some((baseline_solver, current_solver)) = &comparison.solver_mismatch {
                println!(
                    "  solver: baseline {baseline_solver}, now {current_solver} (not comparable)"
                );
            }
            for drift in &comparison.drifts {
                println!(
                    "  {}: baseline {}, now {} (tolerance {:.0}%)",
                    drift.metric,
                    drift.baseline,
                    drift.current,
                    drift.tolerance * 100.0
                );
            }
        }
    }
    Ok(all_passed)
}

const COMPARE_SOLVERS: [(&str, crate::report::SolverKind); 3] = [
    (
        "sampled_velocity",
        crate::report::SolverKind::SampledVelocity,
    ),
    ("orca", crate::report::SolverKind::Orca),
    ("anticipatory", crate::report::SolverKind::Anticipatory),
];
const COMPARE_SCALES: [u32; 4] = [100, 500, 1000, 2000];

fn command_compare(args: &Args) -> Result<(), String> {
    std::fs::create_dir_all(&args.out).map_err(|e| e.to_string())?;
    let mut reports = Vec::new();
    println!(
        "scene,agents,solver,completion_rate,mean_time_to_collision,penetration_pair_ticks,ticks_per_second_achieved,peak_allocated_bytes"
    );
    for scene in scenes_to_run(args)? {
        for &(_, solver) in &COMPARE_SOLVERS {
            for &agents in &COMPARE_SCALES {
                let options = RunOptions {
                    scene: scene.clone(),
                    agents,
                    seed: args.seed,
                    svg: false,
                    frames: false,
                    frame_interval: args.frame_interval,
                    out_dir: args.out.clone(),
                    solver,
                    trace: false,
                };
                let report = run_scene(&options)?;
                println!(
                    "{},{agents},{},{:.3},{:.2},{},{},{}",
                    report.scene,
                    report.solver,
                    report.metrics.completion_rate,
                    report.metrics.mean_time_to_collision,
                    report.metrics.penetration_pair_ticks,
                    report.metrics.ticks_per_second_achieved as u64,
                    report.metrics.peak_allocated_bytes,
                );
                reports.push(report);
            }
        }
    }
    let date = reports
        .first()
        .map(|r| r.environment.captured_at[..10].to_string())
        .unwrap_or_else(|| "unknown".to_string());
    // When chunking by --scene, each chunk must get its own file: they all
    // share the same date, so without the scene in the name a second chunk
    // run the same day would silently overwrite the first chunk's JSON.
    let path = match &args.scene {
        Some(scene) => args.out.join(format!("compare-{scene}-{date}.json")),
        None => args.out.join(format!("compare-{date}.json")),
    };
    let json = serde_json::to_string_pretty(&reports).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    println!("wrote {}", path.display());
    Ok(())
}

fn command_nav_reroute(args: &Args) -> Result<(), String> {
    std::fs::create_dir_all(&args.out).map_err(|e| e.to_string())?;
    let options = nav_bench::NavRerouteOptions {
        agents: args.agents,
        seed: args.seed,
        out_dir: args.out.clone(),
        svg: args.svg,
        settle_ticks: 600,
    };
    let report = nav_bench::run_nav_reroute(&options)?;
    let path = args
        .out
        .join(format!("two_room-reroute-{}.json", args.agents));
    let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    println!(
        "two_room: {} invalidated / {} untouched on close, {} of the invalidated crossed \
         north_door, {} arrived after reroute -> {}",
        report.invalidated_on_close,
        report.untouched_on_close,
        report.crossed_north_door,
        report.arrived_after_reroute,
        path.display()
    );
    Ok(())
}

fn command_cache_experiment(args: &Args) -> Result<(), String> {
    let report = run_experiment(&ExperimentOptions {
        agents: args.agents,
        frames: 120,
        seed: args.seed,
        out_dir: args.out.clone(),
    })?;
    write_report(&report, &args.out)?;
    println!(
        "cache experiment: {} candidates; selected {} with {}-tick chunks -> {}",
        report.results.len(),
        report.selected.position_encoding,
        report.selected.chunk_ticks,
        args.out.join("report.json").display()
    );
    Ok(())
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = argv.split_first() else {
        eprintln!("{}", usage());
        return ExitCode::FAILURE;
    };

    let args = match parse_args(rest) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}\n\n{}", usage());
            return ExitCode::FAILURE;
        }
    };

    let outcome = match command.as_str() {
        "run" => command_run(&args).map(|()| true),
        "sweep" => command_sweep(&args).map(|()| true),
        "baseline" => command_baseline(&args).map(|()| true),
        "check" => command_check(&args),
        "compare" => command_compare(&args).map(|()| true),
        "nav-reroute" => command_nav_reroute(&args).map(|()| true),
        "cache-experiment" => command_cache_experiment(&args).map(|()| true),
        other => Err(format!("unknown command: {other}\n\n{}", usage())),
    };

    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        // A drift is a reportable result, not a crash; the distinct exit code
        // lets CI treat it differently from a broken build.
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn trace_combined_with_svg_is_rejected() {
        let error = match parse_args(&strings(&["--trace", "--svg"])) {
            Err(error) => error,
            Ok(_) => panic!("expected --trace --svg to be rejected"),
        };
        assert!(
            error.contains("--trace") && error.contains("--svg"),
            "unhelpful error: {error}"
        );
    }

    #[test]
    fn trace_combined_with_frames_is_rejected() {
        let error = match parse_args(&strings(&["--trace", "--frames"])) {
            Err(error) => error,
            Ok(_) => panic!("expected --trace --frames to be rejected"),
        };
        assert!(
            error.contains("--trace") && error.contains("--frames"),
            "unhelpful error: {error}"
        );
    }

    #[test]
    fn trace_alone_is_accepted() {
        let args = parse_args(&strings(&["--trace"])).unwrap();
        assert!(args.trace);
        assert!(!args.svg);
        assert!(!args.frames);
    }
}
