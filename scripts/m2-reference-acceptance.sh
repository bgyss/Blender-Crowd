#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORT_DIR="$REPO_ROOT/benchmarks/reports"
mkdir -p "$REPORT_DIR"
REPORT_PATH="$REPORT_DIR/m2-authorable-reference-$(date -u +%Y-%m-%dT%H-%M-%SZ).json"

M2_REFERENCE_REPORT="$REPORT_PATH" M2_REFERENCE_TICKS="${M2_REFERENCE_TICKS:-10000}" cargo test --release -p crowd-core \
  --test m2_reference_acceptance -- --ignored --exact \
  authorable_reference_1000_agents_emits_queue_group_and_decision_evidence

echo "M2 runtime-evidence report: $REPORT_PATH"
