#!/usr/bin/env python3
"""Produce a deterministic SPDX 2.3 inventory from Cargo metadata."""

import argparse
import json
import subprocess
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    metadata = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--locked", "--format-version", "1"], text=True
        )
    )
    packages = []
    for package in sorted(metadata["packages"], key=lambda item: (item["name"], item["version"])):
        packages.append(
            {
                "SPDXID": "SPDXRef-{}-{}".format(package["name"].replace("-", "_"), package["version"]),
                "name": package["name"],
                "versionInfo": package["version"],
                "downloadLocation": package.get("source") or "NOASSERTION",
                "licenseConcluded": package.get("license") or "NOASSERTION",
                "licenseDeclared": package.get("license") or "NOASSERTION",
                "filesAnalyzed": False,
            }
        )
    document = {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": "Blender-Crowd-1.0.0",
        "documentNamespace": "https://github.com/bgyss/Blender-Crowd/spdx/1.0.0",
        "creationInfo": {"creators": ["Tool: scripts/m3_sbom.py"], "created": "1970-01-01T00:00:00Z"},
        "packages": packages,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
