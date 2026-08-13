#!/usr/bin/env python3
"""Normalize wheel ZIP metadata so identical contents produce identical bytes."""

import argparse
import os
import time
import zipfile
from pathlib import Path


def normalize_wheel(path, epoch):
    path = Path(path)
    timestamp = time.gmtime(max(int(epoch), 315532800))[:6]
    temporary = path.with_suffix(path.suffix + ".normalized")
    with zipfile.ZipFile(path, "r") as source:
        members = [(item, source.read(item.filename)) for item in source.infolist()]
    with zipfile.ZipFile(temporary, "w") as output:
        for original, payload in sorted(members, key=lambda pair: pair[0].filename):
            normalized = zipfile.ZipInfo(original.filename, timestamp)
            normalized.compress_type = original.compress_type
            normalized.comment = original.comment
            normalized.create_system = original.create_system
            normalized.external_attr = original.external_attr
            normalized.internal_attr = original.internal_attr
            normalized.flag_bits = original.flag_bits
            output.writestr(normalized, payload)
    os.replace(temporary, path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("wheel", type=Path)
    parser.add_argument("--epoch", required=True, type=int)
    args = parser.parse_args()
    normalize_wheel(args.wheel, args.epoch)


if __name__ == "__main__":
    main()
