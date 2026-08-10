#!/usr/bin/env bash
# Build the native module as an abi3 wheel for the Blender extension.
#
# abi3 rather than a cp3xx-specific wheel: Blender 5.2 resolves an `abi3` tag
# as "any CPython 3" and lets it override the `cp3xx` tag, so one wheel keeps
# working if a future Blender ships a newer CPython. (This was broken in 4.2
# and 4.3 -- see blender issue #130561 -- and is fixed in 5.2.)
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="$REPO_ROOT/addon/blender_crowd/wheels"

if command -v maturin >/dev/null; then
    MATURIN="$(command -v maturin)"
elif command -v mise >/dev/null && MATURIN="$(mise which maturin 2>/dev/null)"; then
    : # Use the repository-pinned mise install even when shims are not on PATH.
else
    echo "maturin is required: mise install (or 'uv tool install maturin==1.9.6')" >&2
    exit 1
fi

# Blender's `extension build` fails with a bare Errno 2 rather than creating
# a missing output directory, so every directory is made explicitly.
mkdir -p "$OUT_DIR"
rm -f "$OUT_DIR"/*.whl

"$MATURIN" build \
    --release \
    --manifest-path "$REPO_ROOT/crates/crowd-blender/Cargo.toml" \
    --out "$OUT_DIR"

echo "built: $(ls "$OUT_DIR"/*.whl)"
