#!/usr/bin/env python3
"""Validate the reviewed license, signing, documentation, and budget policy."""

import argparse
import hashlib
import json
import sys
import zipfile
from pathlib import Path


def sha256(payload):
    return hashlib.sha256(payload).hexdigest()


def member_by_suffix(names, suffix):
    matches = [name for name in names if name.rstrip("/").endswith(suffix)]
    if len(matches) != 1:
        raise ValueError("expected one archive member ending in {}".format(suffix))
    return matches[0]


def audit(archive):
    errors = []
    with zipfile.ZipFile(archive) as bundle:
        names = [item.filename for item in bundle.infolist() if not item.is_dir()]
        review = json.loads(bundle.read(member_by_suffix(names, "docs/release/1.0-release-review.json")))
        sbom_payload = bundle.read(member_by_suffix(names, "sbom.spdx.json"))
        sbom = json.loads(sbom_payload)
        dependency = review["dependency_review"]
        lock_payload = bundle.read(member_by_suffix(names, "Cargo.lock"))
        if sha256(lock_payload) != dependency["cargo_lock_sha256"]:
            errors.append("Cargo.lock changed after the recorded license review")
        if sha256(sbom_payload) != dependency["sbom_sha256"]:
            errors.append("SBOM changed after the recorded license review")
        packages = sbom.get("packages", [])
        if len(packages) != dependency["package_count"]:
            errors.append("SBOM package count changed after review")
        noassertion = sum(
            package.get("licenseDeclared") in (None, "", "NOASSERTION")
            for package in packages
        )
        if noassertion != dependency["noassertion_count"] or noassertion:
            errors.append("SBOM contains an unreviewed license assertion")
        if not dependency.get("reviewed"):
            errors.append("dependency review is not approved")
        signing = review["signing_review"]
        if signing.get("applicable") is not False or not signing.get("decision"):
            errors.append("signing applicability decision is incomplete")
        budgets_payload = bundle.read(
            member_by_suffix(names, "docs/release/1.0-budgets.json")
        )
        if sha256(budgets_payload) != review.get("budgets_sha256"):
            errors.append("release budgets changed after policy review")
        documentation = review["documentation_review"]
        if not documentation.get("reviewed") or not documentation.get("archive_exercised"):
            errors.append("documentation review is incomplete")
        for document in documentation.get("documents", []):
            if not any(name.rstrip("/").endswith(document) for name in names):
                errors.append("reviewed document is absent from archive: {}".format(document))
    return {"schema_version": 1, "passed": not errors, "errors": errors}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    report = audit(args.archive)
    encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(encoded, encoding="utf-8")
    print(encoded, end="")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
