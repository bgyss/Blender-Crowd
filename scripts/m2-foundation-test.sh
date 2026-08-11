#!/bin/sh
set -eu

cargo test -p crowd-core --test behavior_graph --test behavior_runtime
cargo test -p crowd-core --test authorable_project --test authorable_runtime --test groups_queues --test assets_motion
cargo test -p crowd-cache --test override_layer --test override_layer_v2
cargo test -p crowd-blender --lib
cargo clippy --workspace --all-targets -- -D warnings
