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

## 100K acceptance gate

The 100K work starts only after the 10K report passes. It repeats the same proof
categories with a declared tier mix and must show that streaming and procedural
extraction avoid expanding all characters into Blender scene objects. Any public
headline states the number of S0/S1/S2/S3 and R0-R4 agents, hardware, frame/tick
rates, quality limitations, cache size, and render path.

## Validation and proof

Run determinism/tolerance comparisons, tier-transition stress, streaming seek
and corruption tests, GPU/CPU contract comparisons, viewport/playback profiling,
render extraction tests, and all scale scenes from reproducible release builds.
Visual demos supplement the numerical and failure evidence.

## Definition of done and stop conditions

M5 is done only when both scale gates have separate passing reports. Stop before
100K if 10K fails, and stop a backend claim on correctness, determinism, driver,
memory, or portability failure rather than weakening the shared contracts.
