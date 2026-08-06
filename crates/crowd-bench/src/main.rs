//! Benchmark runner.
//!
//! Argument parsing is hand-rolled to keep the dependency set at serde alone;
//! the surface is small enough that a parser crate would cost more than it
//! saves.

mod alloc;
mod baseline;
mod report;
mod svg;

use std::path::PathBuf;
use std::process::ExitCode;

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
  crowd-bench run [--scene NAME] [--agents N] [--seed N] [--svg] [--out DIR] [--solver NAME]
  crowd-bench sweep [--scene NAME] [--seed N]
  crowd-bench baseline [--scene NAME] [--agents N] [--seed N] [--solver NAME]
  crowd-bench check [--agents N] [--seed N] [--solver NAME]
  crowd-bench compare [--out DIR]

Omitting --scene runs every scene."
}

struct Args {
    scene: Option<String>,
    agents: u32,
    seed: u64,
    svg: bool,
    out: PathBuf,
    solver: crate::report::SolverKind,
}

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let mut args = Args {
        scene: None,
        agents: DEFAULT_AGENTS,
        seed: DEFAULT_SEED,
        svg: false,
        out: PathBuf::from(REPORT_DIR),
        solver: crate::report::SolverKind::SampledVelocity,
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
            "--solver" => {
                index += 1;
                let name = raw.get(index).ok_or("--solver needs a value")?;
                args.solver = crate::report::SolverKind::parse(name)?;
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        index += 1;
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
        out_dir: args.out.clone(),
        solver: args.solver,
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
            // Never record SVGs during a sweep: the per-tick sampling would
            // skew the very timing numbers the sweep exists to measure.
            let sweep_args = Args {
                scene: Some(scene.clone()),
                agents,
                seed: args.seed,
                svg: false,
                out: args.out.clone(),
                solver: args.solver,
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

        // Replay against the solver the baseline was captured with, not
        // whatever --solver this invocation passed: `check` verifies the
        // stored numbers still hold for their own solver, and a mismatched
        // solver is reported distinctly below rather than silently swapped in.
        let stored_solver = crate::report::SolverKind::parse(&stored.solver)?;
        let report = run_scene(&options_for(
            &scene,
            &Args {
                scene: Some(scene.clone()),
                agents: stored.agents,
                seed: stored.seed,
                svg: false,
                out: args.out.clone(),
                solver: stored_solver,
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
    for scene in scenes::SCENE_NAMES {
        for &(_, solver) in &COMPARE_SOLVERS {
            for &agents in &COMPARE_SCALES {
                let options = RunOptions {
                    scene: scene.to_string(),
                    agents,
                    seed: args.seed,
                    svg: false,
                    out_dir: args.out.clone(),
                    solver,
                };
                let report = run_scene(&options)?;
                println!(
                    "{},{},{agents},{:.3},{:.2},{},{},{}",
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
    let path = args.out.join(format!("compare-{date}.json"));
    let json = serde_json::to_string_pretty(&reports).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    println!("wrote {}", path.display());
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
