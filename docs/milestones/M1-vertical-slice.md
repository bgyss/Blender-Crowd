# M1 — 1,000-agent vertical slice

## Objective

Deliver one narrow but complete crowd shot: author a simple concourse, compile
it, simulate 1,000 agents headlessly, bake a recoverable cache, and render from
that cache without a live simulation session.

## Sources of truth

- [Blender Crowd 1.0 sections 4-12 and Phase 1](../blender-crowd-1.0.md)
- [Industrial capability roadmap](../industrial-crowd-capability-roadmap.md)
- [M0 proving grounds](M0-proving-grounds.md)
- [Final M1 acceptance evidence](../benchmarks/2026-08-10-m1-vertical-slice.md)
- [Post-acceptance standstill correction](../benchmarks/2026-08-11-standstill-correction.md)
- [Clean-file reference walkthrough](../user/m1-reference-walkthrough.md)

## Prerequisites

M0 is accepted with selected architecture decisions and measured baselines.
Any M0 limitation that changes M1 scope must be resolved in the canonical
contract rather than silently weakened here.

## In scope

- Crowd Project settings; stable population, spawn, destination, walkable,
  blocked, and portal data; deterministic weighted archetype/appearance choice.
- The selected tiled-navigation and avoidance implementation.
- A fixed built-in commuter state machine with destination assignment,
  arrival, portal reroute, idle/walk/run or jog clip state, phase, and speed.
- Bake, cancellation, chunk recovery diagnostics, cache attachment, GN instance
  playback, render tiers, and one pinned hero/transform override.
- Selected-agent path, velocity, state, and decision-reason overlays.
- Headless validation, bake, cache playback, and representative Eevee/Cycles
  render on the declared initial platform.

## Explicit exclusions

No general behavior graph editor, full groups/queues, motion matching, ragdolls,
USD, GPU scale claims, arbitrary rig conversion, or production support matrix.
The built-in behavior must use the future compiled runtime boundary rather than
becoming special-case Blender logic.

## Required artifacts

- Minimal implemented `addon/`, Rust facade/core/cache modules, versioned
  `schemas/`, GN asset, redistributable character/clip fixtures, and concourse
  scene required by behavior—not pre-created empty package trees.
- Headless test scripts and a dated M1 vertical-slice report.
- User walkthrough that starts from a clean file and uses no code edits.

## Acceptance criteria

1. A documented clean install enables the extension and validates the reference
   assets.
2. Exactly 1,000 agents retain stable IDs and reproducible variants across an
   unchanged strict rebake.
3. Agents navigate the corridor and doorway, react to a portal open/close event,
   and complete destinations within measured, declared thresholds.
4. Cache cancellation is safe; a completed cache plays and renders with the
   simulation session destroyed or unavailable.
5. Clip, phase, playback rate, orientation, variant, visibility, and render tier
   channels survive cache round trip.
6. A selected agent exposes readable navigation and decision evidence.
7. A pinned sparse override changes one agent without mutating the base cache.
8. Simulation, Blender playback, armature evaluation, memory, cache size, and
   render costs are reported separately.

## Validation and proof

The implementation must check in exact runners for clean installation, headless
scene compilation/bake, strict repeat comparison, cache round trip, GN attribute
compatibility, and render smoke. The dated report links their outputs and the M0
baseline. A viewport recording is useful evidence but cannot replace these
tests.

## Definition of done and stop conditions

M1 is done only when the same documented input produces the accepted headless
bake and cache-only render. Stop if the shot requires Python/Rust edits, hidden
runtime Blender state, manual cache repair, or relaxed stable-ID/determinism
rules; those failures block M2.
