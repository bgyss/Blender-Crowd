# Task 13 — Blender physics/hero layers and mixed-tier performance

Status: DONE

## Implementation

- Added cache-bound Blender attachment for deterministic paired-interaction,
  contact, physics-transition, and hero-support artifacts. Attachment requires
  the complete Cache v1 `base_cache_hash` and exposes owners, intervals,
  contacts, interaction/motion/solver provenance, recovery, failure policies,
  and declared hero support boundaries.
- Lowered sparse interaction edits and cached physics handoff samples into the
  existing native layout composer. M6 overlays use a dedicated playback list,
  preserving the existing M4 stack and immutable base cache while affecting
  only declared target IDs and intervals.
- Made native selected-agent inspection read the same composed layout state as
  viewport playback, without moving simulation or interaction authority out of
  Rust.
- Added Blender operators and UI for load, mute/unmute, remove, inspect, and
  reload lifecycle actions.
- Added a backend-neutral Rust benchmark lane for exactly 10,000 agents over 30
  ticks: 10 S0 hero, 990 S1 promoted, and 9,000 S2 background. It times
  perception, brain, activity, group, motion, and interaction separately;
  accounts for fallbacks and evidence degradation; checks deterministic replay,
  hard safety, and unrelated-agent isolation; and uses total elapsed time for
  the 10 ticks/s gate.

## RED / GREEN

The inherited Task 13 slice retained its original RED evidence in the two dated
benchmark reports. Before implementation, the Blender runner reached Blender
but failed because `bpy.ops.crowd.load_m6_layers` did not exist. The Rust focused
test failed because the public mixed-tier module and report binary did not
exist. A later Blender GREEN attempt exposed that `read_tick` composed layout
layers while `inspect_agent` still returned base animation state; the native
inspection bridge was corrected to inspect the same composed state.

Fresh takeover verification found no failing production behavior. The only
completion defect was stale checked benchmark prose from an earlier run; its
timings and hashes were updated from the exact fresh optimized run below.

## Focused tests

```text
python3 -m unittest -q tests/test_m6_layer_bundle.py
Ran 3 tests in 0.006s
OK
```

```text
cargo test -p crowd-blender --quiet
running 9 tests
.........
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

```text
cargo test -p crowd-bench --test m6_mixed_tier
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

```text
cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s)
```

`cargo fmt --all`, `cargo fmt --all -- --check`, and `git diff --check` also
exited zero.

## Exact optimized performance run

Command:

```text
scripts/m6-performance-test.sh
```

Exact output, with only machine-local repository and temporary-directory paths
normalized to `<repo>` and `<tmp>`:

```text
   Compiling crowd-bench v1.0.0 (<repo>/crates/crowd-bench)
    Finished `release` profile [optimized + debuginfo] target(s) in 11.73s
     Running tests/m6_mixed_tier.rs (target/release/deps/m6_mixed_tier-2174b9611cbfb07e)

running 4 tests
test checked_fixture_has_exact_m5_mix_and_debug_evidence_boundaries ... ok
test mixed_tier_run_reports_each_authoritative_phase_and_hard_safety ... ok
test replay_hash_excludes_measurement_noise_but_covers_output_state_and_accounting ... ok
test report_binary_writes_the_fixed_fixture_and_rejects_unknown_arguments ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s

   Compiling crowd-bench v1.0.0 (<repo>/crates/crowd-bench)
    Finished `release` profile [optimized + debuginfo] target(s) in 5.90s
     Running `target/release/m6-mixed-tier --out <repo>/benchmarks/reports/m6-mixed-tier-10k.json`
M6 mixed-tier: 10000 agents, 1540.694 ticks/s, replay 42031d1b03ded2a5595a31b38d9199c093941ed641a3e8d6b53e5bfb83353b47 -> <repo>/benchmarks/reports/m6-mixed-tier-10k.json
    Finished `release` profile [optimized + debuginfo] target(s) in 0.03s
     Running `target/release/m6-mixed-tier --out <tmp>/blender-crowd-m6-mixed-tier.n8dQHX.json`
M6 mixed-tier: 10000 agents, 1788.589 ticks/s, replay 42031d1b03ded2a5595a31b38d9199c093941ed641a3e8d6b53e5bfb83353b47 -> <tmp>/blender-crowd-m6-mixed-tier.n8dQHX.json
M6 mixed-tier performance passed: 1540.694 ticks/s; replay 42031d1b03ded2a5595a31b38d9199c093941ed641a3e8d6b53e5bfb83353b47; report <repo>/benchmarks/reports/m6-mixed-tier-10k.json
```

