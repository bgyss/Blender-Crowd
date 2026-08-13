# M5 scale and procedural rendering foundation

Date: 2026-08-12  
Milestone: [M5 — scale, GPU tiers, and procedural rendering](../milestones/M5-scale-rendering.md)  
Status: **foundation implemented; 10K and 100K acceptance remains unearned**

## Implemented boundary

- `crowd-core::fidelity` defines typed, independent S0-S3 simulation and
  R0-R4 presentation policies, with validated camera-distance hysteresis.
  Tier decisions occur after authoritative root-motion commit, so presentation
  scheduling cannot change a tick's trajectory.
- Artist pins override the camera policy by stable agent ID. Pins are sorted
  before lookup, so UI collection order cannot alter results.
- The existing Cache v1 `render_tier` byte remains the serialized compatibility
  field and is updated from the typed render tier. The typed simulation and
  render state is included in the strict state hash.
- `CacheReader::read_range` decodes only chunks intersecting an inclusive tick
  range and retains only the selected frames. This supports seek/extraction
  without materializing the rest of a complete cache.
- `SpatialFieldKernel` provides a backend-neutral, read-only density/velocity
  field contract. The checked CPU reference implementation rejects non-finite
  input and keeps aggregates separate from root motion and identity; a GPU
  backend can be compared against this contract without changing public state.
- When fidelity is enabled, S0/S1 perception runs every tick, S2 every fourth
  tick, and S3 has no individual neighbor list. The scheduler preserves a
  deterministic empty arena entry for skipped slots.
- The [backend support matrix](../backend-support-matrix.md) declares the CPU
  reference as the only implemented backend and makes every GPU path an
  explicit CPU fallback rather than an implied feature.

## Local proof

```sh
scripts/m5-foundation-test.sh
```

The runner proves hysteresis, invalid policy rejection, stable-ID pins,
root-motion finiteness while scheduling, and a range crossing cache chunk
boundaries.

It also verifies that 100,000 background records extract to 95,000 visible
instance records using three prototypes. This is a cache-side architectural
test; it does not establish Blender render time or a 100K scale-gate pass.

The first measured [10K cache preflight](2026-08-12-m5-cache-10k-preflight.md)
is recorded separately because its eight fixture frames are intentionally not
a full scale-gate workload.

The Blender **M5 Scale and Profiling** panel exposes only aggregate declared
S/R tier counts, backend/fallback, estimates, measurements, and a selected
bottleneck. It labels estimates explicitly and does not enumerate agents.

`m5_city_flow` is a separately addressable city-flow fixture with a stable
scene identity and a 2,400-tick minimum duration. The 100-agent smoke run
completed with all agents arrived; this confirms runner wiring only, not scale
acceptance.

## Still required for M5 acceptance

This foundation does not introduce the S2 shared-flow simulator, scheduled
perception/decision execution, an implemented GPU kernel backend, 10K/100K reference scenes,
Blender profiling UI, render extraction benchmark, or dated scale reports.
Consequently it does not meet either the 10K or 100K gate and must not be used
to make a scale or GPU support claim.
