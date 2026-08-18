# M5 — Scale, GPU tiers, and procedural rendering

## Objective

Earn industrial background-crowd scale through explicit simulation and render
tiers, cache streaming, procedural instancing, and measured CPU/GPU backends—
first at 10,000 agents and only then at 100,000.

## Sources of truth

- [Blender Crowd 1.0 sections 8, 13 Phase 4, and 17](../blender-crowd-1.0.md)
- [Industrial capability roadmap fidelity model](../industrial-crowd-capability-roadmap.md#fidelity-model)
- [M4 layered layout and interchange](M4-layout-interchange.md)
- [UI/UX roadmap](../ui-ux-roadmap.md)

## Prerequisites

M3 is accepted. M4's procedural extraction and layer/cache changes are stable or
coordinated as prerequisites. The 1K quality baseline remains a regression gate
for individually simulated S0/S1 agents.

## In scope

1. Explicit point, capsule, skeleton, full-character, and hero presentation/
   physics policies mapped to independent S0-S3 simulation and R0-R4 render
   tiers, with deterministic transitions and hysteresis.
2. Shared paths, flow/density/velocity fields, scheduled perception/decisions,
   camera/focus-prioritized animation evaluation, and tier promotion without
   freezing authoritative root motion or identity.
3. Streaming cache/index formats, partial range loading, background extraction,
   bounded viewport debug data, culling, proxies, and render-time prototypes.
4. Measured GPU candidates for spatial queries, density/flow fields, simple
   perception, steering, or coarse behavior. Backend contracts may target CPU,
   Metal, CUDA, or Vulkan, but support is declared only for implemented and
   tested paths.
5. Reference scenes for stadium/seating, bidirectional city flow, formations,
   and mixed 40-hero/1,000-animated/background populations.
6. Published quality/performance/memory/cache/playback/render reports at 1K,
   10K, and, after 10K acceptance, 100K.

## UI/UX goals and gate

- Make S0-S3 simulation tiers, R0-R4 render tiers, promotion rules, hysteresis,
  culling, proxies, and quality limitations inspectable without requiring users
  to infer them from performance symptoms.
- Provide preflight estimates for memory, cache size, extraction cost, playback,
  and render impact, and distinguish measured values from estimates.
- Keep long scale operations responsive with stage progress, throughput, elapsed
  time, bounded diagnostics, cancellation, and explicit partial/recoverable
  states.
- Provide a scale/profiling view that relates bottlenecks to populations, tiers,
  cameras, cache ranges, backends, and fallbacks rather than exposing only global
  frame time.
- Use aggregation and drill-down for metrics and debug evidence; the UI must not
  attempt to list or draw every agent at 10K or 100K.

The M5 UI gate is evaluated separately at 10K and 100K. An artist must configure
the declared tier mix, understand its quality/cost tradeoff, identify an injected
bottleneck, cancel and resume one long operation, and explain the active fallback
from the interface. Capture responsiveness, time on task, memory overhead of the
UI/debug views, and screenshots of estimates versus measured results.

## Explicit exclusions

No claim that 100,000 agents are fully autonomous, skinned heroes. No GPU path
may change public graph semantics, stable identity, layer composition, or cache
meaning. Cross-vendor parity is not claimed from a single device.

## Required artifacts

- Tier scheduler and transition tests; backend-neutral kernel interfaces;
  streaming cache/extraction implementation; procedural presentation assets.
- Scale fixtures, runners, environment capture, baselines, visual evidence, and
  dated 10K and 100K reports.
- Backend/support matrix describing correctness, determinism mode, fallback,
  driver/API requirements, and unsupported features.

`m5_city_flow` is the dedicated background-flow fixture. It is excluded from
the legacy baseline sweep and must be invoked by name for M5 evidence.

Current backend availability is recorded in the
[backend support matrix](../backend-support-matrix.md). It is intentionally a
support boundary, not evidence that an unimplemented backend has passed M5.

Use the [10K and 100K scale-gate runbook](../runbooks/m5-scale-gates.md) for
the exact long-running command-line procedure and evidence checklist.

The [10K gate report](../benchmarks/2026-08-14-m5-10k.md) is the accepted
result. Per-tier quality is adjudicated against fixed thresholds in
`benchmarks/thresholds/m5-city-flow.json`, compiled into `crowd-bench` and
applied by `crowd-bench m5-gate`. The same file gates 1K, 10K, and 100K,
because its limits are rates per observed agent-tick rather than counts.

The earlier [10K failed baseline](../benchmarks/2026-08-13-m5-10k-failed-baseline.md)
is retained as optimization evidence. It is not an accepted gate, and its
rejected candidates are replayed through the threshold file as a test that the
bar still discriminates.

## 10K acceptance gate

1. The declared 10K scene and tier mix meet fixed simulation, memory, cache,
   viewport/playback, and render budgets on each claimed configuration.
2. Destination, penetration, stall, throughput, oscillation, and group metrics
   remain within declared thresholds for the fidelity assigned to each tier.
3. Tier transitions do not change stable IDs, create visible popping beyond the
   accepted presentation tolerance, lose interaction state, or invalidate layers.
4. Camera/focus animation scheduling changes evaluation cost, not cached root
   trajectories or required contacts.
5. CPU fallback produces contract-compatible output, with documented numeric
   tolerance where bitwise parity is not demonstrated.

The 10K gate passed on 2026-08-14. Its five items map to checked-in evidence:
`crowd-bench m5-gate` for items 1 and 2, `crates/crowd-core/tests/m5_tier_transitions.rs`
for items 3 and 4, `crates/crowd-core/tests/m5_cpu_fallback.rs` for item 5, and
`scripts/m5-blender-test.sh` for the viewport/playback and render budgets.

The M5 UI gate's artist task — configuring the mix, identifying an injected
bottleneck, cancelling and resuming a long operation, and explaining the active
fallback from the interface, with responsiveness and time-on-task captured —
has **not** been conducted. The panel now carries the information that task
needs; the task itself remains outstanding for both scales.

The first 100K attempt failed; see
[2026-08-14-m5-100k-failed.md](../benchmarks/2026-08-14-m5-100k-failed.md). It
completed all 100,000 agents but missed the throughput budget (5.27 ticks/s
against 10) and two per-tier quality limits. That report also records the
optimisation round it prompted — a 3.08x measured speedup with bitwise-identical
results — and raises a specification defect in two of the threshold file's
limits that needs a contract decision rather than an edit.

That decision was taken, and resolving it uncovered two further defects in the
metrics rather than in the solver. All three are recorded in
[2026-08-15-m5-100k-scale-invariance.md](../benchmarks/2026-08-15-m5-100k-scale-invariance.md):
two gated figures were not scale-invariant and are now reported rather than
gated; background-tier contact was undercounted 2x because a skipped perception
tick counted as clean exposure; and the solver fix the first report proposed is
refuted by measurement and ships disabled. Every metric change is
behaviour-neutral — `final_state_hash` is unchanged at both scales — so no M0-M4
evidence was invalidated.

## 100K acceptance gate

**The 100K gate passed on 2026-08-18.** See
[2026-08-18-m5-100k.md](../benchmarks/2026-08-18-m5-100k.md).

The 100K work starts only after the 10K report passes. It repeats the same proof
categories with a declared tier mix and must show that streaming and procedural
extraction avoid expanding all characters into Blender scene objects. Any public
headline states the number of S0/S1/S2/S3 and R0-R4 agents, hardware, frame/tick
rates, quality limitations, cache size, and render path.

The disclosure that rule requires, for the accepted run:

| Field | Value |
| --- | --- |
| Population | 100,000: 10,029 S1/R1 and 89,971 S2/R2; no S0, S3, R0, R3, or R4 |
| Hardware | Apple M1 Max, 64 GiB, macOS aarch64; Blender 5.2 LTS |
| Simulation rate | 13.696 ticks/s against a 30 tick/s scene — **about 0.46x real time**, a bake-and-cache workflow, not interactive |
| Completion | 100,000 of 100,000 agents arrive |
| Cache | 0.67 GiB for 120 frames (5.7 MiB/frame), f32, 120-tick chunks, 0.0 m position error, cancellation recovered |
| Render path | Procedural. One scene object carries all 100,000 agents as point data; at the inspected frame 1,200 agents were present and evaluated as 1,200 procedural instances (R1 128, R2 1,072). Render 1.381 s, bake 72.197 s |
| Quality | Every per-tier limit met with 1.7x-3.0x margin |

Two limitations belong with any citation of that headline. The render evidence
proves the population is **not** expanded into per-agent scene objects; it does
not show 100,000 agents drawn in one frame, because the reference scene emits
over time and 1,200 were present at the frame inspected. And the residual scale
trend is unexplained: both tiers' contact rates rise ~1.74x from 40K to 100K,
which points at fixture geometry rather than the solver, so a 1M claim would
need its own calibration rather than an extrapolation.

The M5 UI gate's artist task remains outstanding at both scales, so M5 is
functionally accepted but not operator-validated.

## Validation and proof

Run determinism/tolerance comparisons, tier-transition stress, streaming seek
and corruption tests, GPU/CPU contract comparisons, viewport/playback profiling,
render extraction tests, and all scale scenes from reproducible release builds.
Visual demos supplement the numerical and failure evidence.

## Definition of done and stop conditions

M5 is done only when both scale gates have separate passing reports. Stop before
100K if 10K fails, and stop a backend claim on correctness, determinism, driver,
memory, or portability failure rather than weakening the shared contracts.
