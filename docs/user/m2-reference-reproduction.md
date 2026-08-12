# M2 reference reproduction

This is the handoff procedure for a Blender crowd TD who did not develop M2.
It is deliberately a clean-install workflow: do not reuse a previously loaded
extension or a cache from another run.

## Prerequisites

- macOS with Blender 5.2 LTS available through `BLENDER` (or the runner's
  platform-default Blender location).
- The repository checkout and its pinned Rust toolchain.
- Host GPU access. Sandboxed automation that cannot create a Metal device will
  crash Blender before Python starts and is not a valid reproduction host.

## One-command evidence run

From the repository root, choose a new empty output directory and run:

```sh
scripts/m2-full-acceptance.sh --out /path/to/blender-crowd-m2-proof
```

For the Blender UI workflow, either enter a path that does not exist yet or
select a directory that exists but is completely empty. A non-empty path is
rejected deliberately so a previous cache cannot be overwritten.

The runner builds the wheel, removes any prior extension copy, installs the
extension, creates the 1,000-agent reference concourse, performs a full
10,000-tick **authorable** bake, attaches only the completed cache, inspects a
selected agent's cached graph evidence, and writes Eevee and Cycles PNGs.

It succeeds only when all of these files exist:

- `cache/manifest.json` with `status: "complete"`, `agent_count: 1000`, and
  `tick_end: 9999`;
- `cache/events/behavior-v1.json` containing decision, queue, and group
  lifecycle evidence;
- `render/m1-eevee.png` and `render/m1-cycles.png`;
- `m2-full-acceptance.json` with `acceptance_subgate_passed: true` and
  `cache_only_render: true`.

The output directory is evidence, not an input. Keep it intact for review.

## Blender UI spot check

Open a new Blender session and use the installed **Crowd Project** panel:

1. Select **Create Reference Concourse** and confirm the status names 1,000
   agents.
2. Under **M2 Social Groups**, confirm `reference_pair` has two numeric stable
   agent IDs, shared destination `east_exit`, and `leader_first` policy.
3. Select **Validate M2 Authorable Project**. It must report one graph and
   1,000 agents.
4. Set a fresh cache folder, select **Bake Crowd Cache**, then attach it only
   after the status is complete.
5. Copy an `agent_id` from `events/behavior-v1.json`, paste its decimal value
   into **Selected Agent ID**, then select **Inspect Agent**. Confirm the
   graph, decisive node, state, and cached-event count appear in the
   **Selected Agent Debug** box. In the 3D Viewport, enable **Overlays** and
   frame the agent area: `Crowd Debug selected_path`,
   `Crowd Debug desired_velocity`, and `Crowd Debug solved_velocity` are
   in-front scene objects visible in the Outliner and viewport (not renders).
6. Select a separate Empty as a hero pin, move it slightly, then select
   **Add/Update Pinned Override**. Confirm a new override JSON is written under
   `cache/overrides/` while `manifest.json`, `agents.bin`, and every frame chunk
   are unchanged.

Record the operator status messages, the final acceptance JSON, Blender/OS
version, GPU model, and any deviation. M2 was accepted on 2026-08-12 after the
project operator completed this six-step spot check; see the
[acceptance record](../benchmarks/2026-08-12-m2-acceptance.md). Future runs remain
useful regression evidence, especially for the deferred UI/UX backlog.
