#!/usr/bin/env python3
"""Executable Python example for the versioned M6 extension boundary."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = REPO_ROOT / "addon" / "blender_crowd" / "m6_extensions.py"
SPEC = importlib.util.spec_from_file_location("m6_extensions", MODULE_PATH)
m6_extensions = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(m6_extensions)

CHANNEL_VERSION = 1
COST_BUDGET_MILLIONTHS = 100_000
FALLBACK = {"gaze_offset": [0, 0, 0]}


def manifest():
    return {
        "schema_version": m6_extensions.SCHEMA_VERSION,
        "id": "studio-look-at-python",
        "channels": [
            {
                "name": "look_at",
                "version": CHANNEL_VERSION,
                "inputs": ["attention_target"],
                "outputs": ["gaze_offset"],
                "cost_budget_millionths": COST_BUDGET_MILLIONTHS,
                "deterministic": True,
                "failure_isolated": True,
            }
        ],
    }


def record(case, status, reason=None, value=None):
    return {
        "case": case,
        "status": status,
        "reason": reason,
        "schema_version": m6_extensions.SCHEMA_VERSION,
        "channel_version": CHANNEL_VERSION,
        "inputs": ["attention_target"],
        "outputs": ["gaze_offset"],
        "cost_budget_millionths": COST_BUDGET_MILLIONTHS,
        "deterministic": True,
        "failure_isolated": True,
        "value": value,
    }


def emit(value):
    print(json.dumps(value, sort_keys=True, separators=(",", ":")))


def main():
    declared = manifest()
    accepted = m6_extensions.run_isolated(
        declared,
        "look_at",
        ["attention_target"],
        50_000,
        lambda: FALLBACK,
        fallback=FALLBACK,
    )
    emit(record("accepted_call", accepted["status"], value=accepted["value"]))

    try:
        m6_extensions.validate_call(
            declared,
            "look_at",
            ["attention_target"],
            COST_BUDGET_MILLIONTHS + 1,
        )
    except ValueError as error:
        assert str(error) == "extension cost budget exceeded"
        emit(
            record(
                "over_budget_call",
                "fallback",
                reason="cost_budget_exceeded",
                value=FALLBACK,
            )
        )

    try:
        m6_extensions.validate_call(declared, "look_at", ["private_state"], 50_000)
    except ValueError as error:
        assert str(error) == "undeclared extension input private_state"
        emit(
            record(
                "undeclared_channel_call",
                "rejected",
                reason="undeclared_input",
            )
        )

    incompatible = manifest()
    incompatible["schema_version"] = m6_extensions.SCHEMA_VERSION + 1
    try:
        m6_extensions.validate_call(
            incompatible,
            "look_at",
            ["attention_target"],
            50_000,
        )
    except ValueError as error:
        assert str(error) == "unsupported extension schema version"
        emit(
            record(
                "version_mismatch_call",
                "rejected",
                reason="unsupported_version",
            )
        )


if __name__ == "__main__":
    main()
