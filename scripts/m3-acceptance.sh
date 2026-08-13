#!/usr/bin/env bash
# Archive-first M3 acceptance runner. A checkout pass is deliberately insufficient.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARCHIVE=""
OUT=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --archive) ARCHIVE="$2"; shift 2 ;;
        --out) OUT="$2"; shift 2 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

[ -n "$ARCHIVE" ] || { echo "--archive is required" >&2; exit 2; }
[ -n "$OUT" ] || { echo "--out is required" >&2; exit 2; }
mkdir -p "$OUT"

python3 "$REPO_ROOT/scripts/m3_release_audit.py" "$ARCHIVE" --out "$OUT/archive-audit.json"
"$REPO_ROOT/scripts/m2-full-acceptance.sh" --archive "$ARCHIVE" --out "$OUT/reference-shot" | tee "$OUT/reference-shot.log"
"$REPO_ROOT/scripts/blender-install-test.sh" --archive "$ARCHIVE" --python tests/blender/test_m3_production.py | tee "$OUT/recovery-drill.log"
"$REPO_ROOT/scripts/blender-install-test.sh" --archive "$ARCHIVE" --python tests/blender/test_m3_accessibility.py | tee "$OUT/accessibility.log"
python3 "$REPO_ROOT/scripts/m3_budget_audit.py" \
    --archive "$ARCHIVE" \
    --reference-root "$OUT/reference-shot" \
    --out "$OUT/budget-audit.json"
python3 "$REPO_ROOT/scripts/m3_policy_audit.py" \
    --archive "$ARCHIVE" \
    --out "$OUT/release-policy-audit.json"
echo "M3 archive acceptance runner: PASS $OUT"
