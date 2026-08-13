#!/usr/bin/env sh
# Local M4 contract checks: composition, migration, cache-only bridge, and artifacts.
set -eu
cargo test -p crowd-cache --test layout
cargo test -p crowd-blender --lib
python3 -m unittest -q tests/test_m4_layout_artifacts.py
