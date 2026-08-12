#!/usr/bin/env python3
"""Verify a Blender Crowd release archive before it is offered for installation.

The audit is deliberately archive-only: a development checkout can hide missing
files or local-path dependencies that a released extension cannot.  It does not
claim that a passing archive has completed the human support-matrix gate.
"""

import argparse
import hashlib
import json
import re
import sys
import zipfile
from pathlib import Path, PurePosixPath


TEXT_SUFFIXES = {".md", ".py", ".toml", ".json", ".txt", ".rst", ".yml", ".yaml"}
FORBIDDEN_PATHS = (b"/Users/", b"/home/", b"C:\\\\Users\\")
REQUIRED_FILES = {
    "blender_manifest.toml",
    "docs/blender-crowd-1.0.md",
    "docs/cache-format-v1.md",
    "docs/dependencies/cache-v1.md",
    "docs/user/m1-reference-walkthrough.md",
    "docs/user/m2-behavior-graph.md",
    "docs/user/m2-authoring-foundation.md",
    "docs/user/m3-production-recovery.md",
    "docs/user/m3-headless-release.md",
    "docs/release/1.0-compatibility.md",
    "docs/release/1.0-support-matrix.md",
    "docs/release/1.0-release-checklist.md",
    "docs/release/1.0-known-limitations.md",
    "sbom.spdx.json",
    "release-provenance.json",
}


def fail(errors, message):
    errors.append(message)


def find_member(names, suffix):
    matches = [name for name in names if name.rstrip("/").endswith(suffix)]
    return matches[0] if len(matches) == 1 else None


def toml_value(text, key):
    match = re.search(r"^{}\s*=\s*\"([^\"]+)\"\s*$".format(re.escape(key)), text, re.M)
    return match.group(1) if match else None


def toml_array_values(text, key):
    match = re.search(r"^{}\s*=\s*\[([^]]*)\]\s*$".format(re.escape(key)), text, re.M)
    return re.findall(r'\"([^\"]+)\"', match.group(1)) if match else []


def audit(archive):
    archive = Path(archive)
    errors = []
    if not archive.is_file():
        return {"archive": str(archive), "passed": False, "errors": ["archive does not exist"]}
    try:
        with zipfile.ZipFile(archive) as bundle:
            names = [item.filename for item in bundle.infolist() if not item.is_dir()]
            normalized = [PurePosixPath(name) for name in names]
            if any(path.is_absolute() or ".." in path.parts for path in normalized):
                fail(errors, "archive has an unsafe member path")
            for required in REQUIRED_FILES:
                if find_member(names, required) is None:
                    fail(errors, "required release file is missing: {}".format(required))
            manifest_name = find_member(names, "blender_manifest.toml")
            manifest = {}
            if manifest_name:
                manifest_text = bundle.read(manifest_name).decode("utf-8")
                manifest["id"] = toml_value(manifest_text, "id")
                manifest["version"] = toml_value(manifest_text, "version")
                manifest["blender_version_min"] = toml_value(manifest_text, "blender_version_min")
                manifest["platforms"] = toml_array_values(manifest_text, "platforms")
                manifest["license"] = re.findall(r"SPDX:([A-Za-z0-9.+-]+)", manifest_text)
                if manifest["id"] != "blender_crowd":
                    fail(errors, "manifest id must be blender_crowd")
                if manifest["version"] != "1.0.0":
                    fail(errors, "manifest version must be 1.0.0")
                if manifest["blender_version_min"] != "5.2.0":
                    fail(errors, "manifest must declare Blender 5.2.0 minimum")
                if manifest["platforms"] != ["macos-arm64"]:
                    fail(errors, "Blender Crowd 1.0 must claim exactly macos-arm64")
                if "GPL-3.0-or-later" not in manifest["license"]:
                    fail(errors, "manifest must declare GPL-3.0-or-later")
                wheels = re.findall(r"\.\/wheels\/([^\"]+\.whl)", manifest_text)
                if not wheels:
                    fail(errors, "manifest has no bundled native wheel")
                for wheel in wheels:
                    if find_member(names, "wheels/{}".format(wheel)) is None:
                        fail(errors, "manifest wheel is absent from archive: {}".format(wheel))
            for name in names:
                path = PurePosixPath(name)
                if path.suffix.lower() not in TEXT_SUFFIXES:
                    continue
                payload = bundle.read(name)
                if any(marker in payload for marker in FORBIDDEN_PATHS):
                    fail(errors, "contributor path found in archive text: {}".format(name))
            sbom_name = find_member(names, "sbom.spdx.json")
            if sbom_name:
                try:
                    sbom = json.loads(bundle.read(sbom_name))
                    if sbom.get("spdxVersion") != "SPDX-2.3":
                        fail(errors, "SBOM is not SPDX-2.3")
                    if not sbom.get("packages"):
                        fail(errors, "SBOM has no packages")
                except (UnicodeDecodeError, json.JSONDecodeError) as error:
                    fail(errors, "SBOM is invalid JSON: {}".format(error))
            provenance_name = find_member(names, "release-provenance.json")
            if provenance_name:
                try:
                    provenance = json.loads(bundle.read(provenance_name))
                    if provenance.get("version") != "1.0.0":
                        fail(errors, "release provenance version must be 1.0.0")
                    if not provenance.get("source_revision"):
                        fail(errors, "release provenance has no source revision")
                    if provenance.get("source_dirty") is not False:
                        fail(errors, "release provenance must attest to a clean source tree")
                    if not isinstance(provenance.get("source_date_epoch"), int) or provenance["source_date_epoch"] <= 0:
                        fail(errors, "release provenance has no valid source date epoch")
                except (UnicodeDecodeError, json.JSONDecodeError) as error:
                    fail(errors, "release provenance is invalid JSON: {}".format(error))
    except zipfile.BadZipFile:
        errors.append("archive is not a readable zip")
    return {
        "archive": str(archive),
        "sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
        "manifest": manifest,
        "passed": not errors,
        "errors": errors,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", help="release extension zip")
    parser.add_argument("--out", type=Path, help="write JSON report to this path")
    args = parser.parse_args()
    report = audit(args.archive)
    encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(encoded, encoding="utf-8")
    print(encoded, end="")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
