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

## Fix Round 1 — 2026-08-19

Status: VERIFIED. This section supersedes the pre-review Task 13 completion
evidence above where counts, timings, hashes, lifecycle scope, or support
wording differ. No subagent or reviewer was dispatched.

### Review findings closed

- **Synthetic 10K authority:** every one of the 10,000 runtime agents now
  executes authoritative perception, typed-blackboard brain work, reservation,
  formation, evidence-cache output, and one atomic interaction. S0/S1 execute
  motion matching every tick; S2 executes the checked two-tick M5 cadence.
  Tier counts, phase operations, cache records, fallbacks, hard-safety failures,
  and unrelated mutations are derived from runtime state.
- **Blender bypassed Rust motion validation:** the live load operator now sends
  the complete interaction layer and motion JSON through
  `blender_crowd_native.validate_interaction_motion_attachment` before lowering.
  Rust accepts valid interval/root/contact/provenance evidence and rejects
  incomplete roots, invalid contacts, and invalid provenance.
- **Incomplete physics/hero lifecycle:** physics bindings carry the complete
  cache hash, target IDs, and interval; cached samples must cover every declared
  tick. Native inspection exposes `physics_active`, and the host smoke proves
  attach, mute, unmute, remove, and reload at ticks 15 and 25. Hero cloth is
  explicitly `declaration-only unsupported` and `not attached`, with its
  requested cache/target/interval binding visible rather than implied as a run.
- **M4 override loss:** playback retains independent M4 and M6 lists and
  composes both. An unrelated-agent M4 transform survives M6 attach, failed
  replacement, mute, unmute, remove, and reload.
- **Non-atomic attachment:** candidate native layout state is validated and
  played at the current tick before Python commits either list. Failure restores
  the previous native stack. The invalid-cache-target host case proves the old
  M6 and M4 states remain active after native rejection.
- **Stale removal labels:** removal resets owner, interval, contacts,
  provenance, recovery, failure policy, and hero boundary to their explicit
  `No M6 ... loaded` states.

### Focused RED/GREEN evidence

The inherited pure-Python test first failed with
`KeyError: 'hero_execution_status'`; after the explicit boundary/binding change:

```text
python3 -m unittest -v tests/test_m6_layer_bundle.py
Ran 3 tests in 0.005s
OK
```

The inherited host smoke first failed at the hero wording, then exposed missing
`physics_active`, then proved the Rust validator was live by rejecting the
intentionally incomplete root artifact. The final native focused result was:

```text
cargo test -p crowd-blender --lib
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The mixed-tier test added exact per-tier runtime and isolation assertions. Its
initial per-tier-isolation compile failed because `TierEvidence` did not yet
carry `unrelated_agent_mutations`; after runtime delta accounting:

```text
cargo test -p crowd-bench --test m6_mixed_tier
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Exact optimized two-pass evidence

Command:

```text
scripts/m6-performance-test.sh
```

Exact significant output:

```text
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
M6 mixed-tier: 10000 agents, 126.007 ticks/s, replay a66bdd02ead627f21d161dec5dc03a7b4575bb169ef37768c4c51c338f06b0fd
M6 mixed-tier: 10000 agents, 124.653 ticks/s, replay a66bdd02ead627f21d161dec5dc03a7b4575bb169ef37768c4c51c338f06b0fd
M6 mixed-tier performance passed: 126.007 ticks/s; replay a66bdd02ead627f21d161dec5dc03a7b4575bb169ef37768c4c51c338f06b0fd
```

The retained first run recorded 238,082,792 ns total elapsed,
213,564,253 ns across the six separately timed phases, 24,518,539 ns explicit
overhead, 31,895,245 bytes of owned-allocation lower-bound working set, and a
17,700,000-byte deterministic evidence payload (300,000 records at 59 bytes).
It recorded zero fallbacks, zero hard-safety failures, and zero unrelated-agent
mutations globally and for S0, S1, and S2.

S2 runtime totals were 270,000 perception, brain, activity, and group
operations; 135,000 motion operations; 9,000 completed interaction operations;
and 270,000 evidence-cache records. S2 remains aggregate-only: individual
perception, brain, and interaction diagnostics are explicitly unavailable.

Deterministic hashes:

```text
replay: a66bdd02ead627f21d161dec5dc03a7b4575bb169ef37768c4c51c338f06b0fd
final state: 2b6e804be85489bbe888d1ec285d25c2934acc02b91312f8bfc3a9aa42215e3d
evidence payload: 71d613da5353c5f10ef617f7027b7c6b76ed6cf09cde1eb22550e8406f522762
```

Environment: macOS 27.0 build `26A5416b`, arm64, Rust 1.94.1, Cargo
1.94.1, optimized release profile.

### Exact host Blender / Metal evidence

Command executed outside the restricted automation sandbox:

```text
scripts/m6-blender-test.sh
```

Significant output:

```text
M6 debugger Blender smoke: PASS
Error: E_INTERACTION_MOTION: root samples must cover the complete request interval; agent 2506968674689638394 motion roots must cover the complete interval
Error: E_LAYOUT: layer m6-animation-interaction-pair-10293130296351569156-15 targets an agent absent from the base
Info: M6 layers muted
Info: M6 layers unmuted
Info: M6 layers removed; source artifacts and base cache retained
M6 Blender physics/hero layers: PASS
Blender 5.2.0 LTS (hash fbe6228777e7 built 2026-07-14 01:31:22)
```

Both Blender processes reached Python with normal host Metal access; there was
no `gpu::MTLBackend::metal_is_supported` or pre-Python Metal abort.

### Regression and quality gates

```text
scripts/m6-foundation-test.sh
M6 R0 foundation passed

cargo fmt --all -- --check
exit 0

cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo]

git diff --check
exit 0
```

### Updated durable evidence

- `docs/benchmarks/2026-08-18-m6-blender-layers.md`
- `docs/benchmarks/2026-08-18-m6-mixed-tier.md`
- generated retained JSON: `benchmarks/reports/m6-mixed-tier-10k.json`
  (ignored by Git and reproducible with the performance runner)

### Remaining boundaries

- Cloth, hair, and Geometry Nodes deformation remain declaration-only and are
  neither attached nor benchmarked.
- The cached deterministic handoff is not Blender rigid-body parity or evidence
  for arbitrary collision scenes.
- Neural motion and external model workers remain unsupported and unmeasured.
- The 30-tick fixed fixture is not production-scene, long-duration, GPU,
  viewport/render, Cache v1 disk/streaming, or artist-usability evidence.
- The CMU candidate remains rejected at 3,587 joint-limit violations against
  the unchanged hard limit of zero; the checked CC0 baseline remains the
  accepted motion source.
- Task 14 still owns milestone-level M6 acceptance promotion.
