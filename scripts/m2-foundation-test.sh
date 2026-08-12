#!/bin/sh
set -eu

cargo test -p crowd-core --test behavior_graph --test behavior_runtime
cargo test -p crowd-core --test authorable_project --test authorable_runtime --test groups_queues --test assets_motion
cargo test -p crowd-cache --test override_layer --test override_layer_v2
# PyO3 extension tests require a Python development library at link time.  Keep
# this runner useful on a Blender-only workstation by type-checking that target
# by default; CI or a configured development host can opt in to its link/run.
if [ "${CROWD_RUN_EMBEDDED_PYTHON_TESTS:-0}" = "1" ]; then
    cargo test -p crowd-blender --lib
else
    cargo check -p crowd-blender --tests
fi
cargo clippy --workspace --all-targets -- -D warnings
