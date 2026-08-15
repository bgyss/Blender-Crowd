#!/usr/bin/env bash
# M5 100K scale gate, end to end, in one command.
#
# This is a multi-hour run. Start it inside tmux so it survives a disconnected
# terminal:
#
#   tmux new -s blender-crowd-m5-100k
#   scripts/m5-100k-gate.sh
#
# Everything lands in one directory (default ~/blender-crowd-m5/100k), with a
# per-stage log beside each artifact. Nothing is written into the repository
# worktree.
#
# Stage order matters and is the runbook's order. The simulation gate is
# adjudicated before any Blender work: the milestone says to publish a failed
# report and stop rather than gather supporting evidence for a failed run, so
# that is what this does.
#
# Environment overrides:
#   M5_OUT             output directory (default ~/blender-crowd-m5/100k)
#   M5_AGENTS          population (default 100000)
#   M5_BLENDER_AGENTS  Blender proof population (default: same as M5_AGENTS)
#   M5_SKIP_BLENDER=1  stop after the cache matrix
#   M5_RESUME=1        reuse a simulation report that is already present
#   BLENDER            path to the Blender binary
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

AGENTS="${M5_AGENTS:-100000}"
OUT="${M5_OUT:-$HOME/blender-crowd-m5/100k}"
BLENDER_AGENTS="${M5_BLENDER_AGENTS:-$AGENTS}"
REPORT="$OUT/simulation/m5_city_flow-$AGENTS.json"
ADJUDICATION="$OUT/adjudication.json"

mkdir -p "$OUT/simulation" "$OUT/cache" "$OUT/blender" "$OUT/logs"

say() { printf '\n=== %s ===\n' "$*"; }

# Stage timing is reported per stage rather than only at the end: on a run this
# long, knowing which stage you are in is most of what you want from the log.
run_stage() {
    local name="$1"; shift
    local log="$OUT/logs/$name.log"
    local started elapsed status
    say "$name (log: $log)"
    started=$SECONDS
    "$@" >"$log" 2>&1
    status=$?
    elapsed=$((SECONDS - started))
    printf '%s: exit %d after %dm%02ds\n' "$name" "$status" "$((elapsed / 60))" "$((elapsed % 60))"
    if [ "$status" -ne 0 ]; then
        printf 'last 20 lines of %s:\n' "$log"
        tail -20 "$log"
    fi
    return "$status"
}

say "environment"
{
    date -u +"captured_at=%Y-%m-%dT%H:%M:%SZ"
    echo "commit=$(git rev-parse HEAD)"
    echo "dirty=$(git status --porcelain | wc -l | tr -d ' ') file(s)"
    echo "agents=$AGENTS blender_agents=$BLENDER_AGENTS"
    rustc --version
    uname -a
} | tee "$OUT/logs/environment.log"

# Capture the exact diff when the tree is dirty. A multi-hour measurement whose
# source state cannot be reconstructed is not evidence.
if ! git diff --quiet || [ -n "$(git status --porcelain)" ]; then
    git diff >"$OUT/logs/worktree.diff"
    git status --porcelain >"$OUT/logs/worktree-status.txt"
    echo "worktree is dirty; diff saved to $OUT/logs/worktree.diff"
fi

run_stage foundation-tests scripts/m5-foundation-test.sh || exit 1
run_stage build cargo build --release -p crowd-bench || exit 1

BENCH="$REPO_ROOT/target/release/crowd-bench"

if [ -s "$REPORT" ] && [ "${M5_RESUME:-0}" = "1" ]; then
    say "simulation: reusing existing report (M5_RESUME=1)"
