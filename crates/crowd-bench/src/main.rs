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
  crowd-bench run [--scene NAME] [--agents N] [--seed N] [--svg] [--out DIR]
  crowd-bench sweep [--scene NAME] [--seed N]
  crowd-bench baseline [--scene NAME] [--agents N] [--seed N]
  crowd-bench check [--agents N] [--seed N]

Omitting --scene runs every scene."
}

struct Args {
    scene: Option<String>,
    agents: u32,
    seed: u64,
    svg: bool,
    out: PathBuf,
}

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let mut args = Args {
        scene: None,
        agents: DEFAULT_AGENTS,
        seed: DEFAULT_SEED,
        svg: false,
        out: PathBuf::from(REPORT_DIR),
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

        let report = run_scene(&options_for(
            &scene,
            &Args {
                scene: Some(scene.clone()),
                agents: stored.agents,
                seed: stored.seed,
                svg: false,
                out: args.out.clone(),
            },
        ))?;

        let comparison = baseline::compare(&stored, &report);
        if comparison.passed {
            println!("{scene}: OK");
        } else {
            all_passed = false;
            println!("{scene}: DRIFT");
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
