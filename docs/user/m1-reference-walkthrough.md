# M1 reference concourse walkthrough

This walkthrough starts from a clean Blender 5.2 LTS file and requires no
Python, Rust, or JSON edits. The extension bundles the same project and asset
fixtures used by the headless acceptance runner.

## Install and create

1. Build and install the extension with `scripts/blender-install-test.sh`.
2. Open Blender's Scene properties and find **Crowd Project**.
3. Choose **Create Reference Concourse**. The action creates typed concourse,
   hall, kiosk, spawn, destination, and named-door objects plus three
   procedural commuter proportions, four materials, a canonical proxy rig,
   and idle/walk/jog actions.
4. Choose **Validate Project**. The panel reports the 1,000-agent count and the
   compiled source-hash prefix.

Headless equivalent:

```sh
scripts/m1-blender-test.sh --only project
```

## Bake, cancel, and rebake

1. Set **Cache Path** to a new directory.
2. Choose **Bake Crowd Cache**. Simulation and cache writing run in native code
   on a worker; Blender's main thread only polls status.
3. To exercise recovery, choose **Cancel Bake**. The resulting manifest is
   `canceled`, finalized chunks remain inspectable, and opening it as a
   complete cache is rejected.
4. Select a new empty directory and bake again. A complete reference bake is
   10,000 ticks for exactly 1,000 stable agents.

The strict headless bake/rebake/cancel proof is:

```sh
scripts/m1-bake-test.sh
```

## Attach and inspect

1. Choose **Attach Crowd Cache**. The scene timeline changes to the cache's
   declared tick range. Playback owns a complete-cache reader and point cloud;
   it does not create or retain a simulation session.
2. Enter a stable ID as its signed 32-bit low/high halves, move to the desired
   frame, and choose **Inspect Selected Agent**. The panel reports cached
   behavior/decision state; scene overlays show the corridor plus desired and
   solved velocity vectors for the selected evidence record.

Headless equivalent:

```sh
scripts/m1-blender-test.sh --only cache-playback
```

## Pin one agent

1. Add or select an Empty and animate or position it as the desired additive
   offset.
2. Set **Override Start**, **Override End**, and **Override Enabled**.
3. Choose **Add/Update Pinned Override**. Blender samples only the Empty's
   world translation and writes `overrides/hero-pin-v1.json` beside the cache.
   Manifest, agent table, and frame chunks are never modified.
4. Disable the layer to recover byte-identical base playback.

Headless equivalent:

```sh
scripts/m1-blender-test.sh --only override
```

## Render

Choose **Render Reference Frame** after attaching a complete cache. The action
configures the deterministic reference camera, ground, world, and lights, then
renders the same cache tick with Eevee Next and Cycles CPU. It writes two PNGs
and `m1-render-metrics.json`; point upload, canonical armature evaluation, and
each renderer are measured separately.

Headless equivalent:

```sh
scripts/m1-render-test.sh --out /absolute/path/to/m1-render
```

The procedural crowd is an M1 proxy, not a claim about the cost or quality of
1,000 production armatures. The small canonical rig measurement is deliberately
separate from the Geometry Nodes proxy and render timings.