else
    echo
    # Mirrors scenes::m5_city_flow: duration is 4500 ticks times the scene
    # scale, and scale is sqrt(agents / 100). awk rather than python3, which
    # is not guaranteed to be on a bare reference workstation's PATH.
    TICKS="$(awk -v a="$AGENTS" 'BEGIN { printf "%d", 4500 * sqrt(a / 100) }')"
    echo "The $AGENTS-agent simulation runs $TICKS ticks."
    echo "At the 10K gate's measured throughput this stage takes several hours."
    run_stage simulation "$BENCH" run \
        --scene m5_city_flow --agents "$AGENTS" --out "$OUT/simulation" || exit 1
fi

# The gate exits non-zero on a failed adjudication. That is a real result, so
# it is captured and reported rather than allowed to kill the script silently.
run_stage gate "$BENCH" m5-gate --report "$REPORT" --out "$ADJUDICATION"
GATE_STATUS=$?
cat "$OUT/logs/gate.log"

if [ -s "$OUT/cache/report.json" ] && [ "${M5_RESUME:-0}" = "1" ]; then
    say "cache: reusing existing matrix (M5_RESUME=1)"
else
    # `cache-experiment` refuses to write into a directory that already holds a
    # cache, which is correct — it must never quietly overwrite evidence. Move
    # any previous attempt aside rather than deleting it, so a partial run from
    # an interrupted attempt is preserved and this one can still proceed.
    if [ -n "$(ls -A "$OUT/cache" 2>/dev/null)" ]; then
        superseded="$OUT/cache-superseded-$(date -u +%Y%m%dT%H%M%SZ)"
        mv "$OUT/cache" "$superseded"
        mkdir -p "$OUT/cache"
        echo "previous cache artifacts moved to $superseded"
    fi
    run_stage cache "$BENCH" cache-experiment \
        --agents "$AGENTS" --cache-frames 120 --out "$OUT/cache" || exit 1
fi

if [ "$GATE_STATUS" -ne 0 ]; then
    say "100K GATE FAILED"
    cat <<'EOF'
The simulation did not meet the checked-in per-tier thresholds.

Publish a dated failed report under docs/benchmarks/ and stop. Do not loosen
benchmarks/thresholds/m5-city-flow.json to admit this run: the thresholds are
per-agent-tick rates precisely so that 10K and 100K are held to the same bar,
and a 100K result that needs looser numbers is a finding to report.

Blender evidence was skipped: it supports a passing gate, and gathering it for
a failed one would only make the failure look better documented.
EOF
    echo "artifacts: $OUT"
    exit 1
fi

if [ "${M5_SKIP_BLENDER:-0}" = "1" ]; then
    say "Blender stage skipped (M5_SKIP_BLENDER=1)"
    echo "artifacts: $OUT"
    exit 0
fi

# Blender needs normal host Metal access on macOS; a restricted automation
# sandbox returns no Metal device and crashes before Python starts.
M5_BLENDER_AGENTS="$BLENDER_AGENTS" \
M5_ARTIFACT_DIR="$OUT/blender" \
M5_REPORT="$REPORT" \
M5_ADJUDICATION="$ADJUDICATION" \
    run_stage blender scripts/m5-blender-test.sh
BLENDER_STATUS=$?

say "summary"
grep -E "result:" "$OUT/logs/gate.log" || true
grep -E "M5 Blender scale" "$OUT/logs/blender.log" 2>/dev/null || true
echo "artifacts: $OUT"

if [ "$BLENDER_STATUS" -ne 0 ]; then
    cat <<'EOF'

The simulation gate passed but the Blender stage did not.

Check whether the failure is about the procedural claim or about the reference
concourse's own capacity: that scene is authored around 1,000 agents and its
spawn regions are not scaled by population, so a very large M5_BLENDER_AGENTS
can fail for scene-density reasons that say nothing about whether playback
stayed procedural. Rerun that stage alone at a population the scene can hold,
and report the population you used:

  M5_BLENDER_AGENTS=10000 M5_REPORT=... M5_ADJUDICATION=... scripts/m5-blender-test.sh
EOF
    exit 1
fi

say "100K gate stages complete"
echo "Write the dated report under docs/benchmarks/ from these artifacts."
