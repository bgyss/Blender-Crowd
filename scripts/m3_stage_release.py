#!/usr/bin/env python3
"""Create a disposable, self-contained extension source tree for release."""

import argparse
import json
import os
import shutil
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ADDON = ROOT / "addon" / "blender_crowd"
RELEASE_DOCS = (
    (ROOT / "docs" / "blender-crowd-1.0.md", Path("docs/blender-crowd-1.0.md")),
    (ROOT / "docs" / "cache-format-v1.md", Path("docs/cache-format-v1.md")),
    (ROOT / "docs" / "dependencies" / "cache-v1.md", Path("docs/dependencies/cache-v1.md")),
    (ROOT / "docs" / "user" / "m1-reference-walkthrough.md", Path("docs/user/m1-reference-walkthrough.md")),
    (ROOT / "docs" / "user" / "m2-behavior-graph.md", Path("docs/user/m2-behavior-graph.md")),
    (ROOT / "docs" / "user" / "m2-authoring-foundation.md", Path("docs/user/m2-authoring-foundation.md")),
    (ROOT / "docs" / "user" / "m3-production-recovery.md", Path("docs/user/m3-production-recovery.md")),
    (ROOT / "docs" / "user" / "m3-headless-release.md", Path("docs/user/m3-headless-release.md")),
    (ROOT / "docs" / "release" / "1.0-compatibility.md", Path("docs/release/1.0-compatibility.md")),
    (ROOT / "docs" / "release" / "1.0-support-matrix.md", Path("docs/release/1.0-support-matrix.md")),
    (ROOT / "docs" / "release" / "1.0-release-checklist.md", Path("docs/release/1.0-release-checklist.md")),
    (ROOT / "docs" / "release" / "1.0-known-limitations.md", Path("docs/release/1.0-known-limitations.md")),
)


def git_value(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def normalize_timestamps(root, epoch):
    """Give Blender's ZIP builder stable input mtimes for reproducible output."""
    for path in sorted(root.rglob("*"), reverse=True):
        os.utime(path, (epoch, epoch), follow_symlinks=False)
    os.utime(root, (epoch, epoch), follow_symlinks=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    if args.out.exists():
        raise SystemExit("release stage already exists: {}".format(args.out))
    shutil.copytree(ADDON, args.out, ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
    for source, relative in RELEASE_DOCS:
        target = args.out / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    subprocess.check_call(["python3", str(ROOT / "scripts" / "m3_sbom.py"), "--out", str(args.out / "sbom.spdx.json")], cwd=ROOT)
    source_date_epoch = int(
        os.environ.get("SOURCE_DATE_EPOCH")
        or git_value("show", "-s", "--format=%ct", "HEAD")
    )
    provenance = {
        "schema_version": 1,
        "project": "Blender Crowd",
        "version": "1.0.0",
        "source_revision": git_value("rev-parse", "HEAD"),
        "source_dirty": bool(git_value("status", "--porcelain")),
        "source_date_epoch": source_date_epoch,
        "build_recipe": "scripts/build-wheel.sh + scripts/m3_stage_release.py",
    }
    (args.out / "release-provenance.json").write_text(
        json.dumps(provenance, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    normalize_timestamps(args.out, source_date_epoch)


if __name__ == "__main__":
    main()
