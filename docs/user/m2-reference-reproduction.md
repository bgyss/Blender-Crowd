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
5. Enter an agent ID from `events/behavior-v1.json`, select **Inspect Agent**,
   and confirm the selected-path/velocity overlays and graph/node evidence are
   shown.
6. Select a separate Empty as a hero pin, move it slightly, then select
   **Add/Update Pinned Override**. Confirm a new override JSON is written under
   `cache/overrides/` while `manifest.json`, `agents.bin`, and every frame chunk
   are unchanged.

Record the operator status messages, the final acceptance JSON, Blender/OS
version, GPU model, and any deviation. Do not mark M2 accepted merely because
the scripted runner succeeds: a separate person must complete this procedure
and attach their dated evidence to the acceptance report.