The first retained report recorded 17,262,210 ns of phase time, 19,471,750 ns
total elapsed, 2,209,540 ns explicit overhead, zero hard-safety failures, zero
unrelated-agent interaction mutations, zero fallbacks, 4,800,000 cache-payload
bytes, and 5,041,920 bytes of owned Rust allocation capacity. The exact phase
timings and evidence limits are in
`docs/benchmarks/2026-08-18-m6-mixed-tier.md`.

Environment: macOS 27.0 build `26A5416b`, arm64, Rust 1.94.1, Cargo 1.94.1,
optimized release profile.

## Host Blender / Metal

The required runner was executed outside the restricted automation sandbox, so
both Blender processes had normal macOS Metal access. Blender reached Python;
there was no pre-Python Metal abort.

Command:

```text
scripts/m6-blender-test.sh
```

Exact significant output:

```text
Built wheel for abi3 Python >= 3.11 to addon/blender_crowd/wheels/blender_crowd_native-1.0.0-cp311-abi3-macosx_11_0_arm64.whl
M6 debugger Blender smoke: PASS
Blender 5.2.0 LTS (hash fbe6228777e7 built 2026-07-14 01:31:22)
M6 Blender physics/hero layers: PASS
Blender 5.2.0 LTS (hash fbe6228777e7 built 2026-07-14 01:31:22)
```

The full Blender layer result, cache/manifest identity checks, lifecycle proof,
and unsupported boundaries are recorded in
`docs/benchmarks/2026-08-18-m6-blender-layers.md`.

## M6 foundation regression

```text
scripts/m6-foundation-test.sh
M6 R0 foundation passed
```

The runner passed every invoked Rust lane and all 31 pure-Python tests after the
Task 13 performance and host Blender runs.

## Files changed

- `addon/blender_crowd/cache_playback.py`
- `addon/blender_crowd/m6_interaction.py`
- `addon/blender_crowd/m6_physics.py`
- `addon/blender_crowd/operators.py`
- `addon/blender_crowd/panels.py`
- `addon/blender_crowd/properties.py`
- `crates/crowd-bench/src/lib.rs`
- `crates/crowd-bench/src/bin/m6-mixed-tier.rs`
- `crates/crowd-bench/src/m6_mixed_tier.rs`
- `crates/crowd-bench/tests/m6_mixed_tier.rs`
- `crates/crowd-blender/src/lib.rs`
- `scripts/m6-performance-test.sh`
- `scripts/m6-blender-test.sh`
- `tests/blender/test_m6_layers.py`
- `tests/test_m6_layer_bundle.py`
- `docs/benchmarks/2026-08-18-m6-blender-layers.md`
- `docs/benchmarks/2026-08-18-m6-mixed-tier.md`
- `.superpowers/sdd/2026-08-18-m6-advanced-agency-motion/task-13-report.md`

## Concerns and unsupported claims

- The Blender cloth declaration is a validated support boundary only. This run
  does not execute or benchmark Blender cloth, hair, Geometry Nodes deformation,
  rigid-body parity, arbitrary collision scenes, or neural motion.
- The deterministic native physics handoff and fixed 30-tick mixed-tier fixture
  do not establish production visual quality, arbitrary-scene performance,
  long-duration stability, GPU throughput, Cache v1 disk/streaming throughput,
  or artist usability.
- S2 evidence is intentionally aggregate-only; absent per-agent perception,
  brain, and interaction diagnostics are explicitly unavailable and are not
  inferred.
- The unchanged CMU candidate remains rejected at 3,587 joint-limit violations
  against the hard limit of zero. The benchmark uses the checked CC0 baseline;
  it does not promote the CMU candidate or weaken that gate.
- `scripts/m6-acceptance.sh` remains the milestone-level open audit. Task 14
  owns final requirement promotion; Task 13 passing is not full M6 acceptance.

No subagent or reviewer was dispatched.
