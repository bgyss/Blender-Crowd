# M2 — Authorable MVP

## Objective

Turn the vertical slice into an artist-authored crowd product with typed agency,
semantic environments, first-class groups, production variation, deep debugging,
and sparse post-simulation correction.

## Sources of truth

- [Blender Crowd 1.0 sections 4, 6.3, 7, 10, and 12](../blender-crowd-1.0.md)
- [Industrial capability ledger](../industrial-crowd-capability-roadmap.md#industrial-capability-ledger)
- [M1 vertical slice](M1-vertical-slice.md)

## Prerequisites

M1 passes from a clean install. Cache, IR, stable identity, and Blender ownership
boundaries are versioned and migration-tested before authorable variants rely on
them.

## In scope

1. Typed behavior graph compiler/runtime with finite-state, utility, selector,
   sequence, interrupt, fallback, timer, probability, event, and blackboard
   patterns; no arbitrary hot-loop Python.
2. MVP perception for nearby agents, reachability/visibility, portal state,
   semantic regions, density, group state, and interest/danger observations.
3. Portals, queues, directional lanes, interest/danger and preferred/forbidden
   regions, timed events, rerouting, and capacity rules.
4. Couples/small families and leader/follower groups with roles, shared goals,
   cohesion, separation limits, bottleneck policy, and deterministic queues.
5. Weighted mesh/body/clothing/material/prop/clip variation, custom-character
   retarget profiles, clip/contact validation, starts/stops/turns, speed warping,
   basic terrain projection, and foot-lock/IK presentation where supported.
6. Population workflows for regions, curves/lanes, basic formations, and a
   reference seating/stadium layout without making these separate simulators.
7. Agent trace, graph-step explanation, perception/attention overlays, path and
   avoidance constraints, heatmaps, metrics, and actionable validation errors.
8. Sparse hide/delete, transform, timing, speed, appearance, animation, goal,
   pin/guide, hero promotion, and bounded local-resimulation overrides.

## Explicit exclusions

Full physics/ragdoll production workflows, layered USD composition, 10K/100K,
motion matching, cloth/hair, semantic AI, and complete multi-sensory MASSIVE
parity remain later gates.

## Required artifacts

- Behavior/node and semantic schemas with golden migration fixtures.
- Blender node editor, population/asset/environment/layout UI, and undo-safe
  operators.
- Full transit-concourse acceptance scene, redistributable assets, graph presets,
  user guide, node reference, failure guide, and headless example.
- Cross-layer tests and a dated authorable-MVP evidence report.

## Acceptance criteria

1. A crowd TD creates the full section 12.1 reference shot without editing code.
2. Invalid graphs, disconnected destinations, duplicate IDs, bad retarget maps,
   and stale caches identify the entity and corrective action.
3. Groups remain coherent under declared limits; queues are ordered and portal
   capacity is measured; group splits/intrusions are reported.
4. The graph trace explains the observations and decisive node for a selected
   action and reproduces under strict mode.
5. Custom-character and variation selection is stable, inspectable, and
   individually overrideable.
6. Locomotion/terrain presentation passes clip-loop, foot-contact, slope, and
   cache-playback fixtures without changing simulation truth.
7. Every required sparse edit changes only its target layer; base-cache hashes
   remain unchanged, conflicts are visible, and local resimulation records its
   affected IDs/range.
8. A non-developer following the guide reproduces the shot and acceptance report
   without assistance from implementation code authors.

## Validation and proof

Check in runners for graph compile/golden tests, semantic validation, group and
queue scenarios, retarget/clip fixtures, override isolation, undo/save/reload,
headless bake/render, and the full functional acceptance suite. Human usability
evidence supplements but does not replace automation.

## Definition of done and stop conditions

M2 is done when an independent artist can author, debug, bake, correct, and
render the full reference shot and the automated suite passes. Stop if the node
graph becomes an unbounded language, overrides mutate the base cache, or custom
assets require undocumented implementation knowledge.
