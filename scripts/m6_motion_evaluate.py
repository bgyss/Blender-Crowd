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


MAXIMUM_METRICS = (
    "max_root_speed_error_millimeters_per_second",
    "max_foot_slide_millimeters",
    "max_trajectory_deviation_millimeters",
    "max_turn_discontinuity_microradians",
    "rejected_frame_rate_ppm",
)
HARD_METRICS = (
    "root_teleportations",
    "undeclared_contacts",
    "source_hash_drift",
    "cross_cache_mutations",
    "joint_limit_violations",
)
SUM_METRICS = HARD_METRICS + ("retarget_failures", "rejected_frames", "parsed_frames")
SOFT_METRICS = (
    "max_foot_slide_millimeters",
    "max_trajectory_deviation_millimeters",
    "max_turn_discontinuity_microradians",
    "rejected_frame_rate_ppm",
)


def _validated_metrics(clip):
    metrics = clip.get("metrics")
    if metrics is None:
        return None
    expected = set(MAXIMUM_METRICS + SUM_METRICS)
    if not isinstance(metrics, dict) or set(metrics) != expected:
        raise ValueError("motion clip {} metrics do not match the M6 evidence contract".format(clip.get("id", "<unknown>")))
    for key, value in metrics.items():
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            raise ValueError("motion clip metric {} must be a non-negative integer".format(key))
    return {key: metrics[key] for key in MAXIMUM_METRICS + SUM_METRICS}


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
    clip_metrics = []
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
        normalized_clip = {"id": clip_id, "samples": samples}
        metrics = _validated_metrics(clip)
        if metrics is not None:
            normalized_clip["metrics"] = metrics
            clip_metrics.append({"id": clip_id, **metrics})
        canonical_clips.append(normalized_clip)
    if not sample_count:
        raise ValueError("motion database has no samples to evaluate")
    canonical = {
        "schema_version": 1,
        "database_id": database["database_id"],
        "retarget_profile_id": database["retarget_profile_id"],
        "source_provenance": provenance,
        "clips": canonical_clips,
    }
    source_hashes = database.get("source_hashes")
    if source_hashes is not None:
        if not isinstance(source_hashes, dict) or not source_hashes:
            raise ValueError("motion database source hashes must be a non-empty object")
        for identity, digest in source_hashes.items():
            if not isinstance(identity, str) or not identity or not isinstance(digest, str) or len(digest) != 64:
                raise ValueError("motion database source hashes are invalid")
        canonical["source_hashes"] = dict(sorted(source_hashes.items()))
    if "source_manifest_id" in database:
        canonical["source_manifest_id"] = database["source_manifest_id"]
    source_hash = hashlib.sha256(
        json.dumps(canonical, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()
    mean_speed_mmps = sum(speeds) / len(speeds)
    mean_slope = round(sum(slopes) / len(slopes))
    report = {
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
    if source_hashes is not None:
        report["source_hashes"] = canonical["source_hashes"]
        report["source_manifest_id"] = canonical.get("source_manifest_id")
    if clip_metrics:
        quality_metrics = {
            **{key: max(metrics[key] for metrics in clip_metrics) for key in MAXIMUM_METRICS},
            **{key: sum(metrics[key] for metrics in clip_metrics) for key in SUM_METRICS},
        }
        report["clip_metrics"] = clip_metrics
        report["quality_metrics"] = quality_metrics
        report["hard_limit_observations"] = {key: quality_metrics[key] for key in HARD_METRICS}
        report["threshold_baseline"] = {key: quality_metrics[key] for key in SOFT_METRICS}
    return report


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
