#!/usr/bin/env python3
"""Shared fixed-source validation for the production CMU evidence lane."""

import urllib.parse
from pathlib import Path


SOURCE_HOST = "mocap.cs.cmu.edu"
FIXED_MANIFEST_METADATA = {
    "schema_version": 1,
    "dataset_id": "cmu-mocap-subjects-35-36-m6-v1",
    "source_host": SOURCE_HOST,
    "terms_url": "http://mocap.cs.cmu.edu/faqs.php",
    "license_id": "CMU-Mocap-Free-All-Uses",
    "redistribution_allowed": False,
    "required_attribution": "Carnegie Mellon University Graphics Lab Motion Capture Database",
    "source_frame_rate_hz": 120,
    "target_frame_rate_hz": 30,
    "retarget_profile": {
        "profile_id": "cmu-acclaim-humanoid-v1",
        "root_bone": "root",
        "left_foot_bone": "lfoot",
        "right_foot_bone": "rfoot",
        "forward_axis": "+Z",
    },
}
FIXED_FILES = (
    {
        "id": "35_skeleton",
        "kind": "skeleton",
        "subject": 35,
        "description": "CMU subject 35 Acclaim skeleton",
        "url": "http://mocap.cs.cmu.edu/subjects/35/35.asf",
        "filename": "35.asf",
        "sha256": "2a8e2eda3c0d7d828566b2a9a8ab36b2b8b3864110574e8b73c8f069fded416c",
    },
    {
        "id": "35_01_walk",
        "kind": "motion",
        "subject": 35,
        "trial": 1,
        "clip_id": "35_01_walk",
        "description": "Subject 35 walking trial 01",
        "skeleton_id": "35_skeleton",
        "url": "http://mocap.cs.cmu.edu/subjects/35/35_01.amc",
        "filename": "35_01.amc",
        "sha256": "0743f4ea48e7e199cd56b2810b5ce81f8ede08d32ff79aa4e363c44cc4fe33aa",
    },
    {
        "id": "35_24_run",
        "kind": "motion",
        "subject": 35,
        "trial": 24,
        "clip_id": "35_24_run",
        "description": "Subject 35 running trial 24",
        "skeleton_id": "35_skeleton",
        "url": "http://mocap.cs.cmu.edu/subjects/35/35_24.amc",
        "filename": "35_24.amc",
        "sha256": "29059fb2c15493983e4dccdf45453a495fb567dd28ff36cc1a0dbc02ad409445",
    },
    {
        "id": "36_skeleton",
        "kind": "skeleton",
        "subject": 36,
        "description": "CMU subject 36 Acclaim skeleton",
        "url": "http://mocap.cs.cmu.edu/subjects/36/36.asf",
        "filename": "36.asf",
        "sha256": "05e190867ead216b5dcdc94b210aa19b2eaaf383df44f1d9bb247e64fbf1c02b",
    },
    {
        "id": "36_01_uneven_walk",
        "kind": "motion",
        "subject": 36,
        "trial": 1,
        "clip_id": "36_01_uneven_walk",
        "description": "Subject 36 uneven-terrain walking trial 01",
        "skeleton_id": "36_skeleton",
        "url": "http://mocap.cs.cmu.edu/subjects/36/36_01.amc",
        "filename": "36_01.amc",
        "sha256": "882e9f8c35622c2e10e9a3f578b5e0e7033ceb53232f415640b47fc05f3c2fac",
    },
)
FIXED_SOURCES = {entry["url"]: entry["sha256"] for entry in FIXED_FILES}


