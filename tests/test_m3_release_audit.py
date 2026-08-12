"""Contract tests for the archive-only M3 release verifier."""

import importlib.util
import json
import tempfile
import unittest
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("m3_release_audit", ROOT / "scripts/m3_release_audit.py")
AUDIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT)


def write_archive(
    path,
    include_wheel=True,
    contributor_path=False,
    source_dirty=False,
    source_date_epoch=1_786_497_600,
):
    manifest = '''schema_version = "1.0.0"
id = "blender_crowd"
version = "1.0.0"
name = "Blender Crowd"
type = "add-on"
license = ["SPDX:GPL-3.0-or-later"]
blender_version_min = "5.2.0"
wheels = ["./wheels/blender_crowd_native-1.0.0-cp311-abi3-macosx_11_0_arm64.whl"]
'''
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("blender_manifest.toml", manifest)
        for name in (
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
        ):
            archive.writestr(name, "clean text" if not contributor_path else "/Users/example/private")
        archive.writestr("sbom.spdx.json", json.dumps({"spdxVersion": "SPDX-2.3", "packages": [{"name": "crowd-core"}]}))
        archive.writestr(
            "release-provenance.json",
            json.dumps(
                {
                    "version": "1.0.0",
                    "source_revision": "0123456789abcdef",
                    "source_dirty": source_dirty,
                    "source_date_epoch": source_date_epoch,
                }
            ),
        )
        if include_wheel:
            archive.writestr("wheels/blender_crowd_native-1.0.0-cp311-abi3-macosx_11_0_arm64.whl", b"wheel")


class M3ReleaseAuditTests(unittest.TestCase):
    def test_accepts_a_complete_archive_contract(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "release.zip"
            write_archive(archive)
            self.assertTrue(AUDIT.audit(archive)["passed"])

    def test_rejects_a_missing_bundled_wheel(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "release.zip"
            write_archive(archive, include_wheel=False)
            report = AUDIT.audit(archive)
            self.assertFalse(report["passed"])
            self.assertTrue(any("wheel" in error for error in report["errors"]))

    def test_rejects_contributor_paths_in_release_text(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "release.zip"
            write_archive(archive, contributor_path=True)
            report = AUDIT.audit(archive)
            self.assertFalse(report["passed"])
            self.assertTrue(any("contributor path" in error for error in report["errors"]))

    def test_rejects_dirty_source_provenance(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "release.zip"
            write_archive(archive, source_dirty=True)
            report = AUDIT.audit(archive)
            self.assertFalse(report["passed"])
            self.assertTrue(any("clean source" in error for error in report["errors"]))

    def test_rejects_provenance_without_a_source_date_epoch(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "release.zip"
            write_archive(archive, source_date_epoch=None)
            report = AUDIT.audit(archive)
            self.assertFalse(report["passed"])
            self.assertTrue(any("source date epoch" in error for error in report["errors"]))
