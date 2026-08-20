# M6 Blender physics and hero layer proof

Status: PASS on 2026-08-19 with normal macOS host Metal access.

This proof loads the current source add-on and a freshly built native wheel,
validates paired motion through Rust, attaches deterministic interaction and
cached physics layers to a complete base cache, and exercises their failure and
lifecycle paths without rebaking.

## Verified attachment boundary

The Blender operator requires an attached complete Cache v1 and binds every
derived layer to its full `base_cache_hash`. The checked smoke uses two real
stable cache IDs for interaction ticks 10..20 and one of those IDs for a cached
physics handoff over ticks 20..30. Physics samples must cover every tick in the
declared interval before lowering.

Blender passes the complete interaction layer and motion artifacts to
`blender_crowd_native.validate_interaction_motion_attachment` before native
layout composition. The Rust authority validates participant identity,
complete root samples, contacts, and provenance. The live smoke proves a valid
artifact passes and an incomplete-root artifact is rejected with
`E_INTERACTION_MOTION` before it can replace the attached stack.

Hero cloth remains an explicit declaration-only boundary. The UI identifies
its requested cache, targets, interval, solver, cache policy, and render tiers,
while also stating `declaration-only unsupported` and `not attached`. No hero
cloth solver is executed or implied.

## Atomicity, M4 preservation, and lifecycle

M4 and M6 layers remain in separate playback lists and are serialized as one
candidate native stack. A replacement is committed only after native parsing,
layout validation, and current-tick playback succeed; failure restores the
prior native stack and leaves both Python lists unchanged.

The host smoke established an unrelated-agent M4 transform before attaching
M6 and then verified all of the following:

- valid interaction edits changed only the two declared targets at tick 15;
- cached physics became active only for its declared target inside ticks
  20..30;
- an incomplete-root motion artifact was rejected through Rust without
  changing the attached stack or its evidence labels;
- a layer targeting an ID absent from the cache was rejected by native layout
  validation, with both the prior M6 stack and M4 transform preserved;
- mute restored interaction and physics targets while retaining M4;
- unmute restored the interaction and physics effects;
- remove detached M6, preserved M4, and reset all seven evidence labels to
  their `No M6 ... loaded` states; and
- reload restored both M6 intervals without changing the base-cache identity
  or manifest SHA-256.

Native `inspect_agent` now exposes the composed `physics_active` state, so the
physics lifecycle evidence comes from the same composed records used by
playback.

## Host Blender result

Command:

```bash
scripts/m6-blender-test.sh
```

The runner built the release abi3 wheel, unpacked it into an isolated temporary
site root, and launched Blender with the current checkout on `PYTHONPATH` plus
`--python-use-system-env`. Both Blender processes had normal host Metal access;
there was no pre-Python Metal abort.

Significant output:

```text
M6 debugger Blender smoke: PASS
Error: E_INTERACTION_MOTION: root samples must cover the complete request interval; agent 2506968674689638394 motion roots must cover the complete interval
Error: E_LAYOUT: layer m6-animation-interaction-pair-10293130296351569156-15 targets an agent absent from the base
Info: M6 layers muted
Info: M6 layers unmuted
Info: M6 layers removed; source artifacts and base cache retained
M6 Blender physics/hero layers: PASS
Blender 5.2.0 LTS (hash fbe6228777e7 built 2026-07-14 01:31:22)
```

Environment:

- Blender 5.2.0 LTS, build `fbe6228777e7`;
- macOS 27.0 build `26A5416b`, Darwin arm64; and
- current source add-on plus a freshly built CPython 3.11+ abi3 arm64 wheel.

## Focused checks

```text
python3 -m unittest -v tests/test_m6_layer_bundle.py
Ran 3 tests in 0.005s
OK

cargo test -p crowd-blender --lib
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The Rust cases accept checked interval/root/contact/provenance evidence and
reject incomplete roots, invalid contacts, and invalid provenance. The pure
Python cases verify cache/request identity, explicit hero status, physics/hero
bindings, complete physics intervals, sparse lowering, and immutable source
artifacts.

## Unsupported claims

- Blender cloth, hair, and Geometry Nodes deformation are not attached,
  executed, or benchmarked.
- The cached handoff uses the deterministic native reference integrator. It is
  not evidence for Blender rigid-body parity or arbitrary collision scenes.
- No neural motion or external model worker is attached or measured.
- The proof establishes validation, cache isolation, target scoping, and
  lifecycle behavior—not visual quality, artist usability, or production-scene
  performance.
