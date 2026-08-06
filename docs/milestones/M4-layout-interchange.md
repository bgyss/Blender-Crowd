# M4 — Layered layout, physics handoff, and interchange

## Objective

Deliver the post-simulation directing and layered exchange workflow that makes a
crowd system practical in shot production: simulate once, correct selectively,
compose the result non-destructively, and render or exchange it without expanding
the entire crowd into ordinary Blender objects.

## Sources of truth

- [Industrial roadmap: simulate, direct, and render](../industrial-crowd-capability-roadmap.md#simulate-direct-and-render)
- [Blender Crowd 1.0 shot overrides and cache](../blender-crowd-1.0.md)
- [M3 production 1.0](M3-production-1.0.md)

## Prerequisites

M3 passes and the 1.0 identity, cache, animation, asset-reference, and override
schemas have a documented compatibility policy. Schema changes in this milestone
must migrate real 1.0 fixtures.

## In scope

1. Ordered base-simulation, layout, animation-fix, hero, physics, and shot layers
   with stable targets, provenance, priorities, mute/solo, conflict display, and
   reversible flatten/export.
2. Bulk and per-agent move, hide/delete, freeze, time offset/warp, speed, path
   guide, goal, appearance, animation, visibility, render tier, group, and hero
   promotion edits.
3. Region and curve operations for flow redirection, density adjustment,
   formation editing, local retiming, bounded local resimulation, and explicit
   dependency/invalidation display.
4. Ragdoll/rigid-body handoff for a selected agent and frame interval, including
   collision masks, incoming state, cached physics result, recovery/continued
   authored motion policy, and layer isolation.
5. Procedural render extraction using transforms, prototypes, variant/material
   choices, clip/phase, visibility, and LOD without permanent scene expansion.
6. A versioned USD crowd profile for point instancers/prototypes, identities,
   animation references or samples, and layer opinions. A dedicated writer may
   be used when Blender's exporter cannot preserve the contract.
7. Import/export adapters through public cache/IR APIs so Blender, Houdini,
   Unreal, and other consumers can be evaluated without entering core state.

## Explicit exclusions

No claim of universal USD/DCC compatibility, full Blender rigid-body/cloth/hair
support, GPU simulation, or 100K scale. One syntactically valid USD file is not a
passing interchange result.

## Required artifacts

- Versioned layer schema, composition engine, Blender layout UI, conflict and
  invalidation views, migration fixtures, and flatten/recovery tools.
- Physics-handoff reference scene and failure fixtures.
- Procedural extraction/render fixture at a scale large enough to expose scene
  expansion costs.
- USD profile documentation, golden files, and at least two independently
  evaluated consumer or round-trip paths where accessible without unapproved
  external spend.
- Dated layout/interchange evidence report.

## Acceptance criteria

1. Every edit type is sparse, reversible, inspectable, and leaves the base cache
   hash unchanged; layer order/conflicts reproduce after save/reload.
2. Seven arbitrarily selected problem agents can be corrected without a full
   rebake, and affected IDs/time ranges are reported.
3. Region/curve changes invalidate or locally resimulate only documented
   dependencies; stale downstream layers are never silently accepted.
4. A selected animation-to-ragdoll-to-recovery sequence plays from layered cache
   without mutating unrelated agents or requiring the live simulator at render.
5. Procedural extraction/rendering uses bounded scene objects/prototypes and
   matches stable identity, variants, animation timing, materials, visibility,
   and LOD from the composed crowd.
6. USD round trip or consumer evaluation preserves every feature claimed by the
   documented profile, with unsupported features rejected or visibly degraded.
7. Older 1.0 caches and overrides migrate or fail with actionable diagnostics.

## Validation and proof

Check in layer composition/golden/migration tests, override isolation and
conflict tests, local-resimulation dependency tests, Blender undo/save/reload
tests, physics handoff/recovery scenarios, procedural render comparisons, and
USD consumer/round-trip reports. External DCC validation that requires licenses
or remote systems needs explicit authorization; lack of it narrows the claim
rather than the serialization tests.

## Definition of done and stop conditions

M4 is done when a production correction and exchange scene can be composed,
reopened, rendered, and validated without touching its base simulation. Stop if
layers become destructive, consumer fidelity is inferred rather than tested, or
physics output becomes hidden authoritative state outside the cache contract.
