#!/usr/bin/env python3
"""Test-only CLI for exercising M6 status adjudication without running gates."""

from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path
import sys


REPO_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = REPO_ROOT / "scripts" / "m6_acceptance_status.py"
SPEC = importlib.util.spec_from_file_location("m6_acceptance_status", MODULE_PATH)
status_module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(status_module)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--motion", choices=("PASS", "OPEN", "FAILED"), default="OPEN")
    parser.add_argument("--fail", action="append", default=[])
    parser.add_argument("--open", dest="open_gates", action="append", default=[])
    parser.add_argument("--allow-open", action="store_true")
    args = parser.parse_args()

    gates = {gate: "PASS" for gate in status_module.REQUIRED_GATES}
    gates["motion_source"] = args.motion
    for gate in args.fail:
        gates[gate] = "FAILED"
    for gate in args.open_gates:
        gates[gate] = "OPEN"

    result = status_module.adjudicate(gates)
    print(json.dumps(result, sort_keys=True))
    if result["audit_status"] == "PASS":
        return 0
    if result["audit_status"] == "OPEN" and args.allow_open:
        return 0
    return 2


if __name__ == "__main__":
    sys.exit(main())
