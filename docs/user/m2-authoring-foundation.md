# M2 authoring foundation

This document covers the implemented M2 foundation. It is not the final M2
acceptance guide.

## Implemented contracts

- Project IR v2 preserves the canonical Project IR v1 payload and source hash.
- Typed graphs compile and execute finite-state, utility, selector, sequence,
  fallback, interrupt, timer, probability, event, and blackboard patterns.
- Decision outcomes record visited nodes, decisive action, and observations.
- Compiled graphs execute inside the fixed-step decide phase; selected-agent
  graph traces are queryable from the live strict simulation.
- Queues admit stable IDs deterministically, honor per-tick capacity, advance
  slots, and count throughput.
- Couples, families, and leader/follower groups validate stable membership and
  expose split/cohesion evidence.
- Retarget profiles validate canonical feet/hips, scale, root, and forward axis;
  clips validate loop and foot-contact intervals.
- Body, clothing, material, prop, and clip choices are stable per agent and
  individually overrideable.
- Sparse override v2 covers visibility/delete, transform, timing, speed,
  appearance, animation, goal, and hero-tier edits. Conflicts name both layers,
  and local resimulation records affected IDs, tick range, and base hash.

Run the implemented foundation checks with:

```sh
scripts/m2-foundation-test.sh
```

## Remaining M2 exit work

The authorable runtime still needs full 1,000-agent bake acceptance, including
queue slot steering, group bottleneck behavior, richer semantic observations,
event/cache channels, and graph trace persistence.
The Blender workspace still needs the complete node, population, asset,
environment, and layout editors rather than JSON-backed validation controls.
The full reference scene, undo/save/reload runner, locomotion/terrain fixtures,
non-developer reproduction pass, and dated M2 evidence report also remain open.

Until those pass, M2 is not accepted and M3 remains blocked.
