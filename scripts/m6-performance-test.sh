#!/usr/bin/env bash
# Fixed 10K M6 mixed-tier evidence lane. This is a deterministic fixture gate,
# not a general production performance claim.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORT="${M6_PERFORMANCE_REPORT:-$REPO_ROOT/benchmarks/reports/m6-mixed-tier-10k.json}"
SECOND="$(mktemp "${TMPDIR:-/tmp}/blender-crowd-m6-mixed-tier.XXXXXX.json")"
trap 'rm -f "$SECOND"' EXIT

cd "$REPO_ROOT"
cargo test --release -p crowd-bench --test m6_mixed_tier
cargo run --release -p crowd-bench --bin m6-mixed-tier -- --out "$REPORT"
cargo run --release -p crowd-bench --bin m6-mixed-tier -- --out "$SECOND"

python3 - "$REPORT" "$SECOND" <<'PY'
import json
import sys

first_path, second_path = sys.argv[1:]
with open(first_path, encoding="utf-8") as handle:
    first = json.load(handle)
with open(second_path, encoding="utf-8") as handle:
    second = json.load(handle)

expected_tiers = {"S0": 10, "S1": 990, "S2": 9000}
expected_phases = {"perception", "brain", "activity", "group", "motion", "interaction"}
if first["tier_counts"] != expected_tiers or first["agent_count"] != 10000:
    raise SystemExit("M6 mixed-tier report lost the exact 10/990/9000 fixture")
if {item["phase"] for item in first["phase_timings"]} != expected_phases:
    raise SystemExit("M6 mixed-tier report is missing a separately timed phase")
if any(item["nanos"] <= 0 or item["operations"] <= 0 for item in first["phase_timings"]):
    raise SystemExit("M6 mixed-tier report has an unmeasured phase")
for field in (
    "deterministic_replay_hash",
    "final_state_hash",
    "cache_payload_hash",
    "fallbacks",
    "hard_safety_failures",
    "unrelated_agent_mutations",
):
    if first[field] != second[field]:
        raise SystemExit("M6 mixed-tier deterministic field changed: {}".format(field))
if not first["passed"]:
    raise SystemExit("M6 mixed-tier gate failed: {}".format(first["failure_reasons"]))
if first["ticks_per_second"] < first["min_ticks_per_second"]:
    raise SystemExit("M6 mixed-tier throughput missed its checked threshold")
if first["hard_safety_failures"] != 0 or first["unrelated_agent_mutations"] != 0:
    raise SystemExit("M6 mixed-tier hard-safety/isolation gate failed")

print(
    "M6 mixed-tier performance passed: {:.3f} ticks/s; replay {}; report {}".format(
        first["ticks_per_second"], first["deterministic_replay_hash"], first_path
    )
)
PY
