# M6 deterministic mixed-tier 10K fixture

Status: PASS on 2026-08-19 for the fixed evidence fixture. This is not a
general production performance claim.

The lane runs exactly 10,000 agents for 30 ticks with the checked M5 90/10
distribution split into 10 S0 hero, 990 S1 promoted, and 9,000 S2 background
agents. Rust owns all per-agent and aggregate work; there is no per-agent
Python loop.

## Result

Command:

```bash
scripts/m6-performance-test.sh
```

The runner executes the focused Rust tests in release mode, runs the optimized
fixture twice, compares deterministic fields, and writes the retained first
report to `benchmarks/reports/m6-mixed-tier-10k.json`.

```text
test result: ok. 4 passed; 0 failed
first run:  1540.694 ticks/s
second run: 1788.589 ticks/s
replay: 42031d1b03ded2a5595a31b38d9199c093941ed641a3e8d6b53e5bfb83353b47
```

The checked threshold is at least 10 ticks/s. Both runs passed with zero
hard-safety failures and zero unrelated-agent interaction mutations.

## First-run timings

| Phase | Nanoseconds | Operations |
| --- | ---: | ---: |
| Perception | 4,865,999 | 30,000 |
| Brain | 5,390,375 | 30,000 |
| Activity | 1,324,625 | 3,000 |
| Group | 9,625 | 90 |
| Motion | 5,302,583 | 165,000 |
| Interaction | 369,003 | 30 |

Measured phase time was 17,262,210 ns. Total elapsed time was 19,471,750 ns,
leaving 2,209,540 ns of explicit overhead for deterministic cache-payload
packing and loop/report bookkeeping. Throughput uses total elapsed time, not
the sum of selected phase timings.

The measured host was macOS 27.0 build `26A5416b` on arm64, using Rust and
Cargo 1.94.1. The benchmark binary and focused tests used Cargo's optimized
release profile. These host details qualify this run only; they are not a
portable performance guarantee.

## Tier and evidence accounting

| Tier | Agents | Evidence | Individual records | Aggregate records | Explicitly unavailable |
| --- | ---: | --- | ---: | ---: | --- |
| S0 | 10 | Full | 10 | 0 | None |
| S1 | 990 | Reduced | 990 | 0 | Full per-node trace |
| S2 | 9,000 | Aggregate only | 0 | 1 | Individual perception, brain, and interaction diagnostics |

S2 absence is recorded rather than inferred. The lane performs authoritative
M6 perception and blackboard work for the 1,000 promoted agents, uses the M5
two-tick background cadence for S2 motion classification, and advances coarse
clip phase deterministically for every agent.

## Runtime authorities and safety

- perception: `PerceptionEngine` over the promoted set;
- brain: typed `BlackboardStateV1` writes and change draining;
- activity: `ReservationRuntimeV1` admission and unique ownership;
- group: checked `FormationV1` evaluation at authored offsets;
- motion: `MotionMatcher` over the pinned CC0 database, plus aggregate S2
  scheduling; and
- interaction: `InteractionSchedulerV1` atomic two-agent promotion, lock, and
  completion.

The report accounts separately for activity, motion, and interaction
fallbacks. Each count was zero and each path carries an explicit reason. Hard
safety checks cover promoted perception population, blackboard writes,
reservation capacity/unique ownership, formation completeness/cohesion,
interaction atomicity/completion, cache-record count, and target isolation.

## Motion source ruling

The unchanged CMU candidate remains rejected:

- candidate: `cmu-mocap-subjects-35-36-m6-v1`;
- joint-limit violations: 3,587;
- hard limit: 0; and
- decision: rejected.

The benchmark consumes the accepted checked assets directly:

- database: `assets/reference/m6/motion-database-input-v1.json`;
- database BLAKE3:
  `c687fede242e359fb7b94e91e1c17a44ddacd01963697f2e5f4e687c01998e08`;
- provenance: `assets/reference/m6/motion-provenance-v1.json`;
- provenance BLAKE3:
  `60d1bf5aa98f66ab1a37096876140a53b6bb6d63e03f8a83a5ed7370c895340d`;
- license: `CC0-1.0`; and
- decision: accepted.

The source identities and source ruling are part of the deterministic replay
hash.

## Memory, cache, and replay

- owned working-set capacity: 5,041,920 bytes;
- method: owned vector capacities for fixture state, the evidence cache
  payload, blackboards, and promoted IDs;
- deterministic cache payload: 300,000 records at 16 bytes each, 4,800,000
  bytes;
- cache payload BLAKE3:
  `ec9f768cd865a28ed9f5b428352a98dbf6f79aed6603f40bf1f06199fc8bcc99`;
- final state BLAKE3:
  `2ab84a7130d3d73603602682f308c83fb20250ec527c21e1acfa11f17e093c9c`;
  and
- deterministic replay BLAKE3:
  `42031d1b03ded2a5595a31b38d9199c093941ed641a3e8d6b53e5bfb83353b47`.

The cache payload is a compact deterministic fixture-evidence record, not a
measured production Cache v1 file or a streaming/disk-bandwidth result. The
working-set figure is owned Rust allocation capacity, not process RSS or an OS
peak measurement.

## RED/GREEN evidence

The first focused run failed because the public benchmark module and report
binary did not exist. After implementation, the focused debug and release
suites both passed 4 tests. A self-review regression then failed because the
report did not yet expose exact motion-source authority. The final report now
loads the checked database/provenance, enforces 3,587 greater than the unchanged
zero limit, and carries both source hashes and decisions.

## Unsupported claims

- This 30-tick fixed fixture is not evidence for arbitrary scenes, durations,
  hardware, crowd mixes, or production viewport/render throughput.
- It does not measure a GPU backend, Blender playback, cloth, hair, rigid-body,
  neural motion, or external worker.
- S2 is intentionally aggregate-only; no individual S2 trace is claimed.
- The measured cache payload is fixture evidence, not Cache v1 compression,
  streaming, or disk I/O performance.
