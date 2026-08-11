#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

RUN_DIR="$(mktemp -d "${TMPDIR:-/tmp}/blender-crowd-cache-experiment.XXXXXX")"
cleanup() {
    rm -rf "$RUN_DIR"
}
trap cleanup EXIT

export CROWD_EXPERIMENT_GIT_COMMIT
CROWD_EXPERIMENT_GIT_COMMIT="$(git rev-parse HEAD)"
export CROWD_EXPERIMENT_GIT_DIRTY
if [[ -n "$(git status --porcelain --untracked-files=normal)" ]]; then
    CROWD_EXPERIMENT_GIT_DIRTY="true"
else
    CROWD_EXPERIMENT_GIT_DIRTY="false"
fi
export CROWD_EXPERIMENT_UNAME
CROWD_EXPERIMENT_UNAME="$(uname -a)"
export CROWD_EXPERIMENT_CPU
CROWD_EXPERIMENT_CPU="$(sysctl -n machdep.cpu.brand_string 2>/dev/null || printf 'unknown')"
export CROWD_EXPERIMENT_RAM_BYTES
CROWD_EXPERIMENT_RAM_BYTES="$(sysctl -n hw.memsize 2>/dev/null || printf '0')"

cargo build --release -p crowd-bench
"$ROOT_DIR/target/release/crowd-bench" cache-experiment \
    --agents 1000 \
    --seed 2026 \
    --out "$RUN_DIR/output"

mkdir -p "$ROOT_DIR/docs/benchmarks"
cp "$RUN_DIR/output/report.json" \
    "$ROOT_DIR/docs/benchmarks/2026-08-10-cache-v0-experiment.json"
cp "$RUN_DIR/output/report.md" \
    "$ROOT_DIR/docs/benchmarks/2026-08-10-cache-v0-experiment.md"

printf 'cache experiment evidence: %s\n' \
    "$ROOT_DIR/docs/benchmarks/2026-08-10-cache-v0-experiment.json"
