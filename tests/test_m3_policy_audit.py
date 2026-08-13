"""Contract tests for reviewed M3 release policy evidence."""

import hashlib
import importlib.util
import json
import tempfile
import unittest
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "m3_policy_audit", ROOT / "scripts" / "m3_policy_audit.py"
)
POLICY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(POLICY)


def digest(payload):
    return hashlib.sha256(payload).hexdigest()


def write_archive(path, changed_lock=False):
    lock = b"locked dependencies\n"
    budgets = b'{"fixed": true}\n'
    sbom = json.dumps(
        {
            "spdxVersion": "SPDX-2.3",
            "packages": [{"name": "dependency", "licenseDeclared": "MIT"}],
        },
        sort_keys=True,
    ).encode()
    documents = ["docs/user.md"]
    review = {
        "budgets_sha256": digest(budgets),
        "dependency_review": {
            "cargo_lock_sha256": digest(lock),
            "sbom_sha256": digest(sbom),
            "package_count": 1,
            "noassertion_count": 0,
            "reviewed": True,
        },
        "signing_review": {"applicable": False, "decision": "not applicable"},
        "documentation_review": {
            "reviewed": True,
            "archive_exercised": True,
            "documents": documents,
        },
    }
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("Cargo.lock", lock + (b"changed" if changed_lock else b""))
        archive.writestr("sbom.spdx.json", sbom)
        archive.writestr("docs/release/1.0-budgets.json", budgets)
        archive.writestr(
            "docs/release/1.0-release-review.json", json.dumps(review)
        )
        archive.writestr(documents[0], "reviewed")


class M3PolicyAuditTests(unittest.TestCase):
    def test_accepts_policy_evidence_pinned_to_archive_contents(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "release.zip"
            write_archive(archive)
            self.assertTrue(POLICY.audit(archive)["passed"])

    def test_rejects_dependencies_changed_after_review(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "release.zip"
            write_archive(archive, changed_lock=True)
            report = POLICY.audit(archive)
            self.assertFalse(report["passed"])
            self.assertIn("Cargo.lock changed", report["errors"][0])


if __name__ == "__main__":
    unittest.main()
