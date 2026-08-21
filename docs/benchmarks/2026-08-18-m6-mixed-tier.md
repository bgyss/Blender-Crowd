# M6 deterministic mixed-tier 10K fixture

Status: PASS on 2026-08-19 for the fixed evidence fixture. This is not a
general production performance claim.

The lane runs exactly 10,000 agents for 30 ticks with the checked M5 split: 10
S0 hero, 990 S1 promoted, and 9,000 S2 background. Rust owns all per-agent
work and evidence generation; there is no per-agent Python loop.

## Optimized two-pass result

Command:

```bash
scripts/m6-performance-test.sh
```

The runner executes the focused Rust tests in release mode, runs the optimized
fixture twice, compares deterministic fields, and writes the retained first
report to `benchmarks/reports/m6-mixed-tier-10k.json`.

```text
test result: ok. 5 passed; 0 failed
first run:  126.007 ticks/s
second run: 124.653 ticks/s
replay: a66bdd02ead627f21d161dec5dc03a7b4575bb169ef37768c4c51c338f06b0fd
```

Both runs exceeded the fixed 10 ticks/s threshold and produced the same replay
hash. The retained run recorded zero fallbacks, zero hard-safety failures, and
zero unrelated-agent interaction mutations globally and for every tier.

## Authoritative per-tier work

All counts below are aggregated from each runtime agent's output counters. The
report derives tier counts from runtime state (`tier_counts_source` is
`runtime_state`) and rejects missing tier work or phase/tier reconciliation
failures as hard-safety failures.

| Tier | Agents | Perception | Brain | Activity | Group | Motion | Interaction | Cache records |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| S0 | 10 | 300 | 300 | 300 | 300 | 300 | 10 | 300 |
| S1 | 990 | 29,700 | 29,700 | 29,700 | 29,700 | 29,700 | 990 | 29,700 |
| S2 | 9,000 | 270,000 | 270,000 | 270,000 | 270,000 | 135,000 | 9,000 | 270,000 |

Every agent executes authoritative perception, typed-blackboard brain work,
resource reservation, formation evaluation, and one atomic interaction during
the run. S0/S1 execute motion matching every tick; S2 executes the checked M5
two-tick motion cadence. Every agent emits one deterministic evidence-cache
record per tick.

## Retained first-run timings

| Phase | Nanoseconds | Operations |
| --- | ---: | ---: |
| Perception | 50,904,833 | 300,000 |
| Brain | 58,771,587 | 300,000 |
| Activity | 75,132,747 | 300,000 |
| Group | 3,896,375 | 300,000 |
| Motion | 16,850,753 | 165,000 |
| Interaction | 8,007,958 | 10,000 |

Measured phase time was 213,564,253 ns. Total elapsed time was 238,082,792 ns,
leaving 24,518,539 ns of explicit overhead for deterministic evidence packing
and loop/report bookkeeping. Throughput uses total elapsed time, not the sum of
selected phase timings.

The measured host was macOS 27.0 build `26A5416b` on arm64 with Rust and Cargo
1.94.1. The benchmark binary and focused tests used Cargo's optimized release
profile. These details qualify this run only; they are not a portable
performance guarantee.

## Evidence, fallback, safety, and isolation

| Tier | Evidence | Individual records | Aggregate records | Fallbacks | Hard safety | Unrelated mutations | Explicitly unavailable |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| S0 | Full | 10 | 0 | 0 | 0 | 0 | None |
| S1 | Reduced | 990 | 0 | 0 | 0 | 0 | Full per-node trace |
| S2 | Aggregate only | 0 | 30 | 0 | 0 | 0 | Individual perception, brain, and interaction diagnostics |

S2 evidence is generated from runtime operation, cache, fallback, safety, and
interaction-isolation counters. Individual S2 diagnostics remain unavailable
and are recorded as such rather than inferred.

Interaction isolation compares before/after runtime interaction state with the
set of agents actually promoted, locked, completed, and released in each tick.
The per-tier mutation totals sum to the global total. Phase timing operation
counts likewise reconcile exactly with the sum of the three tier counters.

Fallback accounting is present for perception, brain, activity, group, motion,
and interaction. Every count was zero; each field still carries the specific
fallback reason it would represent.

## Memory, cache, and replay

- owned working-set lower bound: 31,895,245 bytes;
- agent state: 560,000 bytes;
- world storage: 2,605,056 bytes;
- blackboards: 1,179,648 bytes;
- activity inputs: 806,000 bytes;
- group runtime: 1,043,421 bytes;
- interaction requests: 8,001,120 bytes;
- deterministic fixture evidence payload: 300,000 records at 59 bytes each,
  17,700,000 bytes;
- evidence payload BLAKE3:
  `71d613da5353c5f10ef617f7027b7c6b76ed6cf09cde1eb22550e8406f522762`;
- final runtime state BLAKE3:
  `2b6e804be85489bbe888d1ec285d25c2934acc02b91312f8bfc3a9aa42215e3d`;
  and
- deterministic replay BLAKE3:
  `a66bdd02ead627f21d161dec5dc03a7b4575bb169ef37768c4c51c338f06b0fd`.

The working-set figure is an owned-allocation lower bound, not process RSS or
an OS peak. The payload is deterministic fixture evidence, not a measured
Cache v1 file or a disk/streaming-throughput result.

## Motion source ruling

The unchanged CMU candidate remains rejected at 3,587 joint-limit violations
against the hard limit of zero. The benchmark consumes the checked CC0
`reference-humanoid-motion` database and pinned provenance instead; both source
decisions and hashes participate in the replay identity.

## RED/GREEN evidence

The inherited review found that only promoted agents reached several runtime
authorities while S2 work and evidence were largely synthetic. The fixed test
now requires every tier to report non-zero authoritative work in all six phases,
exact S2 cadence/counts, per-tier cache/fallback/safety/isolation outputs, and
phase-operation reconciliation. The debug suite passes 5 tests, and the same 5
tests pass in the optimized two-pass runner.

## Unsupported claims

- This fixed 30-tick fixture is not evidence for arbitrary scenes, durations,
  hardware, crowd mixes, or production viewport/render throughput.
- It does not measure a GPU backend, Blender playback, cloth, hair, rigid-body,
  neural motion, or an external worker.
- S2 remains aggregate-only; no individual S2 trace is claimed.
- The evidence payload is not Cache v1 compression, streaming, or disk I/O
  performance.
