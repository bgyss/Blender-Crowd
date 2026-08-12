#!/usr/bin/env bash
# Build, package, and archive-audit one platform-specific 1.0 candidate.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLENDER="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"
OUT=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --out) OUT="$2"; shift 2 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done
[ -n "$OUT" ] || { echo "--out is required" >&2; exit 2; }
[ ! -e "$OUT" ] || { echo "refusing to overwrite existing output: $OUT" >&2; exit 2; }
mkdir -p "$OUT"

"$REPO_ROOT/scripts/build-wheel.sh"
STAGE="$OUT/extension-source"
python3 "$REPO_ROOT/scripts/m3_stage_release.py" --out "$STAGE"
"$BLENDER" --command extension validate "$STAGE"
"$BLENDER" --command extension build --source-dir "$STAGE" --output-dir "$OUT"
ARCHIVE="$(find "$OUT" -maxdepth 1 -name 'blender_crowd-*.zip' -print -quit)"
[ -n "$ARCHIVE" ] || { echo "Blender did not produce an extension archive" >&2; exit 1; }
python3 "$REPO_ROOT/scripts/m3_release_audit.py" "$ARCHIVE" --out "$OUT/archive-audit.json"
shasum -a 256 "$ARCHIVE" > "$OUT/SHA256SUMS"
echo "M3 candidate archive: $ARCHIVE"
