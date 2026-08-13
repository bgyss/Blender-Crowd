# M3 production workflow and cache recovery

This document describes the implemented M3 hardening slice. It does not claim
that Blender Crowd 1.0 is released or that its macOS-arm64 M3 support row has
passed.

## Normal workflow

Open the Scene properties and use the **Crowd Workflow** panel:

1. Create or open a project, then select **Validate**.
2. Choose a cache location and select **Bake**. The panel records the planned
   population/tick work and exposes safe cancellation.
3. Select **Inspect Health** before playback. Only a `complete` cache can be
   attached.
4. Select **Attach Complete Cache**. The panel records the resolved artifact
   location and switches the next action to inspection or rendering.

The advanced authoring controls remain under **Crowd Authoring (Advanced)**;
they are kept separate so normal operation does not require editing raw M2
data contracts.

## Recovery policy

Canceled and incomplete caches can be inspected but are never attached. The
health panel shows the status, readable prefix, valid chunk count, last
complete tick, and a recovery instruction. Rebake the project to produce a
new authoritative cache; do not treat a readable prefix as a deliverable.

On opening a `.blend`, Blender Crowd resolves the saved cache path again and
reattaches it only after a fresh `complete` inspection. If resolution or
validation fails, any saved Crowd Cache Points object is hidden in viewport and
renders, and a persistent diagnostic is added. This prevents saved geometry
from being mistaken for live cache-backed playback.

Diagnostic History is saved with the scene. Each item records severity,
summary, detail, affected file/object when available, and this recovery guide.

## Automated proof

From a clean installed extension archive, run:

```sh
scripts/blender-install-test.sh --python tests/blender/test_m3_production.py
```

The test creates and cancels a cache bake, verifies that health inspection
retains recovery details, rejects attachment, and confirms that those states
and diagnostic history survive save/reload.

## Current boundary

This slice closes a cache-trust and recoverability gap. It does not replace the
remaining M3 release work: migration fixtures, performance/package budgets,
SBOM/license review, signed provenance where applicable, and accessibility
review. Independent evaluator studies are an M7 gate.
