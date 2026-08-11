# M1 reference fixtures

These JSON files are the complete, redistributable inputs for the M1 concourse.

- `concourse-project-v1.json` is the authoritative `ProjectIrV1` input used by
  both the Rust headless bake and Blender's Create Reference Concourse action.
- `commuter-assets-v1.json` contains literal dimensions, colors, rig metadata,
  and idle/walk/jog timing for the procedural proxy commuters. It deliberately
  contains an empty `external_paths` array: no mesh, texture, rig, or clip is
  loaded from a contributor machine.

The Blender extension carries byte-identical packaged copies so a clean
installation can create the reference shot without access to the checkout.
Generated `.blend` data is a test product, not a source fixture.
