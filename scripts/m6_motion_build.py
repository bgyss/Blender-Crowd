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
        normalized.append(
            {
                "id": clip["id"],
                "sample_count": len(samples),
                "feature_channels": ["future_velocity", "contact", "slope"],
            }
        )
    normalized.sort(key=lambda clip: clip["id"])
    canonical = {
        "schema_version": SCHEMA_VERSION,
        "database_id": database["database_id"],
        "retarget_profile_id": database["retarget_profile_id"],
        "source_provenance": database["source_provenance"],
        "clips": normalized,
    }
    encoded = json.dumps(canonical, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return {
        **canonical,
        "clip_ids": [clip["id"] for clip in normalized],
        "clip_count": len(normalized),
        "sample_count": sum(clip["sample_count"] for clip in normalized),
        "content_hash": hashlib.sha256(encoded).hexdigest(),
    }


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
