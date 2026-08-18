#!/usr/bin/env python3
"""Fetch the five fixed CMU Acclaim files without publishing partial data."""

import hashlib
import json
import sys
import urllib.parse
import urllib.request
from pathlib import Path


SOURCE_HOST = "mocap.cs.cmu.edu"
FIXED_SOURCES = {
    "http://mocap.cs.cmu.edu/subjects/35/35.asf": "2a8e2eda3c0d7d828566b2a9a8ab36b2b8b3864110574e8b73c8f069fded416c",
    "http://mocap.cs.cmu.edu/subjects/35/35_01.amc": "0743f4ea48e7e199cd56b2810b5ce81f8ede08d32ff79aa4e363c44cc4fe33aa",
    "http://mocap.cs.cmu.edu/subjects/35/35_24.amc": "29059fb2c15493983e4dccdf45453a495fb567dd28ff36cc1a0dbc02ad409445",
    "http://mocap.cs.cmu.edu/subjects/36/36.asf": "05e190867ead216b5dcdc94b210aa19b2eaaf383df44f1d9bb247e64fbf1c02b",
    "http://mocap.cs.cmu.edu/subjects/36/36_01.amc": "882e9f8c35622c2e10e9a3f578b5e0e7033ceb53232f415640b47fc05f3c2fac",
}


class SameHostRedirectHandler(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        if urllib.parse.urlsplit(newurl).hostname != SOURCE_HOST:
            raise ValueError("refusing redirect away from {}".format(SOURCE_HOST))
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def verify_download(data, expected_sha256):
    actual = hashlib.sha256(data).hexdigest()
    if actual != expected_sha256:
        raise ValueError("SHA-256 mismatch: expected {}, got {}".format(expected_sha256, actual))
    return actual


def validate_manifest(manifest, require_fixed_sources=True):
    if manifest.get("schema_version") != 1:
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
    if require_fixed_sources:
        actual = {entry["url"]: entry["sha256"] for entry in files}
        if actual != FIXED_SOURCES:
            raise ValueError("manifest contains missing, changed, or extra fixed CMU source files")
    return files


def fetch_manifest(manifest, output_dir, opener=None, require_fixed_sources=True):
    files = validate_manifest(manifest, require_fixed_sources=require_fixed_sources)
    output = Path(output_dir)
    output.mkdir(parents=True, exist_ok=True)
    existing = list(output.iterdir())
    if existing:
        raise ValueError("output directory contains extra files")
    client = opener or urllib.request.build_opener(SameHostRedirectHandler())
    published = []
    try:
        for entry in files:
            request = urllib.request.Request(entry["url"], headers={"User-Agent": "Blender-Crowd-M6/1"})
            with client.open(request, timeout=30) as response:
                final_url = response.geturl()
                if urllib.parse.urlsplit(final_url).hostname != SOURCE_HOST:
                    raise ValueError("refusing redirect away from {}".format(SOURCE_HOST))
                data = response.read()
            verify_download(data, entry["sha256"])
            temporary = output / (entry["filename"] + ".part")
            destination = output / entry["filename"]
            temporary.write_bytes(data)
            temporary.replace(destination)
            published.append(destination)
    except Exception:
        for path in list(output.iterdir()):
            if path.is_file():
                path.unlink()
        raise
    return published


def main(argv=None):
    args = list(sys.argv[1:] if argv is None else argv)
    if len(args) != 2:
        print("usage: m6_fetch_cmu_motion.py MANIFEST.json OUTPUT_DIR", file=sys.stderr)
        return 2
    try:
        manifest = json.loads(Path(args[0]).read_text(encoding="utf-8"))
        paths = fetch_manifest(manifest, args[1])
    except (OSError, TypeError, ValueError, json.JSONDecodeError) as error:
        print(str(error), file=sys.stderr)
        return 1
    for path in paths:
        print("fetched {}".format(path))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
