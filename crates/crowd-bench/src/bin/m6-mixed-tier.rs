use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crowd_bench::m6_mixed_tier::{run_fixture, MixedTierFixture};

fn usage() -> &'static str {
    "usage: m6-mixed-tier --out REPORT.json"
}

fn parse_args(args: &[String]) -> Result<PathBuf, String> {
    if args.len() == 2 && args[0] == "--out" && !args[1].is_empty() {
        Ok(PathBuf::from(&args[1]))
    } else {
        Err(usage().to_owned())
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;
    std::fs::rename(&temporary, path)
        .map_err(|error| format!("cannot replace {}: {error}", path.display()))
}

fn main() -> ExitCode {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let out = match parse_args(&args) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let report = match run_fixture(&MixedTierFixture::checked_10k()) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("M6 mixed-tier run failed: {error}");
            return ExitCode::from(2);
        }
    };
    let bytes = match serde_json::to_vec_pretty(&report) {
        Ok(mut bytes) => {
            bytes.push(b'\n');
            bytes
        }
        Err(error) => {
            eprintln!("M6 mixed-tier report serialization failed: {error}");
            return ExitCode::from(2);
        }
    };
    if let Err(error) = write_atomic(&out, &bytes) {
        eprintln!("{error}");
        return ExitCode::from(2);
    }
    println!(
        "M6 mixed-tier: {} agents, {:.3} ticks/s, replay {} -> {}",
        report.agent_count,
        report.ticks_per_second,
        report.deterministic_replay_hash,
        out.display()
    );
    if report.passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
