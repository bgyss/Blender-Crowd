# M0 — Proving grounds

## Objective

Retire the highest-risk architecture assumptions with measured headless and
Blender bridge evidence before building polished authoring UI.

## Sources of truth

- [Blender Crowd 1.0 sections 3, 6, 8, 9, and 13](../blender-crowd-1.0.md)
- [Industrial capability roadmap](../industrial-crowd-capability-roadmap.md)
- [Kernel slice design](../superpowers/specs/2026-08-04-crowd-sim-kernel-design.md)
- [Kernel slice implementation plan](../superpowers/plans/2026-08-04-crowd-sim-kernel.md)

## Prerequisites and baseline

The repository contains the contracts and an unexecuted implementation plan.
No build, benchmark, Blender bridge, or automated test result is currently
claimed. Execute the kernel plan first and preserve its measured-evidence gate.

## In scope

1. Fixed clock, units, coordinates, stable IDs, deterministic randomness,
   structure-of-arrays state, spatial index, ordered tick phases, and metrics.
2. Five checked-in avoidance scenes at 100/500/1,000/2,000 agents.
3. Sampled-velocity, reciprocal/ORCA-style, and scoped anticipatory candidates
   behind one interface, compared on quality, determinism, time, and memory.
4. Tiled navmesh/corridor prototype with cost areas, a portal topology change,
   dynamic obstacle policy, path budgeting, and navigation debug output.
5. Cache v0 experiments for transforms, stable IDs, clip/phase placeholders,
   chunks, quantization, checksums, cancellation, and incomplete state.
6. Blender extension skeleton, native-module packaging spike, cache reader, and
   1,000-point GN instancing/playback prototype.
7. Coarse Python/Rust facade and Blender-bundled CPython ABI validation.

## Explicit exclusions

- Polished behavior-node UI, final character assets, motion matching, physics,
  USD, GPU simulation, 10K/100K claims, or Blender mainline proposals.
- Treating the waypoint stand-in in kernel slice 1 as production navigation.
- Selecting Recast/Detour, an avoidance library, or a cache codec without a
  dependency/license review and measured fit.

## Required artifacts

- Implemented kernel workspace and tests in the locations defined by the slice
  plan, amended only through recorded design decisions.
- Checked-in benchmark scenes, baselines, schema fixtures, SVG/visual traces,
  and a dated M0 report under `docs/benchmarks/`.
- Navigation, cache, bridge, and packaging decision records under `docs/`.
- Minimal Blender/GN fixtures only when the owning behavior exists.

## Acceptance criteria

1. Each avoidance candidate runs the same scenes and produces a comparable
   report; selection documents tradeoffs and rejected alternatives.
2. Strict reruns reproduce discrete state and declared continuous tolerances;
   spawn-order and add-one-agent tests pass.
3. A 1,000-agent tiled-navigation case reroutes after a portal change without
   corrupting unrelated corridors.
4. A canceled cache is readable as incomplete, never as complete; a complete
   cache passes round trip and sequential playback checks.
5. Blender loads the native module from a clean supported install with no
   absolute links to a contributor environment.
6. Blender/GN plays 1,000 cached point transforms with stable IDs, and the report
   records simulation and playback costs separately.
7. The final report states whether the contract's real-time 1K proving budget is
   met and does not extrapolate unmeasured 10K/100K behavior.

## Validation

Run every command established by the implemented kernel plan and bridge slice,
including workspace tests, release density stress, scene runs, baseline checks,
schema round trips, clean Blender install/load, and headless playback. Add those
exact commands to `README.md` and `AGENTS.md` when their runners are checked in.
The current documentation-only checks remain required.

## Definition of done and stop conditions

M0 is done only when one navigation/avoidance/cache/bridge path is selected by a
reproducible report and all seven criteria pass. Stop and record a failed gate if
native packaging, cache throughput, navigation correctness, avoidance quality,
or playback makes the 1K vertical slice non-credible; do not hide the result by
starting M1 UI work.

