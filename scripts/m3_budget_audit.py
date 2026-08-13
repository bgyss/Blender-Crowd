#!/usr/bin/env python3
"""Enforce the fixed Blender Crowd 1.0 macOS arm64 release budgets."""

import argparse
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BUDGETS = ROOT / "docs" / "release" / "1.0-budgets.json"


def directory_size(path):
    return sum(item.stat().st_size for item in Path(path).rglob("*") if item.is_file())


def audit(archive, reference_root, budgets_path=DEFAULT_BUDGETS):
    archive = Path(archive)
    reference_root = Path(reference_root)
    budgets = json.loads(Path(budgets_path).read_text(encoding="utf-8"))
    acceptance = json.loads(
        (reference_root / "m2-full-acceptance.json").read_text(encoding="utf-8")
    )
    render = json.loads(
        (reference_root / "render" / "m1-render-metrics.json").read_text(encoding="utf-8")
    )
    measured = {
        "archive_bytes": archive.stat().st_size,
        "authorable_bake_seconds": acceptance["authorable_bake_seconds"],
        "cache_bytes": directory_size(reference_root / "cache"),
        "debug_inspection_seconds_per_query": acceptance[
            "debug_inspection_seconds_per_query"
        ],
        "peak_resident_bytes": render["peak_resident_bytes"],
        "point_upload_seconds": render["point_upload_seconds"],
        "armature_evaluation_seconds": render["armature_evaluation_seconds"],
        "eevee_render_seconds": render["renders"]["eevee"]["seconds"],
        "cycles_cpu_render_seconds": render["renders"]["cycles"]["seconds"],
        "sequential_cache_ticks_per_second": acceptance[
            "sequential_cache_ticks_per_second"
        ],
    }
    checks = []
    for name, limit in budgets["maximum"].items():
        actual = measured[name]
        checks.append(
            {"metric": name, "comparison": "maximum", "limit": limit, "actual": actual, "passed": actual <= limit}
        )
    for name, limit in budgets["minimum"].items():
        actual = measured[name]
        checks.append(
            {"metric": name, "comparison": "minimum", "limit": limit, "actual": actual, "passed": actual >= limit}
        )
    return {
        "schema_version": 1,
        "platform": budgets["platform"],
        "passed": all(check["passed"] for check in checks),
        "checks": checks,
        "measurements": measured,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", required=True, type=Path)
    parser.add_argument("--reference-root", required=True, type=Path)
    parser.add_argument("--budgets", type=Path, default=DEFAULT_BUDGETS)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    report = audit(args.archive, args.reference_root, args.budgets)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
    args.out.write_text(encoded, encoding="utf-8")
    print(encoded, end="")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
