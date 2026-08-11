# M1 reference fixtures

These JSON files are the complete, redistributable inputs for the M1 concourse.

- `concourse-project-v1.json` is the authoritative `ProjectIrV1` input used by
  both the Rust headless bake and Blender's Create Reference Concourse action.
- `commuter-assets-v1.json` contains literal dimensions, colors, rig metadata,
  and idle/walk/jog timing for the procedural proxy commuters. It deliberately
  contains an empty `external_paths` array: no mesh, texture, rig, or clip is
  loaded from a contributor machine. Each clip declares two amplitudes in
  radians: `swing_radians` is how far a limb travels, which the canonical rig
  animates, and `body_swing_radians` is how far the instanced body leans, which
  Geometry Nodes applies. A body does not travel as far as the limb attached to
  it, so the two differ and neither consumer derives one from the other.

The Blender extension carries byte-identical packaged copies so a clean
installation can create the reference shot without access to the checkout.
Generated `.blend` data is a test product, not a source fixture.
