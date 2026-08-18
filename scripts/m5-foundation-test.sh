#!/usr/bin/env sh
# M5 contract foundation: deterministic tier scheduling, per-tier quality
# accounting, the checked-in per-tier gate, bounded cache reads, tier-transition
# behavior, and CPU-fallback contract compatibility.
set -eu

# Tier scheduling and the declared scale profile.
cargo test -p crowd-core fidelity --lib
cargo test -p crowd-core profile_assignment_is_derived_from_stable_id_not_spawn_order --lib
cargo test -p crowd-core background_perception_is_deterministically_scheduled --lib
cargo test -p crowd-core sparse_s2_tick_reuses_last_solved_target_not_current_velocity --lib
cargo test -p crowd-core sparse_s2_braking_is_counted_as_continuous_not_accumulated_samples --lib
cargo test -p crowd-core m5_city_flow_uses_parallel_lane_strips_without_an_entry_funnel --lib
cargo test -p crowd-bench m5_city_flow_records_its_declared_background_mix --bin crowd-bench

# Per-tier quality accounting, which the fixed thresholds are stated against.
cargo test -p crowd-core per_tier --lib
cargo test -p crowd-core tiers_no_agent_holds_are_omitted --lib

# The checked-in per-tier thresholds and their adjudicator.
cargo test -p crowd-bench m5_gate --lib
cargo test -p crowd-bench a_produced_report_parses_as_the_gate_sees_it --bin crowd-bench

# 10K gate items 3 and 4: transitions and animation scheduling.
cargo test -p crowd-core --test m5_tier_transitions

# 10K gate item 5: CPU fallback and the backend parity harness.
cargo test -p crowd-core --test m5_cpu_fallback

# Backend-neutral field kernel and bounded cache streaming.
cargo test -p crowd-core field --lib
cargo test -p crowd-core fidelity_scheduler --lib
cargo test -p crowd-cache --test cache_lifecycle complete_cache_streams_a_range
cargo test -p crowd-cache --test layout procedural_extraction_keeps_100k_background_agents_as_data

# Blender playback, render, and scale/profiling UI evidence is a separate
# runner because it needs Blender and normal host Metal access:
#   scripts/m5-blender-test.sh
