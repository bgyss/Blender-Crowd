# M2 authoring foundation

This document covers the implemented M2 foundation. It is not the final M2
acceptance guide.

## Implemented contracts

- Project IR v2 preserves the canonical Project IR v1 payload and source hash.
- Typed graphs compile and execute finite-state, utility, selector, sequence,
  fallback, interrupt, timer, probability, event, and blackboard patterns.
- Decision outcomes record visited nodes, decisive action, and observations.
- Compiled graphs execute inside the fixed-step decide phase; selected-agent
  graph traces are queryable from the live strict simulation. The same runtime
  emits deterministic decision, queue, and group evidence records; the cache
  persists them in its optional `events/behavior-v1.json` sidecar during an
  authorable Blender bake. Cache inspection reads this durable evidence before
  falling back to legacy debug files.
- Queues admit stable IDs deterministically, honor per-tick capacity, advance
  slots, and steer live agents to their reserved slots.
- Couples, families, and leader/follower groups validate stable membership,
  contribute bounded live cohesion steering, and expose split/cohesion evidence.
- Retarget profiles validate canonical feet/hips, scale, root, and forward axis;
  clips validate loop and foot-contact intervals.
- Terrain presentation derives a display-only height, normal, slope offset, and
  foot-contact locks from cache XY and clip metadata. Fixtures prove that this
  never mutates the authoritative simulation trajectory and rejects slopes
  beyond the authored presentation limit.
- Cache playback exposes a **Presentation Terrain** object. Its Geometry Nodes
  raycast moves only instances and stores `crowd_terrain_normal`; the cache
  point cloud and its serialized frames remain unchanged.
- Body, clothing, material, prop, and clip choices are stable per agent and
  individually overrideable.
- Sparse override v2 covers visibility/delete, transform, timing, speed,
  appearance, animation, goal, and hero-tier edits. Conflicts name both layers,
  and local resimulation records affected IDs, tick range, and base hash.

Run the implemented foundation checks with:

```sh
scripts/m2-foundation-test.sh
scripts/m2-blender-authoring-test.sh
cargo test -p crowd-core --test terrain_presentation
scripts/m2-reference-acceptance.sh
```

## Remaining M2 exit work

The authorable runtime now has deterministic leader-first group bottleneck
steering as well as individual queue admission. It still needs the full
authorable Blender bake and richer semantic-observation acceptance evidence.
The Blender workspace provides saveable behavior-node, population,
asset/retarget/variation, environment, and layout editors. The reference
project includes a checked 50-seat layout; its guides are generated from the
saved contract and do not become a second simulator. The automated UI-context
runner passes graph/environment, population, clip, and layout edits through
undo, save/reload, and native revalidation on Blender 5.2.
The full authorable bake/render, independent non-developer reproduction pass,
and final dated M2 acceptance report remain open.

`scripts/m2-reference-acceptance.sh` emits a dated 10,000-tick runtime-evidence
report for the 1,000-agent authorable reference. It is intentionally a
subgate: `m2_milestone_accepted` stays false until Blender artist-reproduction
and visual presentation evidence are independently proven.
The completed runtime-evidence artifact is
[2026-08-11-m2-runtime-evidence](../benchmarks/2026-08-11-m2-runtime-evidence.md).

Until those pass, M2 is not accepted and M3 remains blocked.

## Running the implemented gate runner

`scripts/m2-foundation-test.sh` runs the core, cache, and strict workspace
lint gates and type-checks the native Blender extension test target.  A
Blender-only install commonly provides the embedded interpreter but not its
development link library, so the default runner intentionally does not claim
to execute the PyO3 unit binary.  On a host with that library configured, run
`CROWD_RUN_EMBEDDED_PYTHON_TESTS=1 scripts/m2-foundation-test.sh` to link and
execute it too. The separate Blender editor runner performs its clean-install
and native-load checks in background mode, then opens an unattended UI context
for the undo/save/reload proof because Blender disables `ed.undo` under `-b`.

When the runner is launched by a sandboxed automation host, Blender must be
granted host GPU access. Without it, `MTLCreateSystemDefaultDevice()` returns
`nil` and Blender 5.2 crashes in `supports_barycentric_whitelist()` before
Python starts. This is a sandbox boundary; the same command passes when run
with host Metal access.
