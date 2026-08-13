# M4 layered layout and interchange acceptance evidence

Date: 2026-08-12  
Milestone: [M4 — layered layout and interchange](../milestones/M4-layout-interchange.md)  
Status: **accepted on the documented macOS arm64 support environment**

## Implemented and locally exercised

- `layout-layer-v1` is a sparse, ordered, typed layer contract over Cache v1.
  It carries layer kind, order/priority, mute/solo, stable targets/ranges,
  provenance, explicit dependencies, and an immutable BLAKE3 base-cache hash.
- The cache-side composer applies layout, animation-fix, hero, physics, and
  shot edits without modifying source frames or cache files; it reports
  same-channel conflicts and dependent-layer invalidation. Invalidated layers
  are durably marked stale and composition rejects them until they are
  recomputed or explicitly muted.
- The Blender layer panel now persists mute/solo choices in the adjacent stack
  and immediately reapplies them to the cache composer. It also resolves a
  stable agent ID from the visible procedural point nearest the 3D cursor, so
  targeted correction does not require permanent per-agent objects or pasted
  IDs.
- Region-density and curve-retiming corrections apply deterministically to the
  layer's explicit stable-ID scope; the UI keeps that scope inspectable instead
  of relying on viewport iteration order.
- Selected-agent local kinematic re-simulation now emits bounded absolute
  transform samples plus scope/provenance into the layer itself. It is a
  deterministic cache-side redirect, not a hidden live simulation session.
- A selected-agent physics handoff persists incoming state, collision masks,
  interval samples, and a recovery tick entirely in the layer artifact. The
  checked reference integrator deterministically writes the cache interval;
  Blender only presents it.
- Procedural extraction turns composed records into render-time instances with
  stable identity, prototype/material selection, clip/phase, visibility, and
  render tier. A 10,000-record test keeps the prototype set at three rather
  than creating 10,000 scene objects.
- Existing `override-layer-v1` hero-pin files migrate to adjacent M4 layers;
  Cache v1 stays the unchanged base cache.
- The checked-in migration golden validates against the complete
  `layout-layer-v1` JSON Schema, while an under-specified region edit is
  rejected. This prevents the UI or exchange adapters from treating partial
  layer records as valid state.
- The native cache-only Blender bridge applies M4 layers at playback and emits
  a bounded-object PointInstancer USDA profile. The documented profile carries
  stable IDs, composed positions, variant selection, and source-cache hash.

## Local proof command

```sh
scripts/m4-foundation-test.sh
```

The runner covers ordered sparse edits, conflict reporting, physics recovery
isolation, dependency invalidation, mismatch diagnostics, v1 override
migration, USDA identity/variant round trip, OpenUSD `usdcat --loadOnly` and
`usdchecker` consumer checks when available, native bridge compilation, and
JSON layer-stack persistence and full-schema validation/rejection coverage.

## Acceptance audit

| Criterion | Current evidence |
| --- | --- |
| Sparse/reversible edits and unchanged base | Cache composition tests plus the Blender runner's manifest-hash, flatten, save/reload, and stack-reapply checks. |
| Seven selected corrections | The 1,000-agent Blender run selects a stable ID at the 3D cursor, corrects seven explicit IDs for ticks 5..25, confirms the authored +2m move, and writes distinct before/after PNGs. |
| Region/curve scope and stale dependencies | The Blender run persists scoped region density, curve retiming, and local resimulation over the real cache; Rust tests prove dependency invalidation is persisted and stale layers refuse composition until muted/recomputed. |
| Cached physics/recovery | The Blender run writes selected-agent cached physics samples and the cache tests prove deterministic recovery, isolation, and rejection of empty collision masks. |
| Procedural scene scale | The run bakes 1,000 agents through 5,000 ticks, evaluates 700 live instances at tick 4,999, adds only one persistent scene object at cache attach, and writes a populated scale capture. The separate 10,000-record extraction test bounds the prototype set at three. |
| USD profile | Project round trip plus OpenUSD `usdcat --loadOnly` and `usdchecker` cover the features claimed by the documented profile; unsupported channels are explicitly rejected rather than silently degraded. |
| 1.0 migration | Checked v1 override migration fixture, full JSON Schema validation, and actionable mismatch tests are all green. |

These checks meet the defined M4 acceptance scope. They do **not** claim
universal Blender/Houdini/Unreal compatibility, full Blender rigid-body/cloth/
hair, GPU simulation, or 100K scale.

## Blender runner result and host requirement

On 2026-08-12, `scripts/m4-blender-test.sh` passed on an Apple M1 Max running
macOS 27.0 with Blender 5.2.0 LTS arm64, build `fbe6228777e7`. The 1,000-agent,
5,000-tick run built and loaded the abi3 native wheel; selected an agent at the
3D cursor; corrected seven explicit IDs; persisted mute/solo; reported one
injected same-channel conflict; wrote region, curve, local-resimulation, and
cached-physics layers; wrote flatten/USD outputs; preserved the base cache;
reopened the `.blend`; and reapplied all six layers. It recorded 700 evaluated
procedural instances at tick 4,999 while cache attachment added one persistent
scene object. It ended with:

```text
M4 Blender layout: PASS {"base": "17375a61bb4615b21b47f213d100434bc3bd47e419068e504a928ee9cd713786", "capture_seconds": {"after": 0.353, "before": 0.574}, "layers": 6, "procedural_instance_count": 700, "scene_object_count_after_attach": 66, "scene_object_count_before_attach": 65}
```

Run the same gate from the repository root with normal host GPU access:

```sh
scripts/m4-blender-test.sh
```

To retain the three validated captures outside the temporary test directory:

```sh
M4_ARTIFACT_DIR=/path/to/m4-captures scripts/m4-blender-test.sh
```

The earlier pre-Python crash was an automation-sandbox boundary, not a broken
Blender installation or an add-on crash. In the restricted Seatbelt context,
`MTLCreateSystemDefaultDevice()` returned `nil`; Blender then dereferenced a
null device-derived string in `supports_barycentric_whitelist()`. The cache-line
warning occurred in that same restricted context because hardware `sysctl`
queries were denied. The identical executable discovered `Apple M1 Max` and
ran cleanly with host Metal access.

The runner retains `--python-use-system-env` because Blender 5.2 ignores
`PYTHONPATH` by default. It inspects the conflict at its overlapping tick,
checks direct composition displacement before comparing captures, and syncs
the injected layers into Blender's saveable editor collection before the
save/reload proof.
