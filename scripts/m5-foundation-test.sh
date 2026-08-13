#!/usr/bin/env sh
# M5 contract foundation: deterministic tier scheduling and bounded cache reads.
set -eu
cargo test -p crowd-core fidelity --lib
cargo test -p crowd-core background_perception_is_deterministically_scheduled --lib
cargo test -p crowd-bench m5_city_flow_records_its_declared_background_mix --bin crowd-bench
cargo test -p crowd-core field --lib
cargo test -p crowd-core fidelity_scheduler --lib
cargo test -p crowd-cache --test cache_lifecycle complete_cache_streams_a_range
cargo test -p crowd-cache --test layout procedural_extraction_keeps_100k_background_agents_as_data
