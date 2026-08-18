#!/usr/bin/env python3
"""Evaluate reference motion metadata and fit an explainable profile.

The result is an offline critic/report. It never changes the behavior graph or
authoritative runtime policy and never treats a fitted profile as learned
runtime control.
"""

import hashlib
import json
import sys
from pathlib import Path


def evaluate_database(database):
    if database.get("schema_version") != 1:
        raise ValueError("unsupported motion database schema version")
    provenance = database.get("source_provenance")
    if not isinstance(provenance, str) or not provenance.strip():
        raise ValueError("motion database source provenance is required")
    if not database.get("database_id") or not database.get("retarget_profile_id"):
        raise ValueError("motion database ID and retarget profile are required")
    canonical_clips = []
    speeds = []
    slopes = []
    contacts = 0
    sample_count = 0
    for clip in sorted(database.get("clips", []), key=lambda item: item.get("id", "")):
        clip_id = clip.get("id")
        if not isinstance(clip_id, str) or not clip_id:
            raise ValueError("every motion clip needs an ID")
        samples = []
        for sample in sorted(clip.get("samples", []), key=lambda item: item.get("tick", 0)):
            velocity = sample.get("velocity_millimeters_per_second", sample.get("velocity"))
            if not isinstance(velocity, list) or len(velocity) != 2:
                raise ValueError("motion samples need a two-axis velocity")
            speed = (int(velocity[0]) ** 2 + int(velocity[1]) ** 2) ** 0.5
            speeds.append(speed)
            slopes.append(abs(int(sample.get("slope_millionths", 0))))
            if sample.get("contact", "none") != "none":
                contacts += 1
            sample_count += 1
            samples.append(
                {
                    "tick": int(sample.get("tick", 0)),
                    "velocity": [int(velocity[0]), int(velocity[1])],
                    "contact": str(sample.get("contact", "none")),
                    "slope_millionths": int(sample.get("slope_millionths", 0)),
                }
            )
        canonical_clips.append({"id": clip_id, "samples": samples})
    if not sample_count:
        raise ValueError("motion database has no samples to evaluate")
    canonical = {
        "schema_version": 1,
        "database_id": database["database_id"],
        "retarget_profile_id": database["retarget_profile_id"],
        "source_provenance": provenance,
        "clips": canonical_clips,
    }
    source_hash = hashlib.sha256(
        json.dumps(canonical, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()
    mean_speed_mmps = sum(speeds) / len(speeds)
    mean_slope = round(sum(slopes) / len(slopes))
    return {
        "schema_version": 1,
        "database_id": database["database_id"],
        "retarget_profile_id": database["retarget_profile_id"],
        "source_provenance": provenance,
        "source_hash": source_hash,
        "sample_count": sample_count,
        "clip_count": len(canonical_clips),
        "mean_speed_mps": mean_speed_mmps / 1000.0,
        "mean_slope_millionths": mean_slope,
        "contact_fraction_millionths": round(contacts * 1_000_000 / sample_count),
        "fitted_profile": {
            "preferred_speed_mps": mean_speed_mmps / 1000.0,
            "jog_threshold_mps": max(1.8, mean_speed_mmps / 1000.0 * 1.35),
            "confidence_millionths": min(1_000_000, sample_count * 250_000),
            "authority": "deterministic-graph-and-clip-state",
        },
    }


def main(argv=None):
    args = list(sys.argv[1:] if argv is None else argv)
    if len(args) != 2:
        print("usage: m6_motion_evaluate.py DATABASE.json REPORT.json", file=sys.stderr)
        return 2
    source, output = map(Path, args)
    try:
        with source.open(encoding="utf-8") as handle:
            report = evaluate_database(json.load(handle))
        output.parent.mkdir(parents=True, exist_ok=True)
        temporary = output.with_suffix(output.suffix + ".tmp")
        temporary.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        temporary.replace(output)
    except (OSError, TypeError, ValueError, json.JSONDecodeError) as error:
        print(str(error), file=sys.stderr)
        return 1
    print("wrote {}".format(output))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
