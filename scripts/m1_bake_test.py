#!/usr/bin/env python3
"""Run the complete headless M1 bake/cache acceptance path."""

from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
BENCH = REPO_ROOT / "target" / "release" / "crowd-bench"


def run(command: list[str]) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        command,
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode:
        raise RuntimeError(
            f"command failed ({result.returncode}): {' '.join(command)}\n"
            f"{result.stdout}{result.stderr}"
        )
    return result


def run_json(arguments: list[str]) -> dict:
    result = run([str(BENCH), "m1", *arguments])
    return json.loads(result.stdout)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def verify_bake(report: dict) -> None:
    require(report["agent_count"] == 1000, "bake did not contain exactly 1,000 agents")
    require(report["unique_agent_ids"] == 1000, "stable agent IDs are not unique")
    require(report["destination_completion"] >= 0.95, "destination completion is below 95%")
    require(report["static_boundary_escapes"] == 0, "an agent crossed a static boundary")
    require(report["portal_reroute"]["accepted"], "portal reroute gate failed")
    require(not report["required_channels_missing"], "required cache channels are missing")
    require(
        report["position_quantization_bound_m"] <= 0.001,
        "position quantization bound exceeds 1 mm",
    )


def exercise(output: Path) -> dict:
    output.mkdir(parents=True, exist_ok=True)
    first_cache = output / "first.crowd"
    second_cache = output / "second.crowd"
    canceled_cache = output / "canceled.crowd"
    for path in (first_cache, second_cache, canceled_cache):
        require(not path.exists(), f"refusing to overwrite existing cache: {path}")

    validation = run_json(["validate"])
    require(validation["valid"], "reference project validation failed")

    first = run_json(["bake", "--cache", str(first_cache)])
    second = run_json(["bake", "--cache", str(second_cache)])
    verify_bake(first)
    verify_bake(second)

    comparison = run_json(
        ["compare", "--first", str(first_cache), "--second", str(second_cache)]
    )
    require(comparison["accepted"], "strict cache comparison failed")
    require(
        first["discrete_digest"] == second["discrete_digest"],
        "strict discrete digests differ",
    )

    cancellation = run_json(
        ["cancel", "--cache", str(canceled_cache), "--after-ticks", "137"]
    )
    require(cancellation["status"] == "canceled", "canceled bake reported another state")
    require(
        cancellation["complete_reader_rejected"],
        "complete reader accepted a canceled cache",
    )

    inspection = run_json(
        [
            "inspect-agent",
            "--cache",
            str(first_cache),
            "--agent-id",
            str(first["selected_agent_id"]),
            "--tick",
            str(first["selected_agent_tick"]),
        ]
    )
    required_evidence = {
        "position",
        "desired_velocity",
        "solved_velocity",
        "corridor_portal_ids",
        "corridor_points",
        "next_target",
        "destination_id",
        "path_status",
        "commuter_state",
        "clip_id",
        "clip_phase",
        "playback_rate",
        "relevant_portals",
        "decision_code",
        "decision_reason",
    }
    require(
        required_evidence <= inspection.keys(),
        "selected-agent evidence is incomplete",
    )

    report = {
        "schema_version": 1,
        "validation": validation,
        "first_bake": first,
        "second_bake": second,
        "comparison": comparison,
        "cancellation": cancellation,
        "selected_agent": inspection,
    }
    report_path = output / "report.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return report


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--out",
        type=Path,
        help="preserve caches and report under this new/empty directory",
    )
    arguments = parser.parse_args()

    run(["cargo", "build", "--release", "-p", "crowd-bench"])
    if arguments.out:
        report = exercise(arguments.out.resolve())
        print(json.dumps(report, indent=2))
    else:
        with tempfile.TemporaryDirectory(prefix="blender-crowd-m1-") as temporary:
            report = exercise(Path(temporary))
            print(json.dumps(report, indent=2))
    print("M1 headless bake acceptance: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
