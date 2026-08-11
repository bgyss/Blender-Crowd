#!/usr/bin/env python3
"""Run the complete M0 acceptance gate and retain machine-readable evidence."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
import glob
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SUMMARY = REPO_ROOT / "docs" / "benchmarks" / "2026-08-10-m0-acceptance.json"
DEFAULT_LOG_DIR = REPO_ROOT / "benchmarks" / "reports" / "m0-acceptance-logs"


@dataclass(frozen=True)
class Gate:
    id: str
    description: str
    command: str
    evidence: tuple[str, ...] = ()


GATES = (
    Gate(
        "workspace_tests",
        "Workspace tests, with the four density cases deferred to the release gate",
        "cargo test --workspace -- "
        "--skip no_agent_state_goes_non_finite_under_density "
        "--skip no_agent_escapes_far_beyond_the_scene_bounds "
        "--skip speeds_never_exceed_the_per_agent_maximum "
        "--skip the_crowd_does_not_deadlock_wholesale",
    ),
    Gate(
        "release_density",
        "Full release-mode density stress suite",
        "cargo test --release -p crowd-core --test fuzz_density",
    ),
    Gate(
        "release_two_room",
        "1,000-agent timed-portal reroute acceptance",
        "cargo test --release -p crowd-core --test two_room_reroute -- --ignored --nocapture",
    ),
    Gate(
        "solver_baselines",
        "All six selected-solver baseline regressions at their recorded scales",
        "cargo run --release -p crowd-bench -- check --agents 1000",
        ("benchmarks/baselines/*.json",),
    ),
    Gate(
        "cache_lifecycle",
        "Cache manifest, codec, cancellation, corruption, and sequential-read tests",
        "cargo test -p crowd-cache",
    ),
    Gate(
        "cache_experiment",
        "Measured 1,000-agent cache encoding/chunk matrix",
        "scripts/cache-experiment.sh",
        (
            "docs/benchmarks/2026-08-10-cache-v0-experiment.json",
            "docs/benchmarks/2026-08-10-cache-v0-experiment.md",
        ),
    ),
    Gate(
        "wheel_facade",
        "abi3 wheel build plus bare-CPython coarse-facade verification",
        "scripts/build-wheel.sh && scripts/verify-wheel.sh",
        ("addon/blender_crowd/wheels/*.whl",),
    ),
    Gate(
        "blender_clean_install",
        "Clean Blender extension install and native-module load",
        "scripts/blender-install-test.sh",
        ("dist/blender_crowd-*.zip",),
    ),
    Gate(
        "blender_playback",
        "Fresh-process 1,000-point Geometry Nodes playback",
        "scripts/blender-playback-test.sh",
        ("benchmarks/reports/crossing-1000.crowdtrace",),
    ),
    Gate(
        "static_checks",
        "Formatting, lint, documentation structure, and runner contract checks",
        "cargo fmt --check && "
        "cargo clippy --workspace --all-targets -- -D warnings && "
        "python3 tests/test_m0_acceptance_runner.py && "
        "git diff --check && "
        "rg '^## ' docs/blender-crowd-1.0.md && "
        "rg '^#' docs/milestones/*.md",
    ),
)


TEST_GATES = (
    Gate("success_probe", "Acceptance-runner success probe", "python3 -c 'print(\"probe ok\")'"),
    Gate("failure_probe", "Acceptance-runner failure probe", "python3 -c 'raise SystemExit(23)'"),
    Gate("unreached_probe", "This gate must not run", "python3 -c 'raise SystemExit(99)'"),
)


def run_capture(*command: str) -> str:
    try:
        completed = subprocess.run(
            command,
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError:
        return "unavailable"
    if completed.returncode != 0:
        return "unavailable"
    return completed.stdout.strip() or completed.stderr.strip() or "unknown"


def file_hash(path: Path) -> str:
    digest = hashlib.blake2b(digest_size=32)
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def repository_hash(summary_path: Path) -> str:
    listed = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
    ).stdout
    excluded = {summary_path.resolve()}
    digest = hashlib.blake2b(digest_size=32)
    for raw_path in sorted(part for part in listed.split(b"\0") if part):
        relative = Path(os.fsdecode(raw_path))
        path = (REPO_ROOT / relative).resolve()
        if path in excluded or not path.is_file():
            continue
        digest.update(raw_path)
        digest.update(b"\0")
        digest.update(bytes.fromhex(file_hash(path)))
    return digest.hexdigest()


def input_hashes() -> dict[str, str]:
    candidates = [
        Path("Cargo.lock"),
        Path("schemas/cache-manifest-v1.schema.json"),
        Path("assets/reference/concourse-project-v1.json"),
        *sorted(Path("benchmarks/baselines").glob("*.json")),
    ]
    return {
        path.as_posix(): file_hash(REPO_ROOT / path)
        for path in candidates
        if (REPO_ROOT / path).is_file()
    }


def collect_environment(summary_path: Path, test_mode: bool) -> dict[str, Any]:
    git_status = run_capture("git", "status", "--short")
    environment: dict[str, Any] = {
        "os": platform.platform(),
        "machine": platform.machine(),
        "cpu": run_capture("sysctl", "-n", "machdep.cpu.brand_string"),
        "ram_bytes": run_capture("sysctl", "-n", "hw.memsize"),
        "python": platform.python_version(),
        "rustc": run_capture("rustc", "--version"),
        "cargo": run_capture("cargo", "--version"),
        "git_head": run_capture("git", "rev-parse", "HEAD"),
        "git_dirty": git_status not in ("", "unknown"),
        "repository_hash_blake2b_256": repository_hash(summary_path),
        "input_hashes_blake2b_256": input_hashes(),
    }
    if not test_mode:
        blender = os.environ.get(
            "BLENDER", "/Applications/Blender.app/Contents/MacOS/Blender"
        )
        environment["blender"] = run_capture(blender, "--version").splitlines()[0]
    else:
        environment["blender"] = "test-mode"
    return environment


def evidence_matches(pattern: str) -> list[str]:
    return [
        Path(match).resolve().relative_to(REPO_ROOT).as_posix()
        for match in sorted(glob.glob(str(REPO_ROOT / pattern)))
        if Path(match).is_file()
    ]


def run_gate(gate: Gate, log_dir: Path) -> dict[str, Any]:
    log_path = log_dir / f"{gate.id}.log"
    print(f"\n== {gate.id}: {gate.description} ==", flush=True)
    print(f"$ {gate.command}", flush=True)
    started_at = datetime.now(timezone.utc)
    start = time.perf_counter()

    process = subprocess.Popen(
        ["bash", "-c", gate.command],
        cwd=REPO_ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
    )
    with log_path.open("w", encoding="utf-8") as log:
        assert process.stdout is not None
        for line in process.stdout:
            print(line, end="", flush=True)
            log.write(line)
    exit_code = process.wait()

    found_evidence: list[str] = []
    missing_evidence: list[str] = []
    if exit_code == 0:
        for pattern in gate.evidence:
            matches = evidence_matches(pattern)
            if matches:
                found_evidence.extend(matches)
            else:
                missing_evidence.append(pattern)
        if missing_evidence:
            exit_code = 90
            message = "missing required evidence: " + ", ".join(missing_evidence)
            print(message, flush=True)
            with log_path.open("a", encoding="utf-8") as log:
                log.write(message + "\n")

    duration = time.perf_counter() - start
    status = "passed" if exit_code == 0 else "failed"
    print(f"{gate.id}: {status.upper()} ({duration:.3f}s)", flush=True)
    return {
        "id": gate.id,
        "description": gate.description,
        "command": gate.command,
        "status": status,
        "exit_code": exit_code,
        "started_at": started_at.isoformat(),
        "duration_seconds": round(duration, 6),
        "log": log_path.resolve().relative_to(REPO_ROOT).as_posix(),
        "evidence": found_evidence,
        "missing_evidence": missing_evidence,
    }


def write_summary(path: Path, summary: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    os.replace(temporary, path)


def usage() -> str:
    return "usage: scripts/m0-acceptance.sh [--list]"


def main(argv: list[str]) -> int:
    if argv == ["--list"]:
        for gate in GATES:
            print(f"{gate.id}\t{gate.command}")
        return 0
    if argv:
        print(usage(), file=sys.stderr)
        return 2

    test_mode = os.environ.get("M0_ACCEPTANCE_TEST_MODE") == "1"
    gates = TEST_GATES if test_mode else GATES
    summary_path = Path(os.environ.get("M0_ACCEPTANCE_SUMMARY", DEFAULT_SUMMARY)).resolve()
    log_dir = Path(os.environ.get("M0_ACCEPTANCE_LOG_DIR", DEFAULT_LOG_DIR)).resolve()
    log_dir.mkdir(parents=True, exist_ok=True)

    started_at = datetime.now(timezone.utc)
    start = time.perf_counter()
    summary: dict[str, Any] = {
        "schema_version": 1,
        "milestone": "M0",
        "status": "running",
        "first_failure": None,
        "started_at": started_at.isoformat(),
        "finished_at": None,
        "duration_seconds": None,
        "all_gates_executed": False,
        "environment": collect_environment(summary_path, test_mode),
        "steps": [],
    }
    write_summary(summary_path, summary)

    for gate in gates:
        result = run_gate(gate, log_dir)
        summary["steps"].append(result)
        if result["exit_code"] != 0:
            summary["status"] = "failed"
            summary["first_failure"] = gate.id
            break
    else:
        summary["status"] = "passed"
        summary["all_gates_executed"] = True

    summary["finished_at"] = datetime.now(timezone.utc).isoformat()
    summary["duration_seconds"] = round(time.perf_counter() - start, 6)
    write_summary(summary_path, summary)
    print(f"\nM0 acceptance: {summary['status'].upper()}", flush=True)
    print(f"summary: {summary_path}", flush=True)
    if summary["first_failure"]:
        print(f"first failure: {summary['first_failure']}", flush=True)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