def _validate_common_manifest(manifest):
    if not isinstance(manifest, dict) or manifest.get("schema_version") != 1:
        raise ValueError("unsupported CMU source manifest schema version")
    if manifest.get("source_host") != SOURCE_HOST:
        raise ValueError("unknown source host")
    if manifest.get("license_id") != "CMU-Mocap-Free-All-Uses":
        raise ValueError("unexpected CMU license identity")
    if manifest.get("redistribution_allowed") is not False:
        raise ValueError("raw and converted CMU redistribution must remain disabled")
    terms_url = manifest.get("terms_url")
    if not isinstance(terms_url, str) or urllib.parse.urlsplit(terms_url).hostname != SOURCE_HOST:
        raise ValueError("CMU terms URL must use the official source host")
    if not isinstance(manifest.get("required_attribution"), str) or not manifest["required_attribution"].strip():
        raise ValueError("CMU required attribution is missing")
    if manifest.get("source_frame_rate_hz") != 120 or manifest.get("target_frame_rate_hz") != 30:
        raise ValueError("CMU manifest frame rates must be 120 Hz and 30 Hz")
    profile = manifest.get("retarget_profile")
    profile_fields = ("profile_id", "root_bone", "left_foot_bone", "right_foot_bone", "forward_axis")
    if not isinstance(profile, dict) or any(not isinstance(profile.get(field), str) or not profile[field] for field in profile_fields):
        raise ValueError("CMU retarget profile metadata is incomplete")
    files = manifest.get("files")
    if not isinstance(files, list) or not files:
        raise ValueError("CMU source manifest files must be a non-empty list")
    identities = set()
    filenames = set()
    urls = set()
    skeletons = set()
    for entry in files:
        if not isinstance(entry, dict):
            raise ValueError("CMU source entries must be objects")
        identity = entry.get("id")
        filename = entry.get("filename")
        url = entry.get("url")
        sha256 = entry.get("sha256")
        if identity in identities or filename in filenames or url in urls:
            raise ValueError("extra or duplicate CMU source file")
        if not isinstance(identity, str) or not identity or not isinstance(filename, str) or not filename:
            raise ValueError("CMU source identity and filename are required")
        if not isinstance(entry.get("subject"), int) or not isinstance(entry.get("description"), str) or not entry["description"].strip():
            raise ValueError("CMU subject and description metadata are required")
        identities.add(identity)
        filenames.add(filename)
        urls.add(url)
        parsed = urllib.parse.urlsplit(url or "")
        if parsed.scheme != "http" or parsed.hostname != SOURCE_HOST:
            raise ValueError("unknown source host or protocol")
        if Path(parsed.path).name != filename:
            raise ValueError("CMU source filename does not match its URL")
        if not isinstance(sha256, str) or len(sha256) != 64 or any(char not in "0123456789abcdef" for char in sha256):
            raise ValueError("invalid source SHA-256")
        if entry.get("kind") == "skeleton":
            skeletons.add(identity)
        elif entry.get("kind") == "motion":
            if not isinstance(entry.get("trial"), int) or entry["trial"] < 1 or not isinstance(entry.get("clip_id"), str) or not entry["clip_id"] or not isinstance(entry.get("skeleton_id"), str) or not entry["skeleton_id"]:
                raise ValueError("CMU motion metadata is incomplete")
        else:
            raise ValueError("unknown CMU source file kind")
    for entry in files:
        if entry.get("kind") == "motion" and entry.get("skeleton_id") not in skeletons:
            raise ValueError("motion source references an unknown skeleton")
    return files


def validate_fixed_manifest(manifest):
    files = _validate_common_manifest(manifest)
    for key, expected in FIXED_MANIFEST_METADATA.items():
        if manifest.get(key) != expected:
            raise ValueError("fixed manifest metadata changed: {}".format(key))
    actual_sources = {entry["url"]: entry["sha256"] for entry in files}
    if actual_sources != FIXED_SOURCES:
        raise ValueError("manifest contains missing, changed, or extra fixed CMU source files")
    expected_by_url = {entry["url"]: entry for entry in FIXED_FILES}
    for entry in files:
        if entry != expected_by_url[entry["url"]]:
            raise ValueError("fixed identity or relationship changed for {}".format(entry["url"]))
    return files


def validate_parser_fixture_manifest(manifest):
    """Validate the hand-authored mini parser fixture; never use for production data."""
    files = _validate_common_manifest(manifest)
    if manifest.get("dataset_id") != "cmu-mini" or len(files) != 2:
        raise ValueError("non-production parser fixture identity is invalid")
    expected = {
        "mini_skeleton": ("skeleton", 0, "cmu-mini.asf", None),
        "mini_walk": ("motion", 0, "cmu-mini.amc", "mini_skeleton"),
    }
    for entry in files:
        identity = entry["id"]
        actual = (entry["kind"], entry["subject"], entry["filename"], entry.get("skeleton_id"))
        if identity not in expected or actual != expected[identity]:
            raise ValueError("non-production parser fixture identity is invalid")
    return files
