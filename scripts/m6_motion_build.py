#!/usr/bin/env python3
"""Build a deterministic, provenance-only M6 motion database report.

This tool records clip identity and feature metadata. It does not download,
copy, retarget, or redistribute motion assets; callers must provide a reviewed
source-provenance string and keep the actual source terms outside this report.
"""

import hashlib
import json
import sys
from pathlib import Path


SCHEMA_VERSION = 1
MAXIMUM_METRICS = (
    "max_root_speed_error_millimeters_per_second",
    "max_foot_slide_millimeters",
    "max_trajectory_deviation_millimeters",
    "max_turn_discontinuity_microradians",
    "rejected_frame_rate_ppm",
)
SUM_METRICS = (
    "joint_limit_violations",
    "retarget_failures",
    "rejected_frames",
    "parsed_frames",
    "root_teleportations",
    "undeclared_contacts",
    "source_hash_drift",
    "cross_cache_mutations",
)


def _validated_metrics(clip):
    metrics = clip.get("metrics")
    if metrics is None:
        return None
    if not isinstance(metrics, dict):
        raise ValueError("motion clip {} metrics must be an object".format(clip["id"]))
    expected = set(MAXIMUM_METRICS + SUM_METRICS)
    if set(metrics) != expected:
        raise ValueError("motion clip {} metrics do not match the M6 evidence contract".format(clip["id"]))
    normalized = {}
    for key in MAXIMUM_METRICS + SUM_METRICS:
        value = metrics[key]
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            raise ValueError("motion clip {} metric {} must be a non-negative integer".format(clip["id"], key))
        normalized[key] = value
    return normalized


def build_database(database):
    if database.get("schema_version") != SCHEMA_VERSION:
        raise ValueError("unsupported trajectory database schema version")
    for field in ("database_id", "retarget_profile_id", "source_provenance"):
        if not isinstance(database.get(field), str) or not database[field]:
            raise ValueError("motion database requires {} provenance metadata".format(field))
    clips = database.get("clips")
    if not isinstance(clips, list):
        raise ValueError("motion database clips must be a list")
    normalized = []
    seen = set()
    for clip in clips:
        if not isinstance(clip, dict) or not isinstance(clip.get("id"), str) or not clip["id"]:
            raise ValueError("every motion clip requires a non-empty ID")
        if clip["id"] in seen:
            raise ValueError("duplicate motion clip ID {}".format(clip["id"]))
        seen.add(clip["id"])
        samples = clip.get("samples", [])
        if not isinstance(samples, list):
            raise ValueError("motion clip {} samples must be a list".format(clip["id"]))
        normalized_clip = {
            "id": clip["id"],
            "sample_count": len(samples),
            "feature_channels": ["future_velocity", "contact", "slope"],
        }
        metrics = _validated_metrics(clip)
        if metrics is not None:
            normalized_clip["metrics"] = metrics
        normalized.append(normalized_clip)
    normalized.sort(key=lambda clip: clip["id"])
    canonical = {
        "schema_version": SCHEMA_VERSION,
        "database_id": database["database_id"],
        "retarget_profile_id": database["retarget_profile_id"],
        "source_provenance": database["source_provenance"],
        "clips": normalized,
    }
    if "source_manifest_id" in database:
        if not isinstance(database["source_manifest_id"], str) or not database["source_manifest_id"]:
            raise ValueError("motion database source manifest ID must be non-empty")
        canonical["source_manifest_id"] = database["source_manifest_id"]
    if "source_hashes" in database:
        hashes = database["source_hashes"]
        if not isinstance(hashes, dict) or not hashes:
            raise ValueError("motion database source hashes must be a non-empty object")
        for identity, digest in hashes.items():
            if not isinstance(identity, str) or not identity or not isinstance(digest, str) or len(digest) != 64:
                raise ValueError("motion database source hashes are invalid")
        canonical["source_hashes"] = dict(sorted(hashes.items()))
    encoded = json.dumps(canonical, sort_keys=True, separators=(",", ":")).encode("utf-8")
    report = {
        **canonical,
        "clip_ids": [clip["id"] for clip in normalized],
        "clip_count": len(normalized),
        "sample_count": sum(clip["sample_count"] for clip in normalized),
        "content_hash": hashlib.sha256(encoded).hexdigest(),
    }
    measured = [clip["metrics"] for clip in normalized if "metrics" in clip]
    if measured:
        report["quality_metrics"] = {
            **{key: max(metrics[key] for metrics in measured) for key in MAXIMUM_METRICS},
            **{key: sum(metrics[key] for metrics in measured) for key in SUM_METRICS},
        }
    return report


def main(argv=None):
    args = list(sys.argv[1:] if argv is None else argv)
    if len(args) != 2:
        print("usage: m6_motion_build.py SOURCE.json REPORT.json", file=sys.stderr)
        return 2
    source_path, output_path = map(Path, args)
    try:
        with source_path.open(encoding="utf-8") as handle:
            database = json.load(handle)
        report = build_database(database)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        temporary = output_path.with_suffix(output_path.suffix + ".tmp")
        temporary.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        temporary.replace(output_path)
    except (OSError, TypeError, ValueError, json.JSONDecodeError) as error:
        print(str(error), file=sys.stderr)
        return 1
    print("wrote {}".format(output_path))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
