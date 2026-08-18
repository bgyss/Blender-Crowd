#!/usr/bin/env python3
"""Fetch the five fixed CMU Acclaim files without publishing partial data."""

import hashlib
import json
import sys
import urllib.parse
import urllib.request
from pathlib import Path

from m6_cmu_motion_source import SOURCE_HOST, validate_fixed_manifest



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


def validate_manifest(manifest):
    return validate_fixed_manifest(manifest)


def fetch_manifest(manifest, output_dir, opener=None):
    files = validate_manifest(manifest)
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
